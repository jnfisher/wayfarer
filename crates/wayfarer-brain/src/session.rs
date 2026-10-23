//! One session as the brain follows it: its parser, its character, the catch-up on its output,
//! the re-sync, and with a world pack its position, the executor, the walker, the loops and
//! the bot. The re-sync's commands and the walker's and the bot's actions are all it sends to
//! the game.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use std::sync::Mutex;
use tokio::time::Instant;

use wayfarer_parse::typed::GameEvent;
use wayfarer_parse::{Event, Parser, Registry};
use wayfarer_proto::{
    BotAction, CharacterState, Chunk, Found, FoundKind, FoundRoom, GeneratedLoop, LoopStop,
    MegaMudLoop, Place, RoomKey, SessionAction, SessionId, SessionInfo, SessionState, ToGateway,
};
use wayfarer_settings::{Layer, Settings};
use wayfarer_store::{BotRecord, BotState, CustomLoop, EventKind, PlaceKind as StoredPlace, Store};
use wayfarer_world::loops::{Area, circuits, hunt_rooms, megamud_through, stop_room};
use wayfarer_world::{
    self as world, HuntRoom, Key, MmLoop, PlaceKind, Reach, Traveller, distances, route,
};

use crate::boss::boss_status;
use crate::brain::Atlas;
use crate::character::{Character, merge_patch};
use crate::clock::Clock;
use crate::commands::{Kind, classify};
use crate::custom;
use crate::errand::{Active, Ctx, Errand, ErrandKind, ErrandNext};
use crate::executor::{Action, Executor, Expect};
use crate::journey::Journey;
use crate::modes::{Bot, Do, Presence, View};
use crate::party::{Health, MemberInfo, PartyRole, WalkBack};
use crate::policy::{COMBAT_QUIET, hp_percent};
use crate::position::Position;
use crate::rules::{self, RuleBook, Sightings};
use crate::session_settings::SettingsSource;
use crate::spellcraft::{SpellRow, Spellbook};
use crate::stats::{Ledger, rfc3339_at, unix};
use crate::task::{Task, TaskLoop, why_unreachable};
use crate::walker::{MAX_SEARCHES, Next, Say, Walker};

/// The rooms a `map_view` holds unless a `map_request` asks for another radius: those within
/// this many steps.
pub const MAP_RADIUS: u32 = 15;

/// The re-sync's commands, in order: the "talk and look" kind (docs/plan/03-engine.md
/// section 6). `look`, not an empty line: an empty line shows the room only in statline FULL
/// modes (`kb/screen-parsing.md`).
pub const RESYNC_COMMANDS: [&str; 5] = ["st", "exp", "i", "look", "spells"];
/// How long a follower that found no way back to its leader waits before it tries again.
pub const STUCK_RETRY: Duration = Duration::from_secs(30);
/// The commands before `spells`, which only a character that casts sends.
const RESYNC_BASE: usize = 4;
/// How often the settings files are looked at for a change.
pub const SETTINGS_POLL: Duration = Duration::from_secs(3);
/// How long nothing must have been sent to the game for a prompt to count as idle.
pub const IDLE: Duration = Duration::from_secs(2);
/// How long a re-sync command waits for its answer before the next one goes.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a line the brain sent waits for the gateway's `sent`. The gateway sends it before
/// the line's echo, so it is normally back within a round trip; one still missing after this
/// was lost (a lagged connection) and is forgotten, so a later line the player types is not
/// taken for the brain's.
pub const OWN_PATIENCE: Duration = Duration::from_secs(10);
/// How long a line the player typed beside the bot waits for a prompt with nothing in flight
/// before it goes anyway (the game showed text and no prompt after it).
pub const TYPED_PATIENCE: Duration = Duration::from_secs(5);
/// Chunks asked for per `history_request`.
pub const HISTORY_CHUNKS: u32 = 2000;
/// The widest area a `loops_request` may ask for, in steps from the anchor.
pub const MAX_LOOP_RADIUS: u32 = 30;

/// What the generated loops of an area depend on: the anchor, the radius, the character's
/// level (the gated exits), strength and picklocks (the locked doors it can open) and whether
/// doors are taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LoopsKey {
    anchor: Key,
    radius: u32,
    level: Option<i64>,
    strength: Option<i64>,
    picklocks: Option<i64>,
    doors: bool,
}

/// A generated loop's `id`: `map:room:radius:doors|nodoors:index`, so a restarted brain can
/// make the same loop again.
fn loop_id(k: &LoopsKey, index: usize) -> String {
    let doors = if k.doors { "doors" } else { "nodoors" };
    format!("{}:{}:{}:{doors}:{index}", k.anchor.0, k.anchor.1, k.radius)
}

/// An `id` back to its area (for the traveller `t`) and the loop's index in it.
fn parse_loop_id(id: &str, t: &Traveller) -> Option<(LoopsKey, usize)> {
    let mut it = id.split(':');
    let map = it.next()?.parse().ok()?;
    let room = it.next()?.parse().ok()?;
    let radius: u32 = it.next()?.parse().ok()?;
    let doors = match it.next()? {
        "doors" => true,
        "nodoors" => false,
        _ => return None,
    };
    let index = it.next()?.parse().ok()?;
    if it.next().is_some() || radius > MAX_LOOP_RADIUS {
        return None;
    }
    let key = LoopsKey {
        anchor: (map, room),
        radius,
        level: t.level,
        strength: t.strength,
        picklocks: t.picklocks,
        doors,
    };
    Some((key, index))
}

/// A hunt room as a loop's stop on the wire.
/// How many loops a `loops_request` sends unless it asks for another number, and the most.
const LOOPS_SHOWN: usize = 12;
const MOST_LOOPS_SHOWN: usize = 50;

/// Whether every word of `query` (any case) is in one of `texts`; an empty query matches.
fn loop_matches<'a>(query: &str, texts: impl IntoIterator<Item = &'a str>) -> bool {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return true;
    }
    let hay = texts
        .into_iter()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("\n");
    words.iter().all(|w| hay.contains(w.as_str()))
}

fn loop_stop(h: &HuntRoom) -> LoopStop {
    LoopStop {
        map: h.key.0,
        room: h.key.1,
        name: h.name.clone(),
        monsters: h.monsters.iter().map(|(_, n)| n.clone()).collect(),
        arena: h.is_arena,
        lair: h.is_lair,
        regen: h.regen,
        bosses: Vec::new(),
    }
}

/// The MegaMud loop an id names: its file name, in any case.
fn mm_loop<'a>(atlas: &'a Atlas, id: &str) -> Option<&'a MmLoop> {
    atlas
        .mm_loops
        .iter()
        .find(|l| l.path.file.eq_ignore_ascii_case(id))
}

/// A MegaMud loop as `find_loop` gives a loop: its file the id, its hunt rooms the stops.
fn mm_as_loop(l: &MmLoop) -> GeneratedLoop {
    GeneratedLoop {
        id: l.path.file.clone(),
        name: l.name().to_owned(),
        stops: l.hunt.iter().map(loop_stop).collect(),
        path: l.rooms.iter().copied().map(room_key).collect(),
        steps: u32::try_from(l.path.verification.steps_checked).unwrap_or(0),
        doors: 0,
        distance: None,
    }
}

fn room_key(k: Key) -> RoomKey {
    RoomKey {
        map: k.0,
        room: k.1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resync {
    Off,
    /// The command `next` goes at the next prompt; with `idle`, at the next idle prompt.
    Want {
        next: usize,
        idle: bool,
    },
    /// Command `step` went at `sent`; its answer has been seen or not. `typed`: the player
    /// sent a line since, so the next command waits for an idle prompt.
    Waiting {
        step: usize,
        deadline: Instant,
        answered: bool,
        typed: bool,
    },
}

/// A line the player typed beside the bot, waiting for its moment.
#[derive(Debug)]
struct TypedLine {
    text: String,
    /// When it came.
    at: Instant,
}

/// How soon an errand in progress is asked again with nothing happening.
const ERRAND_POLL: Duration = Duration::from_millis(500);

/// Whose action is in the executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Walker,
    Bot,
    Errand,
    /// The party coordinator's `invite`, `follow`, `par` or `leave`.
    Party,
}

/// What a re-sync step did, for the brain to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Progress {
    None,
    Done,
}

/// Whether `ev` answers re-sync command `step`.
fn answers(step: usize, ev: &GameEvent) -> bool {
    use GameEvent as G;
    match step {
        0 => matches!(
            ev,
            G::StatusName { .. }
                | G::StatusRace { .. }
                | G::StatusClass { .. }
                | G::StatusHits { .. }
                | G::StatusMana { .. }
                | G::StatusStat { .. }
        ),
        1 => matches!(ev, G::ExpScreen { .. }),
        4 => matches!(ev, G::SpellsHeader | G::SpellRow { .. } | G::SpellsNone),
        2 => matches!(
            ev,
            G::Inventory { .. } | G::Wealth { .. } | G::Encumbrance { .. }
        ),
        _ => matches!(ev, G::RoomTitle { .. } | G::RoomExits { .. } | G::RoomDark),
    }
}

/// Whether `text` shows anything but escapes and white space.
fn visible(text: &str) -> bool {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
        } else if !c.is_whitespace() {
            return true;
        }
    }
    false
}

pub(crate) struct Session {
    pub id: SessionId,
    pub state: SessionState,
    /// The settings in force for this character (`Brain::with_settings_dir`).
    settings: Arc<Settings>,
    /// Where they come from, to resolve them again when the class is known.
    settings_source: SettingsSource,
    /// The warning for settings files that could not be used, until the brain has sent it.
    settings_warning: Option<String>,
    /// The class the settings were last resolved for.
    class_seen: Option<String>,
    /// The settings files as last seen (`SettingsSource::stamp`), and when to look next.
    settings_stamp: Vec<Option<(std::time::SystemTime, u64)>>,
    settings_check: Option<Instant>,
    /// The layer whose own values the forms last asked for: a `settings` message follows a
    /// reload with it.
    settings_layer: Layer,
    /// The rows of the last `spells` screen, and the attacks and heals among them.
    spell_rows: Vec<SpellRow>,
    /// The lasting spells the player ticked for the character, by short name (C-047).
    blessings: Vec<String>,
    /// A `spells` screen has been seen, so empty rows mean no spells rather than not known.
    spells_seen: bool,
    spellbook: Spellbook,
    registry: Arc<Registry>,
    parser: Parser,
    pub character: Character,
    /// The last snapshot the UIs were sent in this attach, as JSON; `None` before the whole
    /// snapshot that ends the catch-up.
    published: Option<serde_json::Value>,
    /// The last output `seq` fed to the parser.
    fed: u64,
    /// Chunks up to this `seq` predate the brain: fed without `event` messages.
    silent_until: u64,
    /// A `history_request` is out; live output waits here meanwhile.
    fetching: bool,
    buffer: Vec<Chunk>,
    /// The last chunk ended in a prompt.
    at_prompt: bool,
    /// When a line last went to the game (or the brain attached).
    last_sent: Instant,
    /// The brain's own lines and when each went, until the gateway's `sent` for each comes
    /// back or [`OWN_PATIENCE`] runs out.
    own: VecDeque<(String, Instant)>,
    pub resync: Resync,
    /// A re-sync finished since the brain attached, and the session stayed in the realm.
    pub synced: bool,
    /// The world pack's rooms: without them there is no position, walking or map.
    atlas: Option<Arc<Atlas>>,
    position: Position,
    executor: Executor,
    walker: Walker,
    /// How far a `map_view` reaches.
    map_radius: u32,
    /// The center of the last `map_view` sent in this attach.
    map_center: Option<Key>,
    /// The route changed since the last `map_view`.
    map_dirty: bool,
    /// Whose action the executor holds.
    owner: Option<Owner>,
    bot: Bot,
    /// Who is in the room, for the bot.
    presence: Presence,
    /// The character's party as the game last showed it.
    party_game: crate::party_game::GameParty,
    /// What the party coordinator makes of it.
    party_role: PartyRole,
    /// The leader's room it could not walk to, why, and when (tried again after
    /// [`STUCK_RETRY`] or when the leader moves).
    party_stuck: Option<(Key, String, Instant)>,
    /// The walk in progress is the bot's.
    bot_walk: bool,
    /// The session stopped the walk in progress (`stop_walk`): its end is no give-up.
    walk_halted: bool,
    /// A walk asked for while the bot's action was in flight: it starts once that is answered.
    walk_after: Option<(Key, bool)>,
    /// The generated loops of the areas asked for, made once each.
    loops: HashMap<LoopsKey, Arc<Vec<GeneratedLoop>>>,
    /// When the character's own auto-combat last showed (`*Combat Engaged*`, its blows), or
    /// the brain attached, or it got up after being knocked down: for how long it has been
    /// quiet, since auto-combat can end without a word (`kb/pitfalls.md` 42).
    last_combat: Instant,
    /// Knocked down (`smashed_you`) and not up yet (`rises`): no blows, but not quiet either.
    down: bool,
    /// The notice that the character is mortally wounded has been given, until it is not.
    wounded_noted: bool,
    /// The errand the player started (pick up all, equip all), with the bot stopped or paused.
    errand: Option<Active>,
    /// The connection was dropped at the hang-up line and the realm has not been left yet.
    hung_up: bool,
    /// The gateway is logging the character off (`logging_off`): nothing more is sent.
    leaving: bool,
    /// The gateway is logging the character off, and it was attacked: the bot flees first,
    /// then the brain answers `ready_to_leave`.
    flee_out: bool,
    /// Lines the player typed beside the bot (`user_input`): each goes at a prompt with
    /// nothing in flight, between the bot's actions.
    typed: VecDeque<TypedLine>,
    /// What the gateway was last told by `arbitrate`.
    arbitrating: bool,
    /// The bot's state and the run, in the store.
    ledger: Ledger,
    /// The brain's clock: every timer's `now`.
    clock: Arc<dyn Clock>,
    /// The bot as a brain before this one left it, running or paused: put back once the
    /// catch-up is done.
    restore: Option<BotRecord>,
    /// The monster rules: the defaults, the session's and the player's (kept with the bot).
    rules: RuleBook,
    /// Where the monsters to avoid were last seen: the router keeps out of those rooms.
    sightings: Sightings,
}

/// The events of the character's own auto-combat: its engaging and its blows, hits, misses,
/// glances and dodged swings, and its spells (a cast or a failed one). A monster's blows are not among them: a monster attacks a
/// character that is not fighting back (`kb/combat.md`, "engage at once").
const OWN_COMBAT: [&str; 10] = [
    "combat_engaged",
    "you_hit",
    "you_miss",
    "you_glance",
    "you_dodged",
    "pvp_dodges",
    "smashed_it",
    // A fight fought with a spell has these where a weapon's has blows: without them the fight
    // would lapse after `COMBAT_QUIET` and the bot would attack again, restarting it.
    "you_cast_generic",
    "spell_cast_you",
    "cast_fail",
];

