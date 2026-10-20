//! Replay (`docs/replay.md`): a recorded session played back as if the game were sending it.
//! A [`ReplayConnector`] stands where the socket would be, so sessions, logs, history and the
//! WebSocket server are the ones a live gateway runs: the session's state follows the text as
//! the tracker does, the UIs see what they saw live, and a brain can attach.
//!
//! A recording is a session's log directory (`logs/<id>/`): `output.log` is fed on its `ts`,
//! scaled by the [`Pace`]; the lines `sent.log` holds are published as `sent` between the
//! chunks they went between, except the brain's; the lines written to the replay go nowhere
//! (into the replay's own `sent.log`, in its own data directory), and only the brain's are
//! published, so a replay's brain hears its own; and `brain.log` says when the brain attached
//! and went and what reached it from the UIs, for [`drive_brain`].

use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::Future;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;
use wayfarer_proto::ToClient;
use wayfarer_telnet::Telnet;

use crate::config::{Character, Config, Credentials, Server};
use crate::session::{Connect, Connecting, Origin, Recorded, Stream};
use crate::{Gateway, GatewayError, Hub, Options};

/// One line of `brain.log`: a brain attached (`attached: true`) or went (`false`), or a message
/// reached it (`msg`); `seq` is the session's latest output `seq` then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainRecord {
    pub seq: u64,
    pub ts: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg: Option<ToClient>,
}

/// One chunk of `output.log`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RecordedChunk {
    pub seq: u64,
    pub ts: u64,
    pub text: String,
}

/// One line of `sent.log`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SentLine {
    pub seq: u64,
    pub ts: u64,
    pub origin: Origin,
    pub text: String,
}

/// A session's recording, read from its log directory.
#[derive(Debug, Clone)]
pub struct Recording {
    /// The directory's name: the session id.
    pub id: String,
    pub chunks: Vec<RecordedChunk>,
    /// Empty for a recording made before `sent.log` existed.
    pub sent: Vec<SentLine>,
    /// Empty when no brain attached.
    pub brain: Vec<BrainRecord>,
}

impl Recording {
    /// Read `dir` (`logs/<id>/`). `output.log` must exist; `sent.log` and `brain.log` may not.
    /// A line that does not parse (a record cut short by a crash) is skipped.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .ok_or_else(|| {
                io::Error::other(format!("{}: not a session's log directory", dir.display()))
            })?
            .to_owned();
        let chunks = read_lines(&dir.join("output.log"), true)?;
        if chunks.is_empty() {
            return Err(io::Error::other(format!(
                "{}: no output to replay",
                dir.join("output.log").display()
            )));
        }
        Ok(Self {
            id,
            chunks,
            sent: read_lines(&dir.join("sent.log"), false)?,
            brain: read_lines(&dir.join("brain.log"), false)?,
        })
    }

    /// The first chunk's `ts`: the replay's time zero.
    pub fn origin_ms(&self) -> u64 {
        self.chunks.first().map_or(0, |c| c.ts)
    }

    /// The last chunk's `ts`.
    pub fn end_ms(&self) -> u64 {
        self.chunks.last().map_or(0, |c| c.ts)
    }

    /// The lines `origin` sent, in order.
    pub fn sent_by(&self, origin: Origin) -> Vec<&str> {
        self.sent
            .iter()
            .filter(|s| s.origin == origin)
            .map(|s| s.text.as_str())
            .collect()
    }
}

fn read_lines<T: for<'de> Deserialize<'de>>(path: &Path, required: bool) -> io::Result<Vec<T>> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound && !required => return Ok(Vec::new()),
        Err(e) => return Err(io::Error::new(e.kind(), format!("{}: {e}", path.display()))),
    };
    let mut out = Vec::new();
    for line in BufReader::new(file).lines() {
        if let Ok(record) = serde_json::from_str(&line?) {
            out.push(record);
        }
    }
    Ok(out)
}

/// How fast a replay goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Speed {
    /// This many recorded seconds per second (1: as recorded).
    Times(f64),
    /// As fast as the session takes the output.
    Instant,
}

/// The replay's clock: when each recorded `ts` comes. Everything that plays a part of the
/// recording (the feeder, [`drive_brain`], the brain's `ReplayClock`) goes by the same one.
pub struct Pace {
    origin_ms: u64,
    start: Instant,
    speed: Speed,
    /// Recorded milliseconds since `origin_ms` fed so far.
    elapsed: watch::Sender<u64>,
}

impl Pace {
    /// The recording's `origin_ms` comes now.
    pub fn new(origin_ms: u64, speed: Speed) -> Self {
        if let Speed::Times(s) = speed {
            assert!(s > 0.0, "a replay's speed is above 0");
        }
        Self {
            origin_ms,
            start: Instant::now(),
            speed,
            elapsed: watch::channel(0).0,
        }
    }

