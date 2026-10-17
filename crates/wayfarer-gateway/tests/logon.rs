//! Logon scripts, reconnects and log-off against in-test fake game servers. Each connection is
//! an in-memory pipe (`tokio::io::duplex`) and tokio's clock is paused, so timeouts and backoff
//! take no real time: when every task is waiting, the clock jumps to the next timer.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use regex::Regex;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;
use wayfarer_gateway::{
    Config, Connect, Credentials, Gateway, Options, ServerOptions, SessionState, StateChange,
    Stream, serve,
};
use wayfarer_telnet::cp437;

use SessionState::{Closed, Connected, Connecting, Logon, Lost, Menu, Realm};

const ID: &str = "aldric";
/// The test server's account (from the captures).
const USER: &str = "Aldric";
const PASSWORD: &str = "aldric1";
/// On the paused clock this is only reached if the gateway is stuck.
const PATIENCE: Duration = Duration::from_secs(24 * 3600);

/// Hands each connection's server end to the test; refuses the first `refuse` attempts.
#[derive(Default)]
struct Pipe {
    servers: Mutex<Option<mpsc::UnboundedSender<DuplexStream>>>,
    refuse: Mutex<u32>,
    attempts: Mutex<Vec<Instant>>,
}

impl Connect for Pipe {
    fn connect<'a>(&'a self, _host: &'a str, _port: u16) -> wayfarer_gateway::Connecting<'a> {
        Box::pin(async move {
            self.attempts.lock().unwrap().push(Instant::now());
            let mut refuse = self.refuse.lock().unwrap();
            if *refuse > 0 {
                *refuse -= 1;
                return Err(std::io::ErrorKind::ConnectionRefused.into());
            }
            let (client, server) = tokio::io::duplex(1 << 16);
            let servers = self.servers.lock().unwrap();
            servers.as_ref().unwrap().send(server).unwrap();
            Ok(Box::new(client) as Box<dyn Stream>)
        })
    }
}

/// The game's end of one connection.
struct Fake {
    sock: DuplexStream,
    pending: Vec<u8>,
}

impl Fake {
    async fn say(&mut self, text: &str) {
        self.say_bytes(text.as_bytes()).await;
    }

    async fn say_bytes(&mut self, bytes: &[u8]) {
        self.sock.write_all(bytes).await.unwrap();
    }

    /// The next line the client sends, without its CR.
    async fn line(&mut self) -> String {
        loop {
            if let Some(i) = self.pending.iter().position(|&b| b == b'\r') {
                let line: Vec<u8> = self.pending.drain(..=i).collect();
                return String::from_utf8(line[..i].to_vec()).unwrap();
            }
            let mut buf = [0; 256];
            let n = tokio::time::timeout(PATIENCE, self.sock.read(&mut buf))
                .await
                .expect("the client sends a line")
                .unwrap();
            assert!(n > 0, "closed while a line was awaited");
            self.pending.extend_from_slice(&buf[..n]);
        }
    }

    /// The client sends nothing more for a second (it waits for the next prompt).
    async fn quiet(&mut self) {
        assert!(self.pending.is_empty(), "sent early: {:?}", self.pending);
        let mut buf = [0; 256];
        let r = tokio::time::timeout(Duration::from_secs(1), self.sock.read(&mut buf)).await;
        assert!(r.is_err(), "sent early: {r:?}");
    }

    /// The client closes the connection.
    async fn closed(&mut self) {
        let mut buf = [0; 256];
        let n = tokio::time::timeout(PATIENCE, self.sock.read(&mut buf))
            .await
            .expect("the client closes")
            .unwrap_or(0);
        assert_eq!(n, 0, "expected a close, got {:?}", &buf[..n]);
    }

    /// The whole MBBSEmu logon, prompt by prompt, ending at the realm prompt.
    async fn log_on(&mut self) {
        for (prompt, reply) in [
            (
                "\r\nEnter Username or enter \"NEW\" to create a new Account\r\nUsername: ",
                USER,
            ),
            ("Aldric\r\nPassword: ", PASSWORD),
            (
                "*******\r\n\r\nMain Menu\r\nMake your selection (X to exit): ",
                "M",
            ),
            ("M\r\n\r\n[E] . Enter the Realm\r\n\r\n[MAJORMUD]: ", "E"),
        ] {
            self.say(prompt).await;
            assert_eq!(self.line().await, reply);
        }
        self.say("E\r\n\r\nNewhaven, Village Entrance\r\n[HP=35]:")
            .await;
    }
}

