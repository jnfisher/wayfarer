//! A `wayfarer gateway` process started for the check, and a WebSocket client to it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use wayfarer_proto::{ClientKind, PROTOCOL, SessionState, ToClient, ToGateway};

use crate::screen;

/// How long the gateway may take to say where it listens.
const START_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the gateway may take to stop: it waits up to two minutes for log-offs itself.
const STOP_TIMEOUT: Duration = Duration::from_secs(150);

/// The session id the check's configs give the character.
pub const SESSION: &str = "probe";

/// A running gateway with one session, and everything it has shown of it.
pub struct Gateway {
    child: Child,
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// The gateway's own log lines.
    log: Arc<Mutex<Vec<String>>>,
    /// The session's output by sequence number (live output and history may overlap).
    output: BTreeMap<u64, String>,
    pub state: SessionState,
    /// When the last output arrived.
    last_output: Instant,
    /// Where it listens (`127.0.0.1:PORT`).
    pub addr: String,
    /// The last `brain_state` (None before one).
    pub brain_attached: Option<bool>,
    /// The session's character as the brain's `character` and `character_patch` built it.
    pub character: serde_json::Value,
    /// The brain's `notice` texts for the session.
    pub notices: Vec<String>,
    /// The brain's `warning` texts for the session.
    pub warnings: Vec<String>,
    /// The center of each `map_view` for the session, (map, room).
    pub map_centers: Vec<(i64, i64)>,
}

/// How a stop went: whether the character logged off, and the gateway's own account.
#[derive(Debug)]
pub struct Stopped {
    pub clean: bool,
    pub detail: String,
}

impl Gateway {
    /// Write `config` and `credentials` into `dir` and start `binary gateway` on them, on a
    /// free local port.
    pub async fn start(
        binary: &Path,
        dir: &Path,
        config: &str,
        credentials: &str,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let config_path = dir.join("wayfarer.toml");
        std::fs::write(&config_path, config).map_err(|e| e.to_string())?;
        crate::fixtures::write_private(&dir.join("credentials.toml"), credentials)
            .map_err(|e| e.to_string())?;
        let mut child = Command::new(binary)
            .arg("gateway")
            .arg("--config")
            .arg(&config_path)
            .args(["--bind", "127.0.0.1:0", "--ui-dir"])
            .arg(dir.join("no-ui"))
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("starting {}: {e}", binary.display()))?;

