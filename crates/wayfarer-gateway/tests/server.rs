//! The WebSocket server against an in-test game server: a tokio listener on 127.0.0.1:0 and a
//! `tokio-tungstenite` client. Nothing outside the process.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use wayfarer_gateway::{
    Config, Credentials, Gateway, Options, ServerError, ServerOptions, create_token, serve,
    token_for_bind,
};
use wayfarer_proto::{ClientKind, PROTOCOL, SessionState, ToClient, ToGateway};

const ID: &str = "kasimir";
/// Far longer than any test should take; only a broken server waits this long.
const PATIENCE: Duration = Duration::from_secs(5);

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Rig {
    addr: SocketAddr,
    game: TcpListener,
    gateway: Gateway,
    stop: Option<oneshot::Sender<()>>,
    server: JoinHandle<Result<(), ServerError>>,
}

impl Rig {
    async fn start(dir: &Path, token: Option<&str>) -> Self {
        let game = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let config = Config::parse(&format!(
            "[[server]]\nid = \"local\"\nhost = \"127.0.0.1\"\nport = {}\n\n\
             [[character]]\nid = \"{ID}\"\nname = \"Kasimir\"\nserver = \"local\"\n",
            game.local_addr().unwrap().port()
        ))
        .unwrap();
        let gateway = Gateway::new(
            &config,
            &Credentials::default(),
            &dir.join("logs"),
            Options::default(),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel();
        let options = ServerOptions {
            token: token.map(str::to_owned),
            ui_dir: dir.join("no-ui-here"),
        };
        let server = tokio::spawn(serve(listener, gateway.clone(), options, async {
            let _ = stopped.await;
        }));
        Self {
            addr,
            game,
            gateway,
            stop: Some(stop),
            server,
        }
    }

    /// Connect the session to the in-test game server and return the game's end.
    async fn connect_game(&self) -> TcpStream {
        self.gateway.open(ID).unwrap();
        tokio::time::timeout(PATIENCE, self.game.accept())
            .await
            .expect("the gateway connects")
            .unwrap()
            .0
    }

    async fn client(&self) -> Client {
        let url = format!("ws://{}/ws", self.addr);
        tokio_tungstenite::connect_async(url).await.unwrap().0
    }

    /// A client that has said hello and been welcomed.
    async fn welcomed(&self) -> Client {
        let mut ws = self.client().await;
        hello(&mut ws, PROTOCOL, None).await;
        assert!(matches!(recv(&mut ws).await, ToClient::Welcome { .. }));
        ws
    }

    async fn stop(mut self) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(PATIENCE, self.server)
            .await
            .expect("the server stops")
            .unwrap()
            .unwrap();
    }
}

async fn send(ws: &mut Client, msg: &ToGateway) {
    ws.send(Message::text(msg.to_json())).await.unwrap();
}

async fn hello(ws: &mut Client, protocol: u32, token: Option<&str>) {
    let msg = ToGateway::Hello {
        protocol,
        client: ClientKind::Ui,
        token: token.map(str::to_owned),
    };
    send(ws, &msg).await;
}

async fn recv(ws: &mut Client) -> ToClient {
    loop {
        let frame = tokio::time::timeout(PATIENCE, ws.next())
            .await
            .expect("a message arrives")
            .expect("the connection is open")
            .unwrap();
        if let Message::Text(text) = frame {
            return ToClient::from_json(text.as_str()).unwrap();
        }
    }
}

/// Output for `ID` until its text adds up to `len` bytes; the chunks' `seq`s and the text.
async fn recv_output(ws: &mut Client, len: usize) -> (Vec<u64>, String) {
    let (mut seqs, mut text) = (Vec::new(), String::new());
    while text.len() < len {
        match recv(ws).await {
            ToClient::Output {
                session,
                seq,
                text: t,
            } => {
                assert_eq!(session, ID);
                seqs.push(seq);
                text.push_str(&t);
            }
            ToClient::SessionState { .. } => {}
            other => panic!("expected output, got {other:?}"),
        }
    }
    (seqs, text)
}

