//! One session: the task that owns a character's socket. It reads the game, answers telnet
//! negotiation, logs and publishes the output, and writes input, one command at a time, so
//! input from several clients never interleaves within a line. It runs the logon script,
//! follows the text to know where the character is, logs it off cleanly when closed, and
//! reconnects with backoff after a lost connection.

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use wayfarer_proto::{Chunk, SessionId, SessionInfo, SessionState};
use wayfarer_telnet::{Capture, Telnet};

use crate::config::Login;
use crate::log::{LineLog, OutputLog, now_ms};
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

/// Who sent a line: `sent.log`'s `origin`. `ui` is a UI's line that went straight to the game;
/// a line the brain lets through for the player (input arbitration) is the brain's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Ui,
    Brain,
    /// A logon or log-off step.
    Script,
}

/// One line of `sent.log`.
#[derive(Serialize)]
struct SentRecord<'a> {
    seq: u64,
    ts: u64,
    origin: Origin,
    text: &'a str,
}

/// A line a replay's recording sent (`replay.rs`): the session publishes it as `sent` once it
/// has read `after` bytes of the replayed output, so it lands between the chunks it went
/// between live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub after: u64,
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

    /// A replay's recorded lines (`replay.rs`), taken once per connection, just after
    /// [`Connect::connect`]. With them the session publishes these as `sent` instead of the
    /// lines it writes (which the replay drops), but for the brain's, and a close disconnects
    /// without logging off. `None`, the default, for a game server.
    fn recorded(&self) -> Option<mpsc::UnboundedReceiver<Recorded>> {
        None
    }
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
    Line(String, Origin),
    Keys(Vec<u8>),
    /// Log off cleanly, then close.
    Close(oneshot::Sender<()>),
    /// Close at once, logged off or not.
    Hangup(oneshot::Sender<()>),
}

/// What a character's session connects with, from the config: the Accounts panel's save can
/// change it while the session lives ([`Slot::set_setup`]). A connection reads it once, when
/// it starts.
#[derive(Clone)]
pub(crate) struct Setup {
    pub name: String,
    pub host: String,
    pub port: u16,
    /// The character's account: the logon script's `{user}` and `{password}`, and a line that
    /// is exactly the password is masked in `sent.bin`.
    pub login: Option<Login>,
    pub script: Option<Arc<Script>>,
}

/// A BBS account to make if the BBS does not know the user: the sign-up's e-mail and gender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signup {
    pub email: String,
    pub gender: String,
}

/// Everything about one character's session that outlives its connections.
pub(crate) struct Slot {
    pub id: SessionId,
    setup: Mutex<Setup>,
    /// Set by the Accounts panel's save for a new character, or one whose user changed: the
    /// next logon the BBS answers "unknown user" signs up instead. Cleared by the first logon
    /// that gets in, or the sign-up.
    signup: Mutex<Option<Signup>>,
    pub log: OutputLog,
    /// Every line written to the game, with who sent it.
    pub sent_log: LineLog,
    /// What reached the brain about this session besides output and `sent` (`replay.rs`).
    pub brain_log: LineLog,
    /// The brain's own notices and warnings about this session (`notes.log`): what the bot
    /// said it was doing and why it stopped.
    pub notes_log: LineLog,
    pub output: broadcast::Sender<Output>,
    pub read_buffer: usize,
    pub reconnect: bool,
    pub connector: Arc<dyn Connect>,
    /// Whether a brain is attached, when the logon waits for one to enter the realm (C-054).
    pub brain: Option<watch::Receiver<bool>>,
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
    pub setup: Setup,
    pub log: OutputLog,
    pub sent_log: LineLog,
    pub brain_log: LineLog,
    pub notes_log: LineLog,
    pub read_buffer: usize,
    pub reconnect: bool,
    pub connector: Arc<dyn Connect>,
    pub subscriber_capacity: usize,
    pub states: broadcast::Sender<StateChange>,
    pub brain: Option<watch::Receiver<bool>>,
}

impl Slot {
    pub fn new(p: SlotParts) -> Self {
        Self {
            id: p.id,
            setup: Mutex::new(p.setup),
            signup: Mutex::new(None),
            log: p.log,
            sent_log: p.sent_log,
            brain_log: p.brain_log,
            notes_log: p.notes_log,
            output: broadcast::channel(p.subscriber_capacity.max(1)).0,
            read_buffer: p.read_buffer.max(1),
            reconnect: p.reconnect,
            connector: p.connector,
            brain: p.brain,
            states: p.states,
            // Never opened: nothing is connected, and on purpose.
            state: Mutex::new(SessionState::Closed),
            reason: Mutex::new(None),
            running: Mutex::new(None),
        }
    }

