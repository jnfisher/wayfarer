//! The [`Gateway`] handle: the sessions named in the config, and everything a server (C-006)
//! and later the brain do with them. The Accounts panel's save rewrites the config and changes
//! the sessions to match while the gateway runs ([`Gateway::save_accounts`]).

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures_util::{FutureExt, Stream, StreamExt};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use wayfarer_proto::{SessionInfo, SessionState, ToClient, ToGateway};
use wayfarer_telnet::Capture;

use crate::config::{self, Changes, Character, Config, ConfigError, Credentials, Paths};
use crate::log::{History, HistoryQuery, LineLog, NoteRecord, OutputLog, now_ms};
use crate::replay::BrainRecord;
use crate::session::{
    self, Command, Connect, Origin, Output, Running, Setup, Signup, Slot, SlotParts, StateChange,
    Tcp,
};

/// The most characters a save may hold: the web UI lays out six tiles at most.
pub const MAX_CHARACTERS: usize = 6;
/// The sign-up's e-mail and gender when a save gives none.
pub const DEFAULT_EMAIL: &str = "h@h.com";
pub const DEFAULT_GENDER: &str = "F";

/// Tunables; the defaults suit a real gateway, tests shrink them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Text kept in memory per session for fast history; older chunks are read from disk.
    pub memory_bytes: usize,
    /// Items (chunks and sent lines) a subscriber may fall behind by before it is told it lagged.
    pub subscriber_capacity: usize,
    /// The socket read size, so the largest chunk.
    pub read_buffer: usize,
    /// Reconnect a lost session with backoff (5 s, 10 s, 30 s, then every 60 s).
    pub reconnect: bool,
    /// How long [`Gateway::shutdown`] waits for the log-offs before it closes what is left:
    /// the brain's 30 s to get ready, the realm step's 240 s of retries and the menus.
    pub logoff_patience: Duration,
    /// Hold each logon's last step (`E`, Enter the Realm) until a brain is attached (C-054): at
    /// the game's menu nothing can attack the character, and in the realm with no brain nothing
    /// fights back. Off here (tests, the live check); `wayfarer gateway` turns it on.
    pub wait_for_brain: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            memory_bytes: 4 << 20,
            subscriber_capacity: 1024,
            read_buffer: 4096,
            reconnect: true,
            logoff_patience: Duration::from_secs(360),
            wait_for_brain: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("unknown session {0:?}")]
    UnknownSession(String),
    #[error("session {0:?} is not connected")]
    NotConnected(String),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("{0}")]
    Script(String),
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{0}")]
    Accounts(String),
}

/// The gateway: one session per configured character. Cheap to clone; every clone is the same
/// gateway. Dropping the last clone stops every session at once (no log-off, which the game
/// punishes: call [`Gateway::shutdown`] first).
///
/// Must be created and used inside a tokio runtime.
#[derive(Clone)]
pub struct Gateway {
    inner: Arc<Inner>,
}

struct Inner {
    /// In config order.
    slots: RwLock<Vec<Arc<Slot>>>,
    /// The config and logins the sessions were made from; a save replaces them. Held across a
    /// whole save, so saves happen one at a time.
    accounts: tokio::sync::Mutex<(Config, Credentials)>,
    /// Where a save writes the files; `None` for a gateway made without them (tests).
    paths: Option<Paths>,
    /// Bumped whenever the list of sessions (or a name or server on it) changes.
    roster: watch::Sender<u64>,
    connector: Arc<dyn Connect>,
    options: Options,
    /// Whether a brain is attached, for logons held at the realm's door
    /// ([`Options::wait_for_brain`]).
    brain: watch::Sender<bool>,
    states: broadcast::Sender<StateChange>,
    logs: PathBuf,
    logoff_patience: Duration,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let slots = self.slots.get_mut().unwrap_or_else(|e| e.into_inner());
        for slot in slots.iter() {
            if let Some(running) = slot.running().take() {
                running.task.abort();
            }
        }
    }
}

impl Gateway {
    /// Read the config and credentials (creating an empty, private `credentials.toml` if there
    /// is none), open the logs and connect the characters marked `autostart`.
    pub fn start(paths: &Paths, options: Options) -> Result<Self, GatewayError> {
        let config = Config::load(&paths.config)?;
        let credentials = Credentials::load_or_create(&paths.credentials)?;
        let gateway = Self::build(
            &config,
            &credentials,
            &paths.logs,
            options,
            Arc::new(Tcp),
            Some(paths.clone()),
        )?;
        for c in config.characters.iter().filter(|c| c.autostart) {
            gateway.open(&c.id)?;
        }
        Ok(gateway)
    }