        // Keep every line of the gateway's log (tracing on stdout, errors on stderr), in
        // memory and in gateway.log.
        let log = Arc::new(Mutex::new(Vec::new()));
        let file = tokio::fs::File::create(dir.join("gateway.log"))
            .await
            .map_err(|e| e.to_string())?;
        let file = Arc::new(tokio::sync::Mutex::new(file));
        let (addr_tx, mut addr_rx) = tokio::sync::mpsc::channel(2);
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        keep_lines(stdout, Arc::clone(&log), Arc::clone(&file), addr_tx.clone());
        keep_lines(stderr, Arc::clone(&log), file, addr_tx);
        let addr = match tokio::time::timeout(START_TIMEOUT, addr_rx.recv()).await {
            Ok(Some(addr)) => addr,
            _ => {
                let log = log.lock().unwrap().join("\n");
                // It may be logged on already: SIGTERM, so it logs off.
                terminate(child.id());
                let _ = tokio::time::timeout(STOP_TIMEOUT, child.wait()).await;
                return Err(format!("the gateway did not start:\n{log}"));
            }
        };

        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .map_err(|e| format!("WebSocket to {addr}: {e}"))?;
        let hello = ToGateway::Hello {
            protocol: PROTOCOL,
            client: ClientKind::Ui,
            token: None,
        };
        ws.send(Message::text(hello.to_json()))
            .await
            .map_err(|e| e.to_string())?;
        let mut gateway = Self {
            child,
            ws,
            log,
            output: BTreeMap::new(),
            state: SessionState::Closed,
            last_output: Instant::now(),
            addr: addr.clone(),
            brain_attached: None,
            character: serde_json::Value::Null,
            notices: Vec::new(),
            warnings: Vec::new(),
            map_centers: Vec::new(),
        };
        match gateway.next(Instant::now() + START_TIMEOUT).await {
            Some(ToClient::Welcome { sessions, .. }) => {
                if let Some(s) = sessions.iter().find(|s| s.id == SESSION) {
                    gateway.state = s.state;
                }
            }
            other => return Err(format!("expected a welcome, got {other:?}")),
        }
        // The session connects as the gateway starts, so some output may predate us.
        gateway
            .send(ToGateway::HistoryRequest {
                session: SESSION.into(),
                after_seq: Some(0),
                before_seq: None,
                max_chunks: 100_000,
            })
            .await?;
        Ok(gateway)
    }

    /// `walk_to` that room.
    pub async fn walk_to(&mut self, map: i64, room: i64) -> Result<(), String> {
        self.send(ToGateway::WalkTo {
            session: SESSION.into(),
            map,
            room,
        })
        .await
    }

    async fn send(&mut self, msg: ToGateway) -> Result<(), String> {
        self.ws
            .send(Message::text(msg.to_json()))
            .await
            .map_err(|e| format!("WebSocket: {e}"))
    }

    pub async fn send_line(&mut self, text: &str) -> Result<(), String> {
        self.send(ToGateway::InputLine {
            session: SESSION.into(),
            text: text.into(),
        })
        .await
    }

    pub async fn send_keys(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send(ToGateway::InputKeys {
            session: SESSION.into(),
            bytes: bytes.to_vec(),
        })
        .await
    }

    /// The next message before `deadline`, applied to what we know; `None` at the deadline or
    /// when the connection ends.
    async fn next(&mut self, deadline: Instant) -> Option<ToClient> {
        loop {
            let frame = tokio::time::timeout_at(deadline, self.ws.next())
                .await
                .ok()??;
            let Ok(Message::Text(text)) = frame else {
                if frame.is_err() {
                    return None;
                }
                continue;
            };
            let Ok(msg) = ToClient::from_json(text.as_str()) else {
                continue;
            };
            match &msg {
                ToClient::Output { seq, text, .. } => {
                    self.output.insert(*seq, text.clone());
                    self.last_output = Instant::now();
                }
                ToClient::History { chunks, .. } => {
                    for c in chunks {
                        self.output.insert(c.seq, c.text.clone());
                    }
                }
                ToClient::SessionState { state, .. } => self.state = *state,
                ToClient::BrainState { attached } => self.brain_attached = Some(*attached),
                ToClient::Character { session, state } if session == SESSION => {
                    self.character = serde_json::to_value(state).unwrap_or_default();
                }
                ToClient::CharacterPatch { session, patch } if session == SESSION => {
                    apply_patch(&mut self.character, patch);
                }
                ToClient::Notice { session, text } if session.as_deref() == Some(SESSION) => {
                    self.notices.push(text.clone());
                }
                ToClient::Warning { session, text } if session.as_deref() == Some(SESSION) => {
                    self.warnings.push(text.clone());
                }
                ToClient::MapView {
                    session, center, ..
                } if session == SESSION => self.map_centers.push((center.map, center.room)),
                ToClient::SessionList { sessions } | ToClient::Welcome { sessions, .. } => {
                    if let Some(s) = sessions.iter().find(|s| s.id == SESSION) {
                        self.state = s.state;
                    }
                }
                _ => {}
            }
            return Some(msg);
        }
    }

    /// The latest sequence number seen.
    pub fn last_seq(&self) -> u64 {
        self.output.keys().next_back().copied().unwrap_or(0)
    }

    /// The output after `seq`, escapes and all.
    pub fn text_after(&self, seq: u64) -> String {
        self.output
            .range(seq + 1..)
            .map(|(_, t)| t.as_str())
            .collect()
    }

    /// Everything the session has shown, without escapes.
    pub fn transcript(&self) -> String {
        screen::plain(&self.text_after(0))
    }

    /// Wait until `done` gives an answer, up to `timeout`; `what` names what was awaited when
    /// it doesn't come.
    pub async fn wait_for<R>(
        &mut self,
        timeout: Duration,
        what: &str,
        mut done: impl FnMut(&Self) -> Option<R>,
    ) -> Result<R, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(r) = done(self) {
                return Ok(r);
            }
            if self.next(deadline).await.is_none() {
                return Err(format!(
                    "{what}: not seen in {} s (session {:?}); the screen ended with:\n{}",
                    timeout.as_secs(),
                    self.state,
                    tail(&self.transcript(), 12)
                ));
            }
        }
    }

    /// Wait until `done` holds and no output has come for `quiet`.
    pub async fn wait_quiet(
        &mut self,
        timeout: Duration,
        quiet: Duration,
        what: &str,
        done: impl Fn(&Self) -> bool,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            if done(self) && self.last_output.elapsed() >= quiet {
                return Ok(());
            }
            let until = if done(self) {
                (self.last_output + quiet).min(deadline)
            } else {
                deadline
            };
            if self.next(until).await.is_none() && Instant::now() >= deadline {
                return Err(format!(
                    "{what}: not seen in {} s; the screen ended with:\n{}",
                    timeout.as_secs(),
                    tail(&self.transcript(), 12)
                ));
            }
        }
    }

    /// Stop the gateway as Ctrl-C would (SIGTERM), which logs the character off, and wait for
    /// it to exit.
    pub async fn stop(mut self) -> Stopped {
        let _ = self.ws.close(None).await;
        terminate(self.child.id());
        let exited = tokio::time::timeout(STOP_TIMEOUT, self.child.wait()).await;
        // Let the log reader catch the last lines.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let log = self.log.lock().unwrap().clone();
        let mut clean = matches!(exited, Ok(Ok(status)) if status.success());
        if exited.is_err() {
            terminate(self.child.id());
            let _ = self.child.wait().await;
        }
        let verdict = logoff_verdict(&log);
        clean &= verdict.is_ok();
        Stopped {
            clean,
            detail: match (exited, verdict) {
                (Err(_), _) => format!("did not exit within {} s", STOP_TIMEOUT.as_secs()),
                (Ok(Ok(status)), Ok(how)) if status.success() => how.to_owned(),
                (Ok(Ok(status)), Ok(_)) => format!("exited with {status}"),
                (Ok(Err(e)), _) => format!("wait: {e}"),
                (_, Err(why)) => why.to_owned(),
            },
        }
    }
}

