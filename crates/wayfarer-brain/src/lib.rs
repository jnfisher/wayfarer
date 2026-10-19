//! wayfarer-brain: the brain process (docs/brain.md). It attaches to the gateway as its brain
//! client, parses every session's output, keeps a character model per session, publishes
//! events and snapshots to the UIs, and re-syncs a character by asking the game (`st`, `exp`,
//! `i`, `look`). It sends nothing else: no bot yet.
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
mod link;
mod session;

pub use brain::{BUILD, Brain, BrainError, World};
pub use character::{Character, apply_patch, merge_patch};
pub use clock::{Clock, FixedClock, SystemClock, rfc3339};
pub use link::{ChannelLink, ConnectError, Link, WsLink};
pub use session::{ANSWER_TIMEOUT, HISTORY_CHUNKS, IDLE, OWN_PATIENCE, RESYNC_COMMANDS};