struct Rig {
    gw: Gateway,
    pipe: Arc<Pipe>,
    servers: mpsc::UnboundedReceiver<DuplexStream>,
    states: broadcast::Receiver<StateChange>,
    dir: tempfile::TempDir,
}

fn rig(ids: &[&str], credentials: bool, options: Options) -> Rig {
    let mut config =
        "[[server]]\nid = \"bbs\"\nhost = \"game.example\"\nport = 23\npreset = \"mbbsemu\"\n"
            .to_owned();
    let mut creds = String::new();
    for id in ids {
        config.push_str(&format!(
            "[[character]]\nid = \"{id}\"\nname = \"{id}\"\nserver = \"bbs\"\n"
        ));
        creds.push_str(&format!(
            "[character.{id}]\nuser = \"{USER}\"\npassword = \"{PASSWORD}\"\n"
        ));
    }
    let config = Config::parse(&config).unwrap();
    let credentials = if credentials {
        Credentials::parse(&creds).unwrap()
    } else {
        Credentials::default()
    };
    let (tx, servers) = mpsc::unbounded_channel();
    let pipe = Arc::new(Pipe::default());
    *pipe.servers.lock().unwrap() = Some(tx);
    let dir = tempfile::tempdir().unwrap();
    let connector: Arc<dyn Connect> = pipe.clone();
    let gw =
        Gateway::with_connector(&config, &credentials, dir.path(), options, connector).unwrap();
    let states = gw.state_changes();
    Rig {
        gw,
        pipe,
        servers,
        states,
        dir,
    }
}

impl Rig {
    fn new() -> Self {
        rig(&[ID], true, Options::default())
    }

    async fn accept(&mut self) -> Fake {
        let sock = tokio::time::timeout(PATIENCE, self.servers.recv())
            .await
            .expect("the gateway connects")
            .unwrap();
        Fake {
            sock,
            pending: Vec::new(),
        }
    }

    /// `id`'s states until (and with) `want`.
    async fn states_until(&mut self, id: &str, want: SessionState) -> Vec<SessionState> {
        let mut seen = Vec::new();
        loop {
            let change = tokio::time::timeout(PATIENCE, self.states.recv())
                .await
                .expect("a state change")
                .unwrap();
            if change.session == id {
                seen.push(change.state);
                if change.state == want {
                    return seen;
                }
            }
        }
    }

    fn no_more_connections(&mut self) {
        assert!(self.servers.try_recv().is_err(), "the gateway reconnected");
    }

    fn sent(&self, id: &str) -> Vec<u8> {
        std::fs::read(self.dir.path().join(id).join("sent.bin")).unwrap()
    }
}

/// A capture cut into what the server sends at connect and after each line from the client,
/// with those lines: each step's `until` ends a piece (`script.json`).
fn capture_dialogue(scenario: &str) -> (Vec<Vec<u8>>, Vec<String>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/captures")
        .join(scenario);
    let raw = std::fs::read(dir.join("raw.bin")).unwrap();
    let script: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("script.json")).unwrap()).unwrap();
    // The prompts are coloured (`ESC[1;30m[ESC[0mMAJORMUD...`): match on the text without its
    // CSI sequences, keeping where in `raw` each of its characters came from.
    let (mut text, mut origin) = (String::new(), Vec::new());
    let mut in_csi = false;
    for (i, &b) in raw.iter().enumerate() {
        match (in_csi, b) {
            (false, 0x1b) => in_csi = true,
            (false, _) => {
                let c = cp437::decode_byte(b);
                origin.extend(std::iter::repeat_n(i, c.len_utf8()));
                text.push(c);
            }
            (true, 0x40..=0x7e) if b != b'[' => in_csi = false,
            (true, _) => {}
        }
    }
    let mut pieces = Vec::new();
    let mut lines = Vec::new();
    let (mut from_text, mut from_raw) = (0, 0);
    for step in script["steps"].as_array().unwrap() {
        if let Some(send) = step["send"].as_str() {
            lines.push(send.to_owned());
        }
        let Some(until) = step["until"].as_str() else {
            break;
        };
        let end = Regex::new(until)
            .unwrap()
            .find_at(&text, from_text)
            .expect(until)
            .end();
        let end_raw = origin[end - 1] + 1;
        pieces.push(raw[from_raw..end_raw].to_vec());
        (from_text, from_raw) = (end, end_raw);
    }
    pieces.push(raw[from_raw..].to_vec());
    (pieces, lines)
}

