//! Wire messages between the gateway, the brain and the UIs (docs/plan/02-architecture.md,
//! "The wire protocol"). No I/O.
//!
//! Every message is a JSON object whose `"type"` field names it. The gateway sends
//! [`ToClient`] and receives [`ToGateway`]. `docs/protocol.md` lists them with an example
//! each; the TypeScript for the web UI is generated into `ui/src/proto/` ([`export_typescript`]).
//!
//! M0 has the hello, session, terminal and input families; M1 adds the brain's: events, state
//! and dev (brain → UIs), control (UI → brain) and `sent` (gateway → brain); M2 adds walking
//! and the map (`walk_to`, `walk_stop`, `map_request`, `map_view`, and the character's
//! `position` and `walking`), the bot's first slice (`bot_control`, the character's `bot`), and loops (`loops_request`,
//! `loops`, `walk_loop`); M3 adds Task mode (`run_loop`, the bot's `task`), the log-off handshake (`logging_off`, `ready_to_leave`) and
//! statistics (the character's `run`, `stats_request`, `stats`), and typing beside the bot
//! (`user_input`, `keys_sent`, `arbitrate`), and go to (`find`, `found`,
//! `walk_to`'s `stop_short`, `walking`'s `short_of`). [`Family`] says who may send what.
//! Adding a variant later bumps nothing; removing or changing one bumps [`PROTOCOL`].

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The protocol version both sides send in the handshake. A mismatch is refused.
pub const PROTOCOL: u32 = 2;

/// A session's id: a short string from the gateway's config (the character key, e.g.
/// `"kasimir"`), stable across restarts.
pub type SessionId = String;

/// What kind of client is attaching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Ui,
    Brain,
}

/// A session's connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Opening the connection to the game server.
    Connecting,
    /// Connected, with no logon script running: none is configured, or it stopped and the
    /// player has the session (character creation, say).
    Connected,
    /// Connected; the logon script is running.
    Logon,
    /// At the game's menus, outside the realm.
    Menu,
    /// In the realm.
    Realm,
    /// The connection dropped without a clean log-off.
    Lost,
    /// Logged off and disconnected on purpose.
    Closed,
}

/// What a UI needs to draw a session's tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SessionInfo {
    pub id: SessionId,
    /// The character's display name.
    pub name: String,
    /// The game server, as configured (e.g. `"bbs.example.org:23"`).
    pub server: String,
    pub state: SessionState,
    /// The `seq` of the latest output chunk, 0 before any output.
    pub last_seq: u64,
}

/// One piece of terminal output. `seq` starts at 1 per session and never resets for the life
/// of the session's log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Chunk {
    pub seq: u64,
    /// UTF-8 with ANSI escapes kept (CP437 already decoded by the gateway).
    pub text: String,
}

/// What the brain knows about one character: the wire's `character` snapshot, and the shape a
/// `character_patch` (a JSON merge patch, RFC 7386) applies to. Every field is left out until
/// it is known; in a patch, `null` clears one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct CharacterState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// Hit points; below zero while dying.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp_max: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana_max: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>,
    /// Experience still needed for the next level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp_next: Option<u64>,
    /// How far through the level, as the game shows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp_percent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lives: Option<u32>,
    /// Unspent character points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cp: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<Stats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_exits: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_combat: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resting: Option<bool>,
    /// `poisoned`, `blind`, `held`, `diseased`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statuses: Option<Vec<String>>,
    /// Which form the character's last prompt had (`hp`, `hp_mana`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse: Option<ParseCounts>,
    /// When the last re-sync finished (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
    /// The output `seq` this snapshot reflects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_seq: Option<u64>,
    /// The coins carried, from the inventory (`i`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coins: Option<Coins>,
    /// The items carried, by name, as the inventory lists them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrying: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encumbrance: Option<Encumbrance>,
    /// Monster deaths seen since the brain attached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kills: Option<u64>,
    /// Where the brain believes the character is (M2; absent without a world pack).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    /// The walk in progress; absent when not walking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walking: Option<Walking>,
    /// The bot (M2's "fight here"); absent without a world pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<Bot>,
    /// The bot's current run, from Start to Stop or a death; absent when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunStats>,
    /// Where the character died, fled from and stopped, newest first (at most ten), whether
    /// or not the bot was running; absent without a world pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub places: Option<Vec<Place>>,
    /// What the errand in progress is doing (`pick_up_all`, `equip_all`); absent when there is
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errand: Option<String>,
}

/// A room the character is remembered to have been in, and why (`CharacterState::places`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Place {
    /// RFC 3339.
    pub at: String,
    /// `death` (its gear is in the room), `flee` or `stopped`.
    pub kind: String,
    pub map: i64,
    pub room: i64,
    /// The room's name; empty when the world pack does not know it.
    pub name: String,
    /// What happened: who it fled from, why the bot stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

/// A run's numbers so far (`docs/plan/03-engine.md` section 2): recomputed on each event and
/// every minute.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RunStats {
    /// When Start was pressed (RFC 3339).
    pub run_started_at: String,
    /// Exp gained over the wall time since Start (a minute at least).
    pub exp_per_hour: u64,
    pub kills: u64,
    pub deaths: u64,
    pub flees: u64,
    pub rests: u64,
    pub exp_gained: i64,
}

/// One of the last runs, in a `stats` message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RunSummary {
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339; absent while the run goes on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ended_at: Option<String>,
    /// The loop it ran (Task mode); absent for "fight here".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub loop_id: Option<String>,
    /// Whole minutes from start to end (to now, while it goes on).
    pub minutes: u64,
    pub exp_per_hour: u64,
    pub kills: u64,
    pub deaths: u64,
    pub flees: u64,
    pub rests: u64,
    pub exp_gained: i64,
}

/// The character's exp and HP at a moment of a run (every minute).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ExpSample {
    /// RFC 3339.
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hp: Option<i32>,
}

/// Whether the bot is playing a character (`docs/plan/03-engine.md` section 5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BotState {
    /// Sends nothing; the player plays. The default.
    #[default]
    Stopped,
    Running,
    /// Sends nothing until resumed: the player took over, or asked for a pause.
    Paused,
}

/// The mode in control (`docs/plan/03-engine.md` section 4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BotMode {
    #[default]
    Idle,
    Fight,
    Recover,
    Flee,
    /// Picking up coins left in the room.
    Loot,
    /// Running a loop (`run_loop`): walking to its next stop, or waiting there for respawns.
    Task,
}