impl Drop for Gateway {
    /// Never leave a gateway behind; SIGTERM still logs its character off.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            terminate(self.child.id());
        }
    }
}

/// Read `from` line by line into `log` and `file`, and send on the address the gateway says
/// it listens on.
pub(crate) fn keep_lines(
    from: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    log: Arc<Mutex<Vec<String>>>,
    file: Arc<tokio::sync::Mutex<tokio::fs::File>>,
    addr: tokio::sync::mpsc::Sender<String>,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(from).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = screen::plain(&line);
            let _ = file
                .lock()
                .await
                .write_all(format!("{line}\n").as_bytes())
                .await;
            if let Some(a) = line.split("listening on ").nth(1) {
                let _ = addr.try_send(a.trim().to_owned());
            }
            log.lock().unwrap().push(line);
        }
    });
}

/// SIGTERM, never SIGKILL: a killed gateway drops its character in the realm.
pub(crate) fn terminate(pid: Option<u32>) {
    if let Some(pid) = pid {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}

/// What the gateway's log says about the log-off at shutdown.
pub fn logoff_verdict(log: &[String]) -> Result<&'static str, &'static str> {
    let has = |s: &str| log.iter().any(|l| l.contains(s));
    if has("log-off timed out") || has("log-off is taking too long") {
        return Err("the log-off timed out and the gateway hung up");
    }
    if has("closing every session at once") {
        return Err("the gateway closed the session without logging off");
    }
    if !log
        .iter()
        .any(|l| l.contains("state") && l.contains("state=Closed"))
    {
        return Err("the session never reached closed");
    }
    if has("logging off") {
        Ok("logged off")
    } else {
        Ok("closed (not in the game)")
    }
}

/// Apply a JSON merge patch (RFC 7386).
fn apply_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    let serde_json::Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = serde_json::Value::Object(Default::default());
    }
    let obj = target.as_object_mut().expect("an object");
    for (k, v) in patch {
        if v.is_null() {
            obj.remove(k);
        } else {
            apply_patch(obj.entry(k.clone()).or_insert(serde_json::Value::Null), v);
        }
    }
}

/// The last `n` lines of `text`.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// The directory for the next gateway of this run.
pub fn run_dir(root: &Path, n: &mut u32, label: &str) -> PathBuf {
    *n += 1;
    root.join(format!("{n:02}-{label}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn a_clean_log_off_is_recognised() {
        let l = log(&[
            "INFO wayfarer_gateway::session: logging off session=probe",
            "INFO wayfarer_gateway::session: state session=probe state=Closed",
        ]);
        assert_eq!(logoff_verdict(&l), Ok("logged off"));
    }

    #[test]
    fn a_timed_out_or_skipped_log_off_is_not() {
        let timed_out = log(&[
            "INFO wayfarer_gateway::session: logging off session=probe",
            "WARN wayfarer_gateway::session: log-off timed out; closing session=probe",
            "INFO wayfarer_gateway::session: state session=probe state=Closed",
        ]);
        assert!(logoff_verdict(&timed_out).is_err());
        let at_once = log(&[
            "WARN wayfarer_gateway::gateway: closing every session at once",
            "INFO wayfarer_gateway::session: state session=probe state=Closed",
        ]);
        assert!(logoff_verdict(&at_once).is_err());
        assert!(logoff_verdict(&log(&["INFO listening on 127.0.0.1:1"])).is_err());
    }
}