    /// A gateway for `config`'s characters, logging under `logs`, with nothing connected yet.
    /// Each session's `seq` continues from its log on disk.
    pub fn new(
        config: &Config,
        credentials: &Credentials,
        logs: &Path,
        options: Options,
    ) -> Result<Self, GatewayError> {
        Self::with_connector(config, credentials, logs, options, Arc::new(Tcp))
    }

    /// [`Gateway::new`], reaching the servers through `connector` (tests: an in-memory pipe).
    pub fn with_connector(
        config: &Config,
        credentials: &Credentials,
        logs: &Path,
        options: Options,
        connector: Arc<dyn Connect>,
    ) -> Result<Self, GatewayError> {
        Self::build(config, credentials, logs, options, connector, None)
    }

    /// [`Gateway::with_connector`], saving to `paths` ([`Gateway::save_accounts`]) when given.
    pub fn build(
        config: &Config,
        credentials: &Credentials,
        logs: &Path,
        options: Options,
        connector: Arc<dyn Connect>,
        paths: Option<Paths>,
    ) -> Result<Self, GatewayError> {
        let gateway = Self {
            inner: Arc::new(Inner {
                slots: RwLock::new(Vec::with_capacity(config.characters.len())),
                accounts: tokio::sync::Mutex::new((config.clone(), credentials.clone())),
                paths,
                roster: watch::channel(0).0,
                connector,
                options,
                brain: watch::channel(false).0,
                states: broadcast::channel(256).0,
                logs: logs.to_path_buf(),
                logoff_patience: options.logoff_patience,
            }),
        };
        let mut slots = Vec::with_capacity(config.characters.len());
        for c in &config.characters {
            slots.push(gateway.new_slot(config, credentials, c)?);
        }
        *gateway.slots_mut() = slots;
        Ok(gateway)
    }

    /// A session for `c`, not connected.
    fn new_slot(
        &self,
        config: &Config,
        credentials: &Credentials,
        c: &Character,
    ) -> Result<Arc<Slot>, GatewayError> {
        let options = &self.inner.options;
        let dir = self.inner.logs.join(&c.id);
        let log =
            OutputLog::open(&dir, options.memory_bytes).map_err(|source| GatewayError::Io {
                path: dir.join("output.log"),
                source,
            })?;
        Ok(Arc::new(Slot::new(SlotParts {
            id: c.id.clone(),
            setup: setup(config, credentials, c)?,
            log,
            sent_log: LineLog::new(dir.join("sent.log")),
            brain_log: LineLog::new(dir.join("brain.log")),
            notes_log: LineLog::new(dir.join("notes.log")),
            read_buffer: options.read_buffer,
            reconnect: options.reconnect,
            connector: Arc::clone(&self.inner.connector),
            subscriber_capacity: options.subscriber_capacity,
            states: self.inner.states.clone(),
            brain: options.wait_for_brain.then(|| self.inner.brain.subscribe()),
        })))
    }