/// What the bot is doing for a character.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Bot {
    pub state: BotState,
    pub mode: BotMode,
    /// One line on what it is doing and why ("Fight: large giant rat").
    pub narration: String,
    /// Why it stopped or paused, when it did so by itself or for the player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
    /// The loop it runs (`run_loop`), while it runs one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub task: Option<Task>,
    /// The monster rules it plays by: the defaults, what this session learned (a stalemate)
    /// and what the player set (`set_monster_rule`). The longest pattern a name holds wins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<MonsterRuleEntry>>", optional)]
    pub monster_rules: Vec<MonsterRuleEntry>,
}

/// What the bot does about a monster, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MonsterRuleKind {
    /// Never attack it; fight back if it attacks.
    NeverFight,
    /// Leave the room as soon as it is seen, whatever the HP.
    FleeFrom,
    /// Keep out of the room it was last seen in when there is another way.
    Avoid,
}

/// Where a monster rule comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RuleSource {
    /// wayfarer's own list.
    Default,
    /// The bot learned it this session: a fight that was a stalemate.
    Session,
    /// The player set it (`set_monster_rule`, or the brain's `--flee-from`).
    Player,
}

/// One monster rule: a pattern matches a name that holds it, in any case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MonsterRuleEntry {
    pub pattern: String,
    pub rule: MonsterRuleKind,
    pub source: RuleSource,
}

/// The loop the bot runs, and where it is on it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Task {
    /// The loop's id, as `loops` listed it.
    pub loop_id: String,
    pub loop_name: String,
    /// The stop it is at or walking to, from 0, an index into `stops`.
    pub stop: u32,
    /// The loop's stops, in walking order.
    pub stops: Vec<RoomKey>,
    /// The stops it passes by (unticked).
    #[serde(default)]
    pub skip: Vec<RoomKey>,
}

/// A `bot_control` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BotAction {
    /// Start playing in the current room.
    Start,
    /// Stop: the player plays.
    Stop,
    /// Hold after the action in flight.
    Pause,
    /// Re-sync, then carry on.
    Resume,
    /// An errand: pick up the coins and items in the room (with the bot stopped or paused).
    PickUpAll,
    /// An errand: wear and wield what is carried, into empty slots only.
    EquipAll,
    /// End the errand in progress.
    CancelErrand,
}

/// A room of the world pack: its map and room number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
pub struct RoomKey {
    pub map: i64,
    pub room: i64,
}

/// The position belief: the room the brain takes the character to be in (the lowest key among
/// the candidates), how many rooms it might be, and the last room title seen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct Position {
    /// Absent when the position is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<i64>,
    /// How many rooms fit what was seen: 1 is certain, 0 unknown.
    pub candidates: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// A walk the brain is making (`walk_to`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Walking {
    pub to: RoomKey,
    /// The destination's name.
    pub name: String,
    /// Steps of the route not yet confirmed, the one in progress included.
    pub steps_left: u32,
    /// The step in progress, from 1, counted over the whole walk (replans included).
    pub step: u32,
    /// What the walker will type for each step left, the one in progress first: `n`, or a
    /// text exit's phrase (`borrow skiff`). A closed door adds an `open` it does not list.
    #[serde(default)]
    pub commands: Vec<String>,
    /// The walk stops one room short of this boss lair (`walk_to`'s `stop_short`): `to` is the
    /// room before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub short_of: Option<BossLair>,
}

/// A boss lair: a room whose spawn pool holds a monster that spawns one at a time, or that a
/// MegaMud waypoint flags boss.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BossLair {
    pub map: i64,
    pub room: i64,
    pub name: String,
    /// Its bosses, by name (none when only a waypoint says so).
    pub bosses: Vec<String>,
}

/// What a `found` result names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FoundKind {
    /// Every room with this exact name.
    Rooms,
    /// Well-known places: a shop, bank or trainer by its room name, or a MegaMud waypoint
    /// (a boss's, or another).
    Shop,
    Bank,
    Trainer,
    Boss,
    Place,
    /// A monster, with the rooms it spawns in.
    Monster,
}

/// A room of a `found` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct FoundRoom {
    pub map: i64,
    pub room: i64,
    pub name: String,
    /// Route length from the character, when within 200 steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub steps: Option<u32>,
    /// No way there from the character's room. Neither this nor `steps`: far, or the
    /// character's room is unknown.
    #[serde(default, skip_serializing_if = "is_false")]
    #[ts(as = "Option<bool>", optional)]
    pub no_route: bool,
    /// A boss lair: `walk_to` stops one room short of it by default.
    pub boss: bool,
}

/// One result of a `find`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Found {
    pub kind: FoundKind,
    pub name: String,
    /// The nearest of its rooms, at most 20.
    pub rooms: Vec<FoundRoom>,
    /// How many rooms it names in all.
    pub count: u32,
    /// The bosses of its boss lairs (for a monster, itself if it is one), by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub monsters: Option<Vec<String>>,
}

/// What kind of place a room is, for the map panel to mark: from the room's type and shop,
/// and from the MegaMud waypoint resolved to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
    Shop,
    Trainer,
    Bank,
    Arena,
    Lair,
}

/// One room of a `map_view`. `x` grows east, `y` south, `level` up; the center is (0, 0, 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MapRoom {
    pub map: i64,
    pub room: i64,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub level: i32,
    /// Monsters spawn here.
    pub spawns: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind: Option<RoomKind>,
    /// The character has been here (certainly) since the brain started.
    pub visited: bool,
    /// One of the rooms the character may be in now.
    pub candidate: bool,
    /// Its cell was taken by another room: drawn there, nothing placed beyond it.
    pub collides: bool,
}

/// How an exit of a `map_view` is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExitKind {
    Plain,
    Door,
    Hidden,
    /// Taken by typing a phrase (`borrow skiff`).
    Text,
}

/// An exit between two rooms of a `map_view`. A two-way pair is listed once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MapExit {
    pub from: RoomKey,
    pub to: RoomKey,
    /// `n`, `s`, ..., `u`, `d`.
    pub dir: String,
    pub kind: ExitKind,
    /// The far room has no exit back.
    pub one_way: bool,
}

