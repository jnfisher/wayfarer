//! wayfarer-store: the brain's own SQLite file, `store.sqlite` in the data directory
//! (docs/plan/02-architecture.md "Storage"). Two things live here: the bot's state per session,
//! so a restarted brain resumes what each character was doing (docs/plan/03-engine.md section
//! 7), and measured statistics: runs, what happened in them, and a sample of exp and HP every
//! minute (section 2).
//!
//! A leaf: it knows nothing of the wire or the game, only rows. Times are Unix seconds from the
//! brain's injected clock, so a replay can run against a throwaway store on its recorded clock.
//!
//! ```no_run
//! use wayfarer_store::{BotRecord, BotState, Store};
//!
//! let (store, note) = Store::open("store.sqlite".as_ref())?;
//! if let Some(note) = note {
//!     eprintln!("{note}");
//! }
//! store.save_bot("aldric", &BotRecord::new(BotState::Running), 1_792_411_200)?;
//! # Ok::<(), wayfarer_store::Error>(())
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

/// A room: (map, room).
pub type Key = (i64, i64);

/// The schema, one step per version: `PRAGMA user_version` is how many have been applied.
const MIGRATIONS: &[&str] = &[
    // 1: the bot's state, runs, their events and samples.
    "CREATE TABLE bot_state (
         session TEXT PRIMARY KEY,
         state TEXT NOT NULL,
         home_map INTEGER,
         home_room INTEGER,
         loop_id TEXT,
         loop_stops TEXT NOT NULL DEFAULT '[]',
         stop_index INTEGER NOT NULL DEFAULT 0,
         skip TEXT NOT NULL DEFAULT '[]',
         paused_reason TEXT,
         updated_at INTEGER NOT NULL
     );
     CREATE TABLE runs (
         id INTEGER PRIMARY KEY,
         session TEXT NOT NULL,
         started_at INTEGER NOT NULL,
         ended_at INTEGER,
         loop_id TEXT
     );
     CREATE INDEX runs_session ON runs (session, started_at);
     CREATE TABLE events (
         id INTEGER PRIMARY KEY,
         run_id INTEGER NOT NULL REFERENCES runs (id),
         at INTEGER NOT NULL,
         kind TEXT NOT NULL,
         value INTEGER NOT NULL DEFAULT 0,
         detail TEXT,
         room_map INTEGER,
         room_room INTEGER
     );
     CREATE INDEX events_run ON events (run_id);
     CREATE TABLE samples (
         run_id INTEGER NOT NULL REFERENCES runs (id),
         at INTEGER NOT NULL,
         exp INTEGER,
         hp INTEGER
     );
     CREATE INDEX samples_run ON samples (run_id, at);",
    // 2: the bot's monster rules, the session's and the player's (C-035).
    "CREATE TABLE monster_rules (
         session TEXT NOT NULL,
         pattern TEXT NOT NULL,
         rule TEXT NOT NULL,
         source TEXT NOT NULL,
         at INTEGER NOT NULL,
         PRIMARY KEY (session, pattern, source)
     );",
    // 3: when each boss was last killed (boss status, the "should be up" display).
    "CREATE TABLE boss_kills (
         session TEXT NOT NULL,
         monster TEXT NOT NULL,
         killed_at INTEGER NOT NULL,
         PRIMARY KEY (session, monster)
     );",
    // 4: loops the player made (custom loops): a name and the stops in order.
    "CREATE TABLE custom_loops (
         id TEXT PRIMARY KEY,
         name TEXT NOT NULL,
         stops TEXT NOT NULL,
         based_on TEXT,
         updated_at INTEGER NOT NULL
     );",
    // 5: where the character died, fled and stopped, whether or not the bot was running.
    "CREATE TABLE places (
         id INTEGER PRIMARY KEY,
         session TEXT NOT NULL,
         at INTEGER NOT NULL,
         kind TEXT NOT NULL,
         map INTEGER NOT NULL,
         room INTEGER NOT NULL,
         detail TEXT
     );
     CREATE INDEX places_session ON places (session, at);",
    // 6: parties the player defined: a leader and its followers, by session id.
    "CREATE TABLE parties (
         id TEXT PRIMARY KEY,
         leader TEXT NOT NULL,
         followers TEXT NOT NULL,
         updated_at INTEGER NOT NULL
     );",
    // 7: the lasting spells each session keeps on itself (C-047), by short name.
    "CREATE TABLE blessings (
         session TEXT PRIMARY KEY,
         spells TEXT NOT NULL,
         updated_at INTEGER NOT NULL
     );",
    // 8: each session's combat numbers since the player last reset them (C-050), as JSON.
    "CREATE TABLE combat_stats (
         session TEXT PRIMARY KEY,
         tally TEXT NOT NULL,
         updated_at INTEGER NOT NULL
     );",
];