    pub fn setup(&self) -> Setup {
        self.setup.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Connect with `setup` from the next connection on.
    pub fn set_setup(&self, setup: Setup) {
        *self.setup.lock().unwrap_or_else(|e| e.into_inner()) = setup;
    }

    pub fn set_signup(&self, signup: Option<Signup>) {
        *self.signup.lock().unwrap_or_else(|e| e.into_inner()) = signup;
    }

    pub fn take_signup(&self) -> Option<Signup> {
        self.signup.lock().unwrap_or_else(|e| e.into_inner()).take()
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

    /// `text` went to the game: into `sent.log`, and to the subscribers. A replay publishes the
    /// recording's lines instead, but the brain's own: a replay's brain hears what it sent.
    fn sent_line(&self, text: String, origin: Origin, replay: bool) {
        self.sent_log.append(&SentRecord {
            seq: self.log.last_seq(),
            ts: now_ms(),
            origin,
            text: &text,
        });
        if !replay || origin == Origin::Brain {
            self.publish_sent(text);
        }
    }

    /// Tell the subscribers that `text` went to the game. Only the session task publishes, so
    /// this lands between the chunks it went between.
    fn publish_sent(&self, text: String) {
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
        let setup = self.setup();
        SessionInfo {
            id: self.id.clone(),
            name: setup.name,
            server: format!("{}:{}", setup.host, setup.port),
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
            self.setup().name
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
    // The account to make on this connection: the last one's logon found the user unknown.
    let mut signup = None;
    let (state, acks) = loop {
        match connection(
            &slot,
            &mut commands,
            &mut capture,
            &mut attempt,
            signup.take(),
        )
        .await
        {
            End::Closed { acks, reason } => {
                slot.set_reason(reason);
                break (SessionState::Closed, acks);
            }
            End::SignUp(account) => {
                tracing::info!(session = %slot.id, "the BBS does not know the user: making the account");
                signup = Some(account);
            }
            End::Lost(why) => {
                // A sign-up cut short is not tried again: the next logon finds out whether the
                // account was made, and a BBS whose prompts the sign-up doesn't know can't
                // keep it trying.
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
    /// The BBS does not know the user and the account is to be made: reconnect at once and
    /// sign up.
    SignUp(Signup),
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

/// The session's reason while its logon waits at the realm's door for a brain.
pub const WAITING_FOR_BRAIN: &str = "waiting for a brain to enter the realm";
/// The session's reason once a sign-up has made the account and stopped at the menu.
pub const ACCOUNT_MADE: &str = "account made: type E in the tile to make the character";
/// The session's reason when the logon got no realm prompt after its last step (a character
/// still being made, say).
pub const NO_REALM: &str = "no realm prompt: finish making the character in the tile";
/// The session's reason when the BBS doesn't know the user and the logon script can't sign up.
pub const CANNOT_SIGN_UP: &str = "bad credentials (this server's logon script can't make accounts)";

/// One connection, from connecting until it ends.
async fn connection(
    slot: &Slot,
    commands: &mut mpsc::Receiver<Command>,
    capture: &mut Capture,
    attempt: &mut u32,
    signup: Option<Signup>,
) -> End {
    let setup = slot.setup();
    slot.set_state(SessionState::Connecting);
    let address = format!("{}:{}", setup.host, setup.port);
    let stream = tokio::select! {
        r = slot.connector.connect(&setup.host, setup.port) => match r {
            Ok(s) => s,
            Err(e) => return End::Lost(format!("connecting to {address}: {e}")),
        },
        ack = wait_for_close(commands) => return End::closed(ack.into_iter().collect()),
    };
    tracing::info!(session = %slot.id, "connected to {address}");
    slot.set_reason(None);
    // A replay's lines, each held until the output it followed has been read.
    let mut recorded = slot.connector.recorded();
    let replay = recorded.is_some();
    let mut held: VecDeque<Recorded> = VecDeque::new();
    let mut read: u64 = 0;

    let mut tracker = setup.script.clone().map(Tracker::new);
    let signing_up = signup.is_some();
    let mut runner = match (&setup.script, &setup.login, &signup) {
        (Some(script), Some(login), Some(account)) => Runner::signup(
            script,
            &login.user,
            &login.password,
            &account.email,
            &account.gender,
        ),
        (Some(script), Some(login), None) => Some(Runner::new(
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
        if setup.script.is_some() {
            tracing::warn!(session = %slot.id, "no credentials: the player logs on by hand");
        }
        // The player has the session.
        slot.set_state(SessionState::Connected);
        *attempt = 0;
    }
    let mut logoff: Option<Logoff> = None;
    let mut acks = Vec::new();
    // The logon's last line (Enter the Realm), held until a brain is attached (C-054).
    let mut brain = slot.brain.clone();
    let mut entry: Option<(String, String)> = None;

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
                read += n as u64;
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
                release(slot, &mut held, read);
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
                            // A sign-up stops at the menu: it never enters.
                            let enters = !signing_up && runner.as_ref().is_some_and(Runner::waiting_for_realm);
                            if enters && brain.as_ref().is_some_and(|b| !*b.borrow()) {
                                tracing::info!(session = %slot.id, "waiting for a brain to enter the realm");
                                slot.set_reason(Some(WAITING_FOR_BRAIN.to_owned()));
                                entry = Some((text, shown));
                                deadline = None;
                                continue;
                            }
                            let (bytes, record) = (telnet.encode_line(&text), telnet.encode_line(&shown));
                            if let Err(e) = write(slot, &mut writer, capture, &bytes, &record).await {
                                return End::Lost(e);
                            }
                            slot.sent_line(shown, Origin::Script, replay);
                            // The next step's timeout runs from here.
                            deadline = runner.as_ref().map(|r| Instant::now() + r.timeout());
                        }
                        Action::Realm if signing_up => {
                            // At the MajorMUD menu: the player makes the character.
                            tracing::info!(session = %slot.id, "account made");
                            slot.set_state(SessionState::Menu);
                            slot.set_reason(Some(ACCOUNT_MADE.to_owned()));
                            *attempt = 0;
                            runner = None;
                            deadline = None;
                        }
                        Action::Realm => {
                            tracing::info!(session = %slot.id, "logged on");
                            // The account was there all along.
                            slot.set_signup(None);
                            slot.set_state(SessionState::Realm);
                            *attempt = 0;
                            runner = None;
                            deadline = None;
                        }
                        Action::Failed { reason, retry, signup } => {
                            let _ = writer.shutdown().await;
                            if signup && let Some(account) = slot.take_signup() {
                                if setup.script.as_ref().is_some_and(|s| s.signup.is_some()) {
                                    return End::SignUp(account);
                                }
                                tracing::warn!(session = %slot.id, "logon failed: {CANNOT_SIGN_UP}");
                                return End::Closed { acks, reason: Some(CANNOT_SIGN_UP.to_owned()) };
                            }
                            if retry {
                                return End::Lost(format!("logon failed: {reason}"));
                            }
                            let what = if signing_up { "sign-up refused" } else { "logon failed" };
                            tracing::warn!(session = %slot.id, "{what}: {reason}");
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
                        slot.sent_line(line, Origin::Script, replay);
                    }
                }
            }
            changed = async { brain.as_mut().expect("held for a brain").changed().await }, if entry.is_some() => {
                // A gateway gone takes its sender with it: enter, as without the wait.
                if changed.is_err() {
                    brain = None;
                }
                if brain.as_ref().is_none_or(|b| *b.borrow()) && runner.is_some() {
                    let (text, shown) = entry.take().expect("held");
                    let (bytes, record) = (telnet.encode_line(&text), telnet.encode_line(&shown));
                    if let Err(e) = write(slot, &mut writer, capture, &bytes, &record).await {
                        return End::Lost(e);
                    }
                    tracing::info!(session = %slot.id, "a brain is attached: entering the realm");
                    slot.set_reason(None);
                    slot.sent_line(shown, Origin::Script, replay);
                    deadline = runner.as_ref().map(|r| Instant::now() + r.timeout());
                }
            }
            item = next_recorded(&mut recorded), if recorded.is_some() => match item {
                Some(line) => {
                    held.push_back(line);
                    release(slot, &mut held, read);
                }
                // The recording is all fed, its lines with it.
                None => recorded = None,
            },
            cmd = commands.recv() => {
                // The bytes, what `sent.bin` records, and the line for `sent` (keys are none).
                let (bytes, record, line) = match cmd {
                    Some(Command::Line(text, origin)) => {
                        let bytes = telnet.encode_line(&text);
                        let is_password = setup.login.as_ref().is_some_and(|l| l.password == text);
                        let shown = if is_password { MASK.to_owned() } else { text };
                        (bytes, telnet.encode_line(&shown), Some((shown, origin)))
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
                        if replay {
                            // Nobody to log off: stopping a replay just stops it.
                            let _ = writer.shutdown().await;
                            return End::closed(acks);
                        }
                        let start = setup.script.as_ref().zip(tracker.as_ref().and_then(Tracker::place));
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
                        (bytes.clone(), bytes, Some((first, Origin::Script)))
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
                if let Some((line, origin)) = line {
                    slot.sent_line(line, origin, replay);
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
                        slot.set_reason(Some(NO_REALM.to_owned()));
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

/// Publish the held recorded lines whose output has all been read (`read` bytes).
fn release(slot: &Slot, held: &mut VecDeque<Recorded>, read: u64) {
    while held.front().is_some_and(|r| r.after <= read) {
        let line = held.pop_front().expect("not empty");
        slot.publish_sent(line.text);
    }
}

async fn next_recorded(
    recorded: &mut Option<mpsc::UnboundedReceiver<Recorded>>,
) -> Option<Recorded> {
    match recorded {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
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
