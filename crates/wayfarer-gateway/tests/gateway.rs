//! The gateway against in-test game servers: a tokio listener on 127.0.0.1:0 that each test
//! scripts. Nothing outside the process.

use std::io::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{self, error::RecvError};
use wayfarer_gateway::{
    Chunk, Config, Credentials, Gateway, GatewayError, HistoryQuery, Options, Paths, SessionState,
    StateChange,
};
use wayfarer_telnet::cp437;

const ID: &str = "kasimir";

/// MBBSEmu's opening offers, as the server sends them on connect (`kb/connection.md`).
const IAC: u8 = 255;
const OFFERS: [u8; 24] = [
    IAC, 251, 0, IAC, 253, 0, IAC, 251, 1, IAC, 254, 1, IAC, 251, 3, IAC, 253, 3, IAC, 252, 31,
    IAC, 253, 31,
];
/// The answer: refuse binary and suppress-go-ahead, accept NAWS and send 80x24.
const ANSWER: [u8; 18] = [
    IAC, 252, 0, IAC, 252, 3, IAC, 251, 31, IAC, 250, 31, 0, 80, 0, 24, IAC, 240,
];

/// Far longer than any test should take; only a broken gateway waits this long.
const PATIENCE: Duration = Duration::from_secs(5);

fn config_text(port: u16, autostart: bool) -> String {
    format!(
        "[[server]]\nid = \"local\"\nhost = \"127.0.0.1\"\nport = {port}\n\n\
         [[character]]\nid = \"{ID}\"\nname = \"Kasimir\"\nserver = \"local\"\nautostart = {autostart}\n"
    )
}

fn gateway(logs: &Path, port: u16, options: Options) -> Gateway {
    // These tests are about one connection; reconnecting is tests/logon.rs's.
    let options = Options {
        reconnect: false,
        ..options
    };
    let config = Config::parse(&config_text(port, false)).unwrap();
    Gateway::new(&config, &Credentials::default(), logs, options).unwrap()
}

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

/// Receive chunks until their text adds up to `len` bytes.
async fn receive_text(rx: &mut broadcast::Receiver<Chunk>, len: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut have = 0;
    while have < len {
        let chunk = tokio::time::timeout(PATIENCE, rx.recv())
            .await
            .expect("output arrives")
            .unwrap();
        have += chunk.text.len();
        chunks.push(chunk);
    }
    assert_eq!(have, len, "more text than was sent");
    chunks
}

async fn next_state(rx: &mut broadcast::Receiver<StateChange>) -> SessionState {
    let change = tokio::time::timeout(PATIENCE, rx.recv())
        .await
        .expect("a state change")
        .unwrap();
    assert_eq!(change.session, ID);
    change.state
}

fn joined(chunks: &[Chunk]) -> String {
    chunks.iter().map(|c| c.text.as_str()).collect()
}

fn seqs(chunks: &[Chunk]) -> Vec<u64> {
    chunks.iter().map(|c| c.seq).collect()
}

#[tokio::test]
async fn a_capture_in_random_pieces_arrives_whole_and_numbered() {
    let raw = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/captures/room-walk/raw.bin"),
    )
    .unwrap();
    let expected: String = raw.iter().map(|&b| cp437::decode_byte(b)).collect();
    let (listener, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let gw = gateway(dir.path(), port, Options::default());
    let mut rx = gw.subscribe(ID).unwrap();
    gw.open(ID).unwrap();

    let sent = raw.clone();
    let server = tokio::spawn(async move {
        let mut sock = accept(&listener).await;
        sock.write_all(&OFFERS).await.unwrap();
        let mut rng = fastrand::Rng::with_seed(7);
        let mut rest = &sent[..];
        while !rest.is_empty() {
            let n = rng.usize(1..=rest.len().min(600));
            sock.write_all(&rest[..n]).await.unwrap();
            rest = &rest[n..];
            if rng.bool() {
                tokio::task::yield_now().await;
            }
        }
        let mut reply = [0; ANSWER.len()];
        sock.read_exact(&mut reply).await.unwrap();
        (reply, sock)
    });

    let chunks = receive_text(&mut rx, expected.len()).await;
    assert!(joined(&chunks) == expected, "decoded text differs");
    assert_eq!(seqs(&chunks), (1..=chunks.len() as u64).collect::<Vec<_>>());
    let (reply, _sock) = server.await.unwrap();
    assert_eq!(reply, ANSWER, "the NAWS reply reaches the server");

    // On disk: the bytes as received, the reply as sent, and every chunk.
    let logs = dir.path().join(ID);
    let mut wire = OFFERS.to_vec();
    wire.extend_from_slice(&raw);
    assert_eq!(std::fs::read(logs.join("raw.bin")).unwrap(), wire);
    assert_eq!(std::fs::read(logs.join("sent.bin")).unwrap(), ANSWER);
    let all = gw
        .history(ID, HistoryQuery::after(0, 100_000))
        .await
        .unwrap();
    assert_eq!(all.chunks, chunks);
    let info = &gw.sessions()[0];
    assert_eq!(
        (info.state, info.last_seq),
        (SessionState::Connected, chunks.len() as u64)
    );
}

