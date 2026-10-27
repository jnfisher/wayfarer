//! The brain's loop: attach, follow every session, answer the UIs' control messages, re-sync.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tokio::time::Instant;
use wayfarer_parse::{GameTexts, Registry};
use wayfarer_proto::{
    BotAction, CharacterState, ClientKind, PROTOCOL, RoomKind, SessionInfo, ToClient, ToGateway,
    WorldPackInfo,
};
use wayfarer_settings::{Settings, SettingsDir};
use wayfarer_store::Store;
use wayfarer_world::{Finder, Key, MmLoop, Monsters, PathKnowledge, RoomGraph, Spells, WorldPack};

use crate::clock::{Clock, rfc3339};
use crate::link::Link;
use crate::party::Parties;
use crate::rules;
use crate::session::{Progress, Session};
use crate::session_settings::SettingsSource;

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
    /// The exits MegaMud's verified paths take: the hidden exits a loop searches for.
    pub knowledge: PathKnowledge,
    /// Rooms, places and monsters by name (`find`), and the boss lairs.
    pub finder: Finder,
    /// The attack and heal spells of the player classes.
    pub spells: Spells,
    /// The scrolls shops sell, and each class's (magery, magery level).
    pub scrolls: Vec<wayfarer_world::spellshop::ScrollOffer>,
    pub class_magery: HashMap<String, (i64, i64)>,
    /// The Bank of Godfrey in Silvermere: where the spell shopping draws money, never another
    /// bank (John, 2026-10-22).
    pub godfrey: Option<Key>,
    /// Key items by lower-case name and by number: the keys a character holds, and the key a
    /// Key exit asks for.
    pub item_numbers: HashMap<String, i64>,
    pub item_names: HashMap<i64, String>,
    /// The shops where `train` takes a character up a level.
    pub trainers: Vec<wayfarer_world::trainers::Trainer>,
    /// The items Item and Key exits ask for that a shop sells: the walker buys one on the way.
    pub exit_items: std::collections::BTreeMap<i64, wayfarer_world::ExitItem>,
}

/// Room types (`kb/game-data.md` "Enumerations") and shop types (`kb/economy.md`).
/// Item type of a key.
const ITEM_KEY: i64 = 7;
const ROOM_SHOP: i64 = 1;
const ROOM_ARENA: i64 = 2;
const ROOM_LAIR: i64 = 3;
const SHOP_BANK: i64 = 7;
const SHOP_TRAINING: i64 = 8;

