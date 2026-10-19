//! The brain's loop: attach, follow every session, answer the UIs' control messages, re-sync.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::time::Instant;
use wayfarer_parse::{GameTexts, Registry};
use wayfarer_proto::{
    CharacterState, ClientKind, PROTOCOL, SessionInfo, ToClient, ToGateway, WorldPackInfo,
};
use wayfarer_world::WorldPack;

use crate::clock::{Clock, rfc3339};
use crate::link::Link;
use crate::session::{Progress, Session};

/// This build: the version, and the commit it was built from when known.
pub const BUILD: &str = env!("WAYFARER_BUILD");

#[derive(Debug, thiserror::Error)]
pub enum BrainError {
    /// The gateway said no (another brain is attached, a wrong token, another protocol).
    #[error("the gateway refused the brain: {0}")]
    Refused(String),
    /// The connection ended before the gateway welcomed the brain.
    #[error("the gateway closed the connection before welcoming the brain")]
    NoWelcome,
    #[error("world pack {path}: {source}")]
    World {
        path: String,
        source: wayfarer_world::Error,
    },
}

/// What the brain parses with: the game texts' pattern registry, and which world pack it came
/// from.
#[derive(Clone)]
pub struct World {
    pub registry: Arc<Registry>,
    pub info: Option<WorldPackInfo>,
}

impl World {
    /// Load a world pack (`wayfarer import-world` builds it).
    pub fn open(path: &Path) -> Result<Self, BrainError> {
        let err = |source| BrainError::World {
            path: path.display().to_string(),
            source,
        };
        let pack = WorldPack::open(path).map_err(err)?;
        let texts = pack.game_texts().map_err(err)?;
        let hash = file_sha256(path).map_err(|source| {
            err(wayfarer_world::Error::Io {
                path: path.to_path_buf(),
                source,
            })
        })?;
        Ok(Self {
            registry: Arc::new(Registry::new(&texts)),
            info: Some(WorldPackInfo {
                path: path.display().to_string(),
                hash: format!("sha256:{hash}"),
            }),
        })
    }

    /// Game texts from elsewhere (the stock-world export, in tests).
    pub fn from_texts(texts: &GameTexts) -> Self {
        Self::from_registry(Arc::new(Registry::new(texts)))
    }

    pub fn from_registry(registry: Arc<Registry>) -> Self {
        Self {
            registry,
            info: None,
        }
    }
}

fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// The brain: every session's parser, character and re-sync, kept across attaches (a
/// gateway that went away and came back). It keeps nothing on disk in M1.
pub struct Brain {
    world: World,
    clock: Arc<dyn Clock>,
    token: Option<String>,
    started_at: String,
    sessions: BTreeMap<String, Session>,
    attaches: u64,
}

impl Brain {
    /// `token`: what the gateway's handshake wants, if it checks one.
    pub fn new(world: World, clock: Arc<dyn Clock>, token: Option<String>) -> Self {
        let started_at = rfc3339(clock.now());
        Self {
            world,
            clock,
            token,
            started_at,
            sessions: BTreeMap::new(),
            attaches: 0,
        }
    }

    /// The character the brain has for `session`, if it knows the session.
    pub fn snapshot(&self, session: &str) -> Option<CharacterState> {
        self.sessions.get(session).map(|s| s.character.snapshot())
    }

    /// Attach over `link` and follow the gateway until the connection ends (`Ok`): the caller
    /// may connect again and call `run` once more, and the brain carries on where it was.
    /// `Err` when the gateway refuses the brain or ends the connection before welcoming it.
    /// Dropping the future stops the brain; it sends nothing to the game on the way out.
    pub async fn run(&mut self, link: &mut impl Link) -> Result<(), BrainError> {
        link.send(ToGateway::Hello {
            protocol: PROTOCOL,
            client: ClientKind::Brain,
            token: self.token.clone(),
        });
        let sessions = loop {
            match link.recv().await {
                None => return Err(BrainError::NoWelcome),
                Some(ToClient::Welcome { sessions, .. }) => break sessions,
                Some(ToClient::Refused { reason }) => return Err(BrainError::Refused(reason)),
                Some(other) => {
                    tracing::debug!("before the welcome: {:?}", other.type_name())
                }
            }
        };
        self.attaches += 1;
        tracing::info!(
            attach = self.attaches,
            sessions = sessions.len(),
            "attached to the gateway"
        );
        let mut out = vec![self.brain_info()];
        self.sessions_seen(&sessions, true, &mut out);
        self.flush(link, &mut out);

        loop {
            let deadline = self.sessions.values().filter_map(Session::deadline).min();
            tokio::select! {
                msg = link.recv() => match msg {
                    None => {
                        tracing::info!("the gateway connection closed");
                        return Ok(());
                    }
                    Some(msg) => self.handle(msg, &mut out),
                },
                () = sleep_until(deadline) => {}
            }
            self.step(&mut out);
            self.flush(link, &mut out);
        }
    }