#[tokio::test]
async fn history_is_right_across_the_memory_disk_boundary() {
    let (listener, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    // 64 bytes of memory: most chunks are only on disk.
    let options = Options {
        memory_bytes: 64,
        ..Options::default()
    };
    let gw = gateway(dir.path(), port, options);
    let mut rx = gw.subscribe(ID).unwrap();
    gw.open(ID).unwrap();
    let mut sock = accept(&listener).await;
    let mut chunks = Vec::new();
    for i in 0..40 {
        let line = format!("\x1b[32mline {i}\x1b[0m\r\n");
        sock.write_all(line.as_bytes()).await.unwrap();
        // One read per line, so 40 chunks.
        chunks.extend(receive_text(&mut rx, line.len()).await);
    }
    let n = chunks.len() as u64;
    assert_eq!(n, 40);
    let want = |lo: u64, hi: u64| -> Vec<Chunk> {
        chunks
            .iter()
            .filter(|c| c.seq >= lo && c.seq <= hi)
            .cloned()
            .collect()
    };
    for max in [1u32, 5, 17, 1000] {
        for seq in 0..=n + 1 {
            let h = gw.history(ID, HistoryQuery::after(seq, max)).await.unwrap();
            let hi = n.min(seq + u64::from(max));
            assert_eq!(h.from_seq, seq + 1);
            assert_eq!(h.chunks, want(seq + 1, hi), "after {seq}, max {max}");

            let h = gw
                .history(ID, HistoryQuery::before(seq, max))
                .await
                .unwrap();
            let hi = seq.saturating_sub(1).min(n);
            let lo = (hi + 1).saturating_sub(u64::from(max)).max(1);
            assert_eq!(h.from_seq, lo);
            assert_eq!(h.chunks, want(lo, hi), "before {seq}, max {max}");
        }
    }
    let latest = gw.history(ID, HistoryQuery::latest(3)).await.unwrap();
    assert_eq!(seqs(&latest.chunks), [38, 39, 40]);
}

#[tokio::test]
async fn a_restarted_gateway_continues_the_seq() {
    let (listener, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::write(&paths.config, config_text(port, true)).unwrap();

    let first = Gateway::start(&paths, Options::default()).unwrap();
    let mut rx = first.subscribe(ID).unwrap();
    let mut sock = accept(&listener).await;
    for text in ["one\r\n", "two\r\n", "three\r\n"] {
        sock.write_all(text.as_bytes()).await.unwrap();
        receive_text(&mut rx, text.len()).await;
    }
    let before = first.sessions()[0].last_seq;
    assert_eq!(before, 3);
    drop(rx);
    drop(first);
    // The dropped gateway's session lets go of the socket.
    let mut buf = [0; 16];
    let n = tokio::time::timeout(PATIENCE, sock.read(&mut buf))
        .await
        .unwrap();
    assert!(matches!(n, Ok(0) | Err(_)), "{n:?}");

    let second = Gateway::start(&paths, Options::default()).unwrap();
    assert_eq!(second.sessions()[0].last_seq, before);
    let mut rx = second.subscribe(ID).unwrap();
    let mut sock = accept(&listener).await;
    sock.write_all(b"four\r\n").await.unwrap();
    let chunk = receive_text(&mut rx, 6).await.remove(0);
    assert_eq!(chunk.seq, before + 1);
    let all = second.history(ID, HistoryQuery::latest(10)).await.unwrap();
    assert_eq!(joined(&all.chunks), "one\r\ntwo\r\nthree\r\nfour\r\n");
    assert_eq!(seqs(&all.chunks), [1, 2, 3, 4]);
}

#[tokio::test]
async fn a_subscriber_that_never_reads_stalls_nobody() {
    let (listener, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let options = Options {
        subscriber_capacity: 4,
        ..Options::default()
    };
    let gw = gateway(dir.path(), port, options);
    let mut reader = gw.subscribe(ID).unwrap();
    let mut idle = gw.subscribe(ID).unwrap();
    gw.open(ID).unwrap();
    let mut sock = accept(&listener).await;
    const N: u64 = 100;
    for i in 1..=N {
        let text = format!("chunk {i}\r\n");
        sock.write_all(text.as_bytes()).await.unwrap();
        let got = receive_text(&mut reader, text.len()).await;
        assert_eq!(got[0].seq, i);
    }
    // The idle one kept only the last few and is told what it missed.
    match idle.recv().await {
        Err(RecvError::Lagged(missed)) => assert_eq!(missed, N - 4),
        other => panic!("expected a lag, got {other:?}"),
    }
    assert_eq!(idle.recv().await.unwrap().seq, N - 3);
    // History fills the gap.
    let gap = gw.history(ID, HistoryQuery::after(0, 1000)).await.unwrap();
    assert_eq!(gap.chunks.len() as u64, N);
}

#[tokio::test]
async fn input_goes_out_whole_and_is_captured() {
    let (listener, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let gw = gateway(dir.path(), port, Options::default());
    assert!(matches!(
        gw.send_line(ID, "look").await,
        Err(GatewayError::NotConnected(_))
    ));
    assert!(matches!(
        gw.send_line("nobody", "look").await,
        Err(GatewayError::UnknownSession(_))
    ));
    let mut states = gw.state_changes();
    gw.open(ID).unwrap();
    let mut sock = accept(&listener).await;
    assert_eq!(next_state(&mut states).await, SessionState::Connecting);
    assert_eq!(next_state(&mut states).await, SessionState::Connected);

    gw.send_line(ID, "look north").await.unwrap();
    gw.send_keys(ID, &[b'n', 255, 13]).await.unwrap();
    let mut got = [0; 15];
    sock.read_exact(&mut got).await.unwrap();
    assert_eq!(&got, b"look north\rn\xff\xff\r");

    // Two clients typing at once: every line arrives whole.
    let typists: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|who| {
            let gw = gw.clone();
            tokio::spawn(async move {
                for i in 0..50 {
                    let line = format!("{who}{i:02}{}", who.repeat(300));
                    gw.send_line(ID, &line).await.unwrap();
                    if i % 3 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
            })
        })
        .collect();
    for t in typists {
        t.await.unwrap();
    }
    let mut got = vec![0; 100 * 304];
    sock.read_exact(&mut got).await.unwrap();
    let lines: Vec<&[u8]> = got
        .split(|&b| b == b'\r')
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines.len(), 100);
    for line in lines {
        let who = &line[..1];
        assert!(line[3..].iter().all(|b| b == &who[0]), "interleaved line");
        assert_eq!(line.len(), 303);
    }
    let sent = std::fs::read(dir.path().join(ID).join("sent.bin")).unwrap();
    assert_eq!(sent, [&b"look north\rn\xff\xff\r"[..], &got].concat());
}

#[tokio::test]
async fn states_lost_and_closed() {
    let (listener, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let gw = gateway(dir.path(), port, Options::default());
    let mut states = gw.state_changes();
    assert_eq!(gw.state(ID).unwrap(), SessionState::Closed);

    // The server hangs up: lost.
    gw.open(ID).unwrap();
    drop(accept(&listener).await);
    assert_eq!(next_state(&mut states).await, SessionState::Connecting);
    assert_eq!(next_state(&mut states).await, SessionState::Connected);
    assert_eq!(next_state(&mut states).await, SessionState::Lost);
    assert!(matches!(
        gw.send_line(ID, "x").await,
        Err(GatewayError::NotConnected(_))
    ));

    // Reopened, then closed on purpose: the server sees the socket close.
    gw.open(ID).unwrap();
    gw.open(ID).unwrap(); // already connecting: nothing happens
    let mut sock = accept(&listener).await;
    assert_eq!(next_state(&mut states).await, SessionState::Connecting);
    assert_eq!(next_state(&mut states).await, SessionState::Connected);
    gw.close(ID).await.unwrap();
    assert_eq!(next_state(&mut states).await, SessionState::Closed);
    assert_eq!(gw.state(ID).unwrap(), SessionState::Closed);
    let mut buf = [0; 8];
    assert_eq!(sock.read(&mut buf).await.unwrap(), 0);

    // Nobody listening: lost again.
    drop(listener);
    gw.open(ID).unwrap();
    assert_eq!(next_state(&mut states).await, SessionState::Connecting);
    assert_eq!(next_state(&mut states).await, SessionState::Lost);
    // Closing a lost session marks it closed on purpose.
    gw.close(ID).await.unwrap();
    assert_eq!(next_state(&mut states).await, SessionState::Closed);
}

/// Everything tracing prints, kept for the test to search.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

// The default (current-thread) runtime runs the session task on this thread, so the
// thread-local subscriber below sees its tracing too.
#[tokio::test]
async fn credentials_are_private_and_the_password_is_never_written() {
    const PASSWORD: &str = "pw-TEST-9f3e1c";
    let traced = Captured::default();
    let sink = traced.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || sink.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let (listener, port) = listener().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::write(&paths.config, config_text(port, false)).unwrap();

    // No credentials.toml: the gateway creates one, private.
    drop(Gateway::start(&paths, Options::default()).unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&paths.credentials)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{mode:o}");
        // Widened by hand: the gateway warns.
        std::fs::set_permissions(&paths.credentials, std::fs::Permissions::from_mode(0o644))
            .unwrap();
        drop(Gateway::start(&paths, Options::default()).unwrap());
        assert!(traced.text().contains("chmod 600"), "{}", traced.text());
        std::fs::set_permissions(&paths.credentials, std::fs::Permissions::from_mode(0o600))
            .unwrap();
    }
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&paths.credentials)
        .unwrap();
    writeln!(
        f,
        "[character.{ID}]\nuser = \"ACCOUNT\"\npassword = \"{PASSWORD}\""
    )
    .unwrap();
    drop(f);
    let credentials = Credentials::load_or_create(&paths.credentials).unwrap();
    assert_eq!(credentials.get(ID).unwrap().password, PASSWORD);

    // M0 has no logon script: the player types the password into the tile.
    let gw = Gateway::start(&paths, Options::default()).unwrap();
    let mut rx = gw.subscribe(ID).unwrap();
    gw.open(ID).unwrap();
    let mut sock = accept(&listener).await;
    sock.write_all(b"Password: ").await.unwrap();
    receive_text(&mut rx, 10).await;
    gw.send_line(ID, PASSWORD).await.unwrap();
    gw.send_line(ID, "look").await.unwrap();
    let mut got = vec![0; PASSWORD.len() + 6];
    sock.read_exact(&mut got).await.unwrap();
    assert_eq!(
        got,
        format!("{PASSWORD}\rlook\r").as_bytes(),
        "the game gets it"
    );
    sock.write_all(b"********\r\n").await.unwrap();
    receive_text(&mut rx, 10).await;
    gw.close(ID).await.unwrap();

    let logs = dir.path().join("logs").join(ID);
    let sent = std::fs::read(logs.join("sent.bin")).unwrap();
    assert_eq!(sent, b"********\rlook\r");
    let mut written = vec![
        ("tracing", traced.text()),
        ("credentials Debug", format!("{credentials:?}")),
    ];
    for file in ["raw.bin", "sent.bin", "output.log"] {
        let bytes = std::fs::read(logs.join(file)).unwrap();
        written.push((file, String::from_utf8_lossy(&bytes).into_owned()));
    }
    assert!(traced.text().contains("connected to 127.0.0.1"));
    for (what, text) in written {
        assert!(!text.contains(PASSWORD), "{what} holds the password");
    }
}