#[tokio::test]
async fn handshake_welcomes_with_the_session_list_and_health_check_answers() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ws = rig.client().await;
    hello(&mut ws, PROTOCOL, None).await;
    let ToClient::Welcome {
        protocol, sessions, ..
    } = recv(&mut ws).await
    else {
        panic!("expected welcome");
    };
    assert_eq!(protocol, PROTOCOL);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, ID);
    assert_eq!(sessions[0].state, SessionState::Closed);

    let mut http = TcpStream::connect(rig.addr).await.unwrap();
    http.write_all(b"GET /healthz HTTP/1.0\r\n\r\n")
        .await
        .unwrap();
    let mut body = String::new();
    http.read_to_string(&mut body).await.unwrap();
    assert!(body.starts_with("HTTP/1.0 200"), "{body}");
    assert!(body.ends_with(r#"{"sessions":1,"status":"ok"}"#), "{body}");
    rig.stop().await;
}

#[tokio::test]
async fn protocol_mismatch_and_a_first_message_that_is_not_hello_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;

    let mut ws = rig.client().await;
    hello(&mut ws, PROTOCOL + 1, None).await;
    let ToClient::Refused { reason } = recv(&mut ws).await else {
        panic!("expected refused");
    };
    assert!(reason.contains("protocol mismatch"), "{reason}");

    let mut ws = rig.client().await;
    send(
        &mut ws,
        &ToGateway::InputLine {
            session: ID.into(),
            text: "look".into(),
        },
    )
    .await;
    assert!(matches!(recv(&mut ws).await, ToClient::Refused { .. }));
    rig.stop().await;
}

#[tokio::test]
async fn output_arrives_and_input_reaches_the_game_server() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ws = rig.welcomed().await;
    let mut game = rig.connect_game().await;

    game.write_all(b"Welcome, traveller.\r\n").await.unwrap();
    let (seqs, text) = recv_output(&mut ws, 21).await;
    assert_eq!(text, "Welcome, traveller.\r\n");
    assert_eq!(seqs[0], 1);

    // Input is refused while the session is still connecting.
    while rig.gateway.state(ID).unwrap() == SessionState::Connecting {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    send(
        &mut ws,
        &ToGateway::InputLine {
            session: ID.into(),
            text: "look".into(),
        },
    )
    .await;
    let mut got = [0u8; 5];
    tokio::time::timeout(PATIENCE, game.read_exact(&mut got))
        .await
        .expect("the line reaches the game")
        .unwrap();
    assert_eq!(&got, b"look\r");
    rig.stop().await;
}

#[tokio::test]
async fn history_request_answers_and_bad_messages_get_an_error_without_closing() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ws = rig.welcomed().await;
    let mut game = rig.connect_game().await;
    game.write_all(b"one\r\n").await.unwrap();
    let (seqs, _) = recv_output(&mut ws, 5).await;

    ws.send(Message::text("not json")).await.unwrap();
    assert!(matches!(recv(&mut ws).await, ToClient::Error { .. }));
    send(
        &mut ws,
        &ToGateway::InputLine {
            session: "nobody".into(),
            text: "hi".into(),
        },
    )
    .await;
    assert!(matches!(recv(&mut ws).await, ToClient::Error { .. }));

    send(
        &mut ws,
        &ToGateway::HistoryRequest {
            session: ID.into(),
            after_seq: None,
            before_seq: None,
            max_chunks: 10,
        },
    )
    .await;
    let ToClient::History {
        session,
        from_seq,
        chunks,
    } = recv(&mut ws).await
    else {
        panic!("expected history");
    };
    assert_eq!(session, ID);
    assert_eq!(from_seq, seqs[0]);
    assert_eq!(
        chunks.iter().map(|c| c.text.as_str()).collect::<String>(),
        "one\r\n"
    );
    rig.stop().await;
}

