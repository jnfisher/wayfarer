//! The [`Gateway`] handle: the sessions named in the config, and everything a server (C-006),
//! the logon scripts (C-005) and later the brain do with them.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, oneshot};
use wayfarer_proto::{Chunk, SessionInfo, SessionState};
use wayfarer_telnet::Capture;

use crate::config::{Config, ConfigError, Credentials, Paths};
use crate::log::{History, HistoryQuery, OutputLog};
use crate::session::{self, Command, Running, Slot, SlotParts, StateChange};

/// Tunables; the defaults suit a real gateway, tests shrink them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Text kept in memory per session for fast history; older chunks are read from disk.
    pub memory_bytes: usize,
    /// Chunks a subscriber may fall behind by before it is told it lagged.
    pub subscriber_capacity: usize,
    /// The socket read size, so the largest chunk.
    pub read_buffer: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            memory_bytes: 4 << 20,
            subscriber_capacity: 1024,
            read_buffer: 4096,
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
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

/// The gateway: one session per configured character. Cheap to clone; every clone is the same
/// gateway. Dropping the last clone stops every session at once (no log-off: call
/// [`Gateway::shutdown`] first for that).
///
/// Must be created and used inside a tokio runtime.
#[derive(Clone)]
pub struct Gateway {
    inner: Arc<Inner>,
}

struct Inner {
    /// In config order.
    slots: Vec<Arc<Slot>>,
    states: broadcast::Sender<StateChange>,
    logs: PathBuf,
}

impl Drop for Inner {
    fn drop(&mut self) {
        for slot in &self.slots {
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
        let gateway = Self::new(&config, &credentials, &paths.logs, options)?;
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
        let states = broadcast::channel(256).0;
        let mut slots = Vec::with_capacity(config.characters.len());
        for c in &config.characters {
            let server = config
                .server(&c.server)
                .expect("Config::parse checks every character's server");
            let dir = logs.join(&c.id);
            let log =
                OutputLog::open(&dir, options.memory_bytes).map_err(|source| GatewayError::Io {
                    path: dir.join("output.log"),
                    source,
                })?;
            slots.push(Arc::new(Slot::new(SlotParts {
                id: c.id.clone(),
                name: c.name.clone(),
                host: server.host.clone(),
                port: server.port,
                log,
                password: credentials.get(&c.id).map(|l| l.password.clone()),
                read_buffer: options.read_buffer,
                subscriber_capacity: options.subscriber_capacity,
                states: states.clone(),
            })));
        }
        Ok(Self {
            inner: Arc::new(Inner {
                slots,
                states,
                logs: logs.to_path_buf(),
            }),
        })
    }

    fn slot(&self, id: &str) -> Result<&Arc<Slot>, GatewayError> {
        self.inner
            .slots
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| GatewayError::UnknownSession(id.to_owned()))
    }

    /// Every session, in config order.
    pub fn sessions(&self) -> Vec<SessionInfo> {
        self.inner.slots.iter().map(|s| s.info()).collect()
    }

    /// One session's state.
    pub fn state(&self, id: &str) -> Result<SessionState, GatewayError> {
        Ok(self.slot(id)?.state())
    }

    /// Every state change of every session, from now on. A listener that lags has missed
    /// changes and should re-read [`Gateway::sessions`].
    pub fn state_changes(&self) -> broadcast::Receiver<StateChange> {
        self.inner.states.subscribe()
    }

    /// Connect a session (state `connecting`, then `logon` once connected, `lost` if the
    /// connection fails or drops). Does nothing if it is already connecting or connected.
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
        slot.set_state(SessionState::Connecting);
        let task = tokio::spawn(session::run(Arc::clone(slot), rx, capture));
        *running = Some(Running { commands, task });
        Ok(())
    }

    /// Disconnect a session on purpose (state `closed`) and wait until it is. C-005 makes this
    /// a clean log-off first.
    pub async fn close(&self, id: &str) -> Result<(), GatewayError> {
        let slot = self.slot(id)?;
        let commands = slot
            .running()
            .as_ref()
            .filter(|r| r.is_live())
            .map(|r| r.commands.clone());
        if let Some(commands) = commands {
            let (ack, done) = oneshot::channel();
            if commands.send(Command::Close(ack)).await.is_ok() {
                let _ = done.await;
            }
        }
        // Not connected (or the connection dropped meanwhile): closing still says "on purpose".
        slot.set_state(SessionState::Closed);
        Ok(())
    }

    /// Close every session, all at once, and wait for them.
    pub async fn shutdown(&self) {
        let mut closing = tokio::task::JoinSet::new();
        for slot in &self.inner.slots {
            let (gateway, id) = (self.clone(), slot.id.clone());
            closing.spawn(async move { gateway.close(&id).await });
        }
        while closing.join_next().await.is_some() {}
    }

    /// Live output of a session from now on, whether or not it is connected. A receiver that
    /// falls more than [`Options::subscriber_capacity`] chunks behind gets
    /// `RecvError::Lagged(missed)` and should fetch the gap with [`Gateway::history`]; it never
    /// holds up the session or other receivers.
    pub fn subscribe(&self, id: &str) -> Result<broadcast::Receiver<Chunk>, GatewayError> {
        Ok(self.slot(id)?.output.subscribe())
    }

    /// Stored output: from memory when it is recent, otherwise from the log on disk.
    pub async fn history(&self, id: &str, query: HistoryQuery) -> Result<History, GatewayError> {
        let slot = Arc::clone(self.slot(id)?);
        let path = self.inner.logs.join(id).join("output.log");
        tokio::task::spawn_blocking(move || slot.log.history(query))
            .await
            .unwrap_or_else(|e| Err(io::Error::other(e)))
            .map_err(|source| GatewayError::Io { path, source })
    }

    /// Send a line (the gateway adds the line ending). Lines from any number of callers go out
    /// whole, one after another.
    pub async fn send_line(&self, id: &str, text: &str) -> Result<(), GatewayError> {
        self.send(id, Command::Line(text.to_owned())).await
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
        if slot.state() == SessionState::Connecting {
            return Err(not_connected());
        }
        commands.send(command).await.map_err(|_| not_connected())
    }
}
