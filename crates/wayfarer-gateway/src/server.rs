//! The network face of the gateway: an HTTP server with the WebSocket endpoint (`/ws`, the
//! C-002 protocol as JSON text frames), a health check and the web UI's static files.
//!
//! No TLS: remote access goes through SSH or Tailscale. On a loopback bind the handshake's
//! token is not checked; on any other bind the gateway will not start without one.

use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tower_http::services::ServeDir;
use wayfarer_proto::{ClientKind, SessionId, ToClient, ToGateway, protocol_mismatch};

use crate::{Gateway, GatewayError, HistoryQuery};

/// How long a new connection may take to say hello.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Messages queued for one connection's socket before its forwarders wait.
const OUTBOX: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error(
        "{0} is not a loopback address: create a token first with `wayfarer gateway --new-token`"
    )]
    TokenRequired(SocketAddr),
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("cannot serve on {addr}: {source}")]
    Bind { addr: String, source: io::Error },
}

/// What [`serve`] needs besides the gateway.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// The token a client's `hello` must carry; `None` checks nothing (loopback binds only).
    pub token: Option<String>,
    /// The built web UI. A missing directory is fine: `/` then says the UI is not built.
    pub ui_dir: PathBuf,
}

/// The token for a bind: none on loopback, otherwise the one in `token_file`, which must exist.
pub fn token_for_bind(bind: SocketAddr, token_file: &Path) -> Result<Option<String>, ServerError> {
    if bind.ip().is_loopback() {
        return Ok(None);
    }
    read_token(token_file)?
        .map(Some)
        .ok_or(ServerError::TokenRequired(bind))
}

/// The token in `path`, if the file exists and holds one.
pub fn read_token(path: &Path) -> Result<Option<String>, ServerError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.trim().to_owned()).filter(|t| !t.is_empty())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ServerError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Make a new random token (replacing any old one) in a file only the owner can read, and
/// return it.
pub fn create_token(path: &Path) -> Result<String, ServerError> {
    let io_err = |source| ServerError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| io_err(io::Error::other(e)))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(io_err)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    io::Write::write_all(&mut options.open(path).map_err(io_err)?, token.as_bytes())
        .map_err(io_err)?;
    Ok(token)
}

#[derive(Clone)]
struct AppState {
    gateway: Gateway,
    token: Option<Arc<str>>,
    ui_dir: Arc<Path>,
    /// Turns `true` when the server is shutting down, so WebSocket handlers end.
    stopping: watch::Receiver<bool>,
}

/// Bind `addr`.
pub async fn bind(addr: SocketAddr) -> Result<TcpListener, ServerError> {
    TcpListener::bind(addr)
        .await
        .map_err(|source| ServerError::Bind {
            addr: addr.to_string(),
            source,
        })
}

/// Serve on `listener` until `shutdown` completes, then stop accepting, end the WebSocket
/// connections and log every session off ([`Gateway::shutdown`]).
pub async fn serve(
    listener: TcpListener,
    gateway: Gateway,
    options: ServerOptions,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ServerError> {
    let (stop, stopping) = watch::channel(false);
    let state = AppState {
        gateway: gateway.clone(),
        token: options.token.map(Arc::from),
        ui_dir: Arc::from(options.ui_dir.as_path()),
        stopping,
    };
    let ui = ServeDir::new(&options.ui_dir).fallback(get(ui_missing).with_state(state.clone()));
    let app = Router::new()
        .route("/ws", get(upgrade))
        .route("/healthz", get(healthz))
        .fallback_service(ui)
        .with_state(state);
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.await;
            let _ = stop.send(true);
        })
        .await;
    gateway.shutdown().await;
    result.map_err(|source| ServerError::Io {
        path: PathBuf::from("<server>"),
        source,
    })
}

async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "sessions": state.gateway.sessions().len(),
    }))
}

/// Anything the UI directory does not hold.
async fn ui_missing(State(state): State<AppState>) -> Response {
    if state.ui_dir.join("index.html").is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Html(format!(
            "<!doctype html><title>wayfarer</title><p>The web UI is not built: nothing in \
             <code>{}</code>. The WebSocket at <code>/ws</code> works.",
            state.ui_dir.display()
        )),
    )
        .into_response()
}

async fn upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| Connection::run(socket, state))
}

/// Same length and content, without stopping at the first difference.
fn tokens_match(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |d, (x, y)| d | (x ^ y)) == 0
}

struct Connection {
    gateway: Gateway,
    out: mpsc::Sender<ToClient>,
    /// The sessions this connection receives, each with its forwarding task.
    subscribed: Vec<(SessionId, JoinHandle<()>)>,
}

