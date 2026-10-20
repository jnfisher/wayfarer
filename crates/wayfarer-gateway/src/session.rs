//! One session: the task that owns a character's socket. It reads the game, answers telnet
//! negotiation, logs and publishes the output, and writes input, one command at a time, so
//! input from several clients never interleaves within a line. It runs the logon script,
//! follows the text to know where the character is, logs it off cleanly when closed, and
//! reconnects with backoff after a lost connection.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use wayfarer_proto::{Chunk, SessionId, SessionInfo, SessionState};
use wayfarer_telnet::{Capture, Telnet};

use crate::config::Login;
use crate::log::OutputLog;
use crate::logon::{self, Action, Logoff, LogoffAction, Place, Runner, Script, Tracker};

/// What `sent.bin` records instead of a password.
pub(crate) const MASK: &str = "********";

/// A session's state changed (the wire's `session_state`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateChange {
    pub session: SessionId,
    pub state: SessionState,
}

/// A line went to the game (the wire's `sent`): typed by a client, or a logon or log-off step.
/// `seq` is the session's latest output `seq` when it went; a password is [`MASK`]ed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub session: SessionId,
    pub seq: u64,
    pub text: String,
}

/// What a session publishes, in the order it happened: a chunk of output, or a line that went
/// to the game. A [`Sent`] comes after every chunk up to its `seq` and before the next one, so
/// a subscriber sees a typed line before the output that echoes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Chunk(Chunk),
    Sent(Sent),
}

/// A connection to a game server: anything the session can read and write.
pub trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

/// A connection being opened.
pub type Connecting<'a> = Pin<Box<dyn Future<Output = io::Result<Box<dyn Stream>>> + Send + 'a>>;

/// How sessions reach their servers: [`Tcp`] for real ones, an in-memory pipe in tests.
pub trait Connect: Send + Sync + 'static {
    fn connect<'a>(&'a self, host: &'a str, port: u16) -> Connecting<'a>;
}

/// Plain TCP, Nagle off (a line goes out as one packet).
pub struct Tcp;

impl Connect for Tcp {
    fn connect<'a>(&'a self, host: &'a str, port: u16) -> Connecting<'a> {
        Box::pin(async move {
            let stream = TcpStream::connect((host, port)).await?;
            let _ = stream.set_nodelay(true);
            Ok(Box::new(stream) as Box<dyn Stream>)
        })
    }
}

pub(crate) enum Command {
    Line(String),
    Keys(Vec<u8>),
    /// Log off cleanly, then close.
    Close(oneshot::Sender<()>),
    /// Close at once, logged off or not.
    Hangup(oneshot::Sender<()>),
}

/// Everything about one character's session that outlives its connections.
pub(crate) struct Slot {
    pub id: SessionId,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub log: OutputLog,
    pub output: broadcast::Sender<Output>,
    /// The character's account: the logon script's `{user}` and `{password}`, and a line that
    /// is exactly the password is masked in `sent.bin`.
    pub login: Option<Login>,
    pub script: Option<Arc<Script>>,
    pub read_buffer: usize,
    pub reconnect: bool,
    pub connector: Arc<dyn Connect>,
    states: broadcast::Sender<StateChange>,
    state: Mutex<SessionState>,
    /// Why the session closed or was lost, when there is more to say than the state.
    reason: Mutex<Option<String>>,
    running: Mutex<Option<Running>>,
}

