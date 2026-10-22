//! The WebSocket server against an in-test game server: a tokio listener on 127.0.0.1:0 and a
//! `tokio-tungstenite` client. Nothing outside the process.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use wayfarer_gateway::{
    Config, Credentials, Gateway, Hub, Options, ServerError, ServerOptions, create_token, serve,
    token_for_bind,
};
use wayfarer_proto::{
    CharacterState, ClientKind, PROTOCOL, SessionAction, SessionState, ToClient, ToGateway,
};

const ID: &str = "kasimir";
/// A second character, never connected.
const OTHER: &str = "brienne";
/// Kasimir's password, which `sent` must never carry.
const PASSWORD: &str = "hunter2";
/// Far longer than any test should take; only a broken server waits this long.
const PATIENCE: Duration = Duration::from_secs(5);

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Rig {
    addr: SocketAddr,
    game: TcpListener,
    gateway: Gateway,
    hub: Hub,
    stop: Option<oneshot::Sender<()>>,
    server: JoinHandle<Result<(), ServerError>>,
}

impl Rig {
    async fn start(dir: &Path, token: Option<&str>) -> Self {
        let game = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let config = Config::parse(&format!(
            "[[server]]\nid = \"local\"\nhost = \"127.0.0.1\"\nport = {}\n\n\
             [[character]]\nid = \"{ID}\"\nname = \"Kasimir\"\nserver = \"local\"\n\n\
             [[character]]\nid = \"{OTHER}\"\nname = \"Brienne\"\nserver = \"local\"\n",
            game.local_addr().unwrap().port()
        ))
        .unwrap();
        // No logon script: the password is only there to be masked.
        let credentials = Credentials::parse(&format!(
            "[character.{ID}]\nuser = \"Kasimir\"\npassword = \"{PASSWORD}\"\n"
        ))
        .unwrap();
        let gateway =
            Gateway::new(&config, &credentials, &dir.join("logs"), Options::default()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel();
        let options = ServerOptions {
            ui_dir: dir.join("no-ui-here"),
        };
        let hub = Hub::new(gateway.clone(), token.map(str::to_owned));
        let stops = futures_util::stream::once(async {
            let _ = stopped.await;
        });
        let server = tokio::spawn(serve(listener, hub.clone(), options, stops));
        Self {
            addr,
            game,
            gateway,
            hub,
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

    /// A UI that has said hello and been welcomed, and told no brain is attached.
    async fn welcomed(&self) -> Client {
        let mut ws = self.client().await;
        hello(&mut ws, PROTOCOL, None).await;
        assert!(matches!(recv(&mut ws).await, ToClient::Welcome { .. }));
        assert_eq!(
            recv(&mut ws).await,
            ToClient::BrainState { attached: false }
        );
        ws
    }

    /// A brain that has said hello and been welcomed.
    async fn brain(&self) -> Client {
        let mut ws = self.client().await;
        send(&mut ws, &brain_hello()).await;
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

fn brain_hello() -> ToGateway {
    ToGateway::Hello {
        protocol: PROTOCOL,
        client: ClientKind::Brain,
        token: None,
    }
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
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, ID);
    assert_eq!(sessions[0].state, SessionState::Closed);

    let mut http = TcpStream::connect(rig.addr).await.unwrap();
    http.write_all(b"GET /healthz HTTP/1.0\r\n\r\n")
        .await
        .unwrap();
    let mut body = String::new();
    http.read_to_string(&mut body).await.unwrap();
    assert!(body.starts_with("HTTP/1.0 200"), "{body}");
    assert!(body.ends_with(r#"{"sessions":2,"status":"ok"}"#), "{body}");
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

/// The next message `want` accepts, skipping the others (output and state changes go on
/// meanwhile).
async fn recv_where(ws: &mut Client, want: impl Fn(&ToClient) -> bool) -> ToClient {
    loop {
        let msg = recv(ws).await;
        if want(&msg) {
            return msg;
        }
    }
}

/// The next message on a channel client; `None` once the gateway has closed it.
async fn recv_channel(rx: &mut mpsc::Receiver<ToClient>) -> Option<ToClient> {
    tokio::time::timeout(PATIENCE, rx.recv())
        .await
        .expect("a message or the end arrives")
}

fn error(message: &str) -> ToClient {
    ToClient::Error {
        message: message.to_owned(),
    }
}

fn kasimir_state() -> CharacterState {
    CharacterState {
        name: Some("Kasimir".into()),
        hp: Some(30),
        hp_max: Some(35),
        room_exits: Some(vec!["north".into(), "south".into()]),
        ..CharacterState::default()
    }
}

#[tokio::test]
async fn one_brain_attaches_and_the_uis_hear_when_it_comes_and_goes() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ui = rig.welcomed().await;
    let brain = rig.brain().await;
    assert_eq!(recv(&mut ui).await, ToClient::BrainState { attached: true });

    let mut second = rig.client().await;
    send(&mut second, &brain_hello()).await;
    assert_eq!(
        recv(&mut second).await,
        ToClient::Refused {
            reason: "a brain is already attached".into()
        }
    );

    // A UI that arrives now is told at once, right after the welcome.
    let mut late = rig.client().await;
    hello(&mut late, PROTOCOL, None).await;
    assert!(matches!(recv(&mut late).await, ToClient::Welcome { .. }));
    assert_eq!(
        recv(&mut late).await,
        ToClient::BrainState { attached: true }
    );

    drop(brain);
    assert_eq!(
        recv(&mut ui).await,
        ToClient::BrainState { attached: false }
    );
    assert_eq!(
        recv(&mut late).await,
        ToClient::BrainState { attached: false }
    );
    // The next brain may attach.
    let _brain = rig.brain().await;
    assert_eq!(recv(&mut ui).await, ToClient::BrainState { attached: true });
    rig.stop().await;
}

#[tokio::test]
async fn brain_messages_reach_the_uis_subscribed_to_their_session_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut all = rig.welcomed().await;
    let mut other = rig.welcomed().await;
    send(
        &mut other,
        &ToGateway::Subscribe {
            sessions: Some(vec![OTHER.into()]),
        },
    )
    .await;
    // Messages are handled in order: once this error is back, the subscription is in force.
    send(&mut other, &ToGateway::Resync { session: ID.into() }).await;
    assert_eq!(recv(&mut other).await, error("no brain attached"));
    let mut brain = rig.brain().await;
    for ui in [&mut all, &mut other] {
        assert_eq!(recv(ui).await, ToClient::BrainState { attached: true });
    }

    let fields =
        serde_json::json!({"damage": 7, "monster": "giant rat", "nested": {"x": [1, null]}});
    let sent = [
        ToGateway::Character {
            session: ID.into(),
            state: kasimir_state().into(),
        },
        ToGateway::CharacterPatch {
            session: ID.into(),
            patch: serde_json::json!({"hp": 23, "room_title": null, "stats": {"charm": 40}}),
        },
        ToGateway::Event {
            session: ID.into(),
            seq: 43,
            line_no: 120,
            name: "you_hit".into(),
            cat: "combat".into(),
            fields: fields.as_object().unwrap().clone(),
            line: "You slash the giant rat for 7 damage!".into(),
        },
        ToGateway::Character {
            session: OTHER.into(),
            state: Box::default(),
        },
        ToGateway::Notice {
            session: None,
            text: "parser reset".into(),
        },
        ToGateway::BrainInfo {
            build: "0.1.0 (abc123)".into(),
            started_at: "2026-10-19T12:00:00Z".into(),
            world_pack: None,
        },
    ];
    for msg in &sent {
        send(&mut brain, msg).await;
    }
    let relayed: Vec<ToClient> = sent
        .into_iter()
        .map(|msg| msg.into_relayed().unwrap())
        .collect();
    for want in &relayed {
        assert_eq!(&recv(&mut all).await, want);
    }
    // Only the other session's snapshot and the messages for everyone, in order.
    for want in &relayed[3..] {
        assert_eq!(&recv(&mut other).await, want);
    }
    rig.stop().await;
}

#[tokio::test]
async fn control_reaches_the_brain_and_each_side_may_send_only_its_own_families() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ui = rig.welcomed().await;
    send(&mut ui, &ToGateway::Resync { session: ID.into() }).await;
    assert_eq!(recv(&mut ui).await, error("no brain attached"));

    let mut brain = rig.brain().await;
    assert_eq!(recv(&mut ui).await, ToClient::BrainState { attached: true });
    send(&mut ui, &ToGateway::Resync { session: ID.into() }).await;
    send(&mut ui, &ToGateway::StateRequest { session: None }).await;
    assert_eq!(
        recv(&mut brain).await,
        ToClient::Resync { session: ID.into() }
    );
    assert_eq!(
        recv(&mut brain).await,
        ToClient::StateRequest { session: None }
    );

    let notice = ToGateway::Notice {
        session: None,
        text: "re-synced".into(),
    };
    send(&mut ui, &notice).await;
    assert_eq!(recv(&mut ui).await, error(r#"a UI may not send "notice""#));
    send(&mut brain, &ToGateway::Resync { session: ID.into() }).await;
    assert_eq!(
        recv(&mut brain).await,
        error(r#"the brain may not send "resync""#)
    );
    // Both connections are still up.
    send(&mut brain, &notice).await;
    assert_eq!(recv(&mut ui).await, notice.into_relayed().unwrap());
    rig.stop().await;
}

#[tokio::test]
async fn sent_tells_the_brain_each_line_that_went_to_the_game_with_the_password_masked() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ui = rig.welcomed().await;
    let mut brain = rig.brain().await;
    assert_eq!(recv(&mut ui).await, ToClient::BrainState { attached: true });
    let mut game = rig.connect_game().await;
    game.write_all(b"Password: ").await.unwrap();
    recv_output(&mut ui, 10).await;
    while rig.gateway.state(ID).unwrap() != SessionState::Connected {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    for text in ["look", PASSWORD] {
        let line = ToGateway::InputLine {
            session: ID.into(),
            text: text.into(),
        };
        send(&mut ui, &line).await;
    }
    // Keys are not lines.
    send(
        &mut ui,
        &ToGateway::InputKeys {
            session: ID.into(),
            bytes: vec![b'q'],
        },
    )
    .await;
    let mut got = [0u8; 14];
    tokio::time::timeout(PATIENCE, game.read_exact(&mut got))
        .await
        .expect("the lines reach the game")
        .unwrap();
    assert_eq!(&got, b"look\rhunter2\rq");
    // The brain's own lines too.
    send(
        &mut brain,
        &ToGateway::InputLine {
            session: ID.into(),
            text: "st".into(),
        },
    )
    .await;
    let mut got = [0u8; 3];
    tokio::time::timeout(PATIENCE, game.read_exact(&mut got))
        .await
        .expect("the line reaches the game")
        .unwrap();
    assert_eq!(&got, b"st\r");

    for text in ["look", "********", "st"] {
        let msg = recv_where(&mut brain, |m| matches!(m, ToClient::Sent { .. })).await;
        assert_eq!(
            msg,
            ToClient::Sent {
                session: ID.into(),
                seq: 1,
                text: text.into()
            }
        );
    }
    // UIs never get `sent`; the line above proves the UI's messages are flowing.
    send(&mut ui, &ToGateway::Resync { session: ID.into() }).await;
    recv_where(&mut brain, |m| matches!(m, ToClient::Resync { .. })).await;
    rig.stop().await;
    while let Some(Ok(Message::Text(text))) = ui.next().await {
        let msg = ToClient::from_json(text.as_str()).unwrap();
        assert!(!matches!(msg, ToClient::Sent { .. }), "a UI got {msg:?}");
    }
}

#[tokio::test]
async fn the_brain_gets_each_sent_line_before_the_output_that_echoes_it() {
    // 1,000 lines typed at random moments while the game streams output in random pieces and
    // echoes each line as soon as it reads it.
    const LINES: usize = 1000;
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ui = rig.welcomed().await;
    let mut brain = rig.brain().await;
    let mut game = rig.connect_game().await;
    while rig.gateway.state(ID).unwrap() != SessionState::Connected {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let game_task = tokio::spawn(async move {
        let mut rng = fastrand::Rng::with_seed(20);
        let (mut buf, mut input) = ([0u8; 256], Vec::new());
        let mut echoed = 0;
        while echoed < LINES {
            tokio::select! {
                n = game.read(&mut buf) => {
                    input.extend_from_slice(&buf[..n.unwrap()]);
                    while let Some(end) = input.iter().position(|&b| b == b'\r') {
                        let line: Vec<u8> = input.drain(..=end).collect();
                        let n = std::str::from_utf8(&line[1..end]).unwrap();
                        game.write_all(format!("[e{n}]\r\n").as_bytes()).await.unwrap();
                        echoed += 1;
                    }
                }
                () = tokio::task::yield_now() => {
                    let filler = ".".repeat(rng.usize(1..40));
                    game.write_all(filler.as_bytes()).await.unwrap();
                }
            }
        }
        game
    });
    let ui_task = tokio::spawn(async move {
        let mut rng = fastrand::Rng::with_seed(17);
        for n in 0..LINES {
            for _ in 0..rng.usize(0..4) {
                tokio::task::yield_now().await;
            }
            let line = ToGateway::InputLine {
                session: ID.into(),
                text: format!("L{n}"),
            };
            send(&mut ui, &line).await;
        }
        ui
    });

    // What the brain got, in order: for each line, where its `sent` came; and the chunks.
    let mut sent_at = vec![None; LINES];
    let mut chunks: Vec<(usize, String)> = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while sent_at.iter().any(Option::is_none) || !text.contains(&format!("[e{}]", LINES - 1)) {
        match recv(&mut brain).await {
            ToClient::Sent { session, text, .. } => {
                assert_eq!(session, ID);
                let n: usize = text[1..].parse().unwrap();
                sent_at[n] = Some(i);
            }
            ToClient::Output { text: t, .. } => {
                text.push_str(&t);
                chunks.push((i, t));
            }
            ToClient::SessionState { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
        i += 1;
    }
    let mut starts = Vec::with_capacity(chunks.len());
    let mut offset = 0;
    for (at, t) in &chunks {
        starts.push((offset, *at));
        offset += t.len();
    }
    for (n, sent) in sent_at.iter().enumerate() {
        let echo = text.find(&format!("[e{n}]")).expect("every line is echoed");
        let chunk = starts.partition_point(|(start, _)| *start <= echo) - 1;
        assert!(
            sent.unwrap() < starts[chunk].1,
            "line {n}: sent after the chunk holding its echo"
        );
    }
    ui_task.await.unwrap();
    drop(game_task.await.unwrap());
    rig.stop().await;
}

#[tokio::test]
async fn a_channel_client_follows_the_same_handshake_relay_and_refusal_rules() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;

    let (tx, mut rx) = rig.hub.connect();
    let mismatched = ToGateway::Hello {
        protocol: PROTOCOL + 1,
        client: ClientKind::Brain,
        token: None,
    };
    tx.send(mismatched).await.unwrap();
    assert!(
        matches!(recv_channel(&mut rx).await, Some(ToClient::Refused { reason }) if reason.contains("protocol mismatch"))
    );
    assert_eq!(recv_channel(&mut rx).await, None, "refused, then closed");

    // A brain over channels: a WebSocket UI hears it attach, a WebSocket brain is refused.
    let mut ui = rig.welcomed().await;
    let (brain_tx, mut brain_rx) = rig.hub.connect();
    brain_tx.send(brain_hello()).await.unwrap();
    assert!(matches!(
        recv_channel(&mut brain_rx).await,
        Some(ToClient::Welcome { .. })
    ));
    assert_eq!(recv(&mut ui).await, ToClient::BrainState { attached: true });
    let mut ws_brain = rig.client().await;
    send(&mut ws_brain, &brain_hello()).await;
    assert!(matches!(
        recv(&mut ws_brain).await,
        ToClient::Refused { .. }
    ));

    // Relay both ways.
    send(&mut ui, &ToGateway::Resync { session: ID.into() }).await;
    assert_eq!(
        recv_channel(&mut brain_rx).await,
        Some(ToClient::Resync { session: ID.into() })
    );
    let character = ToGateway::Character {
        session: ID.into(),
        state: kasimir_state().into(),
    };
    brain_tx.send(character.clone()).await.unwrap();
    assert_eq!(
        recv(&mut ui).await,
        character.clone().into_relayed().unwrap()
    );

    // A UI over channels: told about the brain after its welcome, refused a brain's message,
    // and relayed to like any other.
    let (ui_tx, mut ui_rx) = rig.hub.connect();
    ui_tx
        .send(ToGateway::Hello {
            protocol: PROTOCOL,
            client: ClientKind::Ui,
            token: None,
        })
        .await
        .unwrap();
    assert!(matches!(
        recv_channel(&mut ui_rx).await,
        Some(ToClient::Welcome { .. })
    ));
    assert_eq!(
        recv_channel(&mut ui_rx).await,
        Some(ToClient::BrainState { attached: true })
    );
    ui_tx.send(character.clone()).await.unwrap();
    assert_eq!(
        recv_channel(&mut ui_rx).await,
        Some(error(r#"a UI may not send "character""#))
    );
    brain_tx.send(character.clone()).await.unwrap();
    let relayed = character.into_relayed().unwrap();
    assert_eq!(recv_channel(&mut ui_rx).await, Some(relayed.clone()));
    assert_eq!(recv(&mut ui).await, relayed);

    // Dropping the brain's sender ends its connection.
    drop(brain_tx);
    assert_eq!(
        recv(&mut ui).await,
        ToClient::BrainState { attached: false }
    );
    assert_eq!(
        recv_channel(&mut ui_rx).await,
        Some(ToClient::BrainState { attached: false })
    );
    while recv_channel(&mut brain_rx).await.is_some() {}
    rig.stop().await;
    assert_eq!(recv_channel(&mut ui_rx).await, None, "stopping ends it too");
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

/// The brain sends `msg`, then a notice the UI waits for: the gateway handled `msg` by then
/// (one connection's messages are handled in order).
async fn brain_says(brain: &mut Client, ui: &mut Client, msg: ToGateway) {
    send(brain, &msg).await;
    let mark = ToGateway::Notice {
        session: None,
        text: format!("after {}", msg.type_name()),
    };
    send(brain, &mark).await;
    let want = mark.into_relayed().unwrap();
    recv_where(ui, |m| *m == want).await;
}

#[tokio::test]
async fn the_brains_notices_and_warnings_about_a_session_are_kept_in_notes_log() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ui = rig.welcomed().await;
    let mut brain = rig.brain().await;
    assert_eq!(recv(&mut ui).await, ToClient::BrainState { attached: true });
    for msg in [
        ToGateway::Notice {
            session: Some(ID.into()),
            text: "Fight: large giant rat".into(),
        },
        ToGateway::Warning {
            session: Some(ID.into()),
            text: "Stopped: the loop cannot go round".into(),
        },
        // About no session, or not a notice: not kept.
        ToGateway::Notice {
            session: None,
            text: "re-attached".into(),
        },
    ] {
        brain_says(&mut brain, &mut ui, msg).await;
    }
    let text = std::fs::read_to_string(dir.path().join("logs").join(ID).join("notes.log")).unwrap();
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let kept: Vec<(&str, &str)> = lines
        .iter()
        .map(|l| (l["kind"].as_str().unwrap(), l["text"].as_str().unwrap()))
        .collect();
    assert_eq!(
        kept,
        [
            ("notice", "Fight: large giant rat"),
            ("warning", "Stopped: the loop cannot go round")
        ]
    );
    assert!(lines[0]["ts"].as_u64().unwrap() > 0 && lines[0]["seq"].is_u64());
}

/// Read exactly `want` from the game, and nothing more for a moment.
async fn game_reads(game: &mut TcpStream, want: &[u8]) {
    let mut got = vec![0u8; want.len()];
    tokio::time::timeout(PATIENCE, game.read_exact(&mut got))
        .await
        .expect("the input reaches the game")
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&got), String::from_utf8_lossy(want));
    let mut more = [0u8; 1];
    let extra = tokio::time::timeout(Duration::from_millis(50), game.read(&mut more)).await;
    assert!(extra.is_err(), "the game got more: {more:?}");
}

fn arbitrate(on: bool) -> ToGateway {
    ToGateway::Arbitrate {
        session: ID.into(),
        on,
    }
}

#[tokio::test]
async fn a_session_the_brain_arbitrates_sends_its_ui_lines_to_the_brain_and_its_keys_to_the_game() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ui = rig.welcomed().await;
    let mut brain = rig.brain().await;
    assert_eq!(recv(&mut ui).await, ToClient::BrainState { attached: true });
    let mut game = rig.connect_game().await;
    while rig.gateway.state(ID).unwrap() != SessionState::Connected {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let line = |text: &str| ToGateway::InputLine {
        session: ID.into(),
        text: text.into(),
    };

    // Not arbitrated yet: straight through.
    send(&mut ui, &line("look")).await;
    game_reads(&mut game, b"look\r").await;

    brain_says(&mut brain, &mut ui, arbitrate(true)).await;
    send(&mut ui, &line("gossip hi")).await;
    let ToClient::UserInput {
        session,
        text,
        from,
    } = recv_where(&mut brain, |m| matches!(m, ToClient::UserInput { .. })).await
    else {
        unreachable!()
    };
    assert_eq!((session.as_str(), text.as_str()), (ID, "gossip hi"));
    assert!(from.starts_with("ui-"), "{from}");
    // An attack is a line like any other.
    send(&mut ui, &line("a rat")).await;
    let got = recv_where(&mut brain, |m| matches!(m, ToClient::UserInput { .. })).await;
    assert!(
        matches!(&got, ToClient::UserInput { text, .. } if text == "a rat"),
        "{got:?}"
    );
    // Keys go to the game, and the brain hears of them.
    send(
        &mut ui,
        &ToGateway::InputKeys {
            session: ID.into(),
            bytes: vec![b' '],
        },
    )
    .await;
    assert_eq!(
        recv_where(&mut brain, |m| matches!(m, ToClient::KeysSent { .. })).await,
        ToClient::KeysSent { session: ID.into() }
    );
    // The brain's own lines go to the game: neither UI line above did.
    send(&mut brain, &line("st")).await;
    game_reads(&mut game, b" st\r").await;

    // Off again: straight through.
    brain_says(&mut brain, &mut ui, arbitrate(false)).await;
    send(&mut ui, &line("n")).await;
    game_reads(&mut game, b"n\r").await;

    // On, and then the brain goes: straight through at once.
    brain_says(&mut brain, &mut ui, arbitrate(true)).await;
    drop(brain);
    assert_eq!(
        recv_where(&mut ui, |m| matches!(m, ToClient::BrainState { .. })).await,
        ToClient::BrainState { attached: false }
    );
    send(&mut ui, &line("s")).await;
    game_reads(&mut game, b"s\r").await;
    // A new brain arbitrates nothing until it says so.
    let _brain = rig.brain().await;
    recv_where(&mut ui, |m| matches!(m, ToClient::BrainState { .. })).await;
    send(&mut ui, &line("e")).await;
    game_reads(&mut game, b"e\r").await;
    rig.stop().await;
}

#[tokio::test]
async fn session_control_hangs_a_session_up_and_connects_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::start(dir.path(), None).await;
    let mut ui = rig.welcomed().await;
    let mut game = rig.connect_game().await;
    game.write_all(b"Password: ").await.unwrap();
    while rig.gateway.state(ID).unwrap() != SessionState::Connected {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let control = |action| ToGateway::SessionControl {
        session: ID.into(),
        action,
    };
    send(&mut ui, &control(SessionAction::HangUp)).await;
    while rig.gateway.state(ID).unwrap() != SessionState::Closed {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // Closed on purpose: it stays so until asked.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(rig.gateway.state(ID).unwrap(), SessionState::Closed);
    send(&mut ui, &control(SessionAction::Connect)).await;
    while rig.gateway.state(ID).unwrap() == SessionState::Closed {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // A session nobody knows is an error, not a hang.
    send(
        &mut ui,
        &ToGateway::SessionControl {
            session: "nobody".into(),
            action: SessionAction::LogOff,
        },
    )
    .await;
    let msg = recv_where(&mut ui, |m| matches!(m, ToClient::Error { .. })).await;
    assert!(matches!(msg, ToClient::Error { .. }), "{msg:?}");
}