impl Connection {
    async fn run(socket: WebSocket, state: AppState) {
        let (mut sink, mut stream) = socket.split();
        let frame = |msg: ToClient| Message::text(msg.to_json());
        let mut stopping = state.stopping.clone();

        // The handshake.
        let hello = tokio::time::timeout(HELLO_TIMEOUT, next_message(&mut stream)).await;
        let refusal = match hello.ok().flatten() {
            None => Some("no hello received".to_owned()),
            Some(Err(e)) => Some(format!("expected a hello: {e}")),
            Some(Ok(ToGateway::Hello {
                protocol,
                client: ClientKind::Ui | ClientKind::Brain,
                token,
            })) => protocol_mismatch(protocol).or_else(|| {
                let ok = state
                    .token
                    .as_deref()
                    .is_none_or(|want| token.as_deref().is_some_and(|got| tokens_match(want, got)));
                (!ok).then(|| "missing or wrong token".to_owned())
            }),
            Some(Ok(other)) => Some(format!(
                "expected a hello first, got {:?}",
                other.type_name()
            )),
        };
        if let Some(reason) = refusal {
            let _ = sink.send(frame(ToClient::Refused { reason })).await;
            let _ = sink.close().await;
            return;
        }

        let (out, mut outbox) = mpsc::channel(OUTBOX);
        let mut states = state.gateway.state_changes();
        let mut conn = Connection {
            gateway: state.gateway.clone(),
            out,
            subscribed: Vec::new(),
        };
        // Subscribe before the welcome goes out, so no output falls between the two.
        let sessions = state.gateway.sessions();
        conn.subscribe(sessions.iter().map(|s| s.id.clone()).collect());
        let welcome = ToClient::Welcome {
            protocol: wayfarer_proto::PROTOCOL,
            server_build: env!("CARGO_PKG_VERSION").to_owned(),
            sessions,
        };
        if sink.send(frame(welcome)).await.is_err() {
            return;
        }

        loop {
            let reply = tokio::select! {
                _ = stopping.changed() => break,
                Some(msg) = outbox.recv() => Some(msg),
                change = states.recv() => match change {
                    Ok(change) => conn.is_subscribed(&change.session).then_some(
                        ToClient::SessionState { session: change.session, state: change.state },
                    ),
                    Err(broadcast::error::RecvError::Lagged(_)) => Some(ToClient::SessionList {
                        sessions: conn.gateway.sessions(),
                    }),
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                incoming = next_message(&mut stream) => match incoming {
                    None => break,
                    Some(Ok(msg)) => conn.handle(msg).await,
                    Some(Err(message)) => Some(ToClient::Error { message }),
                },
            };
            if let Some(msg) = reply
                && sink.send(frame(msg)).await.is_err()
            {
                break;
            }
        }
        for (_, task) in conn.subscribed {
            task.abort();
        }
        let _ = sink.close().await;
    }

    fn is_subscribed(&self, id: &str) -> bool {
        self.subscribed.iter().any(|(s, _)| s == id)
    }

    /// Replace the subscription with `ids`, which must be known sessions.
    fn subscribe(&mut self, ids: Vec<SessionId>) {
        for (_, task) in self.subscribed.drain(..) {
            task.abort();
        }
        for id in ids {
            let Ok(mut rx) = self.gateway.subscribe(&id) else {
                continue;
            };
            let out = self.out.clone();
            let session = id.clone();
            let task = tokio::spawn(async move {
                loop {
                    let msg = match rx.recv().await {
                        Ok(chunk) => ToClient::Output {
                            session: session.clone(),
                            seq: chunk.seq,
                            text: chunk.text,
                        },
                        Err(broadcast::error::RecvError::Lagged(missed)) => ToClient::Lagged {
                            session: session.clone(),
                            missed,
                        },
                        Err(broadcast::error::RecvError::Closed) => break,
                    };
                    if out.send(msg).await.is_err() {
                        break;
                    }
                }
            });
            self.subscribed.push((id, task));
        }
    }

    /// Act on one message after the handshake; the reply, if there is one.
    async fn handle(&mut self, msg: ToGateway) -> Option<ToClient> {
        let error = |e: GatewayError| ToClient::Error {
            message: e.to_string(),
        };
        match msg {
            ToGateway::Hello { .. } => Some(ToClient::Error {
                message: "already said hello".to_owned(),
            }),
            ToGateway::InputLine { session, text } => self
                .gateway
                .send_line(&session, &text)
                .await
                .err()
                .map(error),
            ToGateway::InputKeys { session, bytes } => self
                .gateway
                .send_keys(&session, &bytes)
                .await
                .err()
                .map(error),
            ToGateway::HistoryRequest {
                session,
                after_seq,
                before_seq,
                max_chunks,
            } => {
                let query = HistoryQuery {
                    after_seq,
                    before_seq,
                    max_chunks,
                };
                Some(match self.gateway.history(&session, query).await {
                    Ok(h) => ToClient::History {
                        session,
                        from_seq: h.from_seq,
                        chunks: h.chunks,
                    },
                    Err(e) => error(e),
                })
            }
            ToGateway::Subscribe { sessions } => {
                let known: Vec<SessionId> =
                    self.gateway.sessions().into_iter().map(|s| s.id).collect();
                let wanted = sessions.unwrap_or_else(|| known.clone());
                if let Some(bad) = wanted.iter().find(|id| !known.contains(id)) {
                    return Some(ToClient::Error {
                        message: format!("unknown session {bad:?}"),
                    });
                }
                self.subscribe(wanted);
                None
            }
        }
    }
}

/// The next protocol message from the socket: `None` when it closed, `Err` for a frame that
/// is not a protocol message (the caller reports it and carries on).
async fn next_message(
    stream: &mut (impl futures_util::Stream<Item = Result<Message, axum::Error>> + Unpin),
) -> Option<Result<ToGateway, String>> {
    loop {
        match stream.next().await? {
            Ok(Message::Text(text)) => {
                return Some(ToGateway::from_json(text.as_str()).map_err(|e| e.to_string()));
            }
            Ok(Message::Binary(_)) => return Some(Err("send text frames, not binary".to_owned())),
            Ok(Message::Ping(_) | Message::Pong(_)) => continue,
            Ok(Message::Close(_)) | Err(_) => return None,
        }
    }
}
