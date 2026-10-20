//! wayfarer-brain: the brain process (docs/brain.md). It attaches to the gateway as its brain
//! client, parses every session's output, keeps a character model per session, publishes
//! events and snapshots to the UIs, and re-syncs a character by asking the game (`st`, `exp`,
//! `i`, `look`). With a world pack it also keeps each character's position, serves the map
//! panel, walks a character to a room (`walk_to`) through the [`executor`], and plays it where
//! it stands when asked (`bot_control`: "fight here", fleeing below the flee line, with the
//! [`policy`] constants). With a [`Store`] it keeps the bot's state and the runs' statistics
//! on disk, and resumes each bot after a restart.
//!
//! ```no_run
//! # async fn f() -> Result<(), Box<dyn std::error::Error>> {
//! use std::sync::Arc;
//! use wayfarer_brain::{Brain, SystemClock, World, WsLink};
//!
//! let world = World::open("world.sqlite".as_ref())?;
//! let mut brain = Brain::new(world, Arc::new(SystemClock), None);
//! let mut link = WsLink::connect("ws://127.0.0.1:7373/ws").await?;
//! brain.run(&mut link).await?; // until the gateway goes; then connect again
//! # Ok(()) }
//! ```

mod brain;
pub mod character;
mod clock;
pub mod commands;
pub mod executor;
mod fight;
mod flee;
mod link;
mod modes;
pub mod policy;
mod position;
mod recover;
mod rules;
mod session;
mod stats;
mod task;
mod walker;

pub use brain::{Atlas, BUILD, Brain, BrainError, World};
pub use character::{Character, apply_patch, merge_patch};
pub use clock::{Clock, FixedClock, InstantClock, ReplayClock, Sleep, SystemClock, rfc3339};
pub use fight::{ATTACK_TRIES, ROUND_GAP, STALEMATE_ROUNDS};
pub use link::{ChannelLink, ConnectError, Link, WsLink};
pub use position::HIDDEN_FOR;
pub use rules::AVOID_MEMORY;
pub use session::{
    ANSWER_TIMEOUT, HISTORY_CHUNKS, IDLE, MAP_RADIUS, OWN_PATIENCE, RESYNC_COMMANDS,
};
pub use stats::{SAMPLE_EVERY, STATS_RUNS};
pub use task::{GIVE_UPS, LEG_TRIES, STOCK_REGEN, STOP_QUIET, ready_in, stop_ready};
pub use walker::{MAX_BASHES, MAX_RELOCATIONS, MAX_REPLANS, MAX_SEARCHES};
pub use wayfarer_store::Store;