/// A stop of a generated loop: a room that spawns monsters worth hunting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LoopStop {
    pub map: i64,
    pub room: i64,
    pub name: String,
    /// The huntable monsters it spawns, by name.
    pub monsters: Vec<String>,
    /// An Arena: monsters come back with no wait after a kill.
    pub arena: bool,
    pub lair: bool,
    /// Minutes nothing spawns there after a kill (arenas exempt).
    pub regen: i64,
    /// The bosses that spawn there and whether each should be up now, from the character's own
    /// kills; empty when none does. Filled when the `loops` message is built, not cached.
    #[serde(default)]
    pub bosses: Vec<BossStatus>,
}

/// A boss of a stop, and whether it should be up: the character's last kill of it plus the
/// hours it takes to come back. A guess, not a sight: other players kill it too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BossStatus {
    pub name: String,
    /// Hours it takes to come back after a kill.
    pub regen_hours: u32,
    /// Minutes until it should be up; absent when it should be up now (the character never
    /// killed it, or its time has passed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub up_in_minutes: Option<u32>,
}

/// A loop generated from the world data: a closed circuit through an area's hunt rooms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct GeneratedLoop {
    /// What `walk_loop` names it by.
    pub id: String,
    pub name: String,
    /// The stops in walking order; the loop goes back to the first after the last.
    pub stops: Vec<LoopStop>,
    /// Every room it passes, from the first stop back to it.
    pub path: Vec<RoomKey>,
    pub steps: u32,
    /// Doors on the way (each opened, or bashed when locked).
    pub doors: u32,
    /// Steps from the character to the nearest room of the loop, when asked for from a
    /// position (`loops_request`); the list is in this order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub distance: Option<u32>,
}

/// One of MegaMud's loops through the area, only ever one whose verification walk reached the
/// end and closed in its start room. `walk_loop` and `run_loop` name it by its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct MegaMudLoop {
    /// The `.mp` file.
    pub file: String,
    pub description: String,
    pub author: String,
    /// The rooms its walk passed, from its start back to it.
    pub path: Vec<RoomKey>,
    /// Steps the walk checked, and of those how many matched MegaMud's room id exactly.
    pub steps: u32,
    pub exact: u32,
    /// The hunt rooms on its path, in path order, each once: what a task stops at.
    pub stops: Vec<LoopStop>,
    /// Whether `run_loop` runs it for this character; `reason` says why not ("nothing to hunt
    /// on it", or a leg between two stops no route takes).
    pub runnable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Steps from the character to the nearest room of the loop, as for [`GeneratedLoop`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<u32>,
}

/// A character's coins.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct Coins {
    /// Their total value in copper farthings (the inventory's `Wealth:` line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copper: Option<u64>,
    /// Each kind carried, as the inventory lists it (`silver nobles`, 3).
    #[serde(default)]
    pub carried: Vec<CoinCount>,
}

/// So many coins of one kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct CoinCount {
    /// As the game names it: `runic coins`, `platinum pieces`, `gold crowns`, `silver nobles`,
    /// `copper farthings`.
    pub coin: String,
    pub count: u64,
}

/// What a character carries against what it can (`Encumbrance: 520/3792 - None [13%]`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct Encumbrance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    /// The game's word for it: `None`, `Light`, `Medium`, `Heavy`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

/// A settings value on the wire: a whole number, a switch or a word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum SettingValue {
    Int(i64),
    Bool(bool),
    Text(String),
}

/// What a setting holds: a whole number in a range, a switch, or one of some words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingKind {
    Int { min: i64, max: i64 },
    Bool,
    Choice { options: Vec<String> },
}

/// One setting in a `settings` message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct SettingRow {
    /// `group.name`, as in the TOML file.
    pub key: String,
    pub label: String,
    pub help: String,
    /// The group's table name: `health`, `movement`, `bot`, `spells`, `cash`, `combat`.
    pub group: String,
    /// `essential`, `standard` or `advanced`.
    pub tier: String,
    pub kind: SettingKind,
    /// The unit shown beside a number (`%`, `tries`); empty for a switch or words.
    pub unit: String,
    pub default: SettingValue,
    /// The value in force.
    pub value: SettingValue,
    /// Where it comes from: `default`, `preset`, `mine`, `server` or `character`.
    pub source: String,
    /// The value the asked-for layer's own file sets, if it sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own: Option<SettingValue>,
    /// MegaMud's name for it, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub megamud: Option<String>,
}

/// One settings file in a `settings` message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SettingsFile {
    /// `mine`, `server` or `character`.
    pub layer: String,
    pub path: String,
    pub exists: bool,
}

/// One change in a `settings_save`: set `key` to `value`, or take it out of the file when
/// `value` is left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct SettingChange {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<SettingValue>,
}

/// A character's six stats.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct Stats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intellect: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub willpower: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agility: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charm: Option<u32>,
}

/// How the parser is doing on a character, counted since the brain attached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ParseCounts {
    pub lines: u64,
    /// Lines nothing recognised (`unknown` events).
    pub unclassified: u64,
}

/// The world pack a brain loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WorldPackInfo {
    pub path: String,
    pub hash: String,
}

/// Who may send a [`ToGateway`] message, and what the gateway does with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Hello, input, history, subscribe and session control: from any client, handled by the
    /// gateway.
    Gateway,
    /// From a UI, relayed to the brain: `resync`, `state_request`.
    Control,
    /// From the brain, relayed to the UIs: events, state and dev (`event`, `character`,
    /// `character_patch`, `brain_info`, `notice`, `warning`).
    Brain,
    /// From the brain, handled by the gateway: `ready_to_leave`, `arbitrate`.
    BrainToGateway,
}

impl Family {
    /// Whether a client of this kind may send messages of this family.
    pub fn allowed_from(self, client: ClientKind) -> bool {
        match self {
            Self::Gateway => true,
            Self::Control => client == ClientKind::Ui,
            Self::Brain | Self::BrainToGateway => client == ClientKind::Brain,
        }
    }
}

/// What a `session_control` asks of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SessionAction {
    /// Open the connection and log on (nothing if it is open).
    Connect,
    /// Log off cleanly (the brain is asked to get ready first) and disconnect: never
    /// reconnected.
    LogOff,
    /// Drop the connection at once, in the realm or not: the game may punish it.
    HangUp,
}

