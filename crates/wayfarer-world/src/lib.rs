//! The world pack: one SQLite file with the game's rooms and exits, monsters, items, shops,
//! spells, classes, races, messages and text blocks, plus MegaMud's waypoints and paths, each
//! path checked against the rooms (`docs/plan/02-architecture.md` "World data").
//!
//! [`build`] makes a pack from the stock-world export (`wayfarer import-world`);
//! [`WorldPack`] reads one. [`RoomGraph`] holds its rooms and exits in memory for [`route`],
//! the position [`Belief`] and the map [`layout`]; [`Monsters`] says what rooms spawn, and
//! [`loops`] makes circuits over an area's hunt rooms; [`Finder`] finds rooms, places and
//! monsters by name for "go to", and knows the boss lairs. [`combat`] is the game's combat
//! model: a character's blows against a monster, round by round, for the gear planner.

pub mod belief;
pub mod build;
pub mod combat;
pub mod cp437;
pub mod exit_items;
pub mod find;
pub mod gear;
pub mod knowledge;
pub mod layout;
pub mod loops;
pub mod mm;
pub mod monsters;
pub mod nav;
pub mod pack;
pub mod route;
pub mod schema;
pub mod spells;
pub mod spellshop;
pub mod trainers;
pub mod walk;

use std::path::PathBuf;

pub use belief::Belief;
pub use build::{BuildReport, MegaMudReport, build};
pub use combat::{Blow, Buff, Character, Defender, Fight, Fighter, Move, Stats};
pub use exit_items::ExitItem;
pub use find::{Finder, Found, FoundKind, FoundRoom, PlaceKind, Reach};
pub use knowledge::{PathExit, PathKnowledge, Window};
pub use layout::{Layout, Placed, Segment, SegmentKind, layout};
pub use loops::{Area, HuntRoom, Loop, MmLoop, generate, hunt_rooms, megamud_loops};
pub use monsters::{DEFAULT_RULES, MonsterRule, MonsterRules, Monsters, huntable};
pub use nav::{Category, DirSet, Edge, ExitType, Gate, Key, LockState, RoomGraph};
pub use pack::WorldPack;
pub use route::{
    Costs, Purchase, Route, Step, Traveller, distances, distances_any, distances_buying,
    pocket_exits, route, route_buying, route_or_search, route_within, route_within_or_search,
};
pub use spells::{BuffDef, SpellDef, SpellKind, Spells};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Format(String),
}
