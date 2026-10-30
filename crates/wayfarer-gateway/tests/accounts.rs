//! The Accounts panel's save (C-062) against in-test BBSes: a tokio listener on 127.0.0.1:0 that
//! each test scripts. Nothing outside the process.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use wayfarer_gateway::{
    Config, Credentials, Gateway, Hub, Login, Options, Paths, SessionState, Signup, Tcp,
};
use wayfarer_proto::{ClientKind, PROTOCOL, ToClient, ToGateway};

/// Far longer than any test should take; only a broken gateway waits this long.
const PATIENCE: Duration = Duration::from_secs(5);

async fn listener() -> (TcpListener, u16) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    (l, port)
}

async fn accept(l: &TcpListener) -> TcpStream {
    tokio::time::timeout(PATIENCE, l.accept())
        .await
        .expect("the gateway connects")
        .unwrap()
        .0
}

/// The next line the gateway sends, without its `\r`.
async fn line(sock: &mut TcpStream) -> String {
    let mut got = Vec::new();
    loop {
        let mut b = [0u8; 1];
        let n = tokio::time::timeout(PATIENCE, sock.read(&mut b))
            .await
            .expect("a line arrives")
            .unwrap();
        assert!(
            n == 1,
            "closed before a line: {:?}",
            String::from_utf8_lossy(&got)
        );
        match b[0] {
            b'\r' => return String::from_utf8(got).unwrap(),
            b'\n' | 0 => {}
            c => got.push(c),
        }
    }
}

/// Write `prompt` and expect `answer` back.
async fn ask(sock: &mut TcpStream, prompt: &str, answer: &str) {
    sock.write_all(prompt.as_bytes()).await.unwrap();
    assert_eq!(line(sock).await, answer, "after {prompt:?}");
}

/// Whether the gateway has closed `sock` (reading nothing more within a moment, or the end).
async fn closed(sock: &mut TcpStream) -> bool {
    let mut b = [0u8; 64];
    matches!(
        tokio::time::timeout(PATIENCE, sock.read(&mut b)).await,
        Ok(Ok(0) | Err(_))
    )
}

/// Nothing arrives on `sock` for a moment.
async fn silent(sock: &mut TcpStream) -> bool {
    let mut b = [0u8; 64];
    tokio::time::timeout(Duration::from_millis(200), sock.read(&mut b))
        .await
        .is_err()
}

fn config(port: u16, preset: &str, characters: &[(&str, bool)]) -> Config {
    let preset = if preset.is_empty() {
        String::new()
    } else {
        format!("preset = \"{preset}\"\n")
    };
    let mut text =
        format!("[[server]]\nid = \"local\"\nhost = \"127.0.0.1\"\nport = {port}\n{preset}");
    for (id, autostart) in characters {
        text.push_str(&format!(
            "[[character]]\nid = \"{id}\"\nname = \"{}\"\nserver = \"local\"\nautostart = {autostart}\n",
            id.to_uppercase()
        ));
    }
    Config::parse(&text).unwrap()
}

fn login(user: &str, password: &str) -> Login {
    Login {
        user: user.into(),
        password: password.into(),
    }
}

fn gateway(dir: &Path, config: &Config, credentials: &Credentials) -> Gateway {
    let options = Options {
        reconnect: false,
        ..Options::default()
    };
    Gateway::build(
        config,
        credentials,
        &dir.join("logs"),
        options,
        Arc::new(Tcp),
        Some(Paths::in_dir(dir)),
    )
    .unwrap()
}

async fn wait_for(gw: &Gateway, id: &str, state: SessionState) {
    let mut changes = gw.state_changes();
    let reached = async {
        while gw.state(id).unwrap() != state {
            let _ = changes.recv().await;
        }
    };
    tokio::time::timeout(PATIENCE, reached)
        .await
        .unwrap_or_else(|_| panic!("{id} never got to {state:?}: {:?}", gw.state(id)));
}

fn ids(gw: &Gateway) -> Vec<String> {
    gw.sessions().into_iter().map(|s| s.id).collect()
}