/// A session task and its command queue.
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
    pub login: Option<Login>,
    pub script: Option<Arc<Script>>,
    pub read_buffer: usize,
    pub reconnect: bool,
    pub connector: Arc<dyn Connect>,
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
            login: p.login,
            script: p.script,
            read_buffer: p.read_buffer.max(1),
            reconnect: p.reconnect,
            connector: p.connector,
            states: p.states,
            // Never opened: nothing is connected, and on purpose.
            state: Mutex::new(SessionState::Closed),
            reason: Mutex::new(None),
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

    /// Tell the subscribers that `text` went to the game. Only the session task publishes, so
    /// this lands between the chunks it went between.
    fn sent_line(&self, text: String) {
        let _ = self.output.send(Output::Sent(Sent {
            session: self.id.clone(),
            seq: self.log.last_seq(),
            text,
        }));
    }

    pub fn reason(&self) -> Option<String> {
        self.reason
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_reason(&self, reason: Option<String>) {
        *self.reason.lock().unwrap_or_else(|e| e.into_inner()) = reason;
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

    /// Say loudly that the character is being left in the realm, which the game punishes at
    /// the next entry (`kb/connection.md`); the session's reason says so too.
    fn dropped(&self, why: &str) -> Option<String> {
        tracing::warn!(
            session = %self.id,
            "{} dropped in the realm: {why}; the game punishes it at the next entry",
            self.name
        );
        Some(format!("dropped in the realm: {why}"))
    }
}

/// The session task: connect and pump until a `Close` (`closed`, after a clean log-off) or a
/// failure the script says not to retry. A lost connection is `lost`, then reconnected after
/// [`logon::backoff`]; without reconnecting (`Options::reconnect` off) the task ends there.
pub(crate) async fn run(
    slot: Arc<Slot>,
    mut commands: mpsc::Receiver<Command>,
    mut capture: Capture,
) {
    // Failed attempts since the last good logon.
    let mut attempt = 0;
    let (state, acks) = loop {
        match connection(&slot, &mut commands, &mut capture, &mut attempt).await {
            End::Closed { acks, reason } => {
                slot.set_reason(reason);
                break (SessionState::Closed, acks);
            }
            End::Lost(why) => {
                tracing::warn!(session = %slot.id, "connection lost: {why}");
                slot.set_reason(Some(why));
                if !slot.reconnect {
                    break (SessionState::Lost, Vec::new());
                }
                slot.set_state(SessionState::Lost);
                let delay = logon::backoff(attempt);
                attempt += 1;
                tracing::info!(
                    session = %slot.id,
                    "reconnect attempt {attempt} in {} s",
                    delay.as_secs()
                );
                tokio::select! {
                    () = tokio::time::sleep(delay) => {}
                    ack = wait_for_close(&mut commands) => {
                        break (SessionState::Closed, ack.into_iter().collect());
                    }
                }
            }
        }
    };
    // Under the lock `open` takes, so a reopen can't slip in between and have its
    // `connecting` overwritten; once the lock is released the session can be opened again.
    let _running = slot.running();
    commands.close();
    slot.set_state(state);
    for ack in acks {
        let _ = ack.send(());
    }
}

enum End {
    /// Reconnect (unless that is off).
    Lost(String),
    /// On purpose, or a failure not to retry; `acks` are the closers waiting for it.
    Closed {
        acks: Vec<oneshot::Sender<()>>,
        reason: Option<String>,
    },
}

impl End {
    fn closed(acks: Vec<oneshot::Sender<()>>) -> Self {
        Self::Closed { acks, reason: None }
    }
}

/// One connection, from connecting until it ends.
async fn connection(
    slot: &Slot,
    commands: &mut mpsc::Receiver<Command>,
    capture: &mut Capture,
    attempt: &mut u32,
) -> End {
    slot.set_state(SessionState::Connecting);
    let address = format!("{}:{}", slot.host, slot.port);
    let stream = tokio::select! {
        r = slot.connector.connect(&slot.host, slot.port) => match r {
            Ok(s) => s,
            Err(e) => return End::Lost(format!("connecting to {address}: {e}")),
        },
        ack = wait_for_close(commands) => return End::closed(ack.into_iter().collect()),
    };
    tracing::info!(session = %slot.id, "connected to {address}");
    slot.set_reason(None);

    let mut tracker = slot.script.clone().map(Tracker::new);
    let mut runner = match (&slot.script, &slot.login) {
        (Some(script), Some(login)) => Some(Runner::new(
            Arc::clone(script),
            &login.user,
            &login.password,
        )),
        _ => None,
    };
    let mut deadline = None;
    if let Some(r) = &runner {
        slot.set_state(SessionState::Logon);
        deadline = Some(Instant::now() + r.timeout());
    } else {
        if slot.script.is_some() {
            tracing::warn!(session = %slot.id, "no credentials: the player logs on by hand");
        }
        // The player has the session.
        slot.set_state(SessionState::Connected);
        *attempt = 0;
    }
    let mut logoff: Option<Logoff> = None;
    let mut acks = Vec::new();

    let (mut reader, mut writer) = tokio::io::split(stream);
    let mut telnet = Telnet::new();
    let mut buf = vec![0u8; slot.read_buffer];
    loop {
        tokio::select! {
            r = reader.read(&mut buf) => {
                let n = match r {
                    // Logging off ends with the server closing the connection.
                    Ok(0) if logoff.is_some() => return End::closed(acks),
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
                    record_sent(slot, capture, &received.replies);
                }
                if received.text.is_empty() {
                    continue;
                }
                let chunk = slot.log.append(received.text);
                // No subscribers is fine; a slow one lags without holding anyone up.
                let _ = slot.output.send(Output::Chunk(chunk.clone()));
                let text = chunk.text.as_str();

                // Where the character is now, whoever got it there.
                if let Some(place) = tracker.as_mut().and_then(|t| t.feed(text)) {
                    slot.set_state(match place {
                        Place::Realm => SessionState::Realm,
                        Place::Menu(_) => SessionState::Menu,
                    });
                }
                for action in runner.as_mut().map(|r| r.feed(text)).unwrap_or_default() {
                    match action {
                        Action::Send { text, shown } => {
                            let (bytes, record) = (telnet.encode_line(&text), telnet.encode_line(&shown));
                            if let Err(e) = write(slot, &mut writer, capture, &bytes, &record).await {
                                return End::Lost(e);
                            }
                            slot.sent_line(shown);
                            // The next step's timeout runs from here.
                            deadline = runner.as_ref().map(|r| Instant::now() + r.timeout());
                        }
                        Action::Realm => {
                            tracing::info!(session = %slot.id, "logged on");
                            slot.set_state(SessionState::Realm);
                            *attempt = 0;
                            runner = None;
                            deadline = None;
                        }
                        Action::Failed { reason, retry } => {
                            let _ = writer.shutdown().await;
                            if retry {
                                return End::Lost(format!("logon failed: {reason}"));
                            }
                            tracing::warn!(session = %slot.id, "logon failed: {reason}");
                            return End::Closed { acks, reason: Some(reason) };
                        }
                    }
                }
                if let Some(l) = logoff.as_mut() {
                    for action in l.feed(text) {
                        let line = match action {
                            LogoffAction::Send(line) => {
                                // The next step's timeout runs from here.
                                deadline = Some(Instant::now() + l.timeout());
                                line
                            }
                            LogoffAction::Retry { send, retry, of } => {
                                tracing::info!(session = %slot.id, "log-off interrupted: {send:?} again ({retry} of {of})");
                                send
                            }
                            LogoffAction::GiveUp => {
                                let reason = slot.dropped("the log-off was interrupted every time");
                                let _ = writer.shutdown().await;
                                return End::Closed { acks, reason };
                            }
                        };
                        let bytes = telnet.encode_line(&line);
                        if let Err(e) = write(slot, &mut writer, capture, &bytes, &bytes).await {
                            return End::Lost(e);
                        }
                        slot.sent_line(line);
                    }
                }
            }
            cmd = commands.recv() => {
                // The bytes, what `sent.bin` records, and the line for `sent` (keys are none).
                let (bytes, record, line) = match cmd {
                    Some(Command::Line(text)) => {
                        let bytes = telnet.encode_line(&text);
                        let is_password = slot.login.as_ref().is_some_and(|l| l.password == text);
                        let shown = if is_password { MASK.to_owned() } else { text };
                        (bytes, telnet.encode_line(&shown), Some(shown))
                    }
                    Some(Command::Keys(keys)) => {
                        let bytes = telnet.encode_bytes(&keys);
                        (bytes.clone(), bytes, None)
                    }
                    Some(Command::Close(ack)) => {
                        acks.push(ack);
                        if logoff.is_some() {
                            continue;
                        }
                        let start = slot.script.as_ref().zip(tracker.as_ref().and_then(Tracker::place));
                        let Some((l, first)) = start.and_then(|(s, place)| Logoff::start(Arc::clone(s), place)) else {
                            // Not in the game (or no log-off steps): nothing to log off from.
                            let _ = writer.shutdown().await;
                            return End::closed(acks);
                        };
                        tracing::info!(session = %slot.id, "logging off");
                        runner = None;
                        deadline = Some(Instant::now() + l.timeout());
                        logoff = Some(l);
                        let bytes = telnet.encode_line(&first);
                        (bytes.clone(), bytes, Some(first))
                    }
                    Some(Command::Hangup(ack)) => {
                        acks.push(ack);
                        let in_realm = tracker.as_ref().and_then(Tracker::place) == Some(Place::Realm);
                        let reason = if in_realm { slot.dropped("closed at once") } else { None };
                        let _ = writer.shutdown().await;
                        return End::Closed { acks, reason };
                    }
                    // The gateway is gone.
                    None => return End::closed(acks),
                };
                if let Err(e) = write(slot, &mut writer, capture, &bytes, &record).await {
                    return End::Lost(e);
                }
                if let Some(line) = line {
                    slot.sent_line(line);
                }
            }
            () = tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
                deadline = None;
                if let Some(l) = &logoff {
                    tracing::warn!(session = %slot.id, "log-off timed out; closing");
                    let reason = if l.in_realm() { slot.dropped("the log-off timed out") } else { None };
                    let _ = writer.shutdown().await;
                    return End::Closed { acks, reason };
                }
                match &runner {
                    Some(r) if r.waiting_for_realm() => {
                        // Character creation, say: the player takes over.
                        tracing::warn!(session = %slot.id, "no realm prompt: the player has the session");
                        runner = None;
                        slot.set_state(SessionState::Connected);
                    }
                    Some(_) => {
                        let _ = writer.shutdown().await;
                        return End::Lost("logon step timed out".into());
                    }
                    None => {}
                }
            }
        }
    }
}

async fn write(
    slot: &Slot,
    writer: &mut WriteHalf<Box<dyn Stream>>,
    capture: &mut Capture,
    bytes: &[u8],
    record: &[u8],
) -> Result<(), String> {
    writer
        .write_all(bytes)
        .await
        .map_err(|e| format!("write: {e}"))?;
    record_sent(slot, capture, record);
    Ok(())
}

/// While not connected only a close is acted on. The gateway refuses input then, so nothing
/// else should arrive; anything that slips in is dropped.
async fn wait_for_close(commands: &mut mpsc::Receiver<Command>) -> Option<oneshot::Sender<()>> {
    loop {
        match commands.recv().await {
            Some(Command::Close(ack) | Command::Hangup(ack)) => return Some(ack),
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