#[tokio::test(start_paused = true)]
async fn logs_on_and_off_as_the_capture_shows() {
    let (pieces, lines) = capture_dialogue("logoff-in-realm");
    assert_eq!(lines, ["Aldric", "aldric1", "M", "E", "x", "X", "X", "Y"]);
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.say_bytes(&pieces[0]).await;
    for i in 0..4 {
        assert_eq!(game.line().await, lines[i]);
        game.quiet().await;
        game.say_bytes(&pieces[i + 1]).await;
    }
    assert_eq!(
        rig.states_until(ID, Realm).await,
        [Connecting, Logon, Menu, Realm]
    );

    // Closing from the realm logs off: each line only after its prompt.
    let gw = rig.gw.clone();
    let closing = tokio::spawn(async move { gw.close(ID).await });
    for i in 4..8 {
        assert_eq!(game.line().await, lines[i]);
        game.quiet().await;
        game.say_bytes(&pieces[i + 1]).await;
    }
    // The server hangs up after `Y`.
    drop(game);
    closing.await.unwrap().unwrap();
    assert_eq!(rig.states_until(ID, Closed).await, [Menu, Closed]);
    assert_eq!(rig.gw.reason(ID).unwrap(), None);

    // Never reconnected after a deliberate close.
    tokio::time::sleep(Duration::from_secs(600)).await;
    rig.no_more_connections();

    let sent = rig.sent(ID);
    assert_eq!(sent, b"Aldric\r********\rM\rE\rx\rX\rX\rY\r");
}

#[tokio::test(start_paused = true)]
async fn bad_credentials_stop_without_retrying() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.say("Username: ").await;
    assert_eq!(game.line().await, USER);
    game.say("Password: ").await;
    assert_eq!(game.line().await, PASSWORD);
    // MBBSEmu asks again; the script must not try forever.
    game.say("\r\nInvalid Credentials\r\n\r\nUsername: ").await;
    game.closed().await;
    assert_eq!(
        rig.states_until(ID, Closed).await,
        [Connecting, Logon, Closed]
    );
    assert_eq!(
        rig.gw.reason(ID).unwrap().as_deref(),
        Some("bad credentials")
    );
    tokio::time::sleep(Duration::from_secs(3600)).await;
    rig.no_more_connections();
    assert!(!String::from_utf8_lossy(&rig.sent(ID)).contains(PASSWORD));
}

#[tokio::test(start_paused = true)]
async fn already_logged_in_retries_after_backoff() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.say("Username: ").await;
    assert_eq!(game.line().await, USER);
    game.say(
        "\r\nAldric is already logged in -- only 1 connection allowed per user.\r\nUsername: ",
    )
    .await;
    game.closed().await;
    let lost_at = Instant::now();
    assert_eq!(rig.states_until(ID, Lost).await, [Connecting, Logon, Lost]);
    assert_eq!(
        rig.gw.reason(ID).unwrap().as_deref(),
        Some("logon failed: already logged in")
    );

    let mut game = rig.accept().await;
    assert_eq!(lost_at.elapsed().as_secs(), 5);
    game.log_on().await;
    assert_eq!(
        rig.states_until(ID, Realm).await,
        [Connecting, Logon, Menu, Realm]
    );
}

#[tokio::test(start_paused = true)]
async fn a_dropped_connection_in_the_realm_logs_on_again() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    for _ in 0..3 {
        let mut game = rig.accept().await;
        game.log_on().await;
        rig.states_until(ID, Realm).await;
        drop(game);
        let lost_at = Instant::now();
        assert_eq!(rig.states_until(ID, Lost).await, [Lost]);
        assert_eq!(
            rig.gw.reason(ID).unwrap().as_deref(),
            Some("closed by the server")
        );
        assert!(matches!(
            rig.gw.send_line(ID, "look").await,
            Err(wayfarer_gateway::GatewayError::NotConnected(_))
        ));
        // A good logon resets the backoff: 5 s every time.
        let next = rig.accept().await;
        assert_eq!(lost_at.elapsed().as_secs(), 5);
        drop(next);
        // That one is lost before logon; take the next.
        rig.states_until(ID, Lost).await;
    }
}