#[tokio::test]
async fn a_save_adds_changes_and_removes_sessions_and_leaves_the_rest_playing() {
    let (bbs, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let old = config(port, "", &[("a", true), ("b", false)]);
    let mut creds = Credentials::default();
    creds.set("a", login("ua", "pa"));
    let gw = gateway(dir.path(), &old, &creds);
    gw.open("a").unwrap();
    let mut sa = accept(&bbs).await;
    wait_for(&gw, "a", SessionState::Connected).await;
    gw.open("b").unwrap();
    let mut sb = accept(&bbs).await;
    wait_for(&gw, "b", SessionState::Connected).await;
    let roster = gw.roster_changes();

    // b goes, c comes, a is only renamed.
    let mut new = config(port, "", &[("a", true), ("c", false)]);
    new.characters[0].name = "Alda".into();
    let changes = gw
        .save_accounts(new.clone(), creds.clone(), HashMap::new())
        .await
        .unwrap();
    assert_eq!(changes.removed, ["b"]);
    assert_eq!(changes.added, ["c"]);
    assert_eq!(changes.updated, ["a"]);
    assert!(changes.reconnect.is_empty());
    assert!(roster.has_changed().unwrap());
    assert_eq!(ids(&gw), ["a", "c"]);
    assert_eq!(gw.sessions()[0].name, "Alda");
    assert!(closed(&mut sb).await, "b hung up");
    let _sc = accept(&bbs).await;
    wait_for(&gw, "c", SessionState::Connected).await;
    assert!(silent(&mut sa).await, "a untouched");
    assert_eq!(gw.state("a").unwrap(), SessionState::Connected);
    // The files say the same, and read back.
    let paths = Paths::in_dir(dir.path());
    assert_eq!(Config::load(&paths.config).unwrap(), new);
    assert!(dir.path().join("logs/b").is_dir(), "b's logs stay");

    // A changed password hangs a up and logs it on again.
    let mut creds2 = creds.clone();
    creds2.set("a", login("ua", "pa2"));
    let changes = gw
        .save_accounts(new.clone(), creds2.clone(), HashMap::new())
        .await
        .unwrap();
    assert_eq!(changes.reconnect, ["a"]);
    assert!(closed(&mut sa).await, "the old connection hung up");
    let _sa = accept(&bbs).await;
    wait_for(&gw, "a", SessionState::Connected).await;
    assert_eq!(
        Credentials::load_or_create(&paths.credentials).unwrap(),
        creds2
    );

    // A refused save changes nothing.
    let mut bad = new.clone();
    bad.characters[1].server = "nowhere".into();
    let err = gw
        .save_accounts(bad, creds2.clone(), HashMap::new())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not listed"), "{err}");
    assert_eq!(ids(&gw), ["a", "c"]);
    let seven = config(
        port,
        "",
        &[
            ("a", false),
            ("b", false),
            ("c", false),
            ("d", false),
            ("e", false),
            ("f", false),
            ("g", false),
        ],
    );
    let err = gw
        .save_accounts(seven, creds2, HashMap::new())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("at most 6"), "{err}");
}

/// The first connection: the BBS doesn't know `user`.
async fn unknown_user(bbs: &TcpListener, user: &str, password: &str) {
    let mut sock = accept(bbs).await;
    ask(
        &mut sock,
        "Enter Username or enter \"NEW\" to create a new Account\r\nUsername: ",
        user,
    )
    .await;
    ask(&mut sock, "Password: ", password).await;
    sock.write_all(b"\r\nInvalid Credentials\r\nUsername: ")
        .await
        .unwrap();
    assert!(closed(&mut sock).await, "the gateway hangs up");
}

