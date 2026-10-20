//! The brain's loop: attach, follow every session, answer the UIs' control messages, re-sync.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tokio::time::Instant;
use wayfarer_parse::{GameTexts, Registry};
use wayfarer_proto::{
    CharacterState, ClientKind, PROTOCOL, RoomKind, SessionInfo, ToClient, ToGateway, WorldPackInfo,
};
use wayfarer_store::Store;
use wayfarer_world::{Key, MmLoop, Monsters, RoomGraph, WorldPack};

use crate::clock::{Clock, rfc3339};
use crate::link::Link;
use crate::policy::FLEE_BELOW;
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

/// The world pack's rooms in memory, loaded once per brain and shared by every session: the
/// room graph for the position, the walker and the map, and what the map marks.
pub struct Atlas {
    pub graph: RoomGraph,
    /// Shops, trainers, banks, arenas and lairs.
    pub kinds: HashMap<Key, RoomKind>,
    /// Class and race numbers by name, for the gated exits.
    pub classes: HashMap<String, i64>,
    pub races: HashMap<String, i64>,
    /// Every monster, for the bot: who is worth fighting, what a room spawns.
    pub monsters: Monsters,
    /// MegaMud's verified loops, with the rooms each walk passed (none when the pack was built
    /// without MegaMud's paths).
    pub mm_loops: Vec<MmLoop>,
}

/// Room types (`kb/game-data.md` "Enumerations") and shop types (`kb/economy.md`).
const ROOM_SHOP: i64 = 1;
const ROOM_ARENA: i64 = 2;
const ROOM_LAIR: i64 = 3;
const SHOP_BANK: i64 = 7;
const SHOP_TRAINING: i64 = 8;

impl Atlas {
    pub fn load(pack: &WorldPack) -> Result<Atlas, wayfarer_world::Error> {
        let graph = RoomGraph::load(pack)?;
        let shops: HashMap<i64, i64> = pack.shop_types()?.into_iter().collect();
        let mut kinds = HashMap::new();
        for n in graph.nodes() {
            let r = &n.room;
            let kind = match r.room_type {
                ROOM_SHOP => Some(match shops.get(&r.shop_num) {
                    Some(&SHOP_BANK) => RoomKind::Bank,
                    Some(&SHOP_TRAINING) => RoomKind::Trainer,
                    _ => RoomKind::Shop,
                }),
                ROOM_ARENA => Some(RoomKind::Arena),
                ROOM_LAIR => Some(RoomKind::Lair),
                _ => None,
            };
            if let Some(kind) = kind {
                kinds.insert(n.key(), kind);
            }
        }
        // MegaMud's waypoints name places the room data does not mark.
        for w in pack.waypoints()? {
            let kind = if w.is_trainer {
                RoomKind::Trainer
            } else if w.is_bank {
                RoomKind::Bank
            } else if w.is_shop {
                RoomKind::Shop
            } else {
                continue;
            };
            for &k in &w.rooms {
                kinds.entry(k).or_insert(kind);
            }
        }
        let by_name = |v: Vec<(i64, String)>| v.into_iter().map(|(n, s)| (s, n)).collect();
        Ok(Atlas {
            graph,
            kinds,
            classes: by_name(pack.classes()?),
            races: by_name(pack.races()?),
            monsters: Monsters::load(pack)?,
            mm_loops: wayfarer_world::loops::megamud_loops(pack)?,
        })
    }
}

/// What the brain parses with: the game texts' pattern registry, and which world pack it came
/// from; with a pack, the pack itself (kept open) and its [`Atlas`].
#[derive(Clone)]
pub struct World {
    pub registry: Arc<Registry>,
    pub info: Option<WorldPackInfo>,
    pub pack: Option<Arc<Mutex<WorldPack>>>,
    pub atlas: Option<Arc<Atlas>>,
}

impl World {
    /// Load a world pack (`wayfarer import-world` builds it).
    pub fn open(path: &Path) -> Result<Self, BrainError> {
        let err = |source| BrainError::World {
            path: path.display().to_string(),
            source,
        };
        let pack = WorldPack::open(path).map_err(err)?;
        let hash = file_sha256(path).map_err(|source| {
            err(wayfarer_world::Error::Io {
                path: path.to_path_buf(),
                source,
            })
        })?;
        let mut world = Self::from_pack(pack).map_err(err)?;
        world.info = Some(WorldPackInfo {
            path: path.display().to_string(),
            hash: format!("sha256:{hash}"),
        });
        Ok(world)
    }

    /// An open pack: its game texts and its atlas (no `info`).
    pub fn from_pack(pack: WorldPack) -> Result<Self, wayfarer_world::Error> {
        let texts = pack.game_texts()?;
        let atlas = Atlas::load(&pack)?;
        Ok(Self {
            registry: Arc::new(Registry::new(&texts)),
            info: None,
            pack: Some(Arc::new(Mutex::new(pack))),
            atlas: Some(Arc::new(atlas)),
        })
    }

    /// Game texts from elsewhere (the stock-world export, in tests).
    pub fn from_texts(texts: &GameTexts) -> Self {
        Self::from_registry(Arc::new(Registry::new(texts)))
    }

