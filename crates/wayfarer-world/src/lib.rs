//! The world pack: one SQLite file with the game's rooms and exits, monsters, items, shops,
//! spells, classes, races, messages and text blocks, plus MegaMud's waypoints and paths, each
//! path checked against the rooms (`docs/plan/02-architecture.md` "World data").
//!
//! [`build`] makes a pack from the stock-world export (`wayfarer import-world`);
//! [`WorldPack`] reads one.

pub mod build;
pub mod cp437;
pub mod mm;
pub mod pack;
pub mod schema;
pub mod walk;

use std::path::PathBuf;

pub use build::{BuildReport, MegaMudReport, build};
pub use pack::WorldPack;

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