#[tokio::test]
async fn an_unknown_user_gets_an_account_and_stops_at_the_menu() {
    let (bbs, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let gw = gateway(
        dir.path(),
        &config(port, "silvermere", &[]),
        &Credentials::default(),
    );

    let new = config(port, "silvermere", &[("thornwyn", false)]);
    let mut creds = Credentials::default();
    creds.set("thornwyn", login("Thornwyn", "secret1"));
    let signups = HashMap::from([(
        "thornwyn".to_owned(),
        Signup {
            email: "me@example.org".into(),
            gender: "M".into(),
        },
    )]);
    gw.save_accounts(new, creds, signups).await.unwrap();

    unknown_user(&bbs, "Thornwyn", "secret1").await;
    let mut sock = accept(&bbs).await;
    ask(&mut sock, "Username: ", "NEW").await;
    ask(
        &mut sock,
        "\r\nPlease enter a unique Username (Max. 29 Characters):\r\n",
        "Thornwyn",
    )
    .await;
    ask(
        &mut sock,
        "\r\nPlease enter a strong Password:\r\n",
        "secret1",
    )
    .await;
    ask(
        &mut sock,
        "*******\r\nPlease re-enter your password to confirm:\r\n",
        "secret1",
    )
    .await;
    ask(
        &mut sock,
        "*******\r\nPlease enter a valid e-Mail Address:\r\n",
        "me@example.org",
    )
    .await;
    ask(
        &mut sock,
        "\r\nPlease enter your gender 'M' or 'F' (can be changed later):\r\n",
        "M",
    )
    .await;
    ask(
        &mut sock,
        "\r\nMain Menu\r\nMake your selection (X to exit): ",
        "M",
    )
    .await;
    sock.write_all(b"\r\n[E] . Enter the Realm\r\n[MAJORMUD]: ")
        .await
        .unwrap();
    wait_for(&gw, "thornwyn", SessionState::Menu).await;
    assert!(
        silent(&mut sock).await,
        "no E: the player makes the character"
    );
    assert!(
        gw.reason("thornwyn")
            .unwrap()
            .unwrap()
            .contains("account made"),
        "{:?}",
        gw.reason("thornwyn")
    );
    let sent = std::fs::read(dir.path().join("logs/thornwyn/sent.bin")).unwrap();
    assert!(
        !String::from_utf8_lossy(&sent).contains("secret1"),
        "the password is masked in sent.bin"
    );
}

#[tokio::test]
async fn a_refused_sign_up_says_why_and_a_known_user_never_signs_up() {
    let (bbs, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let mut creds = Credentials::default();
    creds.set("old", login("Old", "pw-old"));
    let start = config(port, "silvermere", &[("old", false)]);
    let gw = gateway(dir.path(), &start, &creds);

    // A character from before the save, with a wrong password: bad credentials, no sign-up.
    gw.open("old").unwrap();
    unknown_user(&bbs, "Old", "pw-old").await;
    wait_for(&gw, "old", SessionState::Closed).await;
    assert_eq!(
        gw.reason("old").unwrap().as_deref(),
        Some("bad credentials")
    );

    // A new one whose name is taken.
    let new = config(port, "silvermere", &[("old", false), ("kasimir", false)]);
    creds.set("kasimir", login("Kasimir", "secret1"));
    gw.save_accounts(new, creds, HashMap::new()).await.unwrap();
    unknown_user(&bbs, "Kasimir", "secret1").await;
    let mut sock = accept(&bbs).await;
    ask(&mut sock, "Username: ", "NEW").await;
    ask(
        &mut sock,
        "\r\nPlease enter a unique Username (Max. 29 Characters):\r\n",
        "Kasimir",
    )
    .await;
    sock.write_all(
        b"\r\nThat Username is unavailable, please choose another.\r\n\r\nPlease enter a unique Username (Max. 29 Characters):\r\n",
    )
    .await
    .unwrap();
    assert!(closed(&mut sock).await, "the gateway gives up");
    wait_for(&gw, "kasimir", SessionState::Closed).await;
    assert_eq!(
        gw.reason("kasimir").unwrap().as_deref(),
        Some("That Username is unavailable, please choose another.")
    );
}

/// A channel client that has said hello as `kind`, past its welcome.
async fn client(
    hub: &Hub,
    kind: ClientKind,
) -> (mpsc::Sender<ToGateway>, mpsc::Receiver<ToClient>) {
    let (tx, mut rx) = hub.connect();
    tx.send(ToGateway::Hello {
        protocol: PROTOCOL,
        client: kind,
        token: None,
    })
    .await
    .unwrap();
    assert!(matches!(next(&mut rx).await, ToClient::Welcome { .. }));
    (tx, rx)
}

async fn next(rx: &mut mpsc::Receiver<ToClient>) -> ToClient {
    tokio::time::timeout(PATIENCE, rx.recv())
        .await
        .expect("a message arrives")
        .expect("still connected")
}

async fn next_where(
    rx: &mut mpsc::Receiver<ToClient>,
    want: impl Fn(&ToClient) -> bool,
) -> ToClient {
    loop {
        let msg = next(rx).await;
        if want(&msg) {
            return msg;
        }
    }
}

#[tokio::test]
async fn a_ui_reads_and_saves_the_accounts_and_every_client_hears_of_the_new_session() {
    let (bbs, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let mut creds = Credentials::default();
    creds.set("a", login("ua", "pa"));
    let gw = gateway(dir.path(), &config(port, "", &[("a", false)]), &creds);
    let hub = Hub::new(gw.clone(), None);
    let (ui, mut ui_rx) = client(&hub, ClientKind::Ui).await;
    let (brain, mut brain_rx) = client(&hub, ClientKind::Brain).await;

    // The brain may not read the passwords.
    brain.send(ToGateway::AccountsRequest).await.unwrap();
    assert!(
        matches!(next(&mut brain_rx).await, ToClient::Error { message } if message.contains("only a UI"))
    );

    ui.send(ToGateway::AccountsRequest).await.unwrap();
    let ToClient::Accounts { mut accounts } =
        next_where(&mut ui_rx, |m| matches!(m, ToClient::Accounts { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(accounts.characters[0].password, "pa");
    assert_eq!(accounts.presets, ["mbbsemu", "silvermere"]);

    let mut added = accounts.characters[0].clone();
    added.id = "b".into();
    added.name = "Bee".into();
    added.user = String::new();
    accounts.characters.push(added);
    ui.send(ToGateway::AccountsSave { accounts }).await.unwrap();
    assert_eq!(
        next_where(&mut ui_rx, |m| matches!(m, ToClient::AccountsSaved { .. })).await,
        ToClient::AccountsSaved { error: None }
    );
    for rx in [&mut ui_rx, &mut brain_rx] {
        let ToClient::SessionList { sessions } =
            next_where(rx, |m| matches!(m, ToClient::SessionList { .. })).await
        else {
            unreachable!()
        };
        assert_eq!(
            sessions.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
    }
    // Both hear the new session's output: they were subscribed to every session.
    let mut sb = accept(&bbs).await;
    sb.write_all(b"hello from b").await.unwrap();
    for rx in [&mut ui_rx, &mut brain_rx] {
        let msg = next_where(
            rx,
            |m| matches!(m, ToClient::Output { session, .. } if session == "b"),
        )
        .await;
        assert!(matches!(msg, ToClient::Output { text, .. } if text == "hello from b"));
    }

    // A refused save says why, and changes nothing.
    let mut bad = wayfarer_proto::Accounts::default();
    bad.characters.push(wayfarer_proto::AccountCharacter {
        id: "c".into(),
        name: "C".into(),
        server: "nowhere".into(),
        autostart: false,
        user: String::new(),
        password: String::new(),
        email: None,
        gender: None,
        reason: None,
    });
    ui.send(ToGateway::AccountsSave { accounts: bad })
        .await
        .unwrap();
    let saved = next_where(&mut ui_rx, |m| matches!(m, ToClient::AccountsSaved { .. })).await;
    assert!(
        matches!(&saved, ToClient::AccountsSaved { error: Some(e) } if e.contains("not listed")),
        "{saved:?}"
    );
    assert_eq!(ids(&gw), ["a", "b"]);
}