    pub fn origin_ms(&self) -> u64 {
        self.origin_ms
    }

    /// The moment `origin_ms` came, on tokio's clock.
    pub fn start(&self) -> Instant {
        self.start
    }

    pub fn speed(&self) -> Speed {
        self.speed
    }

    /// Recorded milliseconds since the origin fed so far.
    pub fn elapsed(&self) -> watch::Receiver<u64> {
        self.elapsed.subscribe()
    }

    fn at(&self, ts: u64, speed: f64) -> Instant {
        let recorded = Duration::from_millis(ts.saturating_sub(self.origin_ms));
        self.start + recorded.div_f64(speed)
    }

    /// The feeder: wait until `ts` comes (at once with [`Speed::Instant`]), then say it came.
    async fn reach(&self, ts: u64) {
        if let Speed::Times(speed) = self.speed {
            tokio::time::sleep_until(self.at(ts, speed)).await;
        }
        let ms = ts.saturating_sub(self.origin_ms);
        self.elapsed.send_if_modified(|e| {
            let later = ms > *e;
            *e = (*e).max(ms);
            later
        });
    }

    /// Anyone else: wait until `ts` comes, or with [`Speed::Instant`] until the feeder has
    /// reached it.
    pub async fn until(&self, ts: u64) {
        match self.speed {
            Speed::Times(speed) => tokio::time::sleep_until(self.at(ts, speed)).await,
            Speed::Instant => {
                let ms = ts.saturating_sub(self.origin_ms);
                let _ = self.elapsed.subscribe().wait_for(|&e| e >= ms).await;
            }
        }
    }
}

/// The socket a replayed session reads: one recorded chunk per read (the session's chunks are
/// the recording's, as long as each fits the read buffer); what it writes goes nowhere.
struct ReplayStream {
    rx: mpsc::Receiver<Vec<u8>>,
    pending: Vec<u8>,
    at: usize,
}

