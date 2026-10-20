//! The world pack: one SQLite file with the game's rooms and exits, monsters, items, shops,
//! spells, classes, races, messages and text blocks, plus MegaMud's waypoints and paths, each
//! path checked against the rooms (`docs/plan/02-architecture.md` "World data").
//!
//! [`build`] makes a pack from the stock-world export (`wayfarer import-world`);
//! [`WorldPack`] reads one. [`RoomGraph`] holds its rooms and exits in memory for [`route`],
//! the position [`Belief`] and the map [`layout`]; [`Monsters`] says what rooms spawn, and
//! [`loops`] makes circuits over an area's hunt rooms.

pub mod belief;
pub mod build;
pub mod cp437;
pub mod layout;
pub mod loops;
pub mod mm;
pub mod monsters;
pub mod nav;
pub mod pack;
pub mod route;
pub mod schema;
pub mod walk;

use std::path::PathBuf;

pub use belief::Belief;
pub use build::{BuildReport, MegaMudReport, build};
pub use layout::{Layout, Placed, Segment, SegmentKind, layout};
pub use loops::{Area, HuntRoom, Loop, MmLoop, generate, hunt_rooms};
pub use monsters::{Monsters, huntable};
pub use nav::{Category, DirSet, Edge, ExitType, Gate, Key, LockState, RoomGraph};
pub use pack::WorldPack;
pub use route::{Costs, Route, Step, Traveller, distances, route, route_within};

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