#[tokio::test]
async fn two_clients_each_get_the_output_and_subscribe_narrows_it() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut a = rig.welcomed().await;
    let mut b = rig.welcomed().await;
    let mut game = rig.connect_game().await;

    game.write_all(b"hello all\r\n").await.unwrap();
    assert_eq!(recv_output(&mut a, 11).await.1, "hello all\r\n");
    assert_eq!(recv_output(&mut b, 11).await.1, "hello all\r\n");

    send(
        &mut b,
        &ToGateway::Subscribe {
            sessions: Some(vec![]),
        },
    )
    .await;
    // The server handles messages in order, so the unknown-session error proves the empty
    // subscription is in force before the next output is sent.
    send(
        &mut b,
        &ToGateway::Subscribe {
            sessions: Some(vec!["nobody".into()]),
        },
    )
    .await;
    assert!(matches!(recv(&mut b).await, ToClient::Error { .. }));
    game.write_all(b"second\r\n").await.unwrap();
    assert_eq!(recv_output(&mut a, 8).await.1, "second\r\n");
    assert!(
        tokio::time::timeout(Duration::from_millis(200), b.next())
            .await
            .is_err(),
        "b unsubscribed, so it hears nothing"
    );
    rig.stop().await;
}

#[tokio::test]
async fn state_changes_reach_subscribed_clients() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ws = rig.welcomed().await;
    let _game = rig.connect_game().await;
    let mut seen = Vec::new();
    while seen.last() != Some(&SessionState::Connected) {
        let ToClient::SessionState { session, state } = recv(&mut ws).await else {
            panic!("expected a state change");
        };
        assert_eq!(session, ID);
        seen.push(state);
    }
    assert_eq!(seen, [SessionState::Connecting, SessionState::Connected]);
    rig.stop().await;
}

#[tokio::test]
async fn a_token_is_required_when_one_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), Some("s3cret")).await;
    for token in [None, Some("wrong")] {
        let mut ws = rig.client().await;
        hello(&mut ws, PROTOCOL, token).await;
        let ToClient::Refused { reason } = recv(&mut ws).await else {
            panic!("expected refused");
        };
        assert!(reason.contains("token"), "{reason}");
    }
    let mut ws = rig.client().await;
    hello(&mut ws, PROTOCOL, Some("s3cret")).await;
    assert!(matches!(recv(&mut ws).await, ToClient::Welcome { .. }));
    rig.stop().await;
}

#[tokio::test]
async fn a_missing_ui_directory_says_so_and_stopping_closes_the_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ws = rig.welcomed().await;
    let mut game = rig.connect_game().await;

    let mut http = TcpStream::connect(rig.addr).await.unwrap();
    http.write_all(b"GET / HTTP/1.0\r\n\r\n").await.unwrap();
    let mut body = String::new();
    http.read_to_string(&mut body).await.unwrap();
    assert!(body.starts_with("HTTP/1.0 503"), "{body}");
    assert!(body.contains("not built"), "{body}");

    let gateway = rig.gateway.clone();
    rig.stop().await;
    assert_eq!(gateway.state(ID).unwrap(), SessionState::Closed);
    // The connection to the game was dropped, and the WebSocket ended.
    let mut rest = Vec::new();
    tokio::time::timeout(PATIENCE, game.read_to_end(&mut rest))
        .await
        .expect("the game sees the disconnect")
        .unwrap();
    let ended = tokio::time::timeout(PATIENCE, async { while ws.next().await.is_some() {} }).await;
    assert!(ended.is_ok(), "the WebSocket ends");
}

#[test]
fn a_non_loopback_bind_needs_a_token_and_a_loopback_one_needs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("gateway-token");
    let lan: SocketAddr = "0.0.0.0:7373".parse().unwrap();
    let local: SocketAddr = "127.0.0.1:7373".parse().unwrap();

    assert!(matches!(
        token_for_bind(lan, &file),
        Err(ServerError::TokenRequired(_))
    ));
    assert_eq!(token_for_bind(local, &file).unwrap(), None);

    let token = create_token(&file).unwrap();
    assert_eq!(token.len(), 48);
    assert_eq!(token_for_bind(lan, &file).unwrap(), Some(token.clone()));
    assert_eq!(token_for_bind(local, &file).unwrap(), None);
    assert_ne!(create_token(&file).unwrap(), token, "a new token each time");
}