/// Gateway → client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToClient {
    /// The handshake succeeded.
    Welcome {
        protocol: u32,
        server_build: String,
        sessions: Vec<SessionInfo>,
    },
    /// The handshake failed; the gateway closes the connection after sending it.
    Refused {
        reason: String,
    },
    /// The full list of sessions, sent when it changes.
    SessionList {
        sessions: Vec<SessionInfo>,
    },
    /// A session's connection state changed.
    SessionState {
        session: SessionId,
        state: SessionState,
    },
    /// Live terminal output.
    Output {
        session: SessionId,
        seq: u64,
        text: String,
    },
    /// The answer to a `history_request`: contiguous chunks in ascending `seq`, at most
    /// `max_chunks`. `from_seq` is the `seq` of the first chunk; when `chunks` is empty it is
    /// where the range would have started.
    History {
        session: SessionId,
        from_seq: u64,
        chunks: Vec<Chunk>,
    },
    /// This connection fell behind and `missed` chunks of the session's live output were not
    /// sent to it. Live output carries on after the gap; fetch the gap with a
    /// `history_request` `after_seq` the last `seq` seen.
    Lagged {
        session: SessionId,
        missed: u64,
    },
    /// Something the client sent could not be handled; the connection stays up.
    Error {
        message: String,
    },

    // From the brain, relayed to the UIs subscribed to `session` (to every UI when there is
    // none). `ToGateway` says what each one means.
    Event {
        session: SessionId,
        seq: u64,
        line_no: u64,
        name: String,
        cat: String,
        #[ts(type = "Record<string, unknown>")]
        fields: serde_json::Map<String, serde_json::Value>,
        line: String,
    },
    Character {
        session: SessionId,
        state: Box<CharacterState>,
    },
    CharacterPatch {
        session: SessionId,
        #[ts(type = "unknown")]
        patch: serde_json::Value,
    },
    BrainInfo {
        build: String,
        started_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        world_pack: Option<WorldPackInfo>,
    },
    Notice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
    },
    Warning {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
    },
    MapView {
        session: SessionId,
        center: RoomKey,
        level: i32,
        rooms: Vec<MapRoom>,
        exits: Vec<MapExit>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        route: Option<Vec<RoomKey>>,
    },
    Loops {
        session: SessionId,
        anchor: RoomKey,
        radius: u32,
        doors: bool,
        generated: Vec<GeneratedLoop>,
        megamud: Vec<MegaMudLoop>,
        /// The loops the player made (`save_loop`), by name.
        #[serde(default)]
        custom: Vec<GeneratedLoop>,
        /// How many loops matched the request before it was cut to its `limit`.
        #[serde(default)]
        matches: u32,
    },
    Stats {
        session: SessionId,
        runs: Vec<RunSummary>,
        samples: Vec<ExpSample>,
    },
    /// Brain → UIs: the character's settings, answering `settings_request` and `settings_save`
    /// (and sent again when the files change): every setting with its effective value, where
    /// the value comes from, and the value the `layer` file itself holds.
    Settings {
        session: SessionId,
        /// The layer whose own values `own` shows.
        layer: String,
        files: Vec<SettingsFile>,
        rows: Vec<SettingRow>,
    },
    Found {
        session: SessionId,
        query: String,
        results: Vec<Found>,
    },

    // From a UI, relayed to the brain.
    Resync {
        session: SessionId,
    },
    StateRequest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
    },
    WalkTo {
        session: SessionId,
        map: i64,
        room: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        stop_short: Option<bool>,
    },
    WalkStop {
        session: SessionId,
    },
    MapRequest {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        radius: Option<u32>,
    },
    BotControl {
        session: SessionId,
        action: BotAction,
    },
    LoopsRequest {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        anchor: Option<RoomKey>,
        radius: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        doors: Option<bool>,
        /// Only the loops whose name, file, description, author or a stop's room name holds
        /// every word of this (any case), from anywhere in the world.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        query: Option<String>,
        /// The most loops to send, the nearest first (12 if left out, 50 at most).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        limit: Option<u32>,
    },
    WalkLoop {
        session: SessionId,
        id: String,
    },
    RunLoop {
        session: SessionId,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        skip: Option<Vec<RoomKey>>,
    },
    StatsRequest {
        session: SessionId,
    },
    StatsReset {
        session: SessionId,
    },
    SettingsRequest {
        session: SessionId,
        /// The layer to show the player's own values of: `mine`, `server` or `character`
        /// (left out: `character`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        layer: Option<String>,
    },
    SettingsSave {
        session: SessionId,
        /// The layer to write: `mine`, `server` or `character`.
        layer: String,
        changes: Vec<SettingChange>,
    },
    SaveLoop {
        session: SessionId,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        base: Option<String>,
        stops: Vec<RoomKey>,
    },
    DeleteLoop {
        session: SessionId,
        id: String,
    },
    Find {
        session: SessionId,
        query: String,
    },
    SetMonsterRule {
        session: SessionId,
        pattern: String,
        rule: Option<MonsterRuleKind>,
    },

    /// To the brain only: a line went to the game, typed by a client or sent by a logon or
    /// log-off step; `seq` is the session's latest output `seq` at that moment. A password is
    /// `********`.
    Sent {
        session: SessionId,
        seq: u64,
        text: String,
    },
    /// To UIs only: whether a brain is attached. Sent right after `welcome` and whenever it
    /// changes.
    BrainState {
        attached: bool,
    },
    /// To a UI only: it fell behind and `missed` relayed brain messages were dropped. Recover
    /// with a `state_request`.
    BrainLagged {
        missed: u64,
    },
    /// To the brain only: the gateway is about to log the session off (it is stopping). The
    /// brain stops playing the character and answers `ready_to_leave`; the gateway waits for
    /// that (30 s at most, or until the brain goes) before it sends the first log-off step.
    LoggingOff {
        session: SessionId,
    },
    /// To the brain only: a line a UI typed for a session the brain arbitrates (`arbitrate`),
    /// which the gateway did not send to the game. The brain decides when it goes, and what
    /// the bot does meanwhile. `from` names the UI connection (`ui-3`).
    UserInput {
        session: SessionId,
        text: String,
        from: String,
    },
    /// To the brain only: a UI's raw keys went to the game for a session the brain arbitrates
    /// (they always go straight through): the bot pauses.
    KeysSent {
        session: SessionId,
    },
}