    /// Parsing only: no rooms, so no position, walking or map.
    pub fn from_registry(registry: Arc<Registry>) -> Self {
        Self {
            registry,
            info: None,
            pack: None,
            atlas: None,
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
/// gateway that went away and came back). With a store ([`Brain::with_store`]) the bot's state
/// and the runs' statistics are kept on disk, and a restarted brain resumes each bot.
pub struct Brain {
    world: World,
    clock: Arc<dyn Clock>,
    token: Option<String>,
    started_at: String,
    sessions: BTreeMap<String, Session>,
    attaches: u64,
    store: Option<Arc<Mutex<Store>>>,
    /// The bots' flee line, per cent of the maximum HP.
    flee_below: i32,
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
            store: None,
            flee_below: FLEE_BELOW,
        }
    }

    /// Flee below `percent` HP instead of [`FLEE_BELOW`]: a test knob for the live check
    /// (`wayfarer brain --flee-below`), so the first blow sets a flee off. The settings give
    /// the player the line in M4.
    pub fn with_flee_below(mut self, percent: i32) -> Self {
        self.flee_below = percent;
        self
    }

    /// Keep the bot's state and the statistics in `store`, and resume the bots it has running
    /// or paused as their sessions are first seen.
    pub fn with_store(mut self, store: Store) -> Self {
        self.store = Some(Arc::new(Mutex::new(store)));
        self
    }

    /// The character the brain has for `session`, if it knows the session.
    pub fn snapshot(&self, session: &str) -> Option<CharacterState> {
        self.sessions.get(session).map(Session::snapshot)
    }

    /// How many of the brain's own lines to `session` still wait for the gateway's `sent`.
    pub fn own_pending(&self, session: &str) -> Option<usize> {
        self.sessions.get(session).map(Session::own_pending)
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
                Some(s) => s.set_state(info.state, out),
                None => {
                    let mut s = Session::new(
                        info,
                        self.world.registry.clone(),
                        self.world.atlas.clone(),
                        self.store.clone(),
                        self.clock.clone(),
                        self.flee_below,
                        now,
                    );
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
                    s.resume(out);
                    s.publish(out);
                    s.send_map(out);
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
                    s.set_state(state, out);
                }
            }
            ToClient::SessionList { sessions } => self.sessions_seen(&sessions, false, out),
            ToClient::Sent { session, text, .. } => {
                if let Some(s) = self.sessions.get_mut(&session) {
                    s.sent(&text, now, out);
                }
            }
            ToClient::UserInput {
                session,
                text,
                from,
                force,
            } => match self.sessions.get_mut(&session) {
                Some(s) => {
                    tracing::debug!(%session, %from, "user_input");
                    s.user_input(text, force, now, out);
                }
                None => out.push(no_session("user_input", &session)),
            },
            ToClient::KeysSent { session } => {
                if let Some(s) = self.sessions.get_mut(&session) {
                    s.keys_sent(out);
                }
            }
            ToClient::WalkTo { session, map, room } => match self.sessions.get_mut(&session) {
                Some(s) => s.walk_to((map, room), now, out),
                None => out.push(no_session("walk_to", &session)),
            },
            ToClient::WalkStop { session } => match self.sessions.get_mut(&session) {
                Some(s) => s.walk_stop(out),
                None => out.push(no_session("walk_stop", &session)),
            },
            ToClient::BotControl { session, action } => match self.sessions.get_mut(&session) {
                Some(s) => s.bot_control(action, now, out),
                None => out.push(no_session("bot_control", &session)),
            },
            ToClient::MapRequest { session, radius } => match self.sessions.get_mut(&session) {
                Some(s) => s.map_request(radius, out),
                None => out.push(no_session("map_request", &session)),
            },
            ToClient::LoopsRequest {
                session,
                anchor,
                radius,
                doors,
            } => match self.sessions.get_mut(&session) {
                Some(s) => s.loops_request(
                    anchor.map(|k| (k.map, k.room)),
                    radius,
                    doors.unwrap_or(true),
                    out,
                ),
                None => out.push(no_session("loops_request", &session)),
            },
            ToClient::WalkLoop { session, id } => match self.sessions.get_mut(&session) {
                Some(s) => s.walk_loop(&id, now, out),
                None => out.push(no_session("walk_loop", &session)),
            },
            ToClient::RunLoop { session, id, skip } => match self.sessions.get_mut(&session) {
                Some(s) => {
                    let skip = skip.map(|v| v.into_iter().map(|k| (k.map, k.room)).collect());
                    s.run_loop(&id, skip, now, out);
                }
                None => out.push(no_session("run_loop", &session)),
            },
            ToClient::StatsRequest { session } => match self.sessions.get(&session) {
                Some(s) => s.stats_request(out),
                None => out.push(no_session("stats_request", &session)),
            },
            ToClient::Resync { session } => match self.sessions.get_mut(&session) {
                Some(s) => {
                    tracing::info!(%session, "re-sync asked for");
                    s.request_resync();
                }
                None => out.push(no_session("re-sync", &session)),
            },
            ToClient::StateRequest { session } => {
                for s in self.sessions.values_mut() {
                    if session.as_ref().is_none_or(|id| *id == s.id) {
                        s.publish(out);
                        s.send_map(out);
                    }
                }
                out.push(self.brain_info());
            }
            ToClient::LoggingOff { session } => {
                tracing::info!(%session, "the gateway is logging the character off");
                // An attacked character flees first; its session answers when that is over.
                let ready = match self.sessions.get_mut(&session) {
                    Some(s) => s.logging_off(now, out),
                    None => true,
                };
                if ready {
                    out.push(ToGateway::ReadyToLeave { session });
                }
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
            // After every message and timer: the bot's state, if it changed, to the store, and
            // whether the brain arbitrates the session's input, to the gateway.
            s.persist(out);
            s.arbitrate(out);
        }
    }
}

fn no_session(what: &str, session: &str) -> ToGateway {
    ToGateway::Warning {
        session: None,
        text: format!("{what}: no session {session:?}"),
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