impl Atlas {
    pub fn load(pack: &WorldPack) -> Result<Atlas, wayfarer_world::Error> {
        let mut graph = RoomGraph::load(pack)?;
        // What MegaMud's paths know: the phrases Text exits with a blank message take, and the
        // ways through the pack has no exit for (C-052).
        let knowledge = PathKnowledge::load(pack, &graph)?;
        graph.learn_phrases(&knowledge);
        let crossings = graph.learn_crossings(&knowledge);
        // A room command's phrase must not read as one of the game's commands, or the game
        // never gets to the room's script.
        graph.choose_phrases(|p| crate::commands::classify(p).cmd.is_some());
        // Again, so the paths' steps see the crossings as the ways through they take.
        let knowledge = if crossings > 0 {
            PathKnowledge::load(pack, &graph)?
        } else {
            knowledge
        };
        // The stretches of paths that open a sealed hidden exit from elsewhere and go through.
        graph.learn_windows(&knowledge);
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
        // The keys, and the items Item exits ask for: by number, to know what is carried.
        let mut item_names: HashMap<i64, String> = pack
            .items_of_type(ITEM_KEY)?
            .into_iter()
            .map(|i| (i.number, i.name))
            .collect();
        let wanted: std::collections::BTreeSet<i64> = graph
            .all_edges()
            .iter()
            .filter_map(|e| match e.gate {
                wayfarer_world::Gate::Item { item } => Some(item),
                _ => None,
            })
            .collect();
        for &n in &wanted {
            if let Some(item) = pack.item(n)? {
                item_names.insert(n, item.name);
            }
        }
        let gate_items: std::collections::BTreeSet<i64> = graph
            .all_edges()
            .iter()
            .filter_map(|e| match e.gate {
                wayfarer_world::Gate::Item { item } | wayfarer_world::Gate::Key { item } => {
                    Some(item)
                }
                _ => None,
            })
            .collect();
        let exit_items = wayfarer_world::exit_items::load(pack, &gate_items)?;
        let monsters = Monsters::load(pack)?;
        let mm_loops = wayfarer_world::loops::megamud_loops(pack, &graph, &monsters)?;
        Ok(Atlas {
            finder: Finder::load(pack, &graph, &monsters)?,
            graph,
            kinds,
            classes: by_name(pack.classes()?),
            races: by_name(pack.races()?),
            monsters,
            mm_loops,
            knowledge,
            spells: Spells::load(pack)?,
            scrolls: wayfarer_world::spellshop::load(pack)?,
            item_numbers: item_names
                .iter()
                .map(|(n, name)| (name.to_lowercase(), *n))
                .collect(),
            item_names,
            trainers: wayfarer_world::trainers::load(pack)?,
            exit_items,
            godfrey: pack
                .rooms_by_name("Bank of Godfrey")?
                .into_iter()
                .find(|r| r.room_type == 1)
                .map(|r| (r.map, r.room)),
            class_magery: pack
                .class_magery()?
                .into_iter()
                .map(|(name, magery, level)| (name, (magery, level)))
                .collect(),
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
    /// The gear planner's data (`gear_plan_request`), with a pack.
    pub gear: Option<Arc<crate::gear_plan::Gear>>,
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
        let atlas = Arc::new(Atlas::load(&pack)?);
        let pack = Arc::new(Mutex::new(pack));
        Ok(Self {
            registry: Arc::new(Registry::new(&texts)),
            info: None,
            gear: Some(Arc::new(crate::gear_plan::Gear::new(
                pack.clone(),
                atlas.clone(),
            ))),
            pack: Some(pack),
            atlas: Some(atlas),
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
            gear: None,
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
    /// The parties the player defined, and the coordinator over them.
    parties: Parties,
    /// The lasting spells each session keeps on (C-047), by short name, and the `blessings`
    /// last sent to the UIs.
    blessings: BTreeMap<String, Vec<String>>,
    blessings_sent: Option<Vec<wayfarer_proto::Blessings>>,
    /// The bots' flee line, per cent of the maximum HP.
    flee_below: Option<i32>,
    /// Monsters every bot flees from, as the player's rules (`--flee-from`).
    flee_from: Vec<String>,
    /// The settings every session starts with when there is no settings directory.
    settings: Arc<Settings>,
    /// Where the settings files are: each session gets its own layers (`settings.toml`,
    /// `servers/<server>.toml`, `characters/<character>.toml`).
    settings_dir: Option<SettingsDir>,
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
            parties: Parties::default(),
            blessings: BTreeMap::new(),
            blessings_sent: None,
            flee_below: None,
            flee_from: Vec::new(),
            settings: Arc::new(Settings::default()),
            settings_dir: None,
        }
    }

    /// Flee below `percent` HP whatever the settings say: a test knob for the live check
    /// (`wayfarer brain --flee-below`), so the first blow sets a flee off.
    pub fn with_flee_below(mut self, percent: i32) -> Self {
        self.flee_below = Some(percent);
        self
    }

    /// The settings every session starts with, whatever its character (tests; the settings
    /// files, [`Brain::with_settings_dir`], take over when there are any).
    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = Arc::new(settings);
        self
    }

    /// Resolve each session's settings from the files in `dir` when it is first seen:
    /// `settings.toml` for all characters, `servers/<server id>.toml`, and
    /// `characters/<character id>.toml`, over the style preset. A file that is not valid
    /// leaves that session on the defaults, with a warning on its tile.
    pub fn with_settings_dir(mut self, dir: SettingsDir) -> Self {
        self.settings_dir = Some(dir);
        self
    }

    /// Every bot flees from monsters whose name holds one of `names`, as if the player had set
    /// *flee from* for each (`wayfarer brain --flee-from`, the live check's way to name one
    /// without a UI). Kept with the bot's record like any rule of the player's.
    pub fn with_flee_from(mut self, names: Vec<String>) -> Self {
        self.flee_from = names;
        self
    }

    /// Keep the bot's state and the statistics in `store`, and resume the bots it has running
    /// or paused as their sessions are first seen.
    pub fn with_store(mut self, store: Store) -> Self {
        match store.parties() {
            Ok(parties) => self.parties = Parties::from_records(parties),
            Err(e) => tracing::warn!("the store's parties could not be read: {e}"),
        }
        match store.blessings() {
            Ok(b) => self.blessings = b.into_iter().collect(),
            Err(e) => tracing::warn!("the store's blessings could not be read: {e}"),
        }
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
            let mut deadline = self.sessions.values().filter_map(Session::deadline).min();
            // The party coordinator looks every second while there are parties.
            if self.parties.has_parties() {
                let tick = self.clock.instant() + std::time::Duration::from_secs(1);
                deadline = Some(deadline.map_or(tick, |d| d.min(tick)));
            }
            let clock = self.clock.clone();
            tokio::select! {
                msg = link.recv() => match msg {
                    None => {
                        tracing::info!("the gateway connection closed");
                        return Ok(());
                    }
                    Some(msg) => self.handle(msg, &mut out),
                },
                () = sleep_until(&*clock, deadline) => {}
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
        let now = self.clock.instant();
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
                        SettingsSource::new(
                            self.settings_dir.clone(),
                            info.server.clone(),
                            self.settings.clone(),
                            self.flee_below,
                        ),
                        &self.flee_from,
                        now,
                    );
                    s.set_blessings(self.blessings.get(&info.id).cloned().unwrap_or_default());
                    s.attach(info, now, out);
                    if let Some(why) = s.take_settings_warning() {
                        tracing::warn!(session = %info.id, "{why}");
                        out.push(s.warning(&why));
                    }
                    self.sessions.insert(info.id.clone(), s);
                }
            }
        }
    }

    fn handle(&mut self, msg: ToClient, out: &mut Vec<ToGateway>) {
        let now = self.clock.instant();
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
            } => match self.sessions.get_mut(&session) {
                Some(s) => {
                    tracing::debug!(%session, %from, "user_input");
                    s.user_input(text, now, out);
                }
                None => out.push(no_session("user_input", &session)),
            },
            ToClient::KeysSent { session } => {
                if let Some(s) = self.sessions.get_mut(&session) {
                    s.keys_sent(out);
                }
            }
            ToClient::WalkTo {
                session,
                map,
                room,
                stop_short,
                fight,
            } => match self.sessions.get_mut(&session) {
                Some(s) => s.walk_to((map, room), stop_short, fight, now, out),
                None => out.push(no_session("walk_to", &session)),
            },
            ToClient::JourneyFight { session, fight } => match self.sessions.get_mut(&session) {
                Some(s) => s.journey_fight(fight, now, out),
                None => out.push(no_session("journey_fight", &session)),
            },
            ToClient::Find { session, query } => match self.sessions.get(&session) {
                Some(s) => s.find(&query, out),
                None => out.push(no_session("find", &session)),
            },
            ToClient::WalkStop { session } => match self.sessions.get_mut(&session) {
                Some(s) => s.walk_stop(now, out),
                None => out.push(no_session("walk_stop", &session)),
            },
            ToClient::BotControl { session, action } => match self.sessions.get_mut(&session) {
                // An errand that walks takes a follower out of its party first.
                Some(_)
                    if matches!(
                        action,
                        BotAction::Train | BotAction::RecoverPile | BotAction::BuySpells
                    ) && self.parties.is_follower(&session) =>
                {
                    self.party_leave(&session, out);
                    out.push(ToGateway::Notice {
                        session: Some(session.clone()),
                        text: "Left the party for the errand".to_owned(),
                    });
                    if let Some(s) = self.sessions.get_mut(&session) {
                        s.bot_control(action, now, out);
                    }
                }
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
                query,
                limit,
            } => match self.sessions.get_mut(&session) {
                Some(s) => s.loops_request(
                    anchor.map(|k| (k.map, k.room)),
                    radius,
                    doors.unwrap_or(true),
                    query.as_deref().unwrap_or_default(),
                    limit,
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
            ToClient::SaveLoop {
                session,
                name,
                base,
                stops,
            } => match self.sessions.get_mut(&session) {
                Some(s) => {
                    let stops = stops.into_iter().map(|k| (k.map, k.room)).collect();
                    s.save_loop(&name, base, stops, out);
                }
                None => out.push(no_session("save_loop", &session)),
            },
            ToClient::DeleteLoop { session, id } => match self.sessions.get_mut(&session) {
                Some(s) => s.delete_loop(&id, out),
                None => out.push(no_session("delete_loop", &session)),
            },
            ToClient::SetMonsterRule {
                session,
                pattern,
                rule,
            } => match self.sessions.get_mut(&session) {
                Some(s) => s.set_monster_rule(&pattern, rule.map(rules::from_wire), now, out),
                None => out.push(no_session("set_monster_rule", &session)),
            },
            ToClient::SpellPlanRequest { session } => match self.sessions.get_mut(&session) {
                Some(s) => s.spell_plan(out),
                None => out.push(no_session("spell_plan_request", &session)),
            },
            ToClient::GearPlanRequest(req) => match self.sessions.get_mut(&req.session) {
                Some(s) => s.gear_plan(&req, self.world.gear.as_deref(), out),
                None => out.push(no_session("gear_plan_request", &req.session)),
            },
            ToClient::GearDetailRequest(req) => match self.sessions.get_mut(&req.session) {
                Some(s) => s.gear_detail(&req, self.world.gear.as_deref(), out),
                None => out.push(no_session("gear_detail_request", &req.session)),
            },
            ToClient::StatsRequest { session } => match self.sessions.get_mut(&session) {
                Some(s) => s.stats_request(out),
                None => out.push(no_session("stats_request", &session)),
            },
            ToClient::SettingsRequest { session, layer } => match self.sessions.get_mut(&session) {
                Some(s) => s.settings_request(layer.as_deref(), out),
                None => out.push(no_session("settings_request", &session)),
            },
            ToClient::SettingsSave {
                session,
                layer,
                changes,
            } => match self.sessions.get_mut(&session) {
                Some(s) => s.settings_save(&layer, &changes, now, out),
                None => out.push(no_session("settings_save", &session)),
            },
            ToClient::StatsReset { session, only } => match self.sessions.get_mut(&session) {
                Some(s) => s.stats_reset(only, now, out),
                None => out.push(no_session("stats_reset", &session)),
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
                // On a clean exit an attacked character flees first; its session answers when
                // that is over. A session the brain does not know is hung up.
                let (ready, clean) = match self.sessions.get_mut(&session) {
                    Some(s) => (s.logging_off(now, out), s.clean_log_off()),
                    None => (true, false),
                };
                if ready {
                    out.push(ToGateway::ReadyToLeave { session, clean });
                }
            }
            ToClient::PartiesRequest => {
                self.parties.resend();
                self.parties_tick(out);
            }
            ToClient::PartySave { party } => {
                let known = self.sessions.keys().cloned().collect();
                match self.parties.save(party, &known) {
                    Ok((changed, emptied)) => self.store_parties(&changed, &emptied),
                    Err(why) => out.push(party_warning(&format!("party_save: {why}"))),
                }
                self.parties_tick(out);
            }
            ToClient::PartyDelete { id } => {
                if self.parties.delete(&id) {
                    self.store_parties(&[], &[id]);
                } else {
                    out.push(party_warning(&format!("party_delete: no party {id:?}")));
                }
                self.parties_tick(out);
            }
            ToClient::PartyLeave { session } => self.party_leave(&session, out),
            ToClient::PartyJoin { session, party } => {
                let known = self.sessions.keys().cloned().collect();
                match self.parties.join(&session, &party, &known) {
                    Ok((changed, emptied)) => self.store_parties(&changed, &emptied),
                    Err(why) => out.push(party_warning(&format!("party_join: {why}"))),
                }
                self.parties_tick(out);
            }
            ToClient::PartyResync { party } => {
                match self.parties.resync(party.as_deref()) {
                    Some(members) => {
                        for id in members {
                            if let Some(s) = self.sessions.get_mut(&id) {
                                s.party_resync();
                            }
                        }
                        let what = party.map_or_else(
                            || "the parties are".to_owned(),
                            |id| format!("party {id} is"),
                        );
                        out.push(ToGateway::Notice {
                            session: None,
                            text: format!("Re-sync: {what} forming up again"),
                        });
                    }
                    None => out.push(party_warning(&format!(
                        "party_resync: no party {:?}",
                        party.unwrap_or_default()
                    ))),
                }
                self.parties_tick(out);
            }
            ToClient::BlessingsRequest => {
                self.blessings_sent = None;
                self.blessings_tick(out);
            }
            ToClient::BlessingsSave { session, spells } => {
                let Some(s) = self.sessions.get_mut(&session) else {
                    out.push(no_session("blessings_save", &session));
                    return;
                };
                s.set_blessings(spells.clone());
                if let Some(store) = &self.store {
                    let at = crate::stats::unix(self.clock.now());
                    if let Err(e) = store
                        .lock()
                        .expect("the store's lock")
                        .save_blessings(&session, &spells, at)
                    {
                        tracing::warn!(%session, "the blessings could not be stored: {e}");
                    }
                }
                self.blessings.insert(session, spells);
                self.blessings_tick(out);
            }
            ToClient::Error { message } => tracing::warn!("the gateway says: {message}"),
            other => tracing::debug!("ignored: {:?}", other.type_name()),
        }
    }

    /// Keep the parties changed in the store and forget those deleted.
    fn store_parties(&self, changed: &[wayfarer_proto::PartyDef], deleted: &[String]) {
        let Some(store) = &self.store else { return };
        let store = store.lock().expect("the store's lock");
        let at = crate::stats::unix(self.clock.now());
        for d in changed {
            let rec = wayfarer_store::PartyRecord {
                id: d.id.clone(),
                leader: d.leader.clone(),
                followers: d.followers.clone(),
            };
            if let Err(e) = store.save_party(&rec, at) {
                tracing::warn!(party = %d.id, "the party could not be stored: {e}");
            }
        }
        for id in deleted {
            if let Err(e) = store.delete_party(id) {
                tracing::warn!(party = %id, "the party could not be deleted: {e}");
            }
        }
    }

    /// The party coordinator's tick: each session's role, the lines it plans through each
    /// session's executor, and the parties to the UIs when they changed.
    /// `party_leave`: out of its party, in the game too (`leave`).
    fn party_leave(&mut self, session: &str, out: &mut Vec<ToGateway>) {
        match self.parties.leave(session) {
            Some(Ok(changed)) => self.store_parties(&[changed], &[]),
            Some(Err(deleted)) => self.store_parties(&[], &[deleted]),
            None => out.push(party_warning(&format!(
                "party_leave: {session} is in no party"
            ))),
        }
        self.parties_tick(out);
    }

    fn parties_tick(&mut self, out: &mut Vec<ToGateway>) {
        let now = self.clock.instant();
        let infos = self
            .sessions
            .iter()
            .map(|(id, s)| (id.clone(), s.party_info()))
            .collect();
        let plan = self.parties.plan(&infos, now);
        for (id, role) in plan.roles {
            if let Some(s) = self.sessions.get_mut(&id) {
                s.set_party_role(role, out);
            }
        }
        for (id, line) in plan.lines {
            if let Some(s) = self.sessions.get_mut(&id)
                && s.party_line(&line)
            {
                tracing::info!(session = %id, %line, "party");
                self.parties.sent(&id, &line, now);
            }
        }
        for (id, why) in plan.flee {
            if let Some(s) = self.sessions.get_mut(&id) {
                tracing::info!(session = %id, %why, "party flee");
                s.party_flee(&why);
            }
        }
        for (id, note) in plan.notes {
            if let Some(s) = self.sessions.get(&id) {
                tracing::info!(session = %id, %note, "party rank");
                out.push(s.party_note(&note));
            }
        }
        for (id, why) in plan.give_up {
            if let Some(s) = self.sessions.get_mut(&id) {
                tracing::warn!(session = %id, %why, "party");
                s.party_give_up(&why, out);
            }
        }
        if let Some(parties) = plan.statuses {
            out.push(ToGateway::Parties { parties });
        }
    }

    fn step(&mut self, out: &mut Vec<ToGateway>) {
        let now = self.clock.instant();
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
        self.parties_tick(out);
        self.blessings_tick(out);
    }

    /// Every character's lasting spells to the UIs, when they changed (C-047).
    fn blessings_tick(&mut self, out: &mut Vec<ToGateway>) {
        let characters: Vec<wayfarer_proto::Blessings> = self
            .sessions
            .values()
            .map(Session::blessings_status)
            .collect();
        if self.blessings_sent.as_ref() != Some(&characters) {
            self.blessings_sent = Some(characters.clone());
            out.push(ToGateway::Blessings { characters });
        }
    }
}

fn party_warning(text: &str) -> ToGateway {
    ToGateway::Warning {
        session: None,
        text: text.to_owned(),
    }
}

fn no_session(what: &str, session: &str) -> ToGateway {
    ToGateway::Warning {
        session: None,
        text: format!("{what}: no session {session:?}"),
    }
}

async fn sleep_until(clock: &dyn Clock, deadline: Option<Instant>) {
    match deadline {
        Some(at) => clock.sleep_until(at).await,
        None => std::future::pending().await,
    }
}