impl Session {
    /// A session the brain has just learned of: its history up to now is fed silently. A bot
    /// the store has running or paused for it is resumed once that is done. Its monster rules
    /// are the store's, with *flee from* for each of `flee_from` (`--flee-from`) as the
    /// player's.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        info: &SessionInfo,
        registry: Arc<Registry>,
        atlas: Option<Arc<Atlas>>,
        store: Option<Arc<Mutex<Store>>>,
        clock: Arc<dyn Clock>,
        settings: SettingsSource,
        flee_from: &[String],
        now: Instant,
    ) -> Self {
        let (resolved, settings_warning) = settings.resolve(&info.id, None);
        let flee_below = settings.flee_below(&resolved);
        let ledger = Ledger::new(store, clock.clone(), &info.id);
        let restore = ledger
            .stored()
            .filter(|b| b.state != BotState::Stopped)
            .cloned();
        let mut rules = RuleBook::new(ledger.stored().map_or(&[], |b| &b.monster_rules));
        for name in flee_from {
            rules.set(name, Some(world::MonsterRule::FleeFrom), unix(clock.now()));
        }
        Self {
            rules,
            sightings: Sightings::default(),
            ledger,
            clock,
            restore,
            atlas,
            position: Position::default(),
            executor: Executor::default(),
            walker: Walker::with_search_limit(
                u32::try_from(resolved.int("movement.search_limit")).unwrap_or(MAX_SEARCHES),
            ),
            settings: resolved,
            settings_source: settings,
            settings_warning,
            class_seen: None,
            settings_stamp: Vec::new(),
            settings_check: None,
            settings_layer: Layer::Character,
            spell_rows: Vec::new(),
            blessings: Vec::new(),
            spells_seen: false,
            spellbook: Spellbook::default(),
            map_radius: MAP_RADIUS,
            map_center: None,
            map_dirty: false,
            owner: None,
            bot: Bot::new(flee_below),
            presence: Presence::default(),
            party_game: Default::default(),
            party_role: PartyRole::None,
            party_stuck: None,
            bot_walk: false,
            walk_halted: false,
            walk_after: None,
            loops: HashMap::new(),
            last_combat: now,
            down: false,
            wounded_noted: false,
            errand: None,
            hung_up: false,
            leaving: false,
            flee_out: false,
            typed: VecDeque::new(),
            arbitrating: false,
            id: info.id.clone(),
            state: info.state,
            parser: Parser::new(registry.clone()),
            registry,
            character: Character::new(),
            published: None,
            fed: 0,
            silent_until: info.last_seq,
            fetching: false,
            buffer: Vec::new(),
            at_prompt: false,
            last_sent: now,
            own: VecDeque::new(),
            resync: Resync::Off,
            synced: false,
        }
    }

    /// The brain (re)attached: catch up on the output from where it left off, then send a
    /// whole snapshot. `info` is the gateway's view of the session now.
    pub fn attach(&mut self, info: &SessionInfo, now: Instant, out: &mut Vec<ToGateway>) {
        self.published = None;
        self.map_center = None;
        self.own.clear();
        self.last_sent = now;
        self.buffer.clear();
        self.last_combat = now;
        self.down = false;
        self.leaving = false;
        self.flee_out = false;
        // A new connection: the gateway arbitrates nothing for it yet. Lines typed for the
        // old one went with it.
        self.arbitrating = false;
        self.typed.clear();
        // An answer to the action in flight may have gone by unseen.
        self.cancel(out);
        let next = self.walker.abandon("the brain re-attached");
        self.apply(next, out);
        if info.last_seq < self.fed {
            // The gateway's log started again: what was fed is not this log's.
            tracing::warn!(session = %self.id, "the output log restarted; parsing afresh");
            self.parser = Parser::new(self.registry.clone());
            self.character.parser_reset();
            self.fed = 0;
            // The new log's output so far went by before this attach, like a new session's.
            self.silent_until = info.last_seq;
            self.synced = false;
        }
        if let Resync::Waiting { step, .. } = self.resync {
            // The command's answer may have gone by; ask again.
            self.resync = Resync::Want {
                next: step,
                idle: true,
            };
        }
        self.set_state(info.state, out);
        self.fetch(out);
    }

    fn fetch(&mut self, out: &mut Vec<ToGateway>) {
        self.fetching = true;
        out.push(ToGateway::HistoryRequest {
            session: self.id.clone(),
            after_seq: (self.fed > 0).then_some(self.fed),
            before_seq: None,
            max_chunks: HISTORY_CHUNKS,
        });
    }

    pub fn set_state(&mut self, state: SessionState, out: &mut Vec<ToGateway>) {
        let was = self.state;
        self.state = state;
        if state != SessionState::Realm {
            // Out of the realm the player's lines go as they would without the bot, unless
            // the gateway is logging off: then into the log-off menus they would go.
            if self.leaving || self.flee_out {
                self.drop_typed(out);
            } else {
                for line in std::mem::take(&mut self.typed) {
                    self.send_straight(line.text, out);
                }
            }
            self.synced = false;
            self.leaving = false;
            self.down = false;
            // Nothing in flight will be answered now.
            self.cancel(out);
            let says = self.bot.stop("the session left the realm", true);
            if !says.is_empty() {
                self.say(says, out);
                self.publish_patch(out);
            }
            if self.walker.walking() {
                let next = self.walker.abandon("the session left the realm");
                self.apply(next, out);
                self.flush_views(out);
            }
            if self.resync != Resync::Off {
                // Cut short: start again from `st` in the realm.
                self.request_resync();
            }
            self.flee_out_done(out);
        } else if was != SessionState::Realm {
            self.synced = false;
        }
        if state == SessionState::Realm && !self.synced && self.resync == Resync::Off {
            self.request_resync();
        }
    }

    /// Start a re-sync from the first command (at the next idle prompt in the realm).
    pub fn request_resync(&mut self) {
        self.resync = Resync::Want {
            next: 0,
            idle: true,
        };
    }

    /// Live output.
    pub fn output(&mut self, chunk: Chunk, out: &mut Vec<ToGateway>) -> bool {
        if self.fetching {
            self.buffer.push(chunk);
            return false;
        }
        if chunk.seq <= self.fed {
            return false;
        }
        if chunk.seq > self.fed + 1 {
            self.buffer.push(chunk);
            self.fetch(out);
            return false;
        }
        self.feed(chunk, out);
        true
    }

    /// The connection lagged: fetch what it missed.
    pub fn lagged(&mut self, out: &mut Vec<ToGateway>) {
        // The `sent` for a line of ours may have been among what was missed.
        self.own.clear();
        if !self.fetching {
            self.fetch(out);
        }
    }

    /// The answer to a `history_request`. Whether the catch-up finished (the caller then sends
    /// the whole snapshot).
    pub fn history(&mut self, chunks: Vec<Chunk>, out: &mut Vec<ToGateway>) -> bool {
        let full = chunks.len() >= HISTORY_CHUNKS as usize;
        if let Some(first) = chunks.first()
            && self.fed > 0
            && first.seq > self.fed + 1
        {
            tracing::warn!(session = %self.id, "output went by unseen; re-syncing");
            self.synced = false;
            if self.resync == Resync::Off && self.state == SessionState::Realm {
                self.request_resync();
            }
        }
        for c in chunks {
            if c.seq > self.fed {
                self.feed(c, out);
            }
        }
        if full {
            self.fetch(out);
            return false;
        }
        let mut buffer = std::mem::take(&mut self.buffer);
        buffer.sort_by_key(|c| c.seq);
        let mut rest = buffer.into_iter();
        while let Some(c) = rest.next() {
            if c.seq <= self.fed {
                continue;
            }
            if c.seq > self.fed + 1 {
                self.buffer.push(c);
                self.buffer.extend(rest);
                self.fetch(out);
                return false;
            }
            self.feed(c, out);
        }
        let was_fetching = self.fetching;
        self.fetching = false;
        was_fetching && self.published.is_none()
    }

    fn feed(&mut self, chunk: Chunk, out: &mut Vec<ToGateway>) {
        let silent = chunk.seq <= self.silent_until;
        let events = self.parser.feed_at(chunk.seq, &chunk.text);
        let now = self.clock.instant();
        let mut changed = false;
        let mut combat = false;
        for ev in &events {
            let level = self.character.state().level;
            let applied = self.character.apply_event(ev);
            changed |= applied;
            if applied && self.atlas.is_some() {
                // Every HP value, not only those the bot sees at its ticks: a walk or an
                // action in flight does not hold Flee back.
                let c = self.character.state();
                // A follower with its leader runs when the leader does (the coordinator).
                if !self.party_role.with_leader() && self.bot.hp(hp_percent(c.hp, c.hp_max)) {
                    self.give_way(out);
                }
            }
            let typed = GameEvent::from_event(ev);
            self.spell_event(&typed);
            if let Resync::Waiting {
                step,
                ref mut answered,
                ..
            } = self.resync
                && answers(step, &typed)
            {
                *answered = true;
            }
            if let Some(atlas) = &self.atlas
                && self.position.event(atlas, &typed, now).is_some()
            {
                self.executor
                    .room_seen_in(Some(self.position.belief.clone()));
            }
            self.executor.event(&typed);
            combat |= typed == GameEvent::CombatEngaged;
            if OWN_COMBAT.contains(&ev.name.as_str()) {
                self.last_combat = now;
            }
            match ev.name.as_str() {
                "smashed_you" => self.down = true,
                // `Slightly dazed you get back onto your feet.`: the quiet clock starts again.
                "rises" if ev.text("who").is_none_or(str::is_empty) => {
                    self.down = false;
                    self.last_combat = now;
                }
                _ => {}
            }
            if let Some(who) = self.pvp_attacker(ev) {
                self.presence.pvp(&who);
            }
            // Who died, before the presence forgets them.
            let dead = match (&typed, &self.atlas) {
                (GameEvent::MonsterDeath { monsters }, Some(atlas)) => self
                    .presence
                    .dead(atlas, monsters)
                    .or_else(|| monsters.first().cloned()),
                _ => None,
            };
            if !silent && self.atlas.is_some() {
                self.count(&typed, dead.as_deref(), level);
                // A boss kill is kept whether or not the bot is running: "should be up".
                if let (Some(name), Some(atlas)) = (&dead, &self.atlas)
                    && atlas.monsters.boss_named(name)
                {
                    let name = name.clone();
                    self.ledger.boss_killed(&name);
                }
            }
            if let Some(name) = &dead {
                self.bot.killed(name, now);
            }
            let me = self.character.state().name.clone();
            self.party_game.event(&typed, me.as_deref(), now);
            if let Some(atlas) = self.atlas.clone() {
                self.presence.event(&atlas, &typed);
                self.sighted(&typed, now, out);
                if let Some(errand) = &mut self.errand {
                    errand.on_event(&typed);
                }
                self.walker.heard(&typed);
                // The items carried: the Item exits they open may be routed through.
                if let GameEvent::Inventory { items, .. } = &typed {
                    self.position.traveller.items = items
                        .iter()
                        .filter_map(|i| atlas.item_numbers.get(&i.name.to_lowercase()).copied())
                        .collect();
                }
                // The keys carried: the Key exits they turn may be routed through.
                if let GameEvent::Keys { keys } = &typed {
                    self.position.traveller.keys = keys
                        .iter()
                        .filter_map(|k| atlas.item_numbers.get(&k.to_lowercase()).copied())
                        .collect();
                }
                let says = self.bot.event(&typed, now);
                self.say(says, out);
                self.learn(now, out);
            }
            if !silent {
                out.push(event_message(&self.id, ev));
            }
        }
        self.fed = chunk.seq;
        match events.last() {
            Some(ev) => self.at_prompt = ev.name == "prompt",
            None if visible(&chunk.text) => self.at_prompt = false,
            None => {}
        }
        if changed {
            self.character.set_updated_seq(chunk.seq);
        }
        if combat && self.walker.walking() {
            self.stop_walk("combat started", out);
        }
        // The game noticed a sneaking move, or a move showed no `Sneaking...` (C-048): the
        // bot's walk stops, and it sneaks again before the next one.
        self.bot.sneak_arrived(self.position.best());
        if self.bot.sneak_broken(&self.settings) && self.walker.walking() && self.bot_walk {
            self.stop_walk("the sneak failed", out);
        }
        self.bot_held(out);
        self.drive(now, out);
        self.flush_views(out);
        if let Resync::Waiting {
            step,
            answered: true,
            typed,
            ..
        } = self.resync
            && self.at_prompt
        {
            self.resync = Resync::Want {
                next: step + 1,
                idle: typed,
            };
        }
    }

    /// Who is in the room changed: where the monsters to avoid were last seen (the router
    /// keeps out of those rooms), and a monster to flee from makes what the bot has in flight
    /// give way.
    fn sighted(&mut self, ev: &GameEvent, now: Instant, out: &mut Vec<ToGateway>) {
        use GameEvent as G;
        let shown = matches!(
            ev,
            G::RoomExits { .. }
                | G::RoomDark
                | G::Prompt { .. }
                | G::MonsterArrives { .. }
                | G::MonsterLeaves { .. }
                | G::MonsterDeath { .. }
        );
        if !shown {
            return;
        }
        let room = self.position.best();
        if let Some(room) = room.filter(|_| self.position.belief.certain())
            && self
                .sightings
                .seen(self.rules.rules(), room, &self.presence.here, now)
        {
            self.position.traveller.avoid = self.sightings.rooms(now);
        }
        // A walk of the bot's that only passes through the room (the router keeps out of it
        // when there is another way) goes on: its next step leaves anyway, and a flee from
        // here would only send it back the way it came.
        let passing = self.bot_walk
            && self.walker.walking()
            && room.is_some()
            && self.walker.destination() != room;
        if !passing
            && self
                .bot
                .flee_urgent(self.rules.rules(), &self.presence, room)
        {
            self.give_way(out);
        }
    }

    /// The monsters stalemates gave up become *never fight* for the session (kept with the
    /// bot), and their room is kept out of as a monster to avoid's would be.
    fn learn(&mut self, now: Instant, out: &mut Vec<ToGateway>) {
        for name in self.bot.take_learned() {
            if self.rules.learn(&name, unix(self.clock.now())) {
                out.push(self.notice(&format!("Monster rule: never fight {name} (a stalemate)")));
            }
            if let Some(room) = self.position.best() {
                self.sightings.mark(room, &name, now);
                self.position.traveller.avoid = self.sightings.rooms(now);
            }
        }
    }

    /// `set_monster_rule`: the player's rule for monsters whose name holds `pattern`, or none.
    pub fn set_monster_rule(
        &mut self,
        pattern: &str,
        rule: Option<world::MonsterRule>,
        now: Instant,
        out: &mut Vec<ToGateway>,
    ) {
        let pattern = pattern.trim();
        if pattern.chars().count() < 2 {
            out.push(self.warning("set_monster_rule: the pattern needs two characters or more"));
            return;
        }
        if !self.rules.set(pattern, rule, unix(self.clock.now())) {
            return;
        }
        let text = match rule {
            Some(r) => format!("Monster rule: {} {pattern}", rules::describe(r)),
            None => format!("Monster rule removed: {pattern}"),
        };
        out.push(self.notice(&text));
        // Who is here now, by the new rules.
        if let Some(room) = self
            .position
            .best()
            .filter(|_| self.position.belief.certain())
        {
            self.sightings
                .seen(self.rules.rules(), room, &self.presence.here, now);
            self.position.traveller.avoid = self.sightings.rooms(now);
        }
        if self
            .bot
            .flee_urgent(self.rules.rules(), &self.presence, self.position.best())
        {
            self.give_way(out);
        }
        self.drive(now, out);
        self.flush_views(out);
    }

    /// What the run counts: kills (`dead`, with the exp that follows), deaths, rests, and a
    /// level above `level`, the one before the event.
    fn count(&mut self, ev: &GameEvent, dead: Option<&str>, level: Option<u32>) {
        let room = self.position.best();
        match ev {
            GameEvent::MonsterDeath { .. } => {
                self.ledger.event(EventKind::Kill, 0, dead, room);
            }
            GameEvent::ExpGain { amount } => self.ledger.exp(*amount, room),
            GameEvent::YouKilled => {
                self.ledger.event(EventKind::Death, 0, None, room);
                self.ledger.place(StoredPlace::Death, None, room);
            }
            GameEvent::Resting { .. } => self.ledger.event(EventKind::Rest, 0, None, room),
            _ => {}
        }
        if let (Some(was), Some(now)) = (level, self.character.state().level)
            && now > was
        {
            self.ledger
                .event(EventKind::LevelUp, i64::from(now), None, room);
        }
    }

    /// The bot a brain before this one left running or paused, now that the catch-up is
    /// done: back in that state with its home and loop, narrated; the re-sync the attach
    /// started goes first. Outside the realm it stays stopped.
    pub fn resume(&mut self, out: &mut Vec<ToGateway>) {
        let Some(rec) = self.restore.take() else {
            return;
        };
        let Some(atlas) = self.atlas.clone() else {
            return;
        };
        if self.state != SessionState::Realm {
            out.push(self.notice("Not resuming the bot: the character is not in the realm"));
            return;
        }
        let name = |k: Option<Key>| {
            k.and_then(|k| atlas.graph.room(k))
                .map_or_else(|| "an unknown room".to_owned(), |r| r.name.clone())
        };
        let stored = rec.loop_id.as_ref().map(|id| TaskLoop {
            id: id.clone(),
            stops: rec.loop_stops.clone(),
            index: rec.stop_index,
            skip: rec.skip.clone(),
        });
        let task = stored.map(|t| {
            let name = self.loop_name(&atlas, &t, out);
            Task::restore(&atlas, &self.settings, name, &t)
        });
        let doing = match &task {
            Some(t) => {
                let w = t.snapshot();
                let stops = w.stops.len().max(1);
                format!(
                    "running loop {} at stop {}/{stops}",
                    w.loop_name,
                    w.stop as usize % stops + 1
                )
            }
            None => format!("fighting in {}", name(rec.home)),
        };
        let narration = match &rec.paused_reason {
            Some(why) if rec.state == BotState::Paused => {
                format!("Resumed paused ({why}): {doing} (brain restarted)")
            }
            _ if rec.state == BotState::Paused => {
                format!("Resumed paused: {doing} (brain restarted)")
            }
            _ => format!("Resumed: {doing} (brain restarted)"),
        };
        let says = self.bot.restore(&rec, task, narration);
        self.say(says, out);
        self.ledger.adopt(rec, self.clock.instant());
        if self.resync == Resync::Off {
            self.request_resync();
        }
    }

    /// A stored loop's name: what its id makes now if that has the same stops; otherwise the
    /// id, with a notice that the stored stops are kept (a level-up can change what a generated
    /// id names).
    fn loop_name(&mut self, atlas: &Atlas, t: &TaskLoop, out: &mut Vec<ToGateway>) -> String {
        self.position
            .refresh(atlas, &self.character.snapshot(), self.clock.instant());
        if let Some(l) = mm_loop(atlas, &t.id) {
            self.learn_path(atlas);
            if l.stops() == t.stops {
                return l.name().to_owned();
            }
            tracing::warn!(session = %self.id, id = %t.id, "the stored loop's file no longer has its stops");
            out.push(self.warning(&format!(
                "Loop {} no longer has the stored stops (another world pack?): keeping the \
                 stored stops",
                t.id
            )));
            return t.id.clone();
        }
        let found = parse_loop_id(&t.id, &self.position.traveller)
            .filter(|(k, _)| atlas.graph.contains(k.anchor))
            .and_then(|(k, i)| self.generated(atlas, k).get(i).cloned());
        match found {
            Some(l)
                if l.stops
                    .iter()
                    .map(|s| (s.map, s.room))
                    .eq(t.stops.iter().copied()) =>
            {
                l.name
            }
            _ => {
                tracing::warn!(session = %self.id, id = %t.id, "the stored loop's id no longer matches its stops");
                out.push(self.warning(&format!(
                    "Loop {} no longer makes the stored stops (the level changed?): keeping the \
                     stored stops",
                    t.id
                )));
                t.id.clone()
            }
        }
    }

    /// Save the bot if it changed (the run starts or ends with it). Not before a pending resume:
    /// that would overwrite what is to be resumed.
    pub fn persist(&mut self, out: &mut Vec<ToGateway>) {
        if self.atlas.is_none() || self.restore.is_some() {
            return;
        }
        let mut record = self.bot.record();
        record.monster_rules = self.rules.stored();
        if self.ledger.stopping(&record) {
            let why = self.bot.snapshot().narration;
            let room = self.position.best();
            self.ledger.place(StoredPlace::Stopped, Some(&why), room);
        }
        if self.ledger.save(record, self.clock.instant()) {
            self.publish_patch(out);
        }
    }

    /// The plan for the character: its coins in copper, and the spells its class could learn that
    /// it lacks, each at its best shop ([`wayfarer_world::spellshop::plan`]); an `Err` says what
    /// is not known yet.
    fn spell_plan_items(
        &mut self,
    ) -> Result<(u64, Vec<wayfarer_world::spellshop::PlanItem>), String> {
        // The routes for the character as it is now: its level shuts the Newhaven gate.
        if let Some(atlas) = self.atlas.clone() {
            self.position
                .refresh(&atlas, &self.character.snapshot(), self.clock.instant());
        }
        let atlas = self
            .atlas
            .as_ref()
            .ok_or("spell plan: the brain has no world pack")?;
        let c = self.character.state();
        let class = c
            .class
            .as_deref()
            .ok_or("spell plan: the character's class is not known yet")?;
        let &(magery, magery_level) = atlas
            .class_magery
            .get(class)
            .ok_or_else(|| format!("spell plan: no spells for a {class}"))?;
        let here = self
            .position
            .best()
            .ok_or("spell plan: the position is unknown")?;
        if !self.spells_seen {
            return Err(
                "spell plan: the spellbook is not known yet (re-sync, or wait for the first `spells`)"
                    .to_owned(),
            );
        }
        let who = wayfarer_world::spellshop::Learner {
            magery,
            magery_level,
            level: c.level.map_or(1, i64::from),
            charm: c.stats.as_ref().and_then(|s| s.charm).map(i64::from),
        };
        let known: Vec<String> = self.spell_rows.iter().map(|r| r.3.clone()).collect();
        // The coins on hand, in copper.
        let cash = c.coins.as_ref().map_or(0, |coins| {
            coins
                .carried
                .iter()
                .map(|k| k.count * wayfarer_world::spellshop::unit(coin_currency(&k.coin)))
                .sum::<u64>()
        });
        // A shop up stairs that take a room ticket counts: the walk buys one on the way.
        let dist = wayfarer_world::distances_buying(
            &atlas.graph,
            here,
            10_000,
            &self.position.traveller,
            &atlas.exit_items,
        );
        let items = wayfarer_world::spellshop::plan(&atlas.scrolls, who, &known, cash, |rooms| {
            rooms
                .iter()
                .filter_map(|k| dist.get(k).map(|d| (*d, *k)))
                .min()
        });
        Ok((cash, items))
    }

    /// `spell_plan_request`.
    pub fn spell_plan(&mut self, out: &mut Vec<ToGateway>) {
        let (cash, items) = match self.spell_plan_items() {
            Ok(plan) => plan,
            Err(why) => {
                out.push(self.warning(&why));
                return;
            }
        };
        out.push(ToGateway::SpellPlan {
            session: self.id.clone(),
            cash,
            items: items
                .into_iter()
                .map(|i| wayfarer_proto::SpellPlanItem {
                    spell: i.spell,
                    spell_level: i.spell_level,
                    scroll: i.scroll,
                    shop: i.shop,
                    map: i.room.0,
                    room: i.room.1,
                    price: i.price,
                    steps: i.steps,
                    usable: i.usable,
                    affordable: i.affordable,
                })
                .collect(),
        });
    }

    /// `stats_request`.
    pub fn stats_request(&self, out: &mut Vec<ToGateway>) {
        let (runs, samples) = self.ledger.summaries();
        out.push(ToGateway::Stats {
            session: self.id.clone(),
            runs,
            samples,
        });
    }

    /// `stats_reset`: a new run from now, its numbers at zero.
    pub fn stats_reset(&mut self, now: Instant, out: &mut Vec<ToGateway>) {
        if self.ledger.reset(now) {
            out.push(
                self.notice("Run reset: a new run starts now, the old one is kept in the history"),
            );
            self.stats_request(out);
            self.flush_views(out);
        } else {
            out.push(self.warning("No run to reset: Start the bot first"));
        }
    }

    /// The player a PvP line shows attacking the character: `X moves to attack you!` from a
    /// player, or `X just attacked <the character>!`.
    fn pvp_attacker(&self, ev: &Event) -> Option<String> {
        let who = ev.text("who")?.trim();
        let me = self.character.state().name.as_deref().unwrap_or_default();
        let first = me.split_whitespace().next().unwrap_or_default();
        let attacked = match ev.name.as_str() {
            "attacks_you" => true,
            // `v2` is the other way round: `X just dodged an attack from <victim>.`
            "pvp_attacked" => ["v1", "v3"].iter().any(|v| {
                ev.text(v)
                    .is_some_and(|v| !first.is_empty() && v.trim() == first)
            }),
            _ => false,
        };
        let player = !who.contains(' ') && who.starts_with(|c: char| c.is_ascii_uppercase());
        (attacked && player).then(|| who.to_owned())
    }

    /// Flee takes control: the bot's walk stops after the move in flight, and its action that
    /// has not gone yet is dropped.
    fn give_way(&mut self, out: &mut Vec<ToGateway>) {
        if self.bot_walk {
            self.stop_walk("fleeing", out);
        }
        if self.owner == Some(Owner::Bot)
            && self.executor.busy()
            && self.executor.in_flight().is_none()
        {
            self.executor.cancel();
            self.owner = None;
            self.bot.forget();
        }
    }

    /// `logging_off`: the gateway is about to log the character off. A line the player typed
    /// that has not gone yet is dropped with a warning, as one typed from now on is. If the
    /// character is attacked or fighting, the bot flees first (an attack cancels the exit,
    /// `kb/recovery.md`), whatever it was doing, and `false` is returned: the session answers
    /// `ready_to_leave` itself once the flee is over. Otherwise, at once: the bot stops, a
    /// walk stops after the move in flight, an action not sent yet is dropped, and nothing
    /// more goes to the game (a re-sync included) while the session is in the realm; the
    /// caller answers.
    pub fn logging_off(&mut self, now: Instant, out: &mut Vec<ToGateway>) -> bool {
        // A typed line still waiting would go mid-flee or into the log-off menus.
        self.drop_typed(out);
        if self.attacked(now) {
            if self.walker.walking() {
                self.stop_walk("the gateway is logging off", out);
            }
            if self.executor.in_flight().is_none() {
                self.cancel(out);
            }
            self.resync = Resync::Off;
            // The flee goes on the run before it ends.
            let c = self.character.state();
            let hp = hp_percent(c.hp, c.hp_max).unwrap_or(0);
            let who = self.presence.attackers.first().cloned();
            let room = self.position.best();
            self.ledger
                .event(EventKind::Flee, i64::from(hp), who.as_deref(), room);
            self.ledger.place(StoredPlace::Flee, who.as_deref(), room);
            let says = self.bot.leave();
            self.say(says, out);
            self.flee_out = true;
            self.drive(now, out);
            self.flush_views(out);
            return false;
        }
        self.leave_now(out);
        true
    }

    /// Drop the player's typed lines still waiting, each with a warning (`logging_off`).
    fn drop_typed(&mut self, out: &mut Vec<ToGateway>) {
        for line in std::mem::take(&mut self.typed) {
            out.push(self.not_sent(&line.text));
        }
    }

    fn not_sent(&self, text: &str) -> ToGateway {
        self.warning(&format!("Not sent: {text} (the gateway is logging off)"))
    }

    /// Whether the character is under attack as far as the brain can tell: a monster or a
    /// player attacked it in this room, or its own auto-combat is going on.
    fn attacked(&self, now: Instant) -> bool {
        self.atlas.is_some()
            && self.state == SessionState::Realm
            && self.live()
            && (!self.presence.attackers.is_empty()
                || (self.character.state().in_combat == Some(true) && !self.quiet(now)))
    }

    /// The flee before a log-off is over (safe, cornered, or the bot stopped): nothing more
    /// goes to the game, and the brain answers `ready_to_leave`.
    fn flee_out_done(&mut self, out: &mut Vec<ToGateway>) {
        if !self.flee_out || self.bot.leaving() {
            return;
        }
        self.flee_out = false;
        self.leave_now(out);
        out.push(ToGateway::ReadyToLeave {
            session: self.id.clone(),
        });
    }

    /// Stop the bot and send nothing more (`logging_off`).
    fn leave_now(&mut self, out: &mut Vec<ToGateway>) {
        self.leaving = true;
        let says = self.bot.stop("the gateway is logging off", false);
        self.say(says, out);
        self.bot_held(out);
        if self.walker.walking() {
            self.stop_walk("the gateway is logging off", out);
        }
        if self.executor.in_flight().is_none() {
            self.cancel(out);
        }
        self.resync = Resync::Off;
        self.flush_views(out);
    }

    /// A line went to the game (the gateway's `sent`).
    pub fn sent(&mut self, text: &str, now: Instant, out: &mut Vec<ToGateway>) {
        while self
            .own
            .front()
            .is_some_and(|(_, at)| now.duration_since(*at) > OWN_PATIENCE)
        {
            self.own.pop_front();
        }
        if self.own.front().is_some_and(|(c, _)| c == text) {
            // Ours; the parser and the position were told when it went.
            self.own.pop_front();
            return;
        }
        self.parser.sent(text);
        if let Some(atlas) = &self.atlas {
            self.position.sent(atlas, text);
        }
        self.last_sent = now;
        match &mut self.resync {
            Resync::Waiting { typed, .. } => *typed = true,
            Resync::Want { idle, .. } => *idle = true,
            Resync::Off => {}
        }
        if self
            .atlas
            .as_ref()
            .is_some_and(|a| self.position.is_move(a, text))
        {
            self.hand_move(out);
        } else if self.walker.walking() && !self.bot_walk {
            // The player comes first (docs/plan/02-architecture.md "Input arbitration").
            self.stop_walk("you typed a line", out);
        }
        self.flush_views(out);
    }

    /// `user_input`: a line the player typed while the brain arbitrates the session (its bot
    /// running or paused). Talk and look waits for a prompt with nothing in flight and goes
    /// between the bot's actions; an act pauses the bot after the action in flight (and stops
    /// a walk), then goes the same way. With the bot stopped meanwhile, or out of the realm,
    /// the line goes at once, as the gateway would have sent it.
    pub fn user_input(&mut self, text: String, now: Instant, out: &mut Vec<ToGateway>) {
        if self.leaving || self.flee_out {
            out.push(self.not_sent(&text));
            return;
        }
        if !self.wants_arbitration() || self.state != SessionState::Realm {
            self.send_straight(text, out);
            return;
        }
        let mut kind = classify(&text).kind;
        if kind == Kind::Unknown && self.is_chat(&text) {
            kind = Kind::Talk;
        }
        tracing::info!(session = %self.id, %text, ?kind, "typed beside the bot");
        // A move the player types takes the character elsewhere: the bot drops its loop or
        // journey and plays where it goes. Any other line goes between the bot's actions.
        if kind != Kind::Talk
            && self
                .atlas
                .as_ref()
                .is_some_and(|a| self.position.is_move(a, &text))
        {
            self.hand_move(out);
        }
        let line = TypedLine { text, at: now };
        self.typed.push_back(line);
        self.drive(now, out);
        self.flush_views(out);
    }

    /// A line the game's parser does not take (`heya`) that cannot be a Text exit's phrase from
    /// where the character is: the position is sure and no exit of the room is taken by it.
    /// Such a line is talk to the room, not an act, so it pauses nothing. With the position in
    /// doubt it may be an exit's phrase, so it stays an act.
    fn is_chat(&self, text: &str) -> bool {
        self.atlas.as_ref().is_some_and(|atlas| {
            self.position.belief.certain() && !self.position.is_move(atlas, text)
        })
    }

    /// `keys_sent`: the player's keys went straight to the game. They may be a move, so they
    /// count as one: a walk stops, and a running bot drops its loop or journey and plays where
    /// the character ends up.
    pub fn keys_sent(&mut self, out: &mut Vec<ToGateway>) {
        if self.walker.walking() {
            self.stop_walk("you pressed a key", out);
        }
        self.hand_move(out);
        self.flush_views(out);
    }

    /// Whether the brain arbitrates the session's input: it has a bot, running or paused, and
    /// the gateway is not logging the character off.
    fn wants_arbitration(&self) -> bool {
        self.atlas.is_some() && !self.bot.stopped() && !self.leaving
    }

    /// Tell the gateway whether the brain arbitrates the session's input, when that changed.
    pub fn arbitrate(&mut self, out: &mut Vec<ToGateway>) {
        let on = self.wants_arbitration();
        if on != self.arbitrating {
            self.arbitrating = on;
            out.push(ToGateway::Arbitrate {
                session: self.id.clone(),
                on,
            });
        }
    }

    /// A player's line to the game at once, as the gateway sends one it does not hold: its
    /// `sent` is the player's.
    fn send_straight(&mut self, text: String, out: &mut Vec<ToGateway>) {
        out.push(ToGateway::InputLine {
            session: self.id.clone(),
            text,
        });
    }

    /// The player's next typed line, if its moment has come: a prompt in the realm with nothing
    /// in flight and no re-sync command waiting for its answer (or it has waited
    /// [`TYPED_PATIENCE`] for a prompt). It goes as the brain's own line, so the parser and the
    /// position are told now and its `sent` is not taken for a new one.
    ///
    /// An attack is checked again when its moment comes: the bot's own action in flight may
    /// have engaged meanwhile, and then it is held.
    fn send_typed(&mut self, now: Instant, out: &mut Vec<ToGateway>) -> bool {
        let Some(line) = self.typed.front() else {
            return false;
        };
        let prompt = self.at_prompt || now >= line.at + TYPED_PATIENCE;
        if self.state != SessionState::Realm
            || !self.live()
            || self.leaving
            || self.flee_out
            || !prompt
            || self.executor.in_flight().is_some()
            || matches!(self.resync, Resync::Waiting { .. })
        {
            return false;
        }
        let line = self.typed.pop_front().expect("a typed line");
        self.send_own(&line.text, now, out);
        // A re-sync command after it waits for an idle prompt, as after any typed line.
        if let Resync::Want { idle, .. } = &mut self.resync {
            *idle = true;
        }
        true
    }

    /// The player moved the character by hand (a typed move, keys): the bot's walk stops after
    /// the move in flight, and a running bot drops its loop or journey and plays on where the
    /// character ends up (docs/brain.md "The bot's states").
    fn hand_move(&mut self, out: &mut Vec<ToGateway>) {
        if self.bot_walk && self.walker.walking() {
            self.stop_walk("you moved", out);
        }
        let says = self.bot.hand_move();
        self.say(says, out);
    }

    /// The bot is pausing, paused or stopped: its walk stops after the move in flight, and an
    /// action of its that has not gone yet is dropped.
    fn bot_held(&mut self, out: &mut Vec<ToGateway>) {
        if self.bot.active() {
            return;
        }
        if self.bot_walk {
            self.stop_walk("the bot is not running", out);
        }
        if self.owner == Some(Owner::Bot) && self.executor.in_flight().is_none() {
            self.executor.cancel();
            self.owner = None;
            self.bot.forget();
        }
    }

    /// Send one of the brain's own lines to the game.
    fn send_own(&mut self, cmd: &str, now: Instant, out: &mut Vec<ToGateway>) {
        self.parser.sent(cmd);
        if let Some(atlas) = &self.atlas {
            self.position.sent(atlas, cmd);
        }
        self.own.push_back((cmd.to_owned(), now));
        self.last_sent = now;
        self.at_prompt = false;
        out.push(ToGateway::InputLine {
            session: self.id.clone(),
            text: cmd.to_owned(),
        });
    }

    /// Send a move pipelined ahead of the brain's move in flight: the position expects its room
    /// after that one's.
    fn send_own_ahead(&mut self, cmd: &str, now: Instant, out: &mut Vec<ToGateway>) {
        self.parser.sent(cmd);
        self.position.sent_ahead(cmd);
        self.own.push_back((cmd.to_owned(), now));
        self.last_sent = now;
        self.at_prompt = false;
        out.push(ToGateway::InputLine {
            session: self.id.clone(),
            text: cmd.to_owned(),
        });
    }

    /// Queue the walker's move to go ahead of its move in flight, if the moment allows: the
    /// belief sure of the room, the character known to be out of combat, and no line the
    /// player typed waiting.
    fn pipeline(&mut self, action: Action) {
        let now = self.clock.instant();
        let in_combat = self.combat(now);
        if self.owner == Some(Owner::Walker)
            && self.position.belief.certain()
            && in_combat == Some(false)
            && self.typed.is_empty()
            && self.executor.submit_ahead(action)
        {
            self.walker.went_ahead();
        }
    }

    /// `walk_to`: plan a route from the believed room and start on it. `stop_short` (on when
    /// left out): stop one room short of a boss lair.
    pub fn walk_to(
        &mut self,
        to: Key,
        stop_short: Option<bool>,
        now: Instant,
        out: &mut Vec<ToGateway>,
    ) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("walk_to: the brain has no world pack"));
            return;
        };
        if self.state != SessionState::Realm {
            out.push(self.warning("walk_to: the character is not in the realm"));
            return;
        }
        if let Some(leader) = self.party_role.following() {
            out.push(self.warning(&format!(
                "walk_to: following {leader}: it goes where the party goes"
            )));
            return;
        }
        if self.errand.is_some() {
            out.push(self.warning("walk_to: an errand is in progress; cancel it first"));
            return;
        }
        self.position
            .refresh(&atlas, &self.character.snapshot(), self.clock.instant());
        let name = atlas
            .graph
            .room(to)
            .map_or_else(|| format!("{}/{}", to.0, to.1), |r| r.name.clone());
        let short =
            stop_short.unwrap_or(true) && atlas.finder.boss_lair(to, &atlas.monsters).is_some();
        self.go(Journey::go_to(to, name, short), now, out);
    }

    /// Start the player's walk (or loop walk) to `to`: after the walker's move in flight, or
    /// once the bot's action in flight is answered (one action at a time). `short`: stop one
    /// room short of a boss lair.
    fn start_walk(&mut self, atlas: &Atlas, to: Key, short: bool, out: &mut Vec<ToGateway>) {
        let busy = self.executor.in_flight().is_some();
        if busy && self.owner == Some(Owner::Bot) {
            self.walk_after = Some((to, short));
            return;
        }
        self.walk_after = None;
        self.position
            .refresh(atlas, &self.character.snapshot(), self.clock.instant());
        let next = self
            .walker
            .start_with(&mut self.position, atlas, to, busy, false, short);
        self.apply(next, out);
    }

    /// `find`: rooms, places and monsters by name, nearest the believed room first.
    pub fn find(&self, query: &str, out: &mut Vec<ToGateway>) {
        let Some(atlas) = &self.atlas else {
            out.push(self.warning("find: the brain has no world pack"));
            return;
        };
        let found = atlas.finder.find(
            &atlas.graph,
            &atlas.monsters,
            query,
            self.position.best(),
            &self.position.traveller,
        );
        let results = found
            .into_iter()
            .map(|f| Found {
                kind: match f.kind {
                    world::FoundKind::Rooms => FoundKind::Rooms,
                    world::FoundKind::Monster => FoundKind::Monster,
                    world::FoundKind::Place(p) => match p {
                        PlaceKind::Shop => FoundKind::Shop,
                        PlaceKind::Bank => FoundKind::Bank,
                        PlaceKind::Trainer => FoundKind::Trainer,
                        PlaceKind::Boss => FoundKind::Boss,
                        PlaceKind::Other => FoundKind::Place,
                    },
                },
                name: f.name,
                rooms: f
                    .rooms
                    .into_iter()
                    .map(|r| FoundRoom {
                        map: r.key.0,
                        room: r.key.1,
                        name: r.name,
                        steps: match r.reach {
                            Reach::Steps(n) => Some(n),
                            _ => None,
                        },
                        no_route: r.reach == Reach::NoRoute,
                        boss: r.boss,
                    })
                    .collect(),
                count: u32::try_from(f.count).unwrap_or(u32::MAX),
                monsters: (!f.bosses.is_empty()).then_some(f.bosses),
            })
            .collect();
        out.push(ToGateway::Found {
            session: self.id.clone(),
            query: query.to_owned(),
            results,
        });
    }

    /// `loops_request`: the loops nearest the character (or `anchor`), nearest first, at most
    /// `limit`; with a `query`, the ones that match it, from anywhere in the world the
    /// character can walk to. Distance is the steps from the anchor to the nearest room of the
    /// loop's walk.
    pub fn loops_request(
        &mut self,
        anchor: Option<Key>,
        radius: u32,
        doors: bool,
        query: &str,
        limit: Option<u32>,
        out: &mut Vec<ToGateway>,
    ) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("loops: the brain has no world pack"));
            return;
        };
        let Some(anchor) = anchor.or_else(|| self.position.best()) else {
            out.push(self.warning("loops: the position is unknown"));
            return;
        };
        if !atlas.graph.contains(anchor) {
            out.push(self.warning(&format!(
                "loops: no room {}/{} in the world pack",
                anchor.0, anchor.1
            )));
            return;
        }
        let query = query.trim();
        let limit = limit
            .map_or(LOOPS_SHOWN, |n| n as usize)
            .clamp(1, MOST_LOOPS_SHOWN);
        // A search matches the loops already generated over the area, never a wider one: loops
        // over 30 steps took the brain over a minute to generate, and no character was played.
        let radius = radius.min(MAX_LOOP_RADIUS);
        self.position
            .refresh(&atlas, &self.character.snapshot(), self.clock.instant());
        let key = LoopsKey {
            anchor,
            radius,
            level: self.position.traveller.level,
            strength: self.position.traveller.strength,
            picklocks: self.position.traveller.picklocks,
            doors,
        };
        let generated = self.generated(&atlas, key);
        // Steps from the character to every room it can walk to: a loop's distance is the
        // least of its rooms'.
        let dist = distances(&atlas.graph, anchor, 10_000, &self.loop_traveller(doors));
        let near = |path: &[wayfarer_proto::RoomKey]| {
            path.iter()
                .filter_map(|k| dist.get(&(k.map, k.room)).copied())
                .min()
        };
        enum Pick {
            Generated(usize),
            Megamud(usize),
            Custom(usize),
        }
        let mut picks: Vec<(u32, String, Pick)> = Vec::new();
        for (i, l) in generated.iter().enumerate() {
            let names = l.stops.iter().map(|s| s.name.as_str());
            if let Some(d) = near(&l.path)
                && loop_matches(query, std::iter::once(l.name.as_str()).chain(names))
            {
                picks.push((d, l.name.to_lowercase(), Pick::Generated(i)));
            }
        }
        let mm = megamud_through(&atlas.mm_loops, &dist);
        for (i, l) in mm.iter().enumerate() {
            let d = l.rooms.iter().filter_map(|k| dist.get(k).copied()).min();
            let names = l.hunt.iter().map(|h| h.name.as_str());
            let texts = [
                l.path.file.as_str(),
                l.path.description.as_str(),
                l.path.author.as_str(),
            ];
            if let Some(d) = d
                && loop_matches(query, texts.into_iter().chain(names))
            {
                picks.push((d, l.path.description.to_lowercase(), Pick::Megamud(i)));
            }
        }
        let custom_all: Vec<GeneratedLoop> = self
            .ledger
            .custom_loops()
            .iter()
            .map(|c| self.custom_as_loop(&atlas, c))
            .collect();
        for (i, l) in custom_all.iter().enumerate() {
            let names = l.stops.iter().map(|s| s.name.as_str());
            let stops: Vec<wayfarer_proto::RoomKey> = l
                .stops
                .iter()
                .map(|s| wayfarer_proto::RoomKey {
                    map: s.map,
                    room: s.room,
                })
                .collect();
            if let Some(d) = near(&l.path).or_else(|| near(&stops))
                && loop_matches(query, std::iter::once(l.name.as_str()).chain(names))
            {
                picks.push((d, l.name.to_lowercase(), Pick::Custom(i)));
            }
        }
        picks.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        let matches = u32::try_from(picks.len()).unwrap_or(u32::MAX);
        picks.truncate(limit);
        let (mut gen_out, mut mm_out, mut custom_out) = (Vec::new(), Vec::new(), Vec::new());
        for (d, _, pick) in picks {
            match pick {
                Pick::Generated(i) => gen_out.push(GeneratedLoop {
                    distance: Some(d),
                    ..generated[i].clone()
                }),
                Pick::Custom(i) => custom_out.push(GeneratedLoop {
                    distance: Some(d),
                    ..custom_all[i].clone()
                }),
                Pick::Megamud(i) => {
                    let l = mm[i];
                    let reason = self.mm_not_runnable(&atlas, l);
                    mm_out.push(MegaMudLoop {
                        file: l.path.file.clone(),
                        description: l.path.description.clone(),
                        author: l.path.author.clone(),
                        path: l.rooms.iter().copied().map(room_key).collect(),
                        steps: u32::try_from(l.path.verification.steps_checked).unwrap_or(0),
                        exact: u32::try_from(l.path.verification.steps_exact).unwrap_or(0),
                        stops: l.hunt.iter().map(loop_stop).collect(),
                        runnable: reason.is_none(),
                        reason,
                        distance: Some(d),
                    });
                }
            }
        }
        for stops in gen_out
            .iter_mut()
            .map(|l| &mut l.stops)
            .chain(mm_out.iter_mut().map(|l| &mut l.stops))
            .chain(custom_out.iter_mut().map(|l| &mut l.stops))
        {
            self.mark_bosses(&atlas, stops);
        }
        out.push(ToGateway::Loops {
            session: self.id.clone(),
            anchor: room_key(anchor),
            radius,
            doors,
            generated: gen_out,
            megamud: mm_out,
            custom: custom_out,
            matches,
        });
    }

    /// The bosses that spawn at each stop and whether each should be up: the character's last
    /// kill of it plus its `regenhrs`. A guess, other players kill bosses too.
    fn mark_bosses(&self, atlas: &Atlas, stops: &mut [LoopStop]) {
        let kills = self.ledger.boss_kills();
        let now = self.ledger.unix_now();
        for stop in stops {
            let Some(bosses) = atlas
                .finder
                .boss_lair((stop.map, stop.room), &atlas.monsters)
            else {
                continue;
            };
            stop.bosses = bosses
                .iter()
                .filter_map(|m| {
                    let hours = atlas.monsters.regen_hours(m.number)?;
                    let killed = kills
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(&m.name))
                        .map(|&(_, at)| at);
                    Some(boss_status(&m.name, hours, killed, now))
                })
                .collect();
        }
    }

    /// The traveller loops are made for: the character's, through doors or not.
    fn loop_traveller(&self, doors: bool) -> Traveller {
        Traveller {
            no_doors: !doors,
            ..self.position.traveller.clone()
        }
    }

    /// The hidden exits the verified MegaMud paths go through are known to be there: routes may
    /// take them, and the walker searches for them (`Traveller::searchable`).
    fn learn_path(&mut self, atlas: &Atlas) {
        self.position
            .traveller
            .searchable
            .extend(atlas.knowledge.all_hidden_exits());
    }

    /// Why a MegaMud loop cannot be run for this character, if it cannot: nothing on it to
    /// hunt, or a leg between two of its stops (in loop order, the last back to the first)
    /// that no route takes, its path's hidden exits known.
    fn mm_not_runnable(&self, atlas: &Atlas, l: &MmLoop) -> Option<String> {
        if let Some(why) = l.not_runnable() {
            return Some(why.to_owned());
        }
        let mut t = self.position.traveller.clone();
        t.searchable.extend(atlas.knowledge.all_hidden_exits());
        let stops = l.stops();
        let g = &atlas.graph;
        let name = |k: Key| g.room(k).map_or("", |r| r.name.as_str()).to_owned();
        (0..stops.len()).find_map(|i| {
            let (a, b) = (stops[i], stops[(i + 1) % stops.len()]);
            route(g, a, b, &t).is_none().then(|| {
                format!(
                    "no way from {} to {}: {}",
                    name(a),
                    name(b),
                    why_unreachable(g, &t, &atlas.item_names, a, b)
                )
            })
        })
    }

    /// The generated loops of an area, made the first time they are asked for.
    fn generated(&mut self, atlas: &Atlas, key: LoopsKey) -> Arc<Vec<GeneratedLoop>> {
        if let Some(l) = self.loops.get(&key) {
            return l.clone();
        }
        let t = Traveller {
            level: key.level,
            strength: key.strength,
            picklocks: key.picklocks,
            ..self.loop_traveller(key.doors)
        };
        let area = Area {
            anchor: key.anchor,
            radius: key.radius,
        };
        let rooms = area.rooms(&atlas.graph, &t);
        let hunt = hunt_rooms(&atlas.graph, &atlas.monsters, area, &t);
        let loops = circuits(&atlas.graph, area, &rooms, &hunt, &t);
        let wire: Vec<GeneratedLoop> = loops
            .iter()
            .enumerate()
            .map(|(i, l)| GeneratedLoop {
                id: loop_id(&key, i),
                name: l.name.clone(),
                stops: l
                    .rooms
                    .iter()
                    .filter_map(|k| hunt.iter().find(|h| h.key == *k))
                    .map(loop_stop)
                    .collect(),
                path: std::iter::once(l.rooms[0])
                    .chain(l.steps.iter().map(|s| s.to))
                    .map(room_key)
                    .collect(),
                steps: u32::try_from(l.length).unwrap_or(u32::MAX),
                doors: u32::try_from(l.steps.iter().filter(|s| s.door.is_some()).count())
                    .unwrap_or(u32::MAX),
                distance: None,
            })
            .collect();
        let wire = Arc::new(wire);
        self.loops.insert(key, wire.clone());
        wire
    }

    /// `save_loop`: keep a custom loop, a name and its stops in order (made from `base`, a
    /// loop's id, if the player started from one). The same name again replaces it.
    pub fn save_loop(
        &mut self,
        name: &str,
        base: Option<String>,
        stops: Vec<Key>,
        out: &mut Vec<ToGateway>,
    ) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("save_loop: the brain has no world pack"));
            return;
        };
        let (name, id) = match custom::name_and_id(name) {
            Ok(x) => x,
            Err(why) => {
                out.push(self.warning(&format!("save_loop: {why}")));
                return;
            }
        };
        if stops.is_empty() || stops.len() > custom::STOPS_MAX {
            out.push(self.warning(&format!(
                "save_loop: a loop has 1 to {} stops, not {}",
                custom::STOPS_MAX,
                stops.len()
            )));
            return;
        }
        if let Some(k) = stops.iter().find(|k| !atlas.graph.contains(**k)) {
            out.push(self.warning(&format!("save_loop: no room {}/{}", k.0, k.1)));
            return;
        }
        let replaced = self
            .ledger
            .custom_loops()
            .iter()
            .any(|c| c.id.eq_ignore_ascii_case(&id));
        self.ledger.save_custom_loop(CustomLoop {
            id: id.clone(),
            name: name.clone(),
            stops: stops.clone(),
            based_on: base,
        });
        let n = stops.len();
        out.push(self.notice(&format!(
            "{} loop {name} ({n} stop{}): {id}",
            if replaced { "Changed" } else { "Saved" },
            if n == 1 { "" } else { "s" }
        )));
    }

    /// `delete_loop`: forget a custom loop.
    pub fn delete_loop(&mut self, id: &str, out: &mut Vec<ToGateway>) {
        if !custom::is_custom(id) || !self.ledger.delete_custom_loop(id) {
            out.push(self.warning(&format!("delete_loop: no custom loop {id:?}")));
            return;
        }
        out.push(self.notice(&format!("Deleted loop {id}")));
    }

    /// A custom loop as the wire has a loop: its stops as the world knows them, and the path
    /// the loop would take round them (hidden exits of every verified path counted), the first
    /// stop to the last and back.
    fn custom_as_loop(&self, atlas: &Atlas, c: &CustomLoop) -> GeneratedLoop {
        let rooms: Vec<HuntRoom> = c
            .stops
            .iter()
            .filter_map(|&k| stop_room(&atlas.graph, &atlas.monsters, k))
            .collect();
        let mut t = self.loop_traveller(true);
        t.searchable.extend(atlas.knowledge.all_hidden_exits());
        let keys: Vec<Key> = rooms.iter().map(|h| h.key).collect();
        let (mut path, mut steps, mut doors) = (Vec::new(), 0u32, 0u32);
        if let Some(&first) = keys.first() {
            path.push(room_key(first));
            for (i, &a) in keys.iter().enumerate() {
                let b = keys[(i + 1) % keys.len()];
                for s in route(&atlas.graph, a, b, &t).unwrap_or_default() {
                    path.push(room_key(s.to));
                    steps += 1;
                    doors += u32::from(s.door.is_some());
                }
            }
        }
        GeneratedLoop {
            id: c.id.clone(),
            name: c.name.clone(),
            stops: rooms.iter().map(loop_stop).collect(),
            path,
            steps,
            doors,
            distance: None,
        }
    }

    /// `walk_loop`: walk a generated loop once, from its stop nearest the character round to
    /// that stop again, a `walk_to` per stop.
    pub fn walk_loop(&mut self, id: &str, now: Instant, out: &mut Vec<ToGateway>) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("walk_loop: the brain has no world pack"));
            return;
        };
        if self.state != SessionState::Realm {
            out.push(self.warning("walk_loop: the character is not in the realm"));
            return;
        }
        if let Some(leader) = self.party_role.following() {
            out.push(self.warning(&format!(
                "walk_loop: following {leader}: it goes where the party goes"
            )));
            return;
        }
        if self.errand.is_some() {
            out.push(self.warning("walk_loop: an errand is in progress; cancel it first"));
            return;
        }
        let Some(l) = self.find_loop(&atlas, id) else {
            out.push(self.warning(&format!("walk_loop: no loop {id:?}")));
            return;
        };
        let Some(from) = self.position.best() else {
            out.push(self.warning(&format!(
                "Not walking loop {}: the position is unknown",
                l.name
            )));
            return;
        };
        let stops: Vec<Key> = l.stops.iter().map(|s| (s.map, s.room)).collect();
        // The nearest stop by route, the first of equals in loop order.
        let nearest = stops
            .iter()
            .enumerate()
            .filter_map(|(i, &k)| {
                route(&atlas.graph, from, k, &self.position.traveller).map(|r| (r.len(), i))
            })
            .min();
        let Some((_, first)) = nearest else {
            out.push(self.warning(&format!(
                "Not walking loop {}: no way to any of its stops",
                l.name
            )));
            return;
        };
        let mut order: Vec<Key> = stops[first..]
            .iter()
            .chain(&stops[..first])
            .copied()
            .collect();
        if order.len() > 1 {
            order.push(order[0]);
        }
        let start = order[0];
        let name = |k: Key| {
            atlas
                .graph
                .room(k)
                .map_or("", |r| r.name.as_str())
                .to_owned()
        };
        out.push(self.notice(&format!(
            "Walking loop {}: {} stops, {} steps, from {}",
            l.name,
            l.stops.len(),
            l.steps,
            name(start)
        )));
        let stops = order.into_iter().map(|k| (k, name(k))).collect();
        self.go(Journey::walk_loop(&l.name, stops), now, out);
    }

    /// Go on `journey` (a Go to, a loop walked once): the bot runs it, from whatever state it
    /// was in, replacing a loop or journey it had. A bot that was not running re-syncs first.
    fn go(&mut self, journey: Journey, now: Instant, out: &mut Vec<ToGateway>) {
        if self.walker.walking() {
            self.stop_walk("walking elsewhere", out);
        }
        self.bot_walk = false;
        self.walk_after = None;
        // An action queued but not sent yet is the old walk's (or the bot's): drop it.
        if self.executor.in_flight().is_none() {
            self.cancel(out);
        }
        let resync = !self.bot.running();
        if resync {
            self.request_resync();
        }
        let says = self.bot.go(journey, resync);
        self.say(says, out);
        self.bot_held(out);
        self.drive(now, out);
        self.flush_views(out);
    }

    /// The loop `id` names: a MegaMud loop by its file (its path's hidden exits become ones
    /// the character searches for), else a generated loop at the character's level now.
    fn find_loop(&mut self, atlas: &Atlas, id: &str) -> Option<GeneratedLoop> {
        self.position
            .refresh(atlas, &self.character.snapshot(), self.clock.instant());
        if let Some(l) = mm_loop(atlas, id) {
            self.learn_path(atlas);
            return Some(mm_as_loop(l));
        }
        if custom::is_custom(id)
            && let Some(c) = self
                .ledger
                .custom_loops()
                .into_iter()
                .find(|c| c.id.eq_ignore_ascii_case(id))
        {
            // The hidden exits the verified paths go through are ones to search for (Leo the
            // Quick's room is behind one).
            self.learn_path(atlas);
            return Some(self.custom_as_loop(atlas, &c));
        }
        parse_loop_id(id, &self.position.traveller)
            .filter(|(k, _)| atlas.graph.contains(k.anchor))
            .and_then(|(k, i)| self.generated(atlas, k).get(i).cloned())
    }

    /// `run_loop`: Task mode on a generated loop, from its ticked stop nearest the character
    /// (docs/brain.md "Running a loop"). For the loop the bot already runs, only its `skip`
    /// changes.
    pub fn run_loop(
        &mut self,
        id: &str,
        skip: Option<Vec<Key>>,
        now: Instant,
        out: &mut Vec<ToGateway>,
    ) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("run_loop: the brain has no world pack"));
            return;
        };
        if self.state != SessionState::Realm {
            out.push(self.warning("run_loop: the character is not in the realm"));
            return;
        }
        if let Some(leader) = self.party_role.following() {
            out.push(self.warning(&format!(
                "run_loop: following {leader}: it goes where the party goes"
            )));
            return;
        }
        let skip = skip.unwrap_or_default();
        if !self.bot.stopped()
            && let Some(task) = self.bot.task_mut()
            && task.id() == id
        {
            self.set_skip(&atlas, &skip, out);
            self.drive(now, out);
            self.flush_views(out);
            return;
        }
        let Some(l) = self.find_loop(&atlas, id) else {
            out.push(self.warning(&format!("run_loop: no loop {id:?}")));
            return;
        };
        if let Some(why) = mm_loop(&atlas, id).and_then(|m| self.mm_not_runnable(&atlas, m)) {
            out.push(self.warning(&format!("Not running loop {}: {why}", l.name)));
            return;
        }
        let stops: Vec<Key> = l.stops.iter().map(|s| (s.map, s.room)).collect();
        let skipped: BTreeSet<usize> = (0..stops.len())
            .filter(|&i| skip.contains(&stops[i]))
            .collect();
        let Some(from) = self.position.best() else {
            out.push(self.warning(&format!(
                "Not running loop {}: the position is unknown",
                l.name
            )));
            return;
        };
        // The ticked stop nearest by route, the first of equals in loop order.
        let nearest = stops
            .iter()
            .enumerate()
            .filter(|(i, _)| !skipped.contains(i))
            .filter_map(|(i, &k)| {
                route(&atlas.graph, from, k, &self.position.traveller).map(|r| (r.len(), i))
            })
            .min();
        let Some((_, first)) = nearest else {
            // The ticked stop nearest by any way at all, and what bars it.
            let open = Traveller::anyone();
            let near = stops
                .iter()
                .enumerate()
                .filter(|(i, _)| !skipped.contains(i))
                .filter_map(|(_, &k)| route(&atlas.graph, from, k, &open).map(|r| (r.len(), k)))
                .min();
            let why = near.map_or_else(String::new, |(_, k)| {
                let name = atlas.graph.room(k).map_or("", |r| r.name.as_str());
                format!(
                    " (the nearest, {name}: {})",
                    why_unreachable(
                        &atlas.graph,
                        &self.position.traveller,
                        &atlas.item_names,
                        from,
                        k,
                    )
                )
            });
            out.push(self.warning(&format!(
                "Not running loop {}: no way to any of its ticked stops{why}",
                l.name
            )));
            return;
        };
        if self.walker.walking() {
            self.stop_walk("running a loop", out);
        }
        self.bot_walk = false;
        if self.executor.in_flight().is_none() {
            self.cancel(out);
        }
        let task = Task::new(
            &atlas,
            &self.settings,
            l.id.clone(),
            l.name.clone(),
            &stops,
            first,
            skipped,
        );
        // Re-sync first: the HP maximum, the combat state, who is here.
        self.request_resync();
        let says = self.bot.run_loop(task, &l.name);
        self.say(says, out);
        self.bot_held(out);
        self.drive(now, out);
        self.flush_views(out);
    }

    /// The stops the running loop passes by from now on: narrated, and a leg to one of them
    /// stops after the move in flight.
    fn set_skip(&mut self, atlas: &Atlas, skip: &[Key], out: &mut Vec<ToGateway>) {
        let Some(task) = self.bot.task_mut() else {
            return;
        };
        let stops = task.snapshot().stops;
        let skipped: BTreeSet<usize> = (0..stops.len())
            .filter(|&i| skip.contains(&(stops[i].map, stops[i].room)))
            .collect();
        let names: Vec<String> = skipped
            .iter()
            .map(|&i| {
                let k = (stops[i].map, stops[i].room);
                atlas
                    .graph
                    .room(k)
                    .map_or("", |r| r.name.as_str())
                    .to_owned()
            })
            .collect();
        let name = task.snapshot().loop_name;
        task.set_skip(skipped);
        let away = task.heading_to_skipped();
        let text = if names.is_empty() {
            format!("Loop {name}: every stop ticked")
        } else {
            format!("Loop {name}: passing by {}", names.join(", "))
        };
        out.push(self.notice(&text));
        if away && self.bot_walk {
            self.stop_walk("its stop was unticked", out);
        }
    }

    /// `walk_stop`: the map's `stop`, as ■ when the bot has a loop or journey: it is dropped,
    /// and a running bot plays on where it is. Any other walk just ends.
    pub fn walk_stop(&mut self, now: Instant, out: &mut Vec<ToGateway>) {
        if self.bot.has_route() {
            self.bot_control(BotAction::Stop, now, out);
        }
        self.stop_walk("asked to stop", out);
        self.flush_views(out);
    }

    /// Stop walking after the action in flight (and a move pipelined ahead of it that has gone);
    /// at once if none has gone.
    fn stop_walk(&mut self, why: &str, out: &mut Vec<ToGateway>) {
        self.walk_halted = true;
        if self.owner == Some(Owner::Walker) && self.executor.drop_ahead() {
            self.walker.ahead_dropped();
        }
        if self.executor.in_flight().is_none() && self.owner == Some(Owner::Walker) {
            self.executor.cancel();
            self.owner = None;
        }
        let busy = self.executor.in_flight().is_some();
        let next = self.walker.stop(why, busy);
        self.apply(next, out);
    }

    /// `bot_control`.
    pub fn bot_control(&mut self, action: BotAction, now: Instant, out: &mut Vec<ToGateway>) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("bot: the brain has no world pack"));
            return;
        };
        if matches!(
            action,
            BotAction::PickUpAll
                | BotAction::EquipAll
                | BotAction::BuySpells
                | BotAction::Train
                | BotAction::RecoverPile
                | BotAction::CancelErrand
        ) {
            // A follower's errands that stay in the room are its own; the others walk.
            let walks = !matches!(
                action,
                BotAction::PickUpAll | BotAction::EquipAll | BotAction::CancelErrand
            );
            if walks && let Some(leader) = self.party_role.following() {
                out.push(self.warning(&format!(
                    "errands: following {leader}: it goes where the party goes"
                )));
                return;
            }
            self.errand_control(action, out);
            self.drive(now, out);
            self.flush_views(out);
            return;
        }
        let says = match action {
            BotAction::Start => {
                if self.errand.is_some() {
                    out.push(self.warning("bot: an errand is in progress; cancel it first"));
                    return;
                }
                if self.state != SessionState::Realm {
                    out.push(self.warning("bot: the character is not in the realm"));
                    return;
                }
                if self.bot.running() {
                    return;
                }
                let home = self.position.best();
                let name = home
                    .and_then(|k| atlas.graph.room(k))
                    .map_or_else(|| "an unknown room".to_owned(), |r| r.name.clone());
                // Re-sync first: the HP maximum, the combat state, who is here.
                self.request_resync();
                self.bot.start(home, &name)
            }
            // ■: the loop or journey is dropped; a running bot plays on where it is.
            BotAction::Stop => {
                if self.bot_walk && self.walker.walking() {
                    self.stop_walk("asked to stop", out);
                }
                self.bot.halt(self.position.best())
            }
            BotAction::Pause => {
                let busy = self.executor.in_flight().is_some();
                self.bot.pause("asked to pause", busy)
            }
            BotAction::Resume => {
                let says = self.bot.resume();
                if !says.is_empty() {
                    self.request_resync();
                }
                says
            }
            // Handled above.
            BotAction::PickUpAll
            | BotAction::EquipAll
            | BotAction::BuySpells
            | BotAction::Train
            | BotAction::RecoverPile
            | BotAction::CancelErrand => return,
        };
        self.say(says, out);
        self.bot_held(out);
        self.drive(now, out);
        self.flush_views(out);
    }

    /// With the bot running, under the hang-up line (`health.hangup_below_hp`) and something
    /// hitting the character: the gateway drops the connection (`health.hangup`). A clean
    /// log-off cannot finish under attack, and below 0 nothing else can be done.
    fn check_hangup(&mut self, out: &mut Vec<ToGateway>) {
        if self.state != SessionState::Realm {
            self.hung_up = false;
            return;
        }
        if self.hung_up || self.bot.stopped() || !self.settings.bool("health.hangup") {
            return;
        }
        let c = self.character.state();
        let Some(pct) = hp_percent(c.hp, c.hp_max) else {
            return;
        };
        let line = i32::try_from(self.settings.int("health.hangup_below_hp")).unwrap_or(10);
        if pct >= line || (self.presence.attackers.is_empty() && c.in_combat != Some(true)) {
            return;
        }
        self.hung_up = true;
        let why = format!("hung up at {pct}% HP (under {line}%): something was hitting it");
        out.push(self.warning(&format!("Hanging up: HP {pct}%, under the {line}% line")));
        let says = self.bot.stop(&why, true);
        self.say(says, out);
        out.push(ToGateway::SessionControl {
            session: self.id.clone(),
            action: SessionAction::HangUp,
        });
    }

    /// At 0 HP or below the game refuses every move, attack, cast and rest ("You may not do
    /// that while you are mortally wounded!"), and monsters keep hitting: the character is
    /// left alone until it is above 0 again (helped) or reborn. A walk is dropped without
    /// counting as a failed one.
    fn mortally_wounded(&self) -> bool {
        self.state == SessionState::Realm && self.character.state().hp.is_some_and(|hp| hp <= 0)
    }

    fn lie_still(&mut self, out: &mut Vec<ToGateway>) {
        if !self.mortally_wounded() {
            self.wounded_noted = false;
            return;
        }
        if self.walker.walking() {
            self.stop_walk("mortally wounded", out);
        }
        if !self.wounded_noted {
            self.wounded_noted = true;
            let hp = self.character.state().hp.unwrap_or(0);
            out.push(self.warning(&format!(
                "Mortally wounded (HP {hp}): nothing can be done until it is helped or reborn; waiting"
            )));
        }
    }

    /// `pick_up_all`, `equip_all` and `cancel_errand`.
    fn errand_control(&mut self, action: BotAction, out: &mut Vec<ToGateway>) {
        if action == BotAction::CancelErrand {
            if let Some(e) = self.errand.take() {
                self.executor.cancel();
                if self.owner == Some(Owner::Errand) {
                    self.owner = None;
                }
                if self.walker.walking() {
                    self.stop_walk("the errand was cancelled", out);
                }
                out.push(self.notice(&format!("{} cancelled", e.label())));
            }
            return;
        }
        if self.state != SessionState::Realm {
            out.push(self.warning("errand: the character is not in the realm"));
        } else if self.bot.running() {
            out.push(self.warning("errand: stop or pause the bot first"));
        } else if self.errand.is_some() {
            out.push(self.warning("errand: one is in progress; cancel it first"));
        } else if self.mortally_wounded() {
            out.push(self.warning("errand: the character is mortally wounded"));
        } else {
            let errand = match action {
                BotAction::PickUpAll => Active::Simple(Errand::new(ErrandKind::PickUp)),
                BotAction::EquipAll => Active::Simple(Errand::new(ErrandKind::Equip)),
                other => {
                    let made = match other {
                        BotAction::Train => self.training_errand(),
                        BotAction::RecoverPile => self.pile_errand(),
                        _ => self.shopping_errand(),
                    };
                    match made {
                        Ok(e) => e,
                        Err(why) => {
                            out.push(self.warning(&why));
                            return;
                        }
                    }
                }
            };
            out.push(self.notice(&format!("{} started", errand.label())));
            self.errand = Some(errand);
        }
    }

    /// Train: walk to the trainer that takes the character (the cheapest, then the nearest) and
    /// `train` there; it stays, for the player to train stats.
    fn training_errand(&self) -> Result<Active, String> {
        let atlas = self
            .atlas
            .as_ref()
            .ok_or("Train: the brain has no world pack")?;
        let c = self.character.state();
        let level = c
            .level
            .map(i64::from)
            .ok_or("Train: the character's level is not known yet")?;
        let class = c.class.as_ref().and_then(|n| atlas.classes.get(n).copied());
        let here = self
            .position
            .best()
            .ok_or("Train: the position is unknown")?;
        let dist = distances(&atlas.graph, here, 10_000, &self.position.traveller);
        let (trainer, room, _) =
            wayfarer_world::trainers::choose(&atlas.trainers, class, level, |rooms| {
                rooms
                    .iter()
                    .filter_map(|k| dist.get(k).map(|d| (*d, *k)))
                    .min()
            })
            .ok_or_else(|| {
                format!(
                    "Train: no trainer takes level {} here, or none can be reached",
                    level + 1
                )
            })?;
        let name = trainer.name.clone();
        let after = format!("At {name}: train stats here when ready");
        Ok(Active::Trip(crate::trip::Trip::new(
            "Train",
            (room, name),
            vec![Errand::new(ErrandKind::Train)],
            Some(after),
        )))
    }

    /// Recover death pile: walk to the room the character last died in, then pick up and equip.
    fn pile_errand(&self) -> Result<Active, String> {
        let atlas = self
            .atlas
            .as_ref()
            .ok_or("Recover: the brain has no world pack")?;
        let died = self
            .ledger
            .places()
            .iter()
            .find(|p| p.kind == StoredPlace::Death)
            .ok_or("Recover: no death is on record for this character")?;
        let name = atlas
            .graph
            .room(died.room)
            .map_or_else(|| "where it died".to_owned(), |r| r.name.clone());
        Ok(Active::Trip(crate::trip::Trip::new(
            "Recover",
            (
                died.room,
                format!("{name} ({}:{})", died.room.0, died.room.1),
            ),
            vec![
                Errand::new(ErrandKind::PickUp),
                Errand::new(ErrandKind::Equip),
            ],
            None,
        )))
    }

    /// The spell shopping from the plan: the scrolls usable now, cheapest first, and the Bank of
    /// Godfrey to draw from when the cash on hand falls short.
    fn shopping_errand(&mut self) -> Result<Active, String> {
        let (cash, items) = self.spell_plan_items()?;
        let wants: Vec<crate::shopping::Want> = items
            .into_iter()
            .filter(|i| i.usable)
            .map(|i| crate::shopping::Want {
                spell: i.spell,
                scroll: i.scroll,
                shop: i.shop,
                room: i.room,
                price: i.price,
                steps: i.steps,
            })
            .collect();
        if wants.is_empty() {
            return Err(
                "Spells: nothing to buy: no scroll the class can read now is missing".to_owned(),
            );
        }
        let bank = self.atlas.as_ref().and_then(|a| a.godfrey);
        Ok(Active::Shop(crate::shopping::Shopping::new(
            wants, bank, cash,
        )))
    }

    /// Give the errand its next command when nothing is in flight and the game is at a prompt;
    /// end it when it is done, or when something attacks.
    fn errand_tick(&mut self, out: &mut Vec<ToGateway>) {
        if self.errand.is_none() {
            return;
        }
        let attacked =
            !self.presence.attackers.is_empty() || self.character.state().in_combat == Some(true);
        if self.bot.running() || attacked || self.mortally_wounded() {
            let e = self.errand.take().expect("an errand");
            self.executor.cancel();
            if self.owner == Some(Owner::Errand) {
                self.owner = None;
            }
            let why = if self.bot.running() {
                "the bot started"
            } else {
                "something is attacking"
            };
            out.push(self.warning(&format!("{} stopped: {why}", e.label())));
            return;
        }
        if self.state != SessionState::Realm
            || !self.live()
            || !self.at_prompt
            || self.executor.busy()
            || self.resync != Resync::Off
        {
            return;
        }
        let c = self.character.state();
        let coins: std::collections::BTreeMap<String, u64> = c
            .coins
            .as_ref()
            .map(|c| {
                c.carried
                    .iter()
                    .map(|k| (crate::loot::coin_word(&k.coin).to_owned(), k.count))
                    .collect()
            })
            .unwrap_or_default();
        let load = c
            .encumbrance
            .as_ref()
            .and_then(|e| Some((u64::from(e.current?), u64::from(e.max?))));
        let ctx = Ctx {
            floor: &self.presence.floor,
            floor_items: &self.presence.floor_items,
            carried: &self.character.items,
            coins: &coins,
            load,
            limit: self.settings.choice("cash.max_encumbrance"),
            here: self.position.best(),
            walking: self.walker.walking(),
        };
        let next = self.errand.as_mut().expect("an errand").step(&ctx);
        match next {
            ErrandNext::Send(action) => match self.executor.submit(action) {
                Ok(()) => self.owner = Some(Owner::Errand),
                Err(guard) => {
                    self.errand = None;
                    out.push(self.warning(&format!("refused: {}", guard.describe())));
                }
            },
            ErrandNext::Walk(to) => {
                if let Some(atlas) = self.atlas.clone() {
                    self.start_walk(&atlas, to, false, out);
                }
            }
            ErrandNext::Wait => {}
            ErrandNext::Done(text) => {
                self.errand = None;
                out.push(self.notice(&text));
            }
        }
    }

    /// Forget the action in flight (it will not be answered): its owner learns nothing more.
    fn cancel(&mut self, out: &mut Vec<ToGateway>) {
        self.executor.cancel();
        if self.owner.take() == Some(Owner::Bot) {
            self.walk_after = None;
            self.bot.forget();
            let says = self.bot.settle();
            self.say(says, out);
        }
    }

    /// The bot's narration and warnings, as notices and warnings for the UIs.
    fn say(&self, says: Vec<Say>, out: &mut Vec<ToGateway>) {
        for say in says {
            let session = Some(self.id.clone());
            tracing::info!(session = %self.id, "{say:?}");
            out.push(match say {
                Say::Notice(text) => ToGateway::Notice { session, text },
                Say::Warning(text) => ToGateway::Warning { session, text },
            });
        }
    }

    /// Whether the character is in combat: out of it once the game says so, while resting, or
    /// once it has been quiet for [`COMBAT_QUIET`]; engaged when the game says so; unknown
    /// before either (which the executor's guard takes as engaged).
    fn combat(&self, now: Instant) -> Option<bool> {
        let c = self.character.state();
        if c.in_combat == Some(false) || c.resting == Some(true) || self.quiet(now) {
            return Some(false);
        }
        c.in_combat
    }

    /// The character's own auto-combat has been quiet for [`COMBAT_QUIET`]; knocked down is
    /// never quiet (its blows stop until it rises).
    fn quiet(&self, now: Instant) -> bool {
        !self.down && now >= self.last_combat + COMBAT_QUIET
    }

    /// The bot decides, if it is running and the game is at a prompt with nothing in flight.
    fn bot_tick(&mut self, now: Instant, out: &mut Vec<ToGateway>) {
        if !self.bot.running()
            || self.mortally_wounded()
            || self.state != SessionState::Realm
            || !self.live()
            || !self.at_prompt
            || self.executor.busy()
            || self.walker.walking()
            || self.resync != Resync::Off
        {
            return;
        }
        let Some(atlas) = self.atlas.clone() else {
            return;
        };
        self.position
            .refresh(&atlas, &self.character.snapshot(), self.clock.instant());
        let in_combat = self.combat(now);
        let regen = self.regen();
        let c = self.character.state();
        let view = View {
            atlas: &atlas,
            now,
            hp: hp_percent(c.hp, c.hp_max),
            engaged: in_combat != Some(false),
            resting: c.resting == Some(true),
            poisoned: c
                .statuses
                .as_ref()
                .is_some_and(|s| s.iter().any(|s| s == "poisoned")),
            room: self.position.best(),
            certain: self.position.belief.certain(),
            traveller: &self.position.traveller,
            doors: &self.position.traveller.doors,
            presence: &self.presence,
            party: &self.party_role,
            rules: self.rules.rules(),
            sightings: &self.sightings,
            settings: &self.settings,
            mana: c
                .mana
                .zip(c.mana_max)
                .map(|(m, max)| (i64::from(m), i64::from(max))),
            hp_missing: c.hp.zip(c.hp_max).map(|(h, m)| i64::from(m - h)),
            hp_points: c
                .hp
                .zip(c.hp_max)
                .map(|(h, m)| (i64::from(h), i64::from(m))),
            level: c.level.map_or(1, i64::from),
            book: &self.spellbook,
            load: c
                .encumbrance
                .as_ref()
                .and_then(|e| Some((u64::from(e.current?), u64::from(e.max?)))),
            coins: c.coins.as_ref().map_or(&[], |c| c.carried.as_slice()),
            regen,
        };
        let (act, says) = self.bot.tick(&view);
        self.say(says, out);
        self.learn(now, out);
        if let Some(hp) = self.bot.take_fled() {
            let who = self.presence.attackers.first().cloned();
            let room = self.position.best();
            self.ledger
                .event(EventKind::Flee, i64::from(hp), who.as_deref(), room);
            self.ledger.place(StoredPlace::Flee, who.as_deref(), room);
        }
        match act {
            Some(Do::Send(action)) => match self.executor.submit(action) {
                Ok(()) => self.owner = Some(Owner::Bot),
                Err(guard) => {
                    out.push(self.warning(&format!("refused: {}", guard.describe())));
                    self.bot.forget();
                    let says = self.bot.stop(guard.describe(), true);
                    self.say(says, out);
                }
            },
            Some(Do::Walk(to, short)) => {
                // A loop's legs pipeline their moves; a journey's and a sneaker's do not (a move
                // that showed no `Sneaking...` can be followed by a sneak, C-048).
                let pipelined = self.bot.loop_walking() && !self.settings.bool("movement.sneak");
                let next =
                    self.walker
                        .start_with(&mut self.position, &atlas, to, false, pipelined, short);
                self.bot_walk = self.walker.walking();
                self.walk_halted = false;
                if !self.bot_walk && next.arrived.is_none() {
                    self.bot.walk_failed();
                }
                if !self.bot_walk {
                    self.bot.forget();
                    // The walk back to the party's leader found no way.
                    if self.party_role.rejoin() == Some(to) {
                        let who = self.party_role.following().unwrap_or("the leader");
                        let why = format!("no way to {who}'s room is known");
                        out.push(self.warning(&format!("Can't reach {who}: {why}")));
                        self.party_stuck = Some((to, why, now));
                    }
                }
                self.apply(next, out);
            }
            None => {}
        }
    }

    /// What the party coordinator reads of this session.
    pub(crate) fn party_info(&self) -> MemberInfo {
        let minutes = |key: &str| {
            Duration::from_secs(u64::try_from(self.settings.int(key)).unwrap_or(5) * 60)
        };
        let rejoin = self.party_role.rejoin();
        let walking_back = self.bot_walk && rejoin.is_some() && self.walker.destination() == rejoin;
        MemberInfo {
            realm: self.state == SessionState::Realm && self.live(),
            running: self.bot.running(),
            name: self.character.state().name.clone(),
            room: self.position.best(),
            game: self.party_game.clone(),
            walk: WalkBack {
                steps_left: walking_back
                    .then(|| self.walker.snapshot().map(|w| w.steps_left))
                    .flatten(),
                stuck: self.party_stuck.as_ref().map(|(_, why, _)| why.clone()),
            },
            wait_separated: minutes("party.wait_separated_minutes"),
            wait_rest: minutes("party.wait_rest_minutes"),
            health: self.party_health(),
            target: self.bot.fight_target().map(str::to_owned),
        }
    }

    /// HP and mana against the character's own lines, by the rules the bot's View uses
    /// (`View::rest_below`, `mana_low`, `mana_restored`): for a party rest and flee.
    fn party_health(&self) -> Health {
        let c = self.character.state();
        let hp = hp_percent(c.hp, c.hp_max);
        let mana = c
            .mana
            .zip(c.mana_max)
            .filter(|&(_, max)| max > 0)
            .map(|(m, max)| i32::try_from(i64::from(m) * 100 / i64::from(max)).unwrap_or(100));
        let line = |key: &str| i32::try_from(self.settings.int(key)).unwrap_or(0);
        let casts = (self.spellbook.has_attack()
            && self.settings.choice("spells.attack") == "auto")
            || (self.spellbook.has_heal() && self.settings.choice("spells.heal") == "auto");
        let uses_mana = mana.is_some() && casts;
        let regen = self.regen();
        Health {
            hp,
            missing: c.hp.zip(c.hp_max).map(|(h, m)| i64::from(m - h)),
            low: hp.is_some_and(|h| h < line("health.rest_below_hp"))
                || (uses_mana && mana.is_some_and(|m| m < line("health.rest_mana_below"))),
            rested: hp.is_none_or(|h| h >= line("health.rest_until_hp"))
                && (!uses_mana || mana.is_none_or(|m| m >= line("health.rest_mana_until"))),
            flee: hp.is_some_and(|h| h < self.bot.flee_below()),
            until: line("health.rest_until_hp"),
            short: c.hp.zip(c.hp_max).map(|(h, m)| {
                (i64::from(m) * i64::from(line("health.rest_until_hp")) / 100 - i64::from(h)).max(0)
            }),
            hp_rate: regen.hp,
            mana_wait: if uses_mana {
                c.mana.zip(c.mana_max).and_then(|(m, max)| {
                    let short = i64::from(max) * i64::from(line("health.rest_mana_until")) / 100
                        - i64::from(m);
                    crate::regen::wait_secs(short, regen.mana?)
                })
            } else {
                Some(0)
            },
        }
    }

    /// How fast the character gets HP and mana back, from its level and stats.
    fn regen(&self) -> crate::regen::Regen {
        let c = self.character.state();
        let level = c.level.map_or(1, i64::from);
        let stats = c.stats.clone().unwrap_or_default();
        let mana = self
            .atlas
            .as_ref()
            .zip(c.class.as_deref())
            .and_then(|(a, class)| {
                let &(magery, magery_level) = a.class_magery.get(class)?;
                crate::regen::mana_per_hour(magery, magery_level, level, &stats)
            });
        crate::regen::Regen {
            hp: stats
                .health
                .map(|h| crate::regen::hp_per_hour(level, i64::from(h))),
            mana,
        }
    }

    /// The party's leader runs now, for a member under its flee line.
    pub(crate) fn party_flee(&mut self, why: &str) {
        if self.bot.running() {
            self.bot.party_flee(why);
        }
    }

    /// Send one of the coordinator's lines (`invite`, `follow`, `par`, `leave`) as the bot's
    /// own, between the bot's actions: whether it went to the executor (else it is planned
    /// again next tick).
    pub(crate) fn party_line(&mut self, line: &str) -> bool {
        let free = self.state == SessionState::Realm
            && self.live()
            && !self.leaving
            && self.at_prompt
            && !self.executor.busy()
            && !self.walker.walking()
            && self.resync == Resync::Off;
        if !free {
            return false;
        }
        match self.executor.submit(Action::new(line, Expect::Prompt)) {
            Ok(()) => {
                self.owner = Some(Owner::Party);
                true
            }
            Err(_) => false,
        }
    }

    /// The coordinator's role for this session. Made a follower, it stops the loop it ran:
    /// it goes where the party goes.
    pub(crate) fn set_party_role(&mut self, role: PartyRole, out: &mut Vec<ToGateway>) {
        // A walk back that found no way is tried again later, or when the leader moves.
        let now = self.clock.instant();
        if self.party_stuck.as_ref().is_some_and(|(to, _, at)| {
            role.rejoin().is_some_and(|r| r != *to) || now >= *at + STUCK_RETRY
        }) {
            self.party_stuck = None;
        }
        // Holding: a leg of its loop already walking stops where it is.
        if let Some(why) = role.hold()
            && self.bot_walk
            && self.bot.task_walking()
        {
            let why = why.to_owned();
            self.stop_walk(&why, out);
        }
        if role == self.party_role {
            return;
        }
        if let Some(leader) = role.following()
            && self.party_role.following().is_none()
        {
            let says = self.bot.join_party(leader);
            self.say(says, out);
        }
        self.party_role = role;
    }

    /// The party's leader waited too long for a member: its loop stops, it fights on where it
    /// is, and the player is told.
    pub(crate) fn party_give_up(&mut self, why: &str, out: &mut Vec<ToGateway>) {
        let says = self.bot.stop_loop(why);
        self.say(says, out);
    }

    /// `map_request`.
    pub fn map_request(&mut self, radius: Option<u32>, out: &mut Vec<ToGateway>) {
        if let Some(r) = radius {
            self.map_radius = r;
        }
        if self.atlas.is_none() {
            out.push(self.warning("map: the brain has no world pack"));
        } else if self.position.best().is_none() {
            out.push(self.warning("map: the position is unknown"));
        } else {
            self.send_map(out);
        }
    }

    pub(crate) fn warning(&self, text: &str) -> ToGateway {
        ToGateway::Warning {
            session: Some(self.id.clone()),
            text: text.to_owned(),
        }
    }

    fn notice(&self, text: &str) -> ToGateway {
        ToGateway::Notice {
            session: Some(self.id.clone()),
            text: text.to_owned(),
        }
    }

    /// Do what the walker asked: narrate, and submit its next action; a loop walk goes on.
    fn apply(&mut self, next: Next, out: &mut Vec<ToGateway>) {
        let arrived = next.arrived;
        self.say(next.says, out);
        self.map_dirty |= next.route_changed;
        let ahead = next.ahead;
        if let Some(action) = next.action {
            match self.executor.submit(action) {
                Ok(()) => self.owner = Some(Owner::Walker),
                Err(guard) => {
                    out.push(self.warning(&format!("refused: {}", guard.describe())));
                    let next = self.walker.abandon(guard.describe());
                    self.apply(next, out);
                    return;
                }
            }
        }
        if let Some(action) = ahead {
            self.pipeline(action);
        }
        if self.bot_walk
            && let Some(at) = arrived
        {
            self.bot.walk_arrived(at);
        }
    }

    /// Move the executor on: hand a finished action's outcome to the walker, and send the next
    /// action if this is a prompt boundary in the realm with nothing else in flight.
    /// The warning for settings files that could not be used, once.
    pub fn take_settings_warning(&mut self) -> Option<String> {
        self.settings_warning.take()
    }

    /// How many re-sync commands this character sends: `spells` only if its class casts.
    fn resync_len(&self) -> usize {
        let casts = match (&self.atlas, &self.character.state().class) {
            (Some(atlas), Some(class)) => atlas.spells.casts(class),
            _ => false,
        };
        if casts {
            RESYNC_COMMANDS.len()
        } else {
            RESYNC_BASE
        }
    }

    /// The `spells` screen, row by row: the attacks and heals among them are the spellbook.
    fn spell_event(&mut self, ev: &GameEvent) {
        match ev {
            GameEvent::SpellsHeader | GameEvent::SpellsNone => {
                self.spell_rows.clear();
                self.spells_seen = true;
            }
            GameEvent::SpellRow {
                level,
                mana,
                short,
                name,
            } => self
                .spell_rows
                .push((*level, *mana, short.clone(), name.clone())),
            _ => return,
        }
        if let (Some(atlas), Some(class)) = (&self.atlas, &self.character.state().class) {
            self.spellbook = Spellbook::learn(&atlas.spells, class, &self.spell_rows);
        }
        self.refresh_blessings();
    }

    /// The lasting spells the player ticked for the character (C-047), by short name.
    pub(crate) fn set_blessings(&mut self, chosen: Vec<String>) {
        self.blessings = chosen;
        self.refresh_blessings();
    }

    /// The lasting spells the character knows (its `spells` screen) that it can keep on
    /// itself or its party.
    fn known_buffs(&self) -> Vec<wayfarer_world::BuffDef> {
        let (Some(atlas), Some(class)) = (&self.atlas, &self.character.state().class) else {
            return Vec::new();
        };
        self.spell_rows
            .iter()
            .filter_map(|(_, _, short, name)| atlas.spells.buff(class, short, name))
            .cloned()
            .collect()
    }

    /// The bot keeps on the ticked spells the character knows.
    fn refresh_blessings(&mut self) {
        let kept = self
            .known_buffs()
            .into_iter()
            .filter(|b| self.blessings.contains(&b.short))
            .collect();
        self.bot.set_blessings(kept);
    }

    /// The character's lasting spells for the Blessings panel.
    pub(crate) fn blessings_status(&self) -> wayfarer_proto::Blessings {
        let now = self.clock.instant();
        wayfarer_proto::Blessings {
            session: self.id.clone(),
            spells: self
                .known_buffs()
                .into_iter()
                .map(|b| wayfarer_proto::BlessingSpell {
                    chosen: self.blessings.contains(&b.short),
                    on: self.bot.blessed(&b.short, now),
                    mana: u32::try_from(b.mana).unwrap_or(0),
                    short: b.short,
                    name: b.name,
                })
                .collect(),
        }
    }

    /// The class the status screen showed is not the one the settings were resolved for: do it
    /// again, so the class presets (a Mage flees earlier than a Warrior) apply, and say what
    /// the HP lines are now. Fixed settings (a test's) are left as they are.
    fn refresh_settings(&mut self, out: &mut Vec<ToGateway>) {
        let class = self.character.state().class.clone();
        if class == self.class_seen {
            return;
        }
        self.class_seen = class.clone();
        let Some(name) = class.filter(|_| self.settings_source.reads_files()) else {
            return;
        };
        let before = self.hp_lines();
        let (settings, warning) = self.settings_source.resolve(&self.id, Some(&name));
        if let Some(why) = warning {
            out.push(self.warning(&why));
            return;
        }
        self.apply_settings(settings);
        self.settings_stamp = self.settings_source.stamp(&self.id);
        if self.hp_lines() != before {
            let (flee, below, until) = self.hp_lines();
            out.push(self.notice(&format!(
                "Settings for a {name}: flee below {flee}%, rest below {below}% to {until}%"
            )));
        }
    }

    /// Put `settings` in force: the walker and the flee line take them at once, the modes read
    /// the rest at their next decision.
    fn apply_settings(&mut self, settings: Arc<Settings>) {
        let flee = self.settings_source.flee_below(&settings);
        self.walker.set_search_limit(
            u32::try_from(settings.int("movement.search_limit")).unwrap_or(MAX_SEARCHES),
        );
        self.bot.set_flee_below(flee);
        self.settings = settings;
    }

    /// The settings files were edited: read them again and put them in force, saying what
    /// changed; a file that is not valid leaves the settings in force as they are, with a
    /// warning. Looked at every [`SETTINGS_POLL`].
    fn check_settings(&mut self, now: Instant, out: &mut Vec<ToGateway>) -> bool {
        if !self.settings_source.reads_files() || self.settings_check.is_some_and(|t| now < t) {
            return false;
        }
        let first = self.settings_check.is_none();
        self.settings_check = Some(now + SETTINGS_POLL);
        let stamp = self.settings_source.stamp(&self.id);
        // The first look only notes the files as they are (the starter files may just have been
        // written).
        if first || stamp == self.settings_stamp {
            self.settings_stamp = stamp;
            return false;
        }
        self.settings_stamp = stamp;
        let class = self.character.state().class.clone();
        match self.settings_source.reload(&self.id, class.as_deref()) {
            Err(why) => {
                out.push(self.warning(&format!(
                    "Settings changed but not used, the earlier settings stay: {why}"
                )));
                false
            }
            Ok(new) => {
                let changes: Vec<String> = self
                    .settings
                    .rows()
                    .zip(new.rows())
                    .filter(|((_, old, _), (_, now, _))| old != now)
                    .map(|((s, old, _), (_, now, _))| format!("{} {old} → {now}", s.key))
                    .collect();
                self.apply_settings(new);
                let text = if changes.is_empty() {
                    "Settings reloaded: no change".to_owned()
                } else {
                    format!("Settings reloaded: {}", changes.join(", "))
                };
                out.push(self.notice(&text));
                // The forms follow a change made in the file by hand too.
                self.send_settings(self.settings_layer, out);
                true
            }
        }
    }

    /// `settings_request`: the character's settings and the values `layer`'s file sets.
    pub fn settings_request(&mut self, layer: Option<&str>, out: &mut Vec<ToGateway>) {
        let Some(layer) = self.edited_layer(layer, out) else {
            return;
        };
        self.settings_layer = layer;
        self.send_settings(layer, out);
    }

    /// `settings_save`: change the file of `layer`. A change the settings refuse writes nothing
    /// and says why; otherwise the file is written and put in force at once.
    pub fn settings_save(
        &mut self,
        layer: &str,
        changes: &[wayfarer_proto::SettingChange],
        now: Instant,
        out: &mut Vec<ToGateway>,
    ) {
        let Some(layer) = self.edited_layer(Some(layer), out) else {
            return;
        };
        self.settings_layer = layer;
        let class = self.character.state().class.clone();
        match self
            .settings_source
            .save(&self.id, class.as_deref(), layer, changes)
        {
            Err(why) => out.push(self.warning(&format!("Settings not saved: {why}"))),
            Ok(()) => {
                // Read again now, not at the next look at the files; that sends the answer.
                self.settings_check = Some(now);
                if self.check_settings(now, out) {
                    return;
                }
            }
        }
        self.send_settings(layer, out);
    }

    /// The layer a settings message names (the character's by default); only the player's own
    /// files can be edited.
    fn edited_layer(&self, layer: Option<&str>, out: &mut Vec<ToGateway>) -> Option<Layer> {
        let layer = match layer.unwrap_or("character") {
            "mine" => Layer::Mine,
            "server" => Layer::Server,
            "character" => Layer::Character,
            other => {
                out.push(self.warning(&format!(
                    "settings: no layer {other:?} (mine, server or character)"
                )));
                return None;
            }
        };
        Some(layer)
    }

    fn send_settings(&self, layer: Layer, out: &mut Vec<ToGateway>) {
        out.push(ToGateway::Settings {
            session: self.id.clone(),
            layer: layer.id().to_owned(),
            files: self.settings_source.files(&self.id),
            rows: self.settings_source.rows(&self.settings, &self.id, layer),
        });
    }

    /// The HP lines in force: flee below, rest below, rest until.
    fn hp_lines(&self) -> (i64, i64, i64) {
        (
            self.settings.int("health.flee_below_hp"),
            self.settings.int("health.rest_below_hp"),
            self.settings.int("health.rest_until_hp"),
        )
    }

    fn drive(&mut self, now: Instant, out: &mut Vec<ToGateway>) {
        self.refresh_settings(out);
        self.check_hangup(out);
        self.lie_still(out);
        if let Some((action, outcome)) = self.executor.finished(self.at_prompt, now)
            && let Some(atlas) = self.atlas.clone()
        {
            tracing::debug!(session = %self.id, cmd = %action.command, ?outcome, "action done");
            match self.owner.take() {
                Some(Owner::Errand) => {
                    if let Some(errand) = &mut self.errand {
                        errand.outcome(&action, outcome);
                    }
                }
                // The coordinator reads the answer from the game's lines (`GameParty`).
                Some(Owner::Party) => {}
                Some(Owner::Bot) => {
                    let says = self.bot.outcome(&action, outcome);
                    self.say(says, out);
                    if let Some((to, short)) = self.walk_after.take() {
                        self.position.refresh(
                            &atlas,
                            &self.character.snapshot(),
                            self.clock.instant(),
                        );
                        self.start_walk(&atlas, to, short, out);
                    }
                }
                _ => {
                    if self.executor.busy() {
                        // The move that went ahead is in flight now: the walker's still.
                        self.owner = Some(Owner::Walker);
                    }
                    self.position
                        .refresh(&atlas, &self.character.snapshot(), self.clock.instant());
                    let seen = self.executor.take_belief();
                    let next = self.walker.outcome(
                        &mut self.position,
                        &atlas,
                        &action,
                        outcome,
                        seen.as_ref(),
                    );
                    let arrived = next.arrived;
                    self.apply(next, out);
                    // The walker gave the bot's walk up by itself (lost, a door that will not
                    // open): a journey ends there.
                    if self.bot_walk
                        && !self.walker.walking()
                        && arrived.is_none()
                        && !self.walk_halted
                    {
                        self.bot.walk_failed();
                    }
                    // A leg of Task's stops where Fight would take the character.
                    if self.bot_walk
                        && self.bot.task_walking()
                        && self.walker.walking()
                        && self
                            .bot
                            .would_fight(&atlas, self.rules.rules(), &self.presence)
                    {
                        self.stop_walk("a monster to fight", out);
                    }
                    self.bot.sneak_arrived(self.position.best());
                    if self.bot_walk
                        && self.walker.walking()
                        && self.bot.sneak_broken(&self.settings)
                    {
                        self.stop_walk("the sneak failed", out);
                    }
                    // A pause asked for while the walker's action was in flight.
                    let says = self.bot.settle();
                    self.say(says, out);
                }
            }
        }
        if self.bot_walk && !self.walker.walking() {
            self.bot_walk = false;
            self.bot.forget();
        }
        self.bot_tick(now, out);
        self.errand_tick(out);
        self.flee_out_done(out);
        // The player's line first: the bot's next action waits for the prompt after it.
        if self.send_typed(now, out) {
            return;
        }
        if self.state == SessionState::Realm
            && self.live()
            && !self.leaving
            && self.at_prompt
            && !matches!(self.resync, Resync::Waiting { .. })
            && let Some(cmd) = self.executor.take_ready(now)
        {
            self.send_own(&cmd, now, out);
        }
        // A move pipelined ahead goes at once behind the one in flight, prompt or not.
        if self.state == SessionState::Realm
            && self.live()
            && !self.leaving
            && !matches!(self.resync, Resync::Waiting { .. })
            && let Some(cmd) = self.executor.take_ahead(now)
        {
            self.send_own_ahead(&cmd, now, out);
        }
    }

    /// The places the character died, fled and stopped in, with the rooms' names.
    fn places(&self) -> Vec<Place> {
        let name = |k: Key| {
            self.atlas
                .as_ref()
                .and_then(|a| a.graph.room(k))
                .map_or_else(String::new, |r| r.name.clone())
        };
        self.ledger
            .places()
            .iter()
            .map(|p| Place {
                at: rfc3339_at(p.at),
                kind: p.kind.name().to_owned(),
                map: p.room.0,
                room: p.room.1,
                name: name(p.room),
                detail: p.detail.clone(),
            })
            .collect()
    }

    /// The character with the session's position and walk.
    pub fn snapshot(&self) -> CharacterState {
        let mut s = self.character.snapshot();
        if self.atlas.is_some() {
            s.position = Some(self.position.snapshot());
        }
        s.walking = self.walker.snapshot();
        if self.atlas.is_some() {
            let mut bot = self.bot.snapshot();
            bot.monster_rules = self.rules.wire();
            s.bot = Some(bot);
            s.run = self.ledger.snapshot();
            s.places = Some(self.places());
            s.errand = self.errand.as_ref().map(Active::narration);
        }
        s
    }

    /// Send what changed since the UIs last heard: a patch, and a `map_view` if the believed
    /// room or the route changed.
    fn flush_views(&mut self, out: &mut Vec<ToGateway>) {
        self.publish_patch(out);
        if self.published.is_some() && (self.map_dirty || self.map_center != self.position.best()) {
            self.send_map(out);
        }
    }

    /// A `map_view` now, if there is a world pack and a position.
    pub fn send_map(&mut self, out: &mut Vec<ToGateway>) {
        let Some(atlas) = &self.atlas else { return };
        self.map_dirty = false;
        self.map_center = self.position.best();
        let route = self.walker.route_keys();
        if let Some(view) = self
            .position
            .map_view(atlas, &self.id, self.map_radius, route)
        {
            out.push(view);
        }
    }

    fn publish_patch(&mut self, out: &mut Vec<ToGateway>) {
        let Some(old) = &self.published else { return };
        let new = serde_json::to_value(self.snapshot()).expect("serializes");
        if let Some(patch) = merge_patch(old, &new) {
            out.push(ToGateway::CharacterPatch {
                session: self.id.clone(),
                patch,
            });
            self.published = Some(new);
        }
    }

    /// The whole snapshot, which later patches are relative to.
    pub fn publish(&mut self, out: &mut Vec<ToGateway>) {
        let state = self.snapshot();
        self.published = Some(serde_json::to_value(&state).expect("serializes"));
        out.push(ToGateway::Character {
            session: self.id.clone(),
            state: Box::new(state),
        });
    }

    /// The brain's lines still waiting for the gateway's `sent`.
    pub fn own_pending(&self) -> usize {
        self.own.len()
    }

    /// Whether the session is caught up and published (it may then be sent to).
    fn live(&self) -> bool {
        !self.fetching && self.published.is_some()
    }

    /// When [`Session::step`] next has something to do, if a timer must wake it.
    pub fn deadline(&self) -> Option<Instant> {
        if self.state != SessionState::Realm || !self.live() || self.leaving {
            return None;
        }
        let resync = match self.resync {
            // The executor's action goes first; the re-sync waits for it.
            Resync::Want { idle: true, .. } if self.at_prompt && !self.executor.busy() => {
                Some(self.last_sent + IDLE)
            }
            Resync::Waiting { deadline, .. } => Some(deadline),
            _ => None,
        };
        // The bot decides again when the game has been quiet long enough for a fight to have
        // lapsed, or when a wait of Recover's ends.
        let now = self.clock.instant();
        let lapse = (!self.down).then_some(self.last_combat + COMBAT_QUIET);
        let bot = [lapse, self.bot.wake(now), self.sightings.next_expiry()]
            .into_iter()
            .flatten()
            .filter(|&t| t > now && self.bot.running() && !self.executor.busy());
        // A typed line waiting for a prompt goes anyway once it has waited long enough.
        let typed = self
            .typed
            .front()
            .map(|line| line.at + TYPED_PATIENCE)
            .filter(|_| !self.at_prompt);
        // The snapshot's `in_combat` goes false when the fight lapses.
        let engaged = lapse.filter(|&t| t > now && self.character.state().in_combat == Some(true));
        resync
            .into_iter()
            .chain(self.executor.deadline())
            .chain(bot)
            .chain(engaged)
            .chain(typed)
            .chain(self.ledger.deadline())
            // An errand waits for a walk, a re-sync or the prompt: ask again soon, event or not.
            .chain(self.errand.as_ref().map(|_| now + ERRAND_POLL))
            .chain(
                self.settings_source
                    .reads_files()
                    .then(|| self.settings_check.unwrap_or(now)),
            )
            .min()
    }

    /// Move the executor and the re-sync on: an action whose time ran out, the next re-sync
    /// command if its moment has come, an answer that has not come.
    pub fn step(&mut self, now: Instant, out: &mut Vec<ToGateway>) -> Progress {
        if self.state != SessionState::Realm || !self.live() || self.leaving {
            return Progress::None;
        }
        if self.character.state().in_combat == Some(true) && self.quiet(now) {
            tracing::debug!(session = %self.id, "the fight lapsed: quiet for {COMBAT_QUIET:?}");
            self.character.combat_lapsed();
        }
        if self.sightings.expire(now) {
            self.position.traveller.avoid = self.sightings.rooms(now);
        }
        let c = self.character.state();
        let (exp, hp) = (
            c.exp.and_then(|e| i64::try_from(e).ok()),
            c.hp.map(i64::from),
        );
        self.ledger.sample(exp, hp, now);
        self.check_settings(now, out);
        if let Resync::Waiting {
            step,
            deadline,
            typed,
            ..
        } = self.resync
            && now >= deadline
        {
            tracing::debug!(session = %self.id, cmd = RESYNC_COMMANDS[step], "no answer; going on");
            self.resync = Resync::Want {
                next: step + 1,
                idle: typed,
            };
        }
        self.drive(now, out);
        self.flush_views(out);
        if self.executor.busy() {
            // One line in flight: the re-sync waits for the executor.
            return Progress::None;
        }
        match self.resync {
            Resync::Want { next, .. } if next >= self.resync_len() => {
                self.resync = Resync::Off;
                self.synced = true;
                // Every answer has come, so every `sent` before it has: nothing is pending.
                self.own.clear();
                // The bot waits for the re-sync: it decides now.
                self.drive(now, out);
                Progress::Done
            }
            Resync::Want { next, idle }
                if self.at_prompt && (!idle || now >= self.last_sent + IDLE) =>
            {
                self.send_own(RESYNC_COMMANDS[next], now, out);
                self.resync = Resync::Waiting {
                    step: next,
                    deadline: now + ANSWER_TIMEOUT,
                    answered: false,
                    typed: false,
                };
                Progress::None
            }
            _ => Progress::None,
        }
    }
}

pub(crate) fn event_message(session: &str, ev: &Event) -> ToGateway {
    let fields = match serde_json::to_value(&ev.fields).expect("serializes") {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    ToGateway::Event {
        session: session.to_owned(),
        seq: ev.seq,
        line_no: ev.line_no,
        name: ev.name.clone(),
        cat: ev.cat.clone(),
        fields,
        line: ev.line.clone(),
    }
}

/// The currency number of a coin by the game's name (`gold crowns`): 0 copper to 4 runic.
fn coin_currency(coin: &str) -> i64 {
    match crate::loot::coin_word(coin) {
        "silver" => 1,
        "gold" => 2,
        "platinum" => 3,
        "runic" => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_skips_escapes_and_space() {
        assert!(!visible("\x1b[0;37m\r\n \x1b[K"));
        assert!(visible("\x1b[1;32mYou"));
    }
}