/// The schema version this build writes.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

const TABLES: [&str; 11] = [
    "bot_state",
    "runs",
    "events",
    "samples",
    "monster_rules",
    "boss_kills",
    "custom_loops",
    "places",
    "parties",
    "blessings",
    "combat_stats",
];

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The file was written by a newer wayfarer; it is left alone.
    Newer {
        path: PathBuf,
        version: u32,
    },
    /// A row this build cannot read (a hand-edited file).
    Bad(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "{e}"),
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Newer { path, version } => write!(
                f,
                "{}: schema version {version} is newer than this wayfarer's {SCHEMA_VERSION}",
                path.display()
            ),
            Error::Bad(what) => write!(f, "unreadable row: {what}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sqlite(e) => Some(e),
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// What [`Store::open`] had to do besides opening: worth a line in the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// There was no store: a new one was made.
    Created(PathBuf),
    /// The file could not be read as a store: it was moved aside and a new one made.
    Recreated {
        path: PathBuf,
        moved_to: PathBuf,
        why: String,
    },
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Note::Created(path) => write!(f, "no store at {}: created", path.display()),
            Note::Recreated {
                path,
                moved_to,
                why,
            } => write!(
                f,
                "the store at {} could not be read ({why}): moved to {} and created afresh",
                path.display(),
                moved_to.display()
            ),
        }
    }
}

/// The bot's state as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotState {
    Stopped,
    Running,
    Paused,
}

impl BotState {
    fn as_str(self) -> &'static str {
        match self {
            BotState::Stopped => "stopped",
            BotState::Running => "running",
            BotState::Paused => "paused",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "stopped" => BotState::Stopped,
            "running" => BotState::Running,
            "paused" => BotState::Paused,
            _ => return Err(Error::Bad(format!("bot state {s:?}"))),
        })
    }
}

/// One session's bot, as a restarted brain needs it to carry on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotRecord {
    pub state: BotState,
    /// Where it was started ("fight here"), or the room it rests from.
    pub home: Option<Key>,
    /// The loop it runs (Task mode), by its id.
    pub loop_id: Option<String>,
    /// The loop's stops as they were when it started: a generated loop's id resolves against
    /// the character's level, so after a level-up it may name another loop; these win.
    pub loop_stops: Vec<Key>,
    /// The stop it is at or walking to.
    pub stop_index: usize,
    /// Stops to pass by this time round.
    pub skip: Vec<usize>,
    /// Why it is paused, when it is.
    pub paused_reason: Option<String>,
    /// The monster rules the session learned and the player set, oldest first.
    pub monster_rules: Vec<RuleRecord>,
}

impl BotRecord {
    pub fn new(state: BotState) -> Self {
        Self {
            state,
            home: None,
            loop_id: None,
            loop_stops: Vec::new(),
            stop_index: 0,
            skip: Vec::new(),
            paused_reason: None,
            monster_rules: Vec::new(),
        }
    }
}