/// Client → gateway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToGateway {
    /// The first message on a connection. `token` may be left out where the gateway does not
    /// check it (a loopback bind).
    Hello {
        protocol: u32,
        client: ClientKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        token: Option<String>,
    },
    /// A line of input; the gateway adds the line ending.
    InputLine { session: SessionId, text: String },
    /// Raw key bytes, sent straight through and never line-buffered (the character editor,
    /// `Hit any key`).
    InputKeys { session: SessionId, bytes: Vec<u8> },
    /// Ask for stored output. `after_seq`: the chunks after it (after a reconnect, the last
    /// `seq` seen). `before_seq`: the chunks just before it (scrolling back). Neither: the
    /// latest chunks.
    HistoryRequest {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        after_seq: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        before_seq: Option<u64>,
        max_chunks: u32,
    },
    /// Choose which sessions' output and state this connection receives, replacing the
    /// previous choice. Left out, or before any `subscribe`: all sessions.
    Subscribe {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        sessions: Option<Vec<SessionId>>,
    },
    /// Connect, log off or hang up one session: from a UI (the tile's button) or from the brain
    /// (`hang_up` at the player's HP line).
    SessionControl {
        session: SessionId,
        action: SessionAction,
    },

    /// UI → brain: re-sync the character (`st`, `exp`, `i`, look).
    Resync { session: SessionId },
    /// UI → brain: send a fresh `character` snapshot (of one session, or of all) and a
    /// `brain_info`.
    StateRequest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
    },
    /// UI → brain: walk the character to this room, confirming each step; replaces a walk in
    /// progress. `stop_short`: stop in the room before it when it is a boss lair; left out,
    /// on for a boss lair (it changes nothing for another room).
    WalkTo {
        session: SessionId,
        map: i64,
        room: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        stop_short: Option<bool>,
    },
    /// UI → brain: stop walking after the move in flight.
    WalkStop { session: SessionId },
    /// UI → brain: send a `map_view` now, of the rooms within `radius` steps (15 when left
    /// out; later automatic views use the same radius).
    MapRequest {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        radius: Option<u32>,
    },
    /// UI → brain: start, stop, pause or resume the bot for the character.
    BotControl {
        session: SessionId,
        action: BotAction,
    },
    /// UI → brain: send the `loops` of the area within `radius` steps of `anchor` (the believed
    /// room when left out), through doors unless `doors` is false.
    LoopsRequest {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        anchor: Option<RoomKey>,
        radius: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        doors: Option<bool>,
        /// Only the loops whose name, file, description, author or a stop's room name holds
        /// every word of this (any case), from anywhere in the world.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        query: Option<String>,
        /// The most loops to send, the nearest first (12 if left out, 50 at most).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        limit: Option<u32>,
    },
    /// UI → brain: walk a loop once (a generated loop's id, or a MegaMud loop's file), from its stop nearest the character round to
    /// that stop again; `walk_stop` stops it, and `walk_to` replaces it.
    WalkLoop { session: SessionId, id: String },
    /// UI → brain: run a loop (Task mode; `id` is a generated loop's id or a MegaMud loop's
    /// file, `Mwodloop.mp`): walk to its nearest stop, then round its
    /// stops in order, fighting at each, passing by those in `skip` and those whose respawn
    /// delay has not passed; it replaces a running "fight here". Sent again for the loop
    /// already running, it only changes `skip` (left out: none). `bot_control` stops, pauses
    /// and resumes it.
    RunLoop {
        session: SessionId,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        skip: Option<Vec<RoomKey>>,
    },
    /// UI → brain: send a `stats` message with the character's last runs.
    StatsRequest { session: SessionId },
    /// UI → brain: end the character's current run and start a new one now, with its numbers
    /// at zero (exp/hr, kills, deaths, flees, rests). The ended run stays in the history. A
    /// notice says so, and a `stats` message follows. Nothing happens without a run (the bot
    /// stopped): a warning says so.
    StatsReset { session: SessionId },
    /// UI → brain: send a `settings` message for the character, with the values of `layer`
    /// (`mine`, `server` or `character`; left out: `character`) as that layer's file holds them.
    SettingsRequest {
        session: SessionId,
        /// The layer to show the player's own values of: `mine`, `server` or `character`
        /// (left out: `character`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        layer: Option<String>,
    },
    /// UI → brain: change settings in the `layer` file: each change sets a value, or removes it
    /// (`value` left out) so the layer below shows through again. A value the settings refuse
    /// (out of range, not one of the words) changes nothing and a warning says why; otherwise the
    /// file is written, put in force at once, narrated, and a `settings` message follows.
    SettingsSave {
        session: SessionId,
        /// The layer to write: `mine`, `server` or `character`.
        layer: String,
        changes: Vec<SettingChange>,
    },
    /// UI → brain: make or change a custom loop: `name` and the `stops` in order (the loop it
    /// was made from, `base`, is kept for reference). The id is `custom:` and a slug of the name,
    /// so saving the same name again replaces the loop. The brain answers with a notice, and the
    /// loop is in the next `loops` message's `custom`. `run_loop` and `walk_loop` take its id.
    SaveLoop {
        session: SessionId,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        base: Option<String>,
        stops: Vec<RoomKey>,
    },
    /// UI → brain: delete a custom loop (`custom:...`).
    DeleteLoop { session: SessionId, id: String },
    /// UI → brain: find rooms, well-known places and monsters whose name holds `query` (two
    /// characters or more, any case); the brain answers with `found`.
    Find { session: SessionId, query: String },
    /// UI → brain: set the player's rule for monsters whose name holds `pattern` (any case),
    /// or remove it (`rule` null). Stored with the bot's record; the bot's `monster_rules`
    /// shows the result.
    SetMonsterRule {
        session: SessionId,
        pattern: String,
        rule: Option<MonsterRuleKind>,
    },

    /// Brain → UIs: one parsed event, as `wayfarer-parse` produced it, with the output `seq`
    /// its line came from. The gateway never reads `fields`.
    Event {
        session: SessionId,
        seq: u64,
        line_no: u64,
        name: String,
        cat: String,
        #[ts(type = "Record<string, unknown>")]
        fields: serde_json::Map<String, serde_json::Value>,
        line: String,
    },
    /// Brain → UIs: the whole character snapshot.
    Character {
        session: SessionId,
        state: Box<CharacterState>,
    },
    /// Brain → UIs: a JSON merge patch (RFC 7386) to apply to the last snapshot. The gateway
    /// never reads it.
    CharacterPatch {
        session: SessionId,
        #[ts(type = "unknown")]
        patch: serde_json::Value,
    },
    /// Brain → UIs: who the brain is. `started_at` is RFC 3339.
    BrainInfo {
        build: String,
        started_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        world_pack: Option<WorldPackInfo>,
    },
    /// Brain → UIs: something the brain did or saw, worth a line in the dev panel.
    Notice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
    },
    /// Brain → UIs: something wrong but not fatal.
    Warning {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
    },
    /// Brain → UIs: the rooms around the believed position, laid out on a grid with `center`
    /// at (0, 0) on `level` 0, and the walk's route (the rooms still to pass, from the one the
    /// character is in) while walking. Sent when the believed room changes, when a route
    /// starts, changes or ends, and on `map_request`.
    MapView {
        session: SessionId,
        center: RoomKey,
        /// The center's level: the floor the panel shows first.
        level: i32,
        rooms: Vec<MapRoom>,
        exits: Vec<MapExit>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        route: Option<Vec<RoomKey>>,
    },
    /// Brain → UIs: the answer to a `loops_request`: the loops generated over the area's hunt
    /// rooms, and MegaMud's verified loops through it.
    Loops {
        session: SessionId,
        anchor: RoomKey,
        radius: u32,
        doors: bool,
        generated: Vec<GeneratedLoop>,
        megamud: Vec<MegaMudLoop>,
        /// The loops the player made (`save_loop`), by name.
        #[serde(default)]
        custom: Vec<GeneratedLoop>,
        /// How many loops matched the request before it was cut to its `limit`.
        #[serde(default)]
        matches: u32,
    },
    /// Brain → UIs: the answer to a `stats_request`: the last ten runs, newest first, and the
    /// newest run's samples.
    Stats {
        session: SessionId,
        runs: Vec<RunSummary>,
        samples: Vec<ExpSample>,
    },
    /// Brain → UIs: the character's settings, answering `settings_request` and `settings_save`
    /// (and sent again when the files change): every setting with its effective value, where
    /// the value comes from, and the value the `layer` file itself holds.
    Settings {
        session: SessionId,
        /// The layer whose own values `own` shows.
        layer: String,
        files: Vec<SettingsFile>,
        rows: Vec<SettingRow>,
    },
    /// Brain → UIs: the answer to a `find`, best first: those with no way there last, then the
    /// better match of the name, then the nearer; at most 30.
    Found {
        session: SessionId,
        query: String,
        results: Vec<Found>,
    },

    /// Brain → gateway: the answer to `logging_off`: the brain sends the session nothing more,
    /// and the log-off may start.
    ReadyToLeave { session: SessionId },
    /// Brain → gateway: whether the brain arbitrates the session's input (its bot is running
    /// or paused): while it does, a UI's lines go to the brain as `user_input` and its keys are
    /// reported with `keys_sent`. Off for every session when a brain attaches, and again when
    /// it goes.
    Arbitrate { session: SessionId, on: bool },
}