    fn slots(&self) -> std::sync::RwLockReadGuard<'_, Vec<Arc<Slot>>> {
        self.inner.slots.read().unwrap_or_else(|e| e.into_inner())
    }

    fn slots_mut(&self) -> std::sync::RwLockWriteGuard<'_, Vec<Arc<Slot>>> {
        self.inner.slots.write().unwrap_or_else(|e| e.into_inner())
    }

    fn slot(&self, id: &str) -> Result<Arc<Slot>, GatewayError> {
        self.slots()
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| GatewayError::UnknownSession(id.to_owned()))
    }

    /// Every session, in config order.
    pub fn sessions(&self) -> Vec<SessionInfo> {
        self.slots().iter().map(|s| s.info()).collect()
    }

    /// Changes whenever [`Gateway::sessions`] gains or loses a session, or a session's name or
    /// server changes (a save): send the list again.
    pub fn roster_changes(&self) -> watch::Receiver<u64> {
        self.inner.roster.subscribe()
    }

    /// The config and logins the sessions run on now.
    pub async fn accounts(&self) -> (Config, Credentials) {
        self.inner.accounts.lock().await.clone()
    }

    /// Write `config` and `credentials` to the gateway's files (if it has them) and change the
    /// sessions to match: a new character's session is added and connected, a removed one is
    /// hung up and dropped (its logs stay), one whose server or login changed is hung up and,
    /// if it was open, connected again, and one whose name or autostart changed only shows the
    /// new name. The rest are left alone. A new character, or one whose user changed, makes
    /// its BBS account if the BBS doesn't know the user, with its entry in `signups` (or
    /// [`DEFAULT_EMAIL`] and [`DEFAULT_GENDER`]). Nothing changes if the config is refused or
    /// the files can't be written.
    pub async fn save_accounts(
        &self,
        config: Config,
        credentials: Credentials,
        signups: HashMap<String, Signup>,
    ) -> Result<Changes, GatewayError> {
        let mut accounts = self.inner.accounts.lock().await;
        if config.characters.len() > MAX_CHARACTERS {
            return Err(GatewayError::Accounts(format!(
                "at most {MAX_CHARACTERS} characters (there are {})",
                config.characters.len()
            )));
        }
        config.validate().map_err(GatewayError::Accounts)?;
        let mut setups = HashMap::new();
        for c in &config.characters {
            setups.insert(c.id.clone(), setup(&config, &credentials, c)?);
        }
        if let Some(paths) = &self.inner.paths {
            config::save(paths, &config, &credentials)?;
        }
        let changes = config::diff(&accounts.0, &accounts.1, &config, &credentials);
        let signup_for = |id: &str| {
            signups.get(id).cloned().unwrap_or_else(|| Signup {
                email: DEFAULT_EMAIL.to_owned(),
                gender: DEFAULT_GENDER.to_owned(),
            })
        };

        for id in &changes.removed {
            tracing::info!(session = %id, "removed from the config: hanging up");
            self.close_now(id).await?;
        }
        self.slots_mut()
            .retain(|s| !changes.removed.contains(&s.id));
        for id in changes.reconnect.iter().chain(&changes.updated) {
            self.slot(id)?.set_setup(setups[id].clone());
        }
        for id in &changes.reconnect {
            let slot = self.slot(id)?;
            if changes.new_user.contains(id) {
                slot.set_signup(Some(signup_for(id)));
            }
            let open = slot.running().as_ref().is_some_and(Running::is_live);
            if open {
                tracing::info!(session = %id, "server or login changed: logging on again");
                self.close_now(id).await?;
                self.open(id)?;
            }
        }
        for id in &changes.added {
            let c = config
                .character(id)
                .expect("an added character is in the config");
            let slot = self.new_slot(&config, &credentials, c)?;
            slot.set_signup(Some(signup_for(id)));
            self.slots_mut().push(slot);
        }
        // Config order.
        let order: HashMap<&str, usize> = config
            .characters
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.as_str(), i))
            .collect();
        self.slots_mut().sort_by_key(|s| order[s.id.as_str()]);
        *accounts = (config, credentials);
        drop(accounts);
        self.inner.roster.send_modify(|n| *n += 1);
        for id in &changes.added {
            tracing::info!(session = %id, "added to the config: connecting");
            self.open(id)?;
        }
        Ok(changes)
    }

    /// One session's state.
    pub fn state(&self, id: &str) -> Result<SessionState, GatewayError> {
        Ok(self.slot(id)?.state())
    }

    /// Why a session is `closed` or `lost`, when there is more to say (`"bad credentials"`,
    /// `"logon step timed out"`). Cleared by [`Gateway::open`].
    pub fn reason(&self, id: &str) -> Result<Option<String>, GatewayError> {
        Ok(self.slot(id)?.reason())
    }

    /// Every state change of every session, from now on. A listener that lags has missed
    /// changes and should re-read [`Gateway::sessions`].
    pub fn state_changes(&self) -> broadcast::Receiver<StateChange> {
        self.inner.states.subscribe()
    }

    /// Connect a session: `connecting`, then `logon` while its logon script runs, `menu` and
    /// `realm` as the text shows them (`connected` if it has no script, or no credentials). A
    /// connection that fails or drops is `lost` and reconnected with backoff. Does nothing if
    /// the session is already open (reconnecting included).
    pub fn open(&self, id: &str) -> Result<(), GatewayError> {
        let slot = self.slot(id)?;
        let mut running = slot.running();
        if running.as_ref().is_some_and(Running::is_live) {
            return Ok(());
        }
        let dir = self.inner.logs.join(id);
        let capture = Capture::create(dir.join("raw.bin"), dir.join("sent.bin"))
            .map_err(|source| GatewayError::Io { path: dir, source })?;
        let (commands, rx) = mpsc::channel(64);
        slot.set_reason(None);
        slot.set_state(SessionState::Connecting);
        let task = tokio::spawn(session::run(Arc::clone(&slot), rx, capture));
        *running = Some(Running { commands, task });
        Ok(())
    }

    /// Log a session off cleanly and disconnect it (state `closed`), and wait until it is. From
    /// the realm the script's log-off steps run (`x`, `X`, `X`, `Y` on MBBSEmu, which can take
    /// a minute and a half), from a menu the remaining ones; anywhere else it just disconnects.
    /// Never reconnected.
    pub async fn close(&self, id: &str) -> Result<(), GatewayError> {
        self.close_with(id, Command::Close).await
    }

    /// Disconnect a session at once, logged off or not (state `closed`).
    pub async fn close_now(&self, id: &str) -> Result<(), GatewayError> {
        self.close_with(id, Command::Hangup).await
    }

    async fn close_with(
        &self,
        id: &str,
        command: fn(oneshot::Sender<()>) -> Command,
    ) -> Result<(), GatewayError> {
        let slot = self.slot(id)?;
        let commands = slot
            .running()
            .as_ref()
            .filter(|r| r.is_live())
            .map(|r| r.commands.clone());
        if let Some(commands) = commands {
            let (ack, done) = oneshot::channel();
            if commands.send(command(ack)).await.is_ok() {
                let _ = done.await;
            }
        }
        // Not connected (or the connection dropped meanwhile): closing still says "on purpose".
        // Unless someone opened it again meanwhile.
        let running = slot.running();
        if !running.as_ref().is_some_and(Running::is_live) {
            slot.set_state(SessionState::Closed);
        }
        Ok(())
    }

    /// Log every session off, all at once, and wait for them; after
    /// [`Options::logoff_patience`] (360 s) close what is left at once.
    pub async fn shutdown(&self) {
        self.shutdown_after(|_| async { true }).await;
    }

    /// [`Gateway::shutdown`], each session in the realm first waiting for `ready(id)` (the
    /// brain getting the character ready to leave), which says whether to log off cleanly or
    /// hang up; the patience covers the wait too.
    pub async fn shutdown_after<F, Fut>(&self, ready: F)
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        let ready = Arc::new(ready);
        let all = self.each(move |gateway, id| {
            let ready = Arc::clone(&ready);
            async move {
                if gateway.state(&id)? == SessionState::Realm && !ready(id.clone()).await {
                    return gateway.close_now(&id).await;
                }
                gateway.close(&id).await
            }
        });
        if tokio::time::timeout(self.inner.logoff_patience, all)
            .await
            .is_err()
        {
            tracing::warn!("log-off is taking too long; closing what is left");
            self.shutdown_now().await;
        }
    }

    /// Close every session at once, logged off or not.
    pub async fn shutdown_now(&self) {
        self.each(|gateway, id| async move { gateway.close_now(&id).await })
            .await;
    }

    /// [`Gateway::shutdown`], unless `now` completes first: then close what is left at once
    /// ([`Gateway::shutdown_now`]).
    pub async fn shutdown_unless(&self, now: impl Future<Output = ()>) {
        tokio::select! {
            () = self.shutdown() => {}
            () = now => {
                tracing::warn!("closing every session at once");
                self.shutdown_now().await;
            }
        }
    }

    /// Wait for SIGINT (Ctrl-C) or SIGTERM, then [`Gateway::shutdown`]; a second signal while
    /// that runs closes everything at once.
    pub async fn shutdown_on_signal(&self) -> io::Result<()> {
        let mut signals = std::pin::pin!(stop_signals()?);
        signals.next().await;
        tracing::info!("logging every character off (a second signal closes at once)");
        self.shutdown_unless(signals.next().map(drop)).await;
        Ok(())
    }

    /// Run `f` for every session, all at once, and wait for them.
    async fn each<F, Fut>(&self, f: F)
    where
        F: Fn(Gateway, String) -> Fut,
        Fut: Future<Output = Result<(), GatewayError>> + Send + 'static,
    {
        let mut running = tokio::task::JoinSet::new();
        let ids: Vec<String> = self.slots().iter().map(|s| s.id.clone()).collect();
        for id in ids {
            running.spawn(f(self.clone(), id));
        }
        while running.join_next().await.is_some() {}
    }

    /// Live output of a session from now on, whether or not it is connected: its chunks, and
    /// each line it sends to its game (typed lines, not keys, and logon and log-off steps) once
    /// it is on the socket, with passwords masked, in the order they happened. A receiver that
    /// falls more than [`Options::subscriber_capacity`] items behind gets
    /// `RecvError::Lagged(missed)` and should fetch the gap with [`Gateway::history`] (missed
    /// `Sent`s are gone); it never holds up the session or other receivers.
    pub fn subscribe(&self, id: &str) -> Result<broadcast::Receiver<Output>, GatewayError> {
        Ok(self.slot(id)?.output.subscribe())
    }

    /// Stored output: from memory when it is recent, otherwise from the log on disk.
    pub async fn history(&self, id: &str, query: HistoryQuery) -> Result<History, GatewayError> {
        let slot = self.slot(id)?;
        let path = self.inner.logs.join(id).join("output.log");
        tokio::task::spawn_blocking(move || slot.log.history(query))
            .await
            .unwrap_or_else(|e| Err(io::Error::other(e)))
            .map_err(|source| GatewayError::Io { path, source })
    }

    /// Send a line (the gateway adds the line ending). Lines from any number of callers go out
    /// whole, one after another. `sent.log` says a UI sent it.
    pub async fn send_line(&self, id: &str, text: &str) -> Result<(), GatewayError> {
        self.send_line_as(id, text, Origin::Ui).await
    }

    /// [`Gateway::send_line`], recorded in `sent.log` as sent by `origin`.
    pub async fn send_line_as(
        &self,
        id: &str,
        text: &str,
        origin: Origin,
    ) -> Result<(), GatewayError> {
        self.send(id, Command::Line(text.to_owned(), origin)).await
    }

    /// Record in every session's `brain.log` that a brain attached, or went; a logon held for a
    /// brain goes on into the realm.
    pub fn note_brain(&self, attached: bool) {
        self.inner.brain.send_replace(attached);
        for slot in self.slots().iter() {
            slot.brain_log.append(&BrainRecord {
                seq: slot.log.last_seq(),
                ts: now_ms(),
                attached: Some(attached),
                msg: None,
            });
        }
    }

    /// Record in its session's `brain.log` a message that reached the brain other than output
    /// and `sent` (a UI's control message, `user_input`, `keys_sent`, `logging_off`), so a
    /// replay can hand it to the brain again. Messages about no session are not recorded.
    pub fn note_to_brain(&self, msg: &ToClient) {
        if let Some(slot) = msg.session().and_then(|id| self.slot(id).ok()) {
            slot.brain_log.append(&BrainRecord {
                seq: slot.log.last_seq(),
                ts: now_ms(),
                attached: None,
                msg: Some(msg.clone()),
            });
        }
    }

    /// Record in its session's `notes.log` a notice or warning the brain sent about it (what the
    /// bot said it was doing, and why it stopped), with the session's latest output `seq`. Other
    /// messages, and notices about no session, are not recorded.
    pub fn note_from_brain(&self, msg: &ToGateway) {
        let (session, kind, text) = match msg {
            ToGateway::Notice {
                session: Some(session),
                text,
            } => (session, "notice", text),
            ToGateway::Warning {
                session: Some(session),
                text,
            } => (session, "warning", text),
            _ => return,
        };
        if let Ok(slot) = self.slot(session) {
            slot.notes_log.append(&NoteRecord {
                seq: slot.log.last_seq(),
                ts: now_ms(),
                kind,
                text,
            });
        }
    }

    /// Send raw bytes as they are (`Hit any key`, the character editor).
    pub async fn send_keys(&self, id: &str, bytes: &[u8]) -> Result<(), GatewayError> {
        self.send(id, Command::Keys(bytes.to_vec())).await
    }

    async fn send(&self, id: &str, command: Command) -> Result<(), GatewayError> {
        let slot = self.slot(id)?;
        let not_connected = || GatewayError::NotConnected(id.to_owned());
        let commands = slot
            .running()
            .as_ref()
            .filter(|r| r.is_live())
            .map(|r| r.commands.clone())
            .ok_or_else(not_connected)?;
        // Open, but between connections.
        if matches!(slot.state(), SessionState::Connecting | SessionState::Lost) {
            return Err(not_connected());
        }
        commands.send(command).await.map_err(|_| not_connected())
    }
}

/// What `c`'s session connects with.
fn setup(config: &Config, credentials: &Credentials, c: &Character) -> Result<Setup, GatewayError> {
    let server = config
        .server(&c.server)
        .expect("Config::parse checks every character's server");
    Ok(Setup {
        name: c.name.clone(),
        host: server.host.clone(),
        port: server.port,
        login: credentials.get(&c.id).cloned(),
        script: server.script().map_err(GatewayError::Script)?.map(Arc::new),
    })
}

/// One item per SIGINT (Ctrl-C) or, on Unix, SIGTERM; it never ends. The first should start
/// [`Gateway::shutdown`] and a second close at once: [`Gateway::shutdown_on_signal`] and the
/// binary's [`crate::serve`] both take it that way.
pub fn stop_signals() -> io::Result<impl Stream<Item = ()> + Send + 'static> {
    #[cfg(unix)]
    let term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(not(unix))]
    let term = ();
    Ok(futures_util::stream::unfold(term, |mut term| async move {
        #[cfg(unix)]
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
        Some(((), term))
    }))
}
