//! The network face of the gateway: an HTTP server with the WebSocket endpoint (`/ws`, the
//! C-002 protocol as JSON text frames), a health check and the web UI's static files; and the
//! [`Hub`] behind it, which connects clients to the gateway and relays between the brain and
//! the UIs.
//!
//! No TLS: remote access goes through SSH or Tailscale. On a loopback bind the handshake's
//! token is not checked; on any other bind the gateway will not start without one.

use std::collections::HashSet;
use std::future::{Future, IntoFuture};
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use futures_util::{FutureExt, Sink, SinkExt, Stream, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tower_http::services::ServeDir;
use wayfarer_proto::{ClientKind, Family, SessionId, ToClient, ToGateway, protocol_mismatch};

use crate::session::{Output, Sent};
use crate::{Gateway, GatewayError, HistoryQuery};

/// How long a new connection may take to say hello.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Messages queued for one connection's socket before its forwarders wait.
const OUTBOX: usize = 256;
/// Brain messages a UI may fall behind by before it is told `brain_lagged`.
const RELAY: usize = 1024;
/// How long a session's log-off waits for the brain's `ready_to_leave` after `logging_off`.
pub const LEAVE_PATIENCE: Duration = Duration::from_secs(30);

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

/// What [`serve`] needs besides the hub.
#[derive(Debug, Clone)]
pub struct ServerOptions {
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

/// The clients' side of the gateway: the handshake, each client's subscriptions, the one brain
/// and the relay between it and the UIs. [`serve`] puts it behind the WebSocket at `/ws`;
/// [`Hub::connect`] attaches a client over channels, with no socket (an in-process brain).
/// Both kinds of connection follow the same rules. Cheap to clone; every clone is the same hub.
#[derive(Clone)]
pub struct Hub {
    inner: Arc<HubInner>,
}

struct HubInner {
    gateway: Gateway,
    token: Option<String>,
    /// The attached brain, if any; UIs watch it for `brain_state`.
    brain: watch::Sender<Option<Brain>>,
    /// Brain messages on their way to the UIs.
    relay: broadcast::Sender<ToClient>,
    /// Turns `true` when the hub stops, so every connection ends.
    stopping: watch::Sender<bool>,
    /// The sessions the brain has said `ready_to_leave` for since their `logging_off`.
    ready: watch::Sender<HashSet<SessionId>>,
    /// Numbers brain registrations, so a guard clears only its own.
    next_id: AtomicU64,
}

/// The attached brain: its connection's outbox, where UIs' control messages go.
struct Brain {
    id: u64,
    out: mpsc::Sender<ToClient>,
}

/// Holds the brain's registration while its connection lives; dropping it detaches the brain.
struct BrainGuard {
    hub: Hub,
    id: u64,
}

impl Drop for BrainGuard {
    fn drop(&mut self) {
        self.hub.inner.brain.send_if_modified(|brain| {
            let ours = brain.as_ref().is_some_and(|b| b.id == self.id);
            if ours {
                *brain = None;
            }
            ours
        });
    }
}

impl Hub {
    /// `token`: what a client's `hello` must carry; `None` checks nothing (loopback binds
    /// only, [`token_for_bind`]).
    pub fn new(gateway: Gateway, token: Option<String>) -> Self {
        Self {
            inner: Arc::new(HubInner {
                gateway,
                token,
                brain: watch::channel(None).0,
                relay: broadcast::channel(RELAY).0,
                stopping: watch::channel(false).0,
                ready: watch::channel(HashSet::new()).0,
                next_id: AtomicU64::new(0),
            }),
        }
    }

    pub fn gateway(&self) -> &Gateway {
        &self.inner.gateway
    }

    /// End every connection, and any made later. [`serve`] does this when it stops, once
    /// every session has logged off ([`Hub::shutdown_unless`]).
    pub fn stop(&self) {
        self.inner.stopping.send_replace(true);
    }

    /// Log every session off and then end every connection. Each session in the realm is
    /// announced to the brain first (`logging_off`) and its log-off waits for the brain's
    /// `ready_to_leave`, up to [`LEAVE_PATIENCE`] or until the brain goes
    /// ([`Gateway::shutdown_after`]); the brain's connection, like the UIs', stays open until
    /// every log-off has finished or given up. If `now` completes first, what is left is
    /// closed at once ([`Gateway::shutdown_now`]).
    pub async fn shutdown_unless(&self, now: impl Future<Output = ()>) {
        let gateway = self.gateway().clone();
        let hub = self.clone();
        let logoff = gateway.shutdown_after(move |id| {
            let hub = hub.clone();
            async move { hub.ready_to_leave(id).await }
        });
        tokio::select! {
            () = logoff => {}
            () = now => {
                tracing::warn!("closing every session at once");
                gateway.shutdown_now().await;
            }
        }
        self.stop();
    }

    /// Tell the brain that `session` is about to be logged off, and wait for its
    /// `ready_to_leave` (at most [`LEAVE_PATIENCE`]), or for it to go. At once with no brain.
    async fn ready_to_leave(&self, session: SessionId) {
        let Some((id, out)) = self
            .inner
            .brain
            .borrow()
            .as_ref()
            .map(|b| (b.id, b.out.clone()))
        else {
            return;
        };
        self.inner.ready.send_modify(|ready| {
            ready.remove(&session);
        });
        let mut ready = self.inner.ready.subscribe();
        let mut brain = self.inner.brain.subscribe();
        tracing::info!(%session, "asking the brain to get ready to leave");
        let answer = async {
            if out
                .send(ToClient::LoggingOff {
                    session: session.clone(),
                })
                .await
                .is_err()
            {
                return "the brain has gone";
            }
            tokio::select! {
                _ = ready.wait_for(|r| r.contains(&session)) => "the brain is ready",
                _ = brain.wait_for(|b| b.as_ref().is_none_or(|b| b.id != id)) => "the brain has gone",
            }
        };
        match tokio::time::timeout(LEAVE_PATIENCE, answer).await {
            Ok(why) => tracing::info!(%session, "{why}: logging off"),
            Err(_) => tracing::warn!(
                %session,
                "no ready_to_leave from the brain within {} s: logging off anyway",
                LEAVE_PATIENCE.as_secs()
            ),
        }
    }

    /// Attach a client over a pair of channels instead of a WebSocket: send its `hello` first,
    /// then anything else, on the returned sender; what the gateway sends comes on the
    /// receiver. The connection ends when the sender is dropped (or the hub stops), and the
    /// receiver then ends too.
    pub fn connect(&self) -> (mpsc::Sender<ToGateway>, mpsc::Receiver<ToClient>) {
        let (to_gateway, from_client) = mpsc::channel(OUTBOX);
        let (to_client, from_gateway) = mpsc::channel(OUTBOX);
        let sink = futures_util::sink::unfold(to_client, |tx, msg: ToClient| async move {
            tx.send(msg).await.map(|()| tx)
        });
        let incoming = futures_util::stream::unfold(from_client, |mut rx| async move {
            rx.recv().await.map(|msg| (Ok(msg), rx))
        });
        tokio::spawn(Connection::run(self.clone(), sink, incoming));
        (to_gateway, from_gateway)
    }

    /// Register `out` as the brain's outbox, unless a brain is attached already.
    fn attach_brain(&self, out: mpsc::Sender<ToClient>) -> Option<BrainGuard> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let attached = self.inner.brain.send_if_modified(|brain| {
            let free = brain.is_none();
            if free {
                *brain = Some(Brain { id, out });
            }
            free
        });
        attached.then(|| BrainGuard {
            hub: self.clone(),
            id,
        })
    }

    /// The `refused` reason for a hello, or `None` to welcome it.
    fn check_hello(&self, protocol: u32, token: Option<&str>) -> Option<String> {
        protocol_mismatch(protocol).or_else(|| {
            let ok = self
                .inner
                .token
                .as_deref()
                .is_none_or(|want| token.is_some_and(|got| tokens_match(want, got)));
            (!ok).then(|| "missing or wrong token".to_owned())
        })
    }
}

#[derive(Clone)]
struct AppState {
    hub: Hub,
    ui_dir: Arc<Path>,
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

/// Serve `hub` on `listener` until the first item of `stops`, then stop accepting, log every
/// session off, the brain asked first, and end the connections ([`Hub::shutdown_unless`]); a
/// second item closes what is left at once ([`Gateway::shutdown_now`]). The binary passes
/// [`crate::stop_signals`].
pub async fn serve(
    listener: TcpListener,
    hub: Hub,
    options: ServerOptions,
    stops: impl Stream<Item = ()> + Send,
) -> Result<(), ServerError> {
    let mut stops = std::pin::pin!(stops);
    let mut stopped = hub.inner.stopping.subscribe();
    // Stop accepting as soon as the first stop comes; the connections stay until the hub stops.
    let (closing, mut closed) = watch::channel(false);
    let state = AppState {
        hub: hub.clone(),
        ui_dir: Arc::from(options.ui_dir.as_path()),
    };
    let ui = ServeDir::new(&options.ui_dir).fallback(get(ui_missing).with_state(state.clone()));
    let app = Router::new()
        .route("/ws", get(upgrade))
        .route("/healthz", get(healthz))
        .fallback_service(ui)
        .with_state(state);
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = stopped.wait_for(|stopped| *stopped) => {}
                _ = closed.wait_for(|closed| *closed) => {}
            }
        })
        .into_future();
    let mut server = std::pin::pin!(server);
    let ended = tokio::select! {
        result = &mut server => Some(result),
        () = next_or_never(&mut stops) => {
            tracing::info!("stopping: logging every character off (stop again to close at once)");
            None
        }
    };
    closing.send_replace(true);
    let logoff = hub.shutdown_unless(next_or_never(&mut stops));
    let result = match ended {
        Some(result) => {
            logoff.await;
            result
        }
        // The server finishes once the hub has stopped and its connections have ended.
        None => tokio::join!(server, logoff).0,
    };
    result.map_err(|source| ServerError::Io {
        path: PathBuf::from("<server>"),
        source,
    })
}