/// Why a received message could not be decoded. The receiver reports it (an `error` reply)
/// and carries on.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("not a JSON message: {0}")]
    NotJson(serde_json::Error),
    #[error("message has no \"type\" field")]
    MissingType,
    #[error("unknown message type {0:?}")]
    UnknownType(String),
    #[error("bad {kind:?} message: {source}")]
    Invalid {
        kind: String,
        source: serde_json::Error,
    },
}

impl ToClient {
    /// Every `type` this enum knows, in declaration order.
    pub const TYPES: &[&str] = &[
        "welcome",
        "refused",
        "session_list",
        "session_state",
        "output",
        "history",
        "lagged",
        "error",
        "event",
        "character",
        "character_patch",
        "brain_info",
        "notice",
        "warning",
        "map_view",
        "loops",
        "stats",
        "settings",
        "found",
        "resync",
        "state_request",
        "walk_to",
        "walk_stop",
        "map_request",
        "bot_control",
        "loops_request",
        "walk_loop",
        "run_loop",
        "stats_request",
        "stats_reset",
        "settings_request",
        "settings_save",
        "save_loop",
        "delete_loop",
        "find",
        "set_monster_rule",
        "sent",
        "brain_state",
        "brain_lagged",
        "logging_off",
        "user_input",
        "keys_sent",
    ];