#[tokio::test(start_paused = true)]
async fn the_backoff_schedule() {
    let mut rig = Rig::new();
    *rig.pipe.refuse.lock().unwrap() = 6;
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    let attempts = rig.pipe.attempts.lock().unwrap().clone();
    let gaps: Vec<u64> = attempts
        .windows(2)
        .map(|w| (w[1] - w[0]).as_secs())
        .collect();
    assert_eq!(gaps, [5, 10, 30, 60, 60, 60]);
    game.log_on().await;
    rig.states_until(ID, Realm).await;
    // Closing between attempts stops them.
    drop(game);
    rig.states_until(ID, Lost).await;
    rig.gw.close(ID).await.unwrap();
    assert_eq!(rig.gw.state(ID).unwrap(), Closed);
    tokio::time::sleep(Duration::from_secs(600)).await;
    rig.no_more_connections();
}

#[tokio::test(start_paused = true)]
async fn a_step_timeout_closes_and_retries() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.say("Username: ").await;
    assert_eq!(game.line().await, USER);
    let asked = Instant::now();
    // No password prompt.
    game.closed().await;
    assert_eq!(asked.elapsed().as_secs(), 30);
    rig.states_until(ID, Lost).await;
    assert_eq!(
        rig.gw.reason(ID).unwrap().as_deref(),
        Some("logon step timed out")
    );
    let mut game = rig.accept().await;
    game.log_on().await;
    rig.states_until(ID, Realm).await;
}

#[tokio::test(start_paused = true)]
async fn shutdown_logs_off_from_the_realm_and_from_the_menu() {
    let mut rig = rig(&["kasimir", "brenna"], true, Options::default());
    rig.gw.open("kasimir").unwrap();
    let mut kasimir = rig.accept().await;
    kasimir.log_on().await;
    rig.states_until("kasimir", Realm).await;
    rig.gw.open("brenna").unwrap();
    let mut brenna = rig.accept().await;
    brenna.log_on().await;
    rig.states_until("brenna", Realm).await;
    // Brenna's player leaves the realm by hand.
    rig.gw.send_line("brenna", "x").await.unwrap();
    assert_eq!(brenna.line().await, "x");
    brenna
        .say("x\r\nYou will exit after a period of silent meditation.\r\n....\r\n\r\n[MAJORMUD]: ")
        .await;
    rig.states_until("brenna", Menu).await;

    let gw = rig.gw.clone();
    let shutdown = tokio::spawn(async move { gw.shutdown().await });
    let prompts = [
        "x\r\nYou will exit after a period of silent meditation.\r\n.......\r\n\r\n[MAJORMUD]: ",
        "X\r\n\r\nMain Menu\r\nMake your selection (X to exit): ",
        "X\r\nYou are about to terminate this connection!\r\n\r\nAre you sure (Y/N, or R to re-logon)? ",
    ];
    for (line, prompt) in ["x", "X", "X"].into_iter().zip(prompts) {
        assert_eq!(kasimir.line().await, line);
        kasimir.quiet().await;
        kasimir.say(prompt).await;
    }
    assert_eq!(kasimir.line().await, "Y");
    kasimir.say("Y\r\n\r\nOk, thanks for calling!\r\n").await;
    drop(kasimir);
    // From the menu, only the rest.
    for (line, prompt) in ["X", "X"].into_iter().zip(&prompts[1..]) {
        assert_eq!(brenna.line().await, line);
        brenna.say(prompt).await;
    }
    assert_eq!(brenna.line().await, "Y");
    drop(brenna);
    shutdown.await.unwrap();
    for id in ["kasimir", "brenna"] {
        assert_eq!(rig.gw.state(id).unwrap(), Closed);
    }
    assert_eq!(rig.sent("brenna"), b"Aldric\r********\rM\rE\rx\rX\rX\rY\r");
    assert_eq!(rig.sent("kasimir"), b"Aldric\r********\rM\rE\rx\rX\rX\rY\r");
}

#[tokio::test(start_paused = true)]
async fn shutdown_closes_what_is_left_after_its_patience() {
    let options = Options {
        logoff_patience: Duration::from_secs(30),
        ..Options::default()
    };
    let mut rig = rig(&[ID], true, options);
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.log_on().await;
    rig.states_until(ID, Realm).await;
    let started = Instant::now();
    let gw = rig.gw.clone();
    let shutdown = tokio::spawn(async move { gw.shutdown().await });
    assert_eq!(game.line().await, "x");
    // The game never answers.
    game.closed().await;
    shutdown.await.unwrap();
    assert_eq!(started.elapsed().as_secs(), 30);
    assert_eq!(rig.gw.state(ID).unwrap(), Closed);

    // `close_now` while a log-off is under way: at once.
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.log_on().await;
    rig.states_until(ID, Realm).await;
    let gw = rig.gw.clone();
    let closing = tokio::spawn(async move { gw.close(ID).await });
    assert_eq!(game.line().await, "x");
    let started = Instant::now();
    rig.gw.close_now(ID).await.unwrap();
    game.closed().await;
    closing.await.unwrap().unwrap();
    assert_eq!(started.elapsed(), Duration::ZERO);
    assert_eq!(rig.gw.state(ID).unwrap(), Closed);
}