async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "sessions": state.hub.gateway().sessions().len(),
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
    ws.on_upgrade(move |socket| {
        let (sink, stream) = socket.split();
        let sink = sink.with(|msg: ToClient| {
            std::future::ready(Ok::<_, axum::Error>(Message::text(msg.to_json())))
        });
        let incoming = futures_util::stream::unfold(stream, |mut stream| async move {
            next_message(&mut stream).await.map(|msg| (msg, stream))
        });
        Connection::run(state.hub, sink, incoming)
    })
}

/// The next item of `stops`; never, once it has ended.
async fn next_or_never(stops: &mut (impl Stream<Item = ()> + Unpin)) {
    if stops.next().await.is_none() {
        std::future::pending::<()>().await;
    }
}

/// Same length and content, without stopping at the first difference.
fn tokens_match(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |d, (x, y)| d | (x ^ y)) == 0
}

struct Connection {
    hub: Hub,
    kind: ClientKind,
    out: mpsc::Sender<ToClient>,
    /// The sessions this connection receives, each with its output forwarding task.
    subscribed: Vec<(SessionId, JoinHandle<()>)>,
    /// The same sessions, for the forwarders below.
    sessions: watch::Sender<Vec<SessionId>>,
    /// Forwarders for the life of the connection: relayed brain messages (a UI).
    forwarders: Vec<JoinHandle<()>>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        for task in self
            .subscribed
            .iter()
            .map(|(_, t)| t)
            .chain(&self.forwarders)
        {
            task.abort();
        }
    }
}