    /// This message's `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Welcome { .. } => "welcome",
            Self::Refused { .. } => "refused",
            Self::SessionList { .. } => "session_list",
            Self::SessionState { .. } => "session_state",
            Self::Output { .. } => "output",
            Self::History { .. } => "history",
            Self::Lagged { .. } => "lagged",
            Self::Error { .. } => "error",
            Self::Event { .. } => "event",
            Self::Character { .. } => "character",
            Self::CharacterPatch { .. } => "character_patch",
            Self::BrainInfo { .. } => "brain_info",
            Self::Notice { .. } => "notice",
            Self::Warning { .. } => "warning",
            Self::MapView { .. } => "map_view",
            Self::Loops { .. } => "loops",
            Self::Stats { .. } => "stats",
            Self::Settings { .. } => "settings",
            Self::Found { .. } => "found",
            Self::Resync { .. } => "resync",
            Self::StateRequest { .. } => "state_request",
            Self::WalkTo { .. } => "walk_to",
            Self::WalkStop { .. } => "walk_stop",
            Self::MapRequest { .. } => "map_request",
            Self::BotControl { .. } => "bot_control",
            Self::LoopsRequest { .. } => "loops_request",
            Self::WalkLoop { .. } => "walk_loop",
            Self::RunLoop { .. } => "run_loop",
            Self::StatsRequest { .. } => "stats_request",
            Self::StatsReset { .. } => "stats_reset",
            Self::SettingsRequest { .. } => "settings_request",
            Self::SettingsSave { .. } => "settings_save",
            Self::SaveLoop { .. } => "save_loop",
            Self::DeleteLoop { .. } => "delete_loop",
            Self::Find { .. } => "find",
            Self::SetMonsterRule { .. } => "set_monster_rule",
            Self::Sent { .. } => "sent",
            Self::BrainState { .. } => "brain_state",
            Self::BrainLagged { .. } => "brain_lagged",
            Self::LoggingOff { .. } => "logging_off",
            Self::UserInput { .. } => "user_input",
            Self::KeysSent { .. } => "keys_sent",
        }
    }

    /// The session this message is about, if it names one.
    pub fn session(&self) -> Option<&str> {
        match self {
            Self::SessionState { session, .. }
            | Self::Output { session, .. }
            | Self::History { session, .. }
            | Self::Lagged { session, .. }
            | Self::Event { session, .. }
            | Self::Character { session, .. }
            | Self::CharacterPatch { session, .. }
            | Self::MapView { session, .. }
            | Self::Loops { session, .. }
            | Self::Stats { session, .. }
            | Self::Settings { session, .. }
            | Self::Found { session, .. }
            | Self::Find { session, .. }
            | Self::SetMonsterRule { session, .. }
            | Self::Resync { session }
            | Self::WalkTo { session, .. }
            | Self::WalkStop { session }
            | Self::MapRequest { session, .. }
            | Self::BotControl { session, .. }
            | Self::LoopsRequest { session, .. }
            | Self::WalkLoop { session, .. }
            | Self::RunLoop { session, .. }
            | Self::StatsRequest { session }
            | Self::StatsReset { session }
            | Self::SettingsRequest { session, .. }
            | Self::SettingsSave { session, .. }
            | Self::SaveLoop { session, .. }
            | Self::DeleteLoop { session, .. }
            | Self::Sent { session, .. }
            | Self::LoggingOff { session }
            | Self::UserInput { session, .. }
            | Self::KeysSent { session } => Some(session),
            Self::Notice { session, .. }
            | Self::Warning { session, .. }
            | Self::StateRequest { session } => session.as_deref(),
            Self::Welcome { .. }
            | Self::Refused { .. }
            | Self::SessionList { .. }
            | Self::Error { .. }
            | Self::BrainInfo { .. }
            | Self::BrainState { .. }
            | Self::BrainLagged { .. } => None,
        }
    }

    pub fn from_json(text: &str) -> Result<Self, DecodeError> {
        decode(text, Self::TYPES)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("wire messages always serialize")
    }
}

impl ToGateway {
    /// Every `type` this enum knows, in declaration order.
    pub const TYPES: &[&str] = &[
        "hello",
        "input_line",
        "input_keys",
        "history_request",
        "subscribe",
        "session_control",
        "resync",
        "state_request",
        "walk_to",
        "walk_stop",
        "map_request",
        "bot_control",
        "loops_request",
        "walk_loop",
        "run_loop",
        "stats_request",
        "stats_reset",
        "settings_request",
        "settings_save",
        "save_loop",
        "delete_loop",
        "find",
        "set_monster_rule",
        "event",
        "character",
        "character_patch",
        "brain_info",
        "notice",
        "warning",
        "map_view",
        "loops",
        "stats",
        "settings",
        "found",
        "ready_to_leave",
        "arbitrate",
    ];