/// What the binary does with SIGINT/SIGTERM: the first stop ends the server and starts the
/// log-off, a second closes at once.
#[tokio::test(start_paused = true)]
async fn a_second_stop_closes_the_server_at_once() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.log_on().await;
    rig.states_until(ID, Realm).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let options = ServerOptions {
        token: None,
        ui_dir: rig.dir.path().join("no-ui-here"),
    };
    let (stop, stops) = mpsc::unbounded_channel();
    let stops = futures_util::stream::unfold(stops, |mut stops| async move {
        stops.recv().await.map(|()| ((), stops))
    });
    let server = tokio::spawn(serve(listener, rig.gw.clone(), options, stops));
    stop.send(()).unwrap();
    assert_eq!(game.line().await, "x");
    // The character meditates; the player won't wait.
    let started = Instant::now();
    stop.send(()).unwrap();
    game.closed().await;
    server.await.unwrap().unwrap();
    assert_eq!(started.elapsed(), Duration::ZERO);
    assert_eq!(rig.gw.state(ID).unwrap(), Closed);
    assert_eq!(rig.sent(ID), b"Aldric\r********\rM\rE\rx\r");
}

#[tokio::test(start_paused = true)]
async fn typed_input_works_during_logon() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.say("Username: ").await;
    assert_eq!(game.line().await, USER);
    assert_eq!(rig.gw.state(ID).unwrap(), Logon);
    rig.gw.send_line(ID, "hello").await.unwrap();
    assert_eq!(game.line().await, "hello");
    // The steps carry on.
    game.say("Password: ").await;
    assert_eq!(game.line().await, PASSWORD);
}

#[tokio::test(start_paused = true)]
async fn without_credentials_the_player_logs_on_and_the_session_follows() {
    let mut rig = rig(&[ID], false, Options::default());
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    assert_eq!(
        rig.states_until(ID, Connected).await,
        [Connecting, Connected]
    );
    game.say("Username: ").await;
    game.quiet().await;
    game.say("\r\n[MAJORMUD]: ").await;
    rig.states_until(ID, Menu).await;
    game.say("E\r\n[HP=9]:").await;
    rig.states_until(ID, Realm).await;
    let gw = rig.gw.clone();
    let closing = tokio::spawn(async move { gw.close(ID).await });
    assert_eq!(game.line().await, "x");
    // A server that hangs up mid-log-off has still closed it.
    drop(game);
    closing.await.unwrap().unwrap();
    assert_eq!(rig.gw.state(ID).unwrap(), Closed);
}

#[tokio::test(start_paused = true)]
async fn no_realm_prompt_hands_the_session_to_the_player() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    for (prompt, reply) in [
        ("Username: ", USER),
        ("Password: ", PASSWORD),
        ("Make your selection (X to exit): ", "M"),
        ("[MAJORMUD]: ", "E"),
    ] {
        game.say(prompt).await;
        assert_eq!(game.line().await, reply);
    }
    // A new account: character creation, which the player does in the tile.
    game.say("\r\nPlease choose your race:\r\n 1. Human\r\n")
        .await;
    let entered = Instant::now();
    rig.states_until(ID, Connected).await;
    assert_eq!(entered.elapsed().as_secs(), 30);
    // Still connected, and nothing more is sent for it.
    game.quiet().await;
    rig.gw.send_line(ID, "1").await.unwrap();
    assert_eq!(game.line().await, "1");
}

#[tokio::test(start_paused = true)]
async fn closing_during_logon_just_disconnects() {
    let mut rig = Rig::new();
    rig.gw.open(ID).unwrap();
    let mut game = rig.accept().await;
    game.say("Username: ").await;
    assert_eq!(game.line().await, USER);
    rig.gw.close(ID).await.unwrap();
    game.closed().await;
    assert_eq!(rig.gw.state(ID).unwrap(), Closed);
}