/// A monster rule as stored: the brain's words for the rule and its source (`never_fight`,
/// `session`); the store does not read them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRecord {
    pub pattern: String,
    pub rule: String,
    pub source: String,
    /// When it was made, Unix seconds.
    pub at: i64,
}

/// A loop the player made: the stops in order, and the loop it was made from, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomLoop {
    /// `custom:` and a slug of the name: what `run_loop` names it by.
    pub id: String,
    pub name: String,
    pub stops: Vec<Key>,
    pub based_on: Option<String>,
}

/// A party the player defined: its leader and followers, by session id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRecord {
    pub id: String,
    pub leader: String,
    pub followers: Vec<String>,
}

/// What a run's events count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A monster died; `value` is the exp it gave, `detail` its name.
    Kill,
    /// Exp gained with no kill to put it on; `value` is the amount.
    Exp,
    /// The character was killed.
    Death,
    Flee,
    /// The character started resting.
    Rest,
    /// `value` is the new level.
    LevelUp,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            EventKind::Kill => "kill",
            EventKind::Exp => "exp",
            EventKind::Death => "death",
            EventKind::Flee => "flee",
            EventKind::Rest => "rest",
            EventKind::LevelUp => "level_up",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "kill" => EventKind::Kill,
            "exp" => EventKind::Exp,
            "death" => EventKind::Death,
            "flee" => EventKind::Flee,
            "rest" => EventKind::Rest,
            "level_up" => EventKind::LevelUp,
            _ => return Err(Error::Bad(format!("event kind {s:?}"))),
        })
    }
}

/// Why a room is remembered as a place the character was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceKind {
    /// The character was killed here: its gear is in the room.
    Death,
    /// The character started to flee from here.
    Flee,
    /// The bot stopped or gave up here.
    Stopped,
}

impl PlaceKind {
    fn as_str(self) -> &'static str {
        match self {
            PlaceKind::Death => "death",
            PlaceKind::Flee => "flee",
            PlaceKind::Stopped => "stopped",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "death" => PlaceKind::Death,
            "flee" => PlaceKind::Flee,
            "stopped" => PlaceKind::Stopped,
            _ => return Err(Error::Bad(format!("place kind {s:?}"))),
        })
    }

    /// The name on the wire.
    pub fn name(self) -> &'static str {
        self.as_str()
    }
}

/// A room the character is remembered to have been in, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// Unix seconds.
    pub at: i64,
    pub kind: PlaceKind,
    pub room: Key,
    /// What happened: who it fled from, why the bot stopped.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub at: i64,
    pub kind: EventKind,
    pub value: i64,
    pub detail: Option<String>,
    pub room: Option<Key>,
}

/// A run: from Start to Stop (or a death).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: i64,
    pub session: String,
    pub started_at: i64,
    /// `None` while it goes on.
    pub ended_at: Option<i64>,
    pub loop_id: Option<String>,
}

/// A run's events, counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub kills: u64,
    pub deaths: u64,
    pub flees: u64,
    pub rests: u64,
    pub level_ups: u64,
    /// The exp of the kills and of the gains with no kill.
    pub exp_gained: i64,
}

impl Totals {
    /// Count one more event.
    pub fn add(&mut self, kind: EventKind, value: i64) {
        match kind {
            EventKind::Kill => {
                self.kills += 1;
                self.exp_gained += value;
            }
            EventKind::Exp => self.exp_gained += value,
            EventKind::Death => self.deaths += 1,
            EventKind::Flee => self.flees += 1,
            EventKind::Rest => self.rests += 1,
            EventKind::LevelUp => self.level_ups += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub run: Run,
    pub totals: Totals,
}

/// The exp and HP a minute into a run, and every minute after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub at: i64,
    pub exp: Option<i64>,
    pub hp: Option<i64>,
}

pub struct Store {
    conn: Connection,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("path", &self.conn.path())
            .finish()
    }
}

