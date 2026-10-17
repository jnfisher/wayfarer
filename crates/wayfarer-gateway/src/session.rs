//! One session: the task that owns a character's socket. It reads the game, answers telnet
//! negotiation, logs and publishes the output, and writes input, one command at a time, so
//! input from several clients never interleaves within a line.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use wayfarer_proto::{Chunk, SessionId, SessionInfo, SessionState};
use wayfarer_telnet::{Capture, Telnet};

use crate::log::OutputLog;

/// What `sent.bin` records instead of a password.
pub(crate) const MASK: &str = "********";

/// A session's state changed (the wire's `session_state`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateChange {
    pub session: SessionId,
    pub state: SessionState,
}

pub(crate) enum Command {
    Line(String),
    Keys(Vec<u8>),
    Close(oneshot::Sender<()>),
}

/// Everything about one character's session that outlives its connections.
pub(crate) struct Slot {
    pub id: SessionId,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub log: OutputLog,
    pub output: broadcast::Sender<Chunk>,
    /// The character's password, so a line that is exactly it is masked in `sent.bin`.
    pub password: Option<String>,
    pub read_buffer: usize,
    states: broadcast::Sender<StateChange>,
    state: Mutex<SessionState>,
    running: Mutex<Option<Running>>,
}

/// A connection in progress: the session task and its command queue.
pub(crate) struct Running {
    pub commands: mpsc::Sender<Command>,
    pub task: JoinHandle<()>,
}

impl Running {
    pub fn is_live(&self) -> bool {
        !self.commands.is_closed()
    }
}

pub(crate) struct SlotParts {
    pub id: SessionId,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub log: OutputLog,
    pub password: Option<String>,
    pub read_buffer: usize,
    pub subscriber_capacity: usize,
    pub states: broadcast::Sender<StateChange>,
}

impl Slot {
    pub fn new(p: SlotParts) -> Self {
        Self {
            id: p.id,
            name: p.name,
            host: p.host,
            port: p.port,
            log: p.log,
            output: broadcast::channel(p.subscriber_capacity.max(1)).0,
            password: p.password,
            read_buffer: p.read_buffer.max(1),
            states: p.states,
            // Never opened: nothing is connected, and on purpose.
            state: Mutex::new(SessionState::Closed),
            running: Mutex::new(None),
        }
    }

    pub fn state(&self) -> SessionState {
        *self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_state(&self, state: SessionState) {
        let mut current = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if *current != state {
            *current = state;
            tracing::info!(session = %self.id, ?state, "state");
            // Sent under the lock, so changes reach listeners in the order they happened.
            let _ = self.states.send(StateChange {
                session: self.id.clone(),
                state,
            });
        }
    }

    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            server: format!("{}:{}", self.host, self.port),
            state: self.state(),
            last_seq: self.log.last_seq(),
        }
    }

    pub fn running(&self) -> std::sync::MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The session task: connect, then pump until the socket goes (`lost`) or a `Close` arrives
/// (`closed`).
pub(crate) async fn run(slot: Arc<Slot>, mut commands: mpsc::Receiver<Command>, capture: Capture) {
    let end = connected(&slot, &mut commands, capture).await;
    // Under the lock `open` takes, so a reopen can't slip in between and have its
    // `connecting` overwritten; once the lock is released the session can be opened again.
    let _running = slot.running();
    commands.close();
    match end {
        End::Lost(why) => {
            tracing::warn!(session = %slot.id, "connection lost: {why}");
            slot.set_state(SessionState::Lost);
        }
        End::Closed(ack) => {
            slot.set_state(SessionState::Closed);
            if let Some(ack) = ack {
                let _ = ack.send(());
            }
        }
    }
}

enum End {
    Lost(String),
    Closed(Option<oneshot::Sender<()>>),
}

async fn connected(
    slot: &Slot,
    commands: &mut mpsc::Receiver<Command>,
    mut capture: Capture,
) -> End {
    slot.set_state(SessionState::Connecting);
    let connect = TcpStream::connect((slot.host.as_str(), slot.port));
    let stream = tokio::select! {
        r = connect => match r {
            Ok(s) => s,
            Err(e) => return End::Lost(format!("connecting to {}:{}: {e}", slot.host, slot.port)),
        },
        cmd = wait_for_close(commands) => return End::Closed(cmd),
    };
    let _ = stream.set_nodelay(true);
    tracing::info!(session = %slot.id, "connected to {}:{}", slot.host, slot.port);
    // No logon script yet (C-005 runs one from here): the player logs on by hand.
    slot.set_state(SessionState::Connected);

    let (mut reader, mut writer) = stream.into_split();
    let mut telnet = Telnet::new();
    let mut buf = vec![0u8; slot.read_buffer];
    loop {
        tokio::select! {
            r = reader.read(&mut buf) => {
                let n = match r {
                    Ok(0) => return End::Lost("closed by the server".into()),
                    Ok(n) => n,
                    Err(e) => return End::Lost(format!("read: {e}")),
                };
                let bytes = &buf[..n];
                if let Err(e) = capture.record_received(bytes) {
                    tracing::warn!(session = %slot.id, "raw capture: {e}");
                }
                let received = telnet.receive(bytes);
                if !received.replies.is_empty() {
                    if let Err(e) = writer.write_all(&received.replies).await {
                        return End::Lost(format!("write: {e}"));
                    }
                    record_sent(slot, &mut capture, &received.replies);
                }
                if !received.text.is_empty() {
                    let chunk = slot.log.append(received.text);
                    // No subscribers is fine; a slow one lags without holding anyone up.
                    let _ = slot.output.send(chunk);
                }
            }
            cmd = commands.recv() => {
                let (bytes, record) = match cmd {
                    Some(Command::Line(text)) => {
                        let bytes = telnet.encode_line(&text);
                        let record = if slot.password.as_deref() == Some(text.as_str()) {
                            telnet.encode_line(MASK)
                        } else {
                            bytes.clone()
                        };
                        (bytes, record)
                    }
                    Some(Command::Keys(keys)) => {
                        let bytes = telnet.encode_bytes(&keys);
                        (bytes.clone(), bytes)
                    }
                    Some(Command::Close(ack)) => {
                        let _ = writer.shutdown().await;
                        return End::Closed(Some(ack));
                    }
                    // The gateway is gone.
                    None => return End::Closed(None),
                };
                if let Err(e) = writer.write_all(&bytes).await {
                    return End::Lost(format!("write: {e}"));
                }
                record_sent(slot, &mut capture, &record);
            }
        }
    }
}

/// While connecting only a close is acted on. The gateway refuses input until the session is
/// connected, so nothing else should arrive; anything that slips in is dropped.
async fn wait_for_close(commands: &mut mpsc::Receiver<Command>) -> Option<oneshot::Sender<()>> {
    loop {
        match commands.recv().await {
            Some(Command::Close(ack)) => return Some(ack),
            Some(_) => {}
            None => return None,
        }
    }
}

fn record_sent(slot: &Slot, capture: &mut Capture, bytes: &[u8]) {
    if let Err(e) = capture.record_sent(bytes) {
        tracing::warn!(session = %slot.id, "sent capture: {e}");
    }
}