    fn flush(&self, link: &mut impl Link, out: &mut Vec<ToGateway>) {
        for msg in out.drain(..) {
            link.send(msg);
        }
    }

    fn brain_info(&self) -> ToGateway {
        ToGateway::BrainInfo {
            build: BUILD.to_owned(),
            started_at: self.started_at.clone(),
            world_pack: self.world.info.clone(),
        }
    }

    /// The gateway's list of sessions: new ones are caught up from their history; on an
    /// attach, every one is.
    fn sessions_seen(&mut self, infos: &[SessionInfo], attach: bool, out: &mut Vec<ToGateway>) {
        let now = Instant::now();
        for info in infos {
            match self.sessions.get_mut(&info.id) {
                Some(s) if attach => s.attach(info, now, out),
                Some(s) => s.set_state(info.state),
                None => {
                    let mut s = Session::new(info, self.world.registry.clone(), now);
                    s.attach(info, now, out);
                    self.sessions.insert(info.id.clone(), s);
                }
            }
        }
    }

    fn handle(&mut self, msg: ToClient, out: &mut Vec<ToGateway>) {
        let now = Instant::now();
        match msg {
            ToClient::Output { session, seq, text } => {
                if let Some(s) = self.sessions.get_mut(&session) {
                    s.output(wayfarer_proto::Chunk { seq, text }, out);
                }
            }
            ToClient::History {
                session, chunks, ..
            } => {
                if let Some(s) = self.sessions.get_mut(&session)
                    && s.history(chunks, out)
                {
                    s.publish(out);
                }
            }
            ToClient::Lagged { session, missed } => {
                if let Some(s) = self.sessions.get_mut(&session) {
                    tracing::warn!(%session, missed, "fell behind the session's output");
                    s.lagged(out);
                }
            }
            ToClient::SessionState { session, state } => {
                if let Some(s) = self.sessions.get_mut(&session) {
                    s.set_state(state);
                }
            }
            ToClient::SessionList { sessions } => self.sessions_seen(&sessions, false, out),
            ToClient::Sent { session, text, .. } => {
                if let Some(s) = self.sessions.get_mut(&session) {
                    s.sent(&text, now);
                }
            }
            ToClient::Resync { session } => match self.sessions.get_mut(&session) {
                Some(s) => {
                    tracing::info!(%session, "re-sync asked for");
                    s.request_resync();
                }
                None => out.push(ToGateway::Warning {
                    session: None,
                    text: format!("re-sync: no session {session:?}"),
                }),
            },
            ToClient::StateRequest { session } => {
                for s in self.sessions.values_mut() {
                    if session.as_ref().is_none_or(|id| *id == s.id) {
                        s.publish(out);
                    }
                }
                out.push(self.brain_info());
            }
            ToClient::Error { message } => tracing::warn!("the gateway says: {message}"),
            other => tracing::debug!("ignored: {:?}", other.type_name()),
        }
    }

    fn step(&mut self, out: &mut Vec<ToGateway>) {
        let now = Instant::now();
        for s in self.sessions.values_mut() {
            if s.step(now, out) == Progress::Done {
                s.character.set_synced_at(rfc3339(self.clock.now()));
                s.publish(out);
                out.push(ToGateway::Notice {
                    session: Some(s.id.clone()),
                    text: "re-synced".to_owned(),
                });
                tracing::info!(session = %s.id, "re-synced");
            }
        }
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