impl Connection {
    /// Serve one client: messages to it go into `sink`, and `incoming` yields what it sends
    /// (`Err` for something that is not a protocol message) and ends when it has gone.
    async fn run<S, I>(hub: Hub, sink: S, incoming: I)
    where
        S: Sink<ToClient> + Send,
        I: Stream<Item = Result<ToGateway, String>> + Send,
    {
        let mut sink = std::pin::pin!(sink);
        let mut incoming = std::pin::pin!(incoming);
        let mut stopping = hub.inner.stopping.subscribe();
        let (out, mut outbox) = mpsc::channel(OUTBOX);

        // The handshake.
        let hello = tokio::time::timeout(HELLO_TIMEOUT, incoming.next()).await;
        let mut brain_guard = None;
        let accepted = match hello.ok().flatten() {
            None => Err("no hello received".to_owned()),
            Some(Err(e)) => Err(format!("expected a hello: {e}")),
            Some(Ok(ToGateway::Hello {
                protocol,
                client,
                token,
            })) => match hub.check_hello(protocol, token.as_deref()) {
                Some(reason) => Err(reason),
                None if client == ClientKind::Brain => match hub.attach_brain(out.clone()) {
                    Some(guard) => {
                        brain_guard = Some(guard);
                        Ok(client)
                    }
                    None => Err("a brain is already attached".to_owned()),
                },
                None => Ok(client),
            },
            Some(Ok(other)) => Err(format!(
                "expected a hello first, got {:?}",
                other.type_name()
            )),
        };
        let kind = match accepted {
            Ok(kind) => kind,
            Err(reason) => {
                let _ = sink.send(ToClient::Refused { reason }).await;
                let _ = sink.close().await;
                return;
            }
        };

        let gateway = hub.gateway().clone();
        let mut states = gateway.state_changes();
        let mut brain = hub.inner.brain.subscribe();
        let mut conn = Connection {
            hub: hub.clone(),
            kind,
            out,
            subscribed: Vec::new(),
            sessions: watch::channel(Vec::new()).0,
            forwarders: Vec::new(),
        };
        // Subscribe before the welcome goes out, so nothing falls between the two.
        let sessions = gateway.sessions();
        conn.subscribe(sessions.iter().map(|s| s.id.clone()).collect());
        if kind == ClientKind::Ui {
            conn.forward(hub.inner.relay.subscribe(), |msg| match msg {
                Ok(msg) => Some(msg),
                Err(missed) => Some(ToClient::BrainLagged { missed }),
            });
        }
        let welcome = ToClient::Welcome {
            protocol: wayfarer_proto::PROTOCOL,
            server_build: env!("CARGO_PKG_VERSION").to_owned(),
            sessions,
        };
        if sink.send(welcome).await.is_err() {
            return;
        }
        if kind == ClientKind::Ui {
            let attached = brain.borrow_and_update().is_some();
            if sink.send(ToClient::BrainState { attached }).await.is_err() {
                return;
            }
        }

        loop {
            let reply = tokio::select! {
                () = stopping.wait_for(|stopping| *stopping).map(drop) => break,
                Some(msg) = outbox.recv() => Some(msg),
                change = states.recv() => match change {
                    Ok(change) => conn.is_subscribed(&change.session).then_some(
                        ToClient::SessionState { session: change.session, state: change.state },
                    ),
                    Err(broadcast::error::RecvError::Lagged(_)) => Some(ToClient::SessionList {
                        sessions: gateway.sessions(),
                    }),
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                Ok(()) = brain.changed(), if kind == ClientKind::Ui => Some(ToClient::BrainState {
                    attached: brain.borrow_and_update().is_some(),
                }),
                incoming = incoming.next() => match incoming {
                    None => break,
                    Some(Ok(msg)) => conn.handle(msg).await,
                    Some(Err(message)) => Some(ToClient::Error { message }),
                },
            };
            if let Some(msg) = reply
                && sink.send(msg).await.is_err()
            {
                break;
            }
        }
        drop(brain_guard);
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
        self.sessions.send_replace(ids.clone());
        for id in ids {
            let Ok(mut rx) = self.hub.gateway().subscribe(&id) else {
                continue;
            };
            let out = self.out.clone();
            let session = id.clone();
            // The brain gets `sent` from the same stream as the output, so in its order.
            let sent_too = self.kind == ClientKind::Brain;
            let task = tokio::spawn(async move {
                loop {
                    let msg = match rx.recv().await {
                        Ok(Output::Chunk(chunk)) => ToClient::Output {
                            session: session.clone(),
                            seq: chunk.seq,
                            text: chunk.text,
                        },
                        Ok(Output::Sent(Sent { session, seq, text })) if sent_too => {
                            ToClient::Sent { session, seq, text }
                        }
                        Ok(Output::Sent(_)) => continue,
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

    /// Forward what `rx` carries to this connection while it lives, as `to_msg` turns it into
    /// a message (from `Err(missed)` when the forwarder lagged), keeping only messages without
    /// a session or about a subscribed one. A slow client holds up only its own forwarder,
    /// which then lags.
    fn forward<T: Clone + Send + 'static>(
        &mut self,
        mut rx: broadcast::Receiver<T>,
        to_msg: impl Fn(Result<T, u64>) -> Option<ToClient> + Send + 'static,
    ) {
        let out = self.out.clone();
        let sessions = self.sessions.subscribe();
        self.forwarders.push(tokio::spawn(async move {
            loop {
                let item = match rx.recv().await {
                    Ok(item) => Ok(item),
                    Err(broadcast::error::RecvError::Lagged(missed)) => Err(missed),
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                let Some(msg) = to_msg(item) else { continue };
                let wanted = msg
                    .session()
                    .is_none_or(|id| sessions.borrow().iter().any(|s| s == id));
                if wanted && out.send(msg).await.is_err() {
                    break;
                }
            }
        }));
    }

    /// Act on one message after the handshake; the reply, if there is one.
    async fn handle(&mut self, msg: ToGateway) -> Option<ToClient> {
        let family = msg.family();
        if !family.allowed_from(self.kind) {
            let sender = match self.kind {
                ClientKind::Ui => "a UI",
                ClientKind::Brain => "the brain",
            };
            return Some(ToClient::Error {
                message: format!("{sender} may not send {:?}", msg.type_name()),
            });
        }
        if family == Family::Gateway {
            return self.handle_own(msg).await;
        }
        if let ToGateway::ReadyToLeave { session } = msg {
            self.hub.inner.ready.send_modify(|ready| {
                ready.insert(session);
            });
            return None;
        }
        let msg = msg
            .into_relayed()
            .expect("control and brain messages are relayed");
        if family == Family::Brain {
            // No UI listening is fine.
            let _ = self.hub.inner.relay.send(msg);
            return None;
        }
        let brain = self
            .hub
            .inner
            .brain
            .borrow()
            .as_ref()
            .map(|b| b.out.clone());
        match brain {
            // If the brain's connection closes meanwhile the message is lost, as it would
            // have been a moment later.
            Some(brain) => {
                let _ = brain.send(msg).await;
                None
            }
            None => Some(ToClient::Error {
                message: "no brain attached".to_owned(),
            }),
        }
    }

    /// A message the gateway handles itself ([`Family::Gateway`]).
    async fn handle_own(&mut self, msg: ToGateway) -> Option<ToClient> {
        let gateway = self.hub.gateway().clone();
        let error = |e: GatewayError| ToClient::Error {
            message: e.to_string(),
        };
        match msg {
            ToGateway::Hello { .. } => Some(ToClient::Error {
                message: "already said hello".to_owned(),
            }),
            // Input arbitration hooks in here once a bot plays (docs/plan/02-architecture.md):
            // with a brain attached, a UI's line goes to the brain as `user_input`. No bot
            // plays in M1, so every client's input goes straight to the game.
            ToGateway::InputLine { session, text } => {
                gateway.send_line(&session, &text).await.err().map(error)
            }
            ToGateway::InputKeys { session, bytes } => {
                gateway.send_keys(&session, &bytes).await.err().map(error)
            }
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
                Some(match gateway.history(&session, query).await {
                    Ok(h) => ToClient::History {
                        session,
                        from_seq: h.from_seq,
                        chunks: h.chunks,
                    },
                    Err(e) => error(e),
                })
            }
            ToGateway::Subscribe { sessions } => {
                let known: Vec<SessionId> = gateway.sessions().into_iter().map(|s| s.id).collect();
                let wanted = sessions.unwrap_or_else(|| known.clone());
                if let Some(bad) = wanted.iter().find(|id| !known.contains(id)) {
                    return Some(ToClient::Error {
                        message: format!("unknown session {bad:?}"),
                    });
                }
                self.subscribe(wanted);
                None
            }
            relayed => unreachable!("{:?} is relayed", relayed.type_name()),
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