impl Store {
    /// Open the store at `path`, making it if there is none. A file that cannot be read as a
    /// store (not SQLite, damaged, tables missing) is moved aside to `<path>.corrupt` and a
    /// new one made; the note says so. One from a newer wayfarer is an error, left as it is.
    pub fn open(path: &Path) -> Result<(Store, Option<Note>)> {
        if !path.exists() {
            let store = Self::open_file(path)?;
            return Ok((store, Some(Note::Created(path.to_owned()))));
        }
        match Self::open_file(path) {
            Ok(store) => Ok((store, None)),
            Err(e @ (Error::Newer { .. } | Error::Io { .. })) => Err(e),
            Err(e) => {
                let moved_to = aside(path)?;
                let store = Self::open_file(path)?;
                let note = Note::Recreated {
                    path: path.to_owned(),
                    moved_to,
                    why: e.to_string(),
                };
                Ok((store, Some(note)))
            }
        }
    }

    /// A store in memory, gone when dropped (tests, replay).
    pub fn in_memory() -> Result<Store> {
        let conn = Connection::open_in_memory()?;
        Self::ready(conn, Path::new(":memory:"))
    }

    fn open_file(path: &Path) -> Result<Store> {
        let conn = Connection::open(path)?;
        // WAL: a write is one append and an fsync at most, cheap enough to do on every change.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
        Self::ready(conn, path)
    }