impl AsyncRead for ReplayStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.at >= self.pending.len() {
            match self.rx.poll_recv(cx) {
                Poll::Ready(Some(bytes)) => {
                    self.pending = bytes;
                    self.at = 0;
                }
                // The feeder is gone: the end of the connection.
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
        let this = &mut *self;
        let n = (this.pending.len() - this.at).min(buf.remaining());
        buf.put_slice(&this.pending[this.at..this.at + n]);
        this.at += n;
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ReplayStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Connects a session to a [`Recording`] instead of a server: each connection plays it from
/// the start on the [`Pace`]. After the last chunk the connection stays open and quiet, so the
/// session stays where the recording left it.
pub struct ReplayConnector {
    recording: Arc<Recording>,
    pace: Arc<Pace>,
    /// The recorded lines of the connection being made, for [`Connect::recorded`].
    lines: Mutex<Option<mpsc::UnboundedReceiver<Recorded>>>,
    finished: watch::Sender<bool>,
}

impl ReplayConnector {
    pub fn new(recording: Arc<Recording>, pace: Arc<Pace>) -> Self {
        Self {
            recording,
            pace,
            lines: Mutex::new(None),
            finished: watch::channel(false).0,
        }
    }

    /// Turns `true` once the whole recording has been fed.
    pub fn finished(&self) -> watch::Receiver<bool> {
        self.finished.subscribe()
    }
}

impl Connect for ReplayConnector {
    fn connect<'a>(&'a self, _host: &'a str, _port: u16) -> Connecting<'a> {
        Box::pin(async move {
            let (bytes_tx, rx) = mpsc::channel(1);
            let (lines_tx, lines_rx) = mpsc::unbounded_channel();
            *self.lines.lock().unwrap_or_else(|e| e.into_inner()) = Some(lines_rx);
            self.finished.send_replace(false);
            tokio::spawn(feed(
                Arc::clone(&self.recording),
                Arc::clone(&self.pace),
                bytes_tx,
                lines_tx,
                self.finished.clone(),
            ));
            Ok(Box::new(ReplayStream {
                rx,
                pending: Vec::new(),
                at: 0,
            }) as Box<dyn Stream>)
        })
    }

    fn recorded(&self) -> Option<mpsc::UnboundedReceiver<Recorded>> {
        Some(
            self.lines
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                // A connection made elsewhere: its lines are not these; a replay has none then.
                .unwrap_or_else(|| mpsc::unbounded_channel().1),
        )
    }
}

/// Play `recording` into a connection: each chunk at its `ts`, as the game's bytes; each
/// recorded line after the chunk it followed (its `seq`), at its own `ts`. Not the brain's: a
/// replay's brain hears its own lines as `sent` (it is never fooled into taking the recorded
/// ones, arriving before its own at a high speed, for the player's).
async fn feed(
    recording: Arc<Recording>,
    pace: Arc<Pace>,
    bytes: mpsc::Sender<Vec<u8>>,
    lines: mpsc::UnboundedSender<Recorded>,
    finished: watch::Sender<bool>,
) {
    let telnet = Telnet::new();
    let lines_fed: Vec<SentLine> = recording
        .sent
        .iter()
        .filter(|s| s.origin != Origin::Brain)
        .cloned()
        .collect();
    let mut sent = lines_fed.into_iter().peekable();
    let mut fed: u64 = 0;
    // Recorded time never runs backwards, whatever a clock did live.
    let mut ts = 0;
    for chunk in &recording.chunks {
        while let Some(line) = sent.next_if(|s| s.seq < chunk.seq) {
            ts = ts.max(line.ts);
            pace.reach(ts).await;
            let _ = lines.send(Recorded {
                after: fed,
                text: line.text,
            });
        }
        ts = ts.max(chunk.ts);
        pace.reach(ts).await;
        let encoded = telnet.encode_keys(&chunk.text);
        fed += encoded.len() as u64;
        if bytes.send(encoded).await.is_err() {
            return;
        }
    }
    for line in sent {
        ts = ts.max(line.ts);
        pace.reach(ts).await;
        let _ = lines.send(Recorded {
            after: fed,
            text: line.text.clone(),
        });
    }
    drop(lines);
    tracing::info!(session = %recording.id, "the recording has all been played");
    finished.send_replace(true);
    // Open and quiet until the session lets go.
    bytes.closed().await;
}

/// A gateway with one session, named after the recording, that plays it on `pace`, logging
/// under `logs` (never the recording's own directory). `server` gives the logon script the
/// session's tracker follows (the user's config's server for the character, say); by
/// default the silvermere preset's. No credentials, no reconnect: the session opens at once
/// and runs until the gateway stops, and stopping it logs nothing off.
pub fn gateway(
    recording: Arc<Recording>,
    pace: Arc<Pace>,
    logs: &Path,
    server: Option<Server>,
) -> Result<(Gateway, Arc<ReplayConnector>), GatewayError> {
    let mut server = server.unwrap_or_else(|| Server {
        id: String::new(),
        host: "replay".to_owned(),
        port: 0,
        preset: Some("silvermere".to_owned()),
        logon: None,
    });
    server.id = "replay".to_owned();
    let config = Config {
        servers: vec![server],
        characters: vec![Character {
            id: recording.id.clone(),
            name: recording.id.clone(),
            server: "replay".to_owned(),
            autostart: true,
        }],
    };
    let connector = Arc::new(ReplayConnector::new(Arc::clone(&recording), pace));
    let options = Options {
        reconnect: false,
        ..Options::default()
    };
    let gateway = Gateway::with_connector(
        &config,
        &Credentials::default(),
        logs,
        options,
        Arc::clone(&connector) as Arc<dyn Connect>,
    )?;
    gateway.open(&recording.id)?;
    Ok((gateway, connector))
}

/// The directory a replay of `recording` logs into when none is given: a fresh one under the
/// system's temporary directory.
pub fn temp_logs(recording: &Recording) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    std::env::temp_dir().join(format!(
        "wayfarer-replay-{}-{stamp}-{}",
        recording.id,
        std::process::id()
    ))
}

/// Play `brain.log` against `hub` on `pace`: `attach(true)` when the recorded brain attached
/// and `attach(false)` when it went (the caller connects and disconnects its brain; the
/// future resolves once that is done), and each recorded message to the brain then attached,
/// at its `ts`. A recording without a brain attaches one at the start and keeps it. Ends
/// after the last record.
pub async fn drive_brain<F, Fut>(hub: &Hub, recording: &Recording, pace: &Pace, mut attach: F)
where
    F: FnMut(bool) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut attached = false;
    if !recording.brain.iter().any(|r| r.attached.is_some()) {
        attach(true).await;
        attached = true;
    }
    for record in &recording.brain {
        pace.until(record.ts).await;
        match (&record.attached, &record.msg) {
            (Some(true), _) if !attached => {
                attach(true).await;
                attached = true;
            }
            (Some(false), _) if attached => {
                attach(false).await;
                attached = false;
            }
            (None, Some(msg)) if attached && !hub.to_brain(msg.clone()).await => {
                tracing::warn!(
                    msg = msg.type_name(),
                    "no brain to hand a recorded message to"
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bytes_fed_decode_to_the_recorded_text() {
        let text = "\u{1b}[1;32mObvious exits: north\r\n[HP=31/MA=6]:\u{a0}é░▓█ ÿ";
        let encoded = Telnet::new().encode_keys(text);
        assert_eq!(Telnet::new().receive(&encoded).text, text);
    }
}