    /// This message's `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::InputLine { .. } => "input_line",
            Self::InputKeys { .. } => "input_keys",
            Self::HistoryRequest { .. } => "history_request",
            Self::Subscribe { .. } => "subscribe",
            Self::SessionControl { .. } => "session_control",
            Self::Resync { .. } => "resync",
            Self::StateRequest { .. } => "state_request",
            Self::WalkTo { .. } => "walk_to",
            Self::WalkStop { .. } => "walk_stop",
            Self::MapRequest { .. } => "map_request",
            Self::BotControl { .. } => "bot_control",
            Self::LoopsRequest { .. } => "loops_request",
            Self::WalkLoop { .. } => "walk_loop",
            Self::RunLoop { .. } => "run_loop",
            Self::StatsRequest { .. } => "stats_request",
            Self::StatsReset { .. } => "stats_reset",
            Self::SettingsRequest { .. } => "settings_request",
            Self::SettingsSave { .. } => "settings_save",
            Self::SaveLoop { .. } => "save_loop",
            Self::DeleteLoop { .. } => "delete_loop",
            Self::Find { .. } => "find",
            Self::SetMonsterRule { .. } => "set_monster_rule",
            Self::Event { .. } => "event",
            Self::Character { .. } => "character",
            Self::CharacterPatch { .. } => "character_patch",
            Self::BrainInfo { .. } => "brain_info",
            Self::Notice { .. } => "notice",
            Self::Warning { .. } => "warning",
            Self::MapView { .. } => "map_view",
            Self::Loops { .. } => "loops",
            Self::Stats { .. } => "stats",
            Self::Settings { .. } => "settings",
            Self::Found { .. } => "found",
            Self::ReadyToLeave { .. } => "ready_to_leave",
            Self::Arbitrate { .. } => "arbitrate",
        }
    }

    /// Who may send this message, and whether the gateway relays it.
    pub fn family(&self) -> Family {
        match self {
            Self::Hello { .. }
            | Self::InputLine { .. }
            | Self::InputKeys { .. }
            | Self::HistoryRequest { .. }
            | Self::SessionControl { .. }
            | Self::Subscribe { .. } => Family::Gateway,
            Self::Resync { .. }
            | Self::StateRequest { .. }
            | Self::WalkTo { .. }
            | Self::WalkStop { .. }
            | Self::MapRequest { .. }
            | Self::BotControl { .. }
            | Self::LoopsRequest { .. }
            | Self::WalkLoop { .. }
            | Self::RunLoop { .. }
            | Self::StatsRequest { .. }
            | Self::StatsReset { .. }
            | Self::SettingsRequest { .. }
            | Self::SettingsSave { .. }
            | Self::SaveLoop { .. }
            | Self::DeleteLoop { .. }
            | Self::Find { .. }
            | Self::SetMonsterRule { .. } => Family::Control,
            Self::Event { .. }
            | Self::Character { .. }
            | Self::CharacterPatch { .. }
            | Self::BrainInfo { .. }
            | Self::Notice { .. }
            | Self::Warning { .. }
            | Self::MapView { .. }
            | Self::Loops { .. }
            | Self::Stats { .. }
            | Self::Settings { .. }
            | Self::Found { .. } => Family::Brain,
            Self::ReadyToLeave { .. } | Self::Arbitrate { .. } => Family::BrainToGateway,
        }
    }

    /// A [`Family::Control`] or [`Family::Brain`] message as the gateway relays it, unchanged;
    /// `None` for a message the gateway handles itself ([`Family::Gateway`],
    /// [`Family::BrainToGateway`]).
    pub fn into_relayed(self) -> Option<ToClient> {
        Some(match self {
            Self::Resync { session } => ToClient::Resync { session },
            Self::StateRequest { session } => ToClient::StateRequest { session },
            Self::WalkTo {
                session,
                map,
                room,
                stop_short,
            } => ToClient::WalkTo {
                session,
                map,
                room,
                stop_short,
            },
            Self::WalkStop { session } => ToClient::WalkStop { session },
            Self::MapRequest { session, radius } => ToClient::MapRequest { session, radius },
            Self::BotControl { session, action } => ToClient::BotControl { session, action },
            Self::LoopsRequest {
                session,
                anchor,
                radius,
                doors,
                query,
                limit,
            } => ToClient::LoopsRequest {
                session,
                anchor,
                radius,
                doors,
                query,
                limit,
            },
            Self::WalkLoop { session, id } => ToClient::WalkLoop { session, id },
            Self::RunLoop { session, id, skip } => ToClient::RunLoop { session, id, skip },
            Self::StatsRequest { session } => ToClient::StatsRequest { session },
            Self::StatsReset { session } => ToClient::StatsReset { session },
            Self::SettingsRequest { session, layer } => {
                ToClient::SettingsRequest { session, layer }
            }
            Self::SettingsSave {
                session,
                layer,
                changes,
            } => ToClient::SettingsSave {
                session,
                layer,
                changes,
            },
            Self::SaveLoop {
                session,
                name,
                base,
                stops,
            } => ToClient::SaveLoop {
                session,
                name,
                base,
                stops,
            },
            Self::DeleteLoop { session, id } => ToClient::DeleteLoop { session, id },
            Self::Find { session, query } => ToClient::Find { session, query },
            Self::SetMonsterRule {
                session,
                pattern,
                rule,
            } => ToClient::SetMonsterRule {
                session,
                pattern,
                rule,
            },
            Self::Event {
                session,
                seq,
                line_no,
                name,
                cat,
                fields,
                line,
            } => ToClient::Event {
                session,
                seq,
                line_no,
                name,
                cat,
                fields,
                line,
            },
            Self::Character { session, state } => ToClient::Character { session, state },
            Self::CharacterPatch { session, patch } => ToClient::CharacterPatch { session, patch },
            Self::BrainInfo {
                build,
                started_at,
                world_pack,
            } => ToClient::BrainInfo {
                build,
                started_at,
                world_pack,
            },
            Self::Notice { session, text } => ToClient::Notice { session, text },
            Self::Warning { session, text } => ToClient::Warning { session, text },
            Self::MapView {
                session,
                center,
                level,
                rooms,
                exits,
                route,
            } => ToClient::MapView {
                session,
                center,
                level,
                rooms,
                exits,
                route,
            },
            Self::Loops {
                session,
                anchor,
                radius,
                doors,
                generated,
                megamud,
                custom,
                matches,
            } => ToClient::Loops {
                session,
                anchor,
                radius,
                doors,
                generated,
                megamud,
                custom,
                matches,
            },
            Self::Stats {
                session,
                runs,
                samples,
            } => ToClient::Stats {
                session,
                runs,
                samples,
            },
            Self::Settings {
                session,
                layer,
                files,
                rows,
            } => ToClient::Settings {
                session,
                layer,
                files,
                rows,
            },
            Self::Found {
                session,
                query,
                results,
            } => ToClient::Found {
                session,
                query,
                results,
            },
            Self::Hello { .. }
            | Self::InputLine { .. }
            | Self::InputKeys { .. }
            | Self::HistoryRequest { .. }
            | Self::Subscribe { .. }
            | Self::SessionControl { .. }
            | Self::ReadyToLeave { .. }
            | Self::Arbitrate { .. } => return None,
        })
    }

    pub fn from_json(text: &str) -> Result<Self, DecodeError> {
        decode(text, Self::TYPES)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("wire messages always serialize")
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The `refused` reason for a client speaking another protocol version, or `None` when it
/// matches.
pub fn protocol_mismatch(client_protocol: u32) -> Option<String> {
    (client_protocol != PROTOCOL).then(|| {
        format!("protocol mismatch: client speaks {client_protocol}, gateway speaks {PROTOCOL}")
    })
}

fn decode<T: DeserializeOwned>(text: &str, types: &[&str]) -> Result<T, DecodeError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(DecodeError::NotJson)?;
    let kind = match value.get("type") {
        Some(serde_json::Value::String(kind)) => kind.clone(),
        Some(_) => return Err(DecodeError::UnknownType(value["type"].to_string())),
        None if value.is_object() => return Err(DecodeError::MissingType),
        None => {
            return Err(DecodeError::NotJson(serde::de::Error::custom(
                "expected a JSON object",
            )));
        }
    };
    if !types.contains(&kind.as_str()) {
        return Err(DecodeError::UnknownType(kind));
    }
    serde_json::from_value(value).map_err(|source| DecodeError::Invalid { kind, source })
}

/// Write the TypeScript for every wire type into `dir`, which should be empty: one file per
/// type, plus `index.ts` with [`PROTOCOL`] and a re-export of every other `.ts` file there.
pub fn export_typescript(dir: &Path) -> Result<(), ts_rs::ExportError> {
    // Sequence numbers stay far below 2^53, so plain JSON numbers are safe in JavaScript.
    let cfg = ts_rs::Config::new()
        .with_out_dir(dir)
        .with_large_int("number");
    ToClient::export_all(&cfg)?;
    ToGateway::export_all(&cfg)?;

    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".ts")?;
            (stem != "index").then(|| stem.to_owned())
        })
        .collect();
    names.sort();
    let mut index = String::from(
        "// Generated by wayfarer-proto (`export_typescript`). Do not edit this file manually.\n\n",
    );
    index.push_str(&format!("export const PROTOCOL = {PROTOCOL};\n\n"));
    for name in names {
        index.push_str(&format!("export type {{ {name} }} from \"./{name}\";\n"));
    }
    std::fs::write(dir.join("index.ts"), index)?;
    Ok(())
}