    /// Bring the schema up to date and check every table is there.
    fn ready(mut conn: Connection, path: &Path) -> Result<Store> {
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(Error::Newer {
                path: path.to_owned(),
                version,
            });
        }
        for (i, step) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(step)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        for table in TABLES {
            conn.prepare(&format!("SELECT * FROM {table} LIMIT 0"))?;
        }
        Ok(Store { conn })
    }

    /// The schema version of the open file.
    pub fn version(&self) -> Result<u32> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// The bot's state for `session`, if one was saved.
    pub fn bot(&self, session: &str) -> Result<Option<BotRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT state, home_map, home_room, loop_id, loop_stops, stop_index, skip,
                        paused_reason
                 FROM bot_state WHERE session = ?1",
                [session],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<i64>>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((state, map, room, loop_id, stops, stop_index, skip, paused_reason)) = row else {
            return Ok(None);
        };
        let stops = ints(&stops)?;
        if stops.len() % 2 != 0 {
            return Err(Error::Bad(format!("loop stops of {session:?}")));
        }
        Ok(Some(BotRecord {
            state: BotState::parse(&state)?,
            home: map.zip(room),
            loop_id,
            loop_stops: stops.chunks(2).map(|c| (c[0], c[1])).collect(),
            stop_index: usize::try_from(stop_index).unwrap_or(0),
            skip: ints(&skip)?
                .into_iter()
                .map(|i| usize::try_from(i).unwrap_or(0))
                .collect(),
            paused_reason,
            monster_rules: self.monster_rules(session)?,
        }))
    }

    fn monster_rules(&self, session: &str) -> Result<Vec<RuleRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT pattern, rule, source, at FROM monster_rules WHERE session = ?1
             ORDER BY at, rowid",
        )?;
        let rows = stmt.query_map([session], |r| {
            Ok(RuleRecord {
                pattern: r.get(0)?,
                rule: r.get(1)?,
                source: r.get(2)?,
                at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Keep a custom loop, replacing the one with its id.
    pub fn save_custom_loop(&self, l: &CustomLoop, at: i64) -> Result<()> {
        let stops: Vec<String> = l.stops.iter().map(|(m, r)| format!("[{m},{r}]")).collect();
        self.conn.execute(
            "INSERT OR REPLACE INTO custom_loops (id, name, stops, based_on, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                l.id,
                l.name,
                format!("[{}]", stops.join(",")),
                l.based_on,
                at
            ],
        )?;
        Ok(())
    }

    /// The custom loops, by name.
    pub fn custom_loops(&self) -> Result<Vec<CustomLoop>> {
        let mut q = self
            .conn
            .prepare("SELECT id, name, stops, based_on FROM custom_loops ORDER BY name, id")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, stops, based_on) = row?;
            let flat = ints(&stops)?;
            out.push(CustomLoop {
                id,
                name,
                stops: flat
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| (c[0], c[1]))
                    .collect(),
                based_on,
            });
        }
        Ok(out)
    }

    /// Forget the custom loop `id`; whether there was one.
    pub fn delete_custom_loop(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM custom_loops WHERE id = ?1", params![id])?
            > 0)
    }

    /// Keep a party, replacing the one with its id.
    pub fn save_party(&self, p: &PartyRecord, at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO parties (id, leader, followers, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![p.id, p.leader, p.followers.join("\n"), at],
        )?;
        Ok(())
    }

    /// The parties, by id.
    pub fn parties(&self) -> Result<Vec<PartyRecord>> {
        let mut q = self
            .conn
            .prepare("SELECT id, leader, followers FROM parties ORDER BY id")?;
        let rows = q.query_map([], |r| {
            Ok(PartyRecord {
                id: r.get(0)?,
                leader: r.get(1)?,
                followers: r
                    .get::<_, String>(2)?
                    .split('\n')
                    .filter(|f| !f.is_empty())
                    .map(str::to_owned)
                    .collect(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Forget the party `id`; whether there was one.
    pub fn delete_party(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM parties WHERE id = ?1", params![id])?
            > 0)
    }

    /// Keep the lasting spells (short names) `session` keeps on itself, replacing its last;
    /// none forgets them.
    pub fn save_blessings(&self, session: &str, spells: &[String], at: i64) -> Result<()> {
        if spells.is_empty() {
            self.conn
                .execute("DELETE FROM blessings WHERE session = ?1", params![session])?;
        } else {
            self.conn.execute(
                "INSERT OR REPLACE INTO blessings (session, spells, updated_at)
                 VALUES (?1, ?2, ?3)",
                params![session, spells.join("\n"), at],
            )?;
        }
        Ok(())
    }

    /// Keep `session`'s combat numbers (the brain's JSON), replacing its last.
    pub fn save_combat_stats(&self, session: &str, tally: &str, at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO combat_stats (session, tally, updated_at) VALUES (?1, ?2, ?3)",
            params![session, tally, at],
        )?;
        Ok(())
    }

    /// `session`'s combat numbers as last kept.
    pub fn combat_stats(&self, session: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT tally FROM combat_stats WHERE session = ?1",
                params![session],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Every session's lasting spells, by session.
    pub fn blessings(&self) -> Result<Vec<(String, Vec<String>)>> {
        let mut q = self
            .conn
            .prepare("SELECT session, spells FROM blessings ORDER BY session")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?
                    .split('\n')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A boss (by its lower-case name) the character killed at `at`, unix seconds: kept for the
    /// session whether or not the bot was running, replacing an earlier kill of the same boss.
    pub fn boss_killed(&self, session: &str, monster: &str, at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO boss_kills (session, monster, killed_at) VALUES (?1, ?2, ?3)",
            params![session, monster.to_lowercase(), at],
        )?;
        Ok(())
    }

    /// The bosses the session killed, by lower-case name, with when (unix seconds).
    pub fn boss_kills(&self, session: &str) -> Result<Vec<(String, i64)>> {
        let mut q = self
            .conn
            .prepare("SELECT monster, killed_at FROM boss_kills WHERE session = ?1")?;
        let rows = q.query_map(params![session], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Remember a place of `session`.
    pub fn add_place(&self, session: &str, place: &Place) -> Result<()> {
        self.conn.execute(
            "INSERT INTO places (session, at, kind, map, room, detail) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session,
                place.at,
                place.kind.as_str(),
                place.room.0,
                place.room.1,
                place.detail
            ],
        )?;
        Ok(())
    }

    /// The session's last `limit` places, newest first.
    pub fn places(&self, session: &str, limit: usize) -> Result<Vec<Place>> {
        let mut q = self.conn.prepare(
            "SELECT at, kind, map, room, detail FROM places WHERE session = ?1
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = q.query_map(params![session, limit as i64], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (at, kind, map, room, detail) = row?;
            out.push(Place {
                at,
                kind: PlaceKind::parse(&kind)?,
                room: (map, room),
                detail,
            });
        }
        Ok(out)
    }

    /// Save the bot's state for `session`, replacing what was there (its monster rules too).
    pub fn save_bot(&self, session: &str, bot: &BotRecord, at: i64) -> Result<()> {
        let stops: Vec<String> = bot
            .loop_stops
            .iter()
            .map(|(m, r)| format!("[{m},{r}]"))
            .collect();
        let skip: Vec<String> = bot.skip.iter().map(usize::to_string).collect();
        // The state and its rules together, or neither.
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO bot_state
                 (session, state, home_map, home_room, loop_id, loop_stops, stop_index, skip,
                  paused_reason, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session,
                bot.state.as_str(),
                bot.home.map(|k| k.0),
                bot.home.map(|k| k.1),
                bot.loop_id,
                format!("[{}]", stops.join(",")),
                i64::try_from(bot.stop_index).unwrap_or(i64::MAX),
                format!("[{}]", skip.join(",")),
                bot.paused_reason,
                at,
            ],
        )?;
        tx.execute("DELETE FROM monster_rules WHERE session = ?1", [session])?;
        for r in &bot.monster_rules {
            tx.execute(
                "INSERT OR REPLACE INTO monster_rules (session, pattern, rule, source, at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![session, r.pattern, r.rule, r.source, r.at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Start a run for `session`.
    pub fn start_run(&self, session: &str, at: i64, loop_id: Option<&str>) -> Result<Run> {
        self.conn.execute(
            "INSERT INTO runs (session, started_at, loop_id) VALUES (?1, ?2, ?3)",
            params![session, at, loop_id],
        )?;
        Ok(Run {
            id: self.conn.last_insert_rowid(),
            session: session.to_owned(),
            started_at: at,
            ended_at: None,
            loop_id: loop_id.map(str::to_owned),
        })
    }

    pub fn end_run(&self, run: i64, at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
            params![run, at],
        )?;
        Ok(())
    }

    /// The run of `session` that has not ended, the latest if there are several.
    pub fn open_run(&self, session: &str) -> Result<Option<Run>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, session, started_at, ended_at, loop_id FROM runs
                 WHERE session = ?1 AND ended_at IS NULL ORDER BY id DESC LIMIT 1",
                [session],
                run_row,
            )
            .optional()?)
    }

    /// End every run of `session` but `keep` (left open by a brain that stopped mid-run).
    pub fn end_other_runs(&self, session: &str, keep: Option<i64>, at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE runs SET ended_at = ?3
             WHERE session = ?1 AND ended_at IS NULL AND id IS NOT ?2",
            params![session, keep, at],
        )?;
        Ok(())
    }

    /// Record an event of `run`; its row id.
    pub fn record(&self, run: i64, ev: &Event) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO events (run_id, at, kind, value, detail, room_map, room_room)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run,
                ev.at,
                ev.kind.as_str(),
                ev.value,
                ev.detail,
                ev.room.map(|k| k.0),
                ev.room.map(|k| k.1),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Add to an event's value: the exp that came after a kill.
    pub fn add_value(&self, event: i64, value: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE events SET value = value + ?2 WHERE id = ?1",
            params![event, value],
        )?;
        Ok(())
    }

    /// The events of `run`, in order.
    pub fn events(&self, run: i64) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT at, kind, value, detail, room_map, room_room FROM events
             WHERE run_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([run], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (at, kind, value, detail, map, room) = row?;
            out.push(Event {
                at,
                kind: EventKind::parse(&kind)?,
                value,
                detail,
                room: map.zip(room),
            });
        }
        Ok(out)
    }

    pub fn sample(&self, run: i64, s: &Sample) -> Result<()> {
        self.conn.execute(
            "INSERT INTO samples (run_id, at, exp, hp) VALUES (?1, ?2, ?3, ?4)",
            params![run, s.at, s.exp, s.hp],
        )?;
        Ok(())
    }

    /// The samples of `run`, in time order.
    pub fn samples(&self, run: i64) -> Result<Vec<Sample>> {
        let mut stmt = self
            .conn
            .prepare("SELECT at, exp, hp FROM samples WHERE run_id = ?1 ORDER BY at, rowid")?;
        let rows = stmt.query_map([run], |r| {
            Ok(Sample {
                at: r.get(0)?,
                exp: r.get(1)?,
                hp: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A run's events, counted.
    pub fn totals(&self, run: i64) -> Result<Totals> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, count(*), coalesce(sum(value), 0) FROM events
             WHERE run_id = ?1 GROUP BY kind",
        )?;
        let rows = stmt.query_map([run], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut t = Totals::default();
        for row in rows {
            let (kind, count, sum) = row?;
            let n = u64::try_from(count).unwrap_or(0);
            match EventKind::parse(&kind)? {
                EventKind::Kill => {
                    t.kills = n;
                    t.exp_gained += sum;
                }
                EventKind::Exp => t.exp_gained += sum,
                EventKind::Death => t.deaths = n,
                EventKind::Flee => t.flees = n,
                EventKind::Rest => t.rests = n,
                EventKind::LevelUp => t.level_ups = n,
            }
        }
        Ok(t)
    }

    /// The last `limit` runs of `session`, newest first, with their totals.
    pub fn runs(&self, session: &str, limit: usize) -> Result<Vec<RunSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session, started_at, ended_at, loop_id FROM runs
             WHERE session = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let runs = stmt
            .query_map(
                params![session, i64::try_from(limit).unwrap_or(i64::MAX)],
                run_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        runs.into_iter()
            .map(|run| {
                let totals = self.totals(run.id)?;
                Ok(RunSummary { run, totals })
            })
            .collect()
    }
}

fn run_row(r: &rusqlite::Row) -> rusqlite::Result<Run> {
    Ok(Run {
        id: r.get(0)?,
        session: r.get(1)?,
        started_at: r.get(2)?,
        ended_at: r.get(3)?,
        loop_id: r.get(4)?,
    })
}

/// The integers of a JSON array of integers or of arrays of them (`[[1,2],[3,4]]`, `[0,2]`).
fn ints(json: &str) -> Result<Vec<i64>> {
    json.split(|c: char| c == ',' || c == '[' || c == ']' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| Error::Bad(format!("not a list of integers: {json:?}")))
        })
        .collect()
}

/// Move a store that cannot be read, and its WAL files, to `<path>.corrupt` (replacing an
/// older one); where it went.
fn aside(path: &Path) -> Result<PathBuf> {
    let mut to = path.as_os_str().to_owned();
    to.push(".corrupt");
    let to = PathBuf::from(to);
    let io = |source| Error::Io {
        path: path.to_owned(),
        source,
    };
    std::fs::rename(path, &to).map_err(io)?;
    for ext in ["-wal", "-shm"] {
        let mut from = path.as_os_str().to_owned();
        from.push(ext);
        let mut dest = to.as_os_str().to_owned();
        dest.push(ext);
        if Path::new(&from).exists() {
            std::fs::rename(&from, &dest).map_err(io)?;
        }
    }
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ints_read_flat_and_nested_lists() {
        assert_eq!(ints("[]").unwrap(), Vec::<i64>::new());
        assert_eq!(ints("[0, 2]").unwrap(), [0, 2]);
        assert_eq!(ints("[[1,2150],[1,-2]]").unwrap(), [1, 2150, 1, -2]);
        assert!(ints("[a]").is_err());
    }
}
