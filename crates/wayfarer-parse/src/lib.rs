//! wayfarer-parse: the game's terminal text to a stream of named events with fields.
//!
//! A port of the prototype's `test/play/patterns.py` (jnfisher/mmud-exploration): the static
//! pattern table, patterns generated from the game's own message records (WCCMSG), the prompt
//! in its statline forms, wrapped displays joined, and tables. Text is fed incrementally; a
//! line is classified when it is complete and the prompt when a chunk ends with it.
//! [`typed::GameEvent`] reads the events the character model needs as typed values.
//!
//! ```no_run
//! use std::sync::Arc;
//! use wayfarer_parse::{GameTexts, Parser, Registry};
//!
//! let texts = GameTexts::from_export("tests/fixtures/world").unwrap();
//! let mut parser = Parser::new(Arc::new(Registry::new(&texts)));
//! for ev in parser.feed("\x1b[79D\x1b[K\x1b[0;37m[HP=35]:") {
//!     println!("{} {:?}", ev.name, ev.fields);
//! }
//! ```

mod event;
mod game_texts;
pub mod helpers;
mod parser;
mod prompt;
mod pyre;
mod registry;
mod table;
mod text;
pub mod typed;

pub use event::{Event, Fields, Value};
pub use game_texts::{GameTexts, ItemTexts, Message, MonsterAttack, MonsterTexts};
pub use parser::Parser;
pub use prompt::{Peeled, PromptMode, PromptParser, PromptProgress, compile_statline};
pub use registry::{JoinRule, Pattern, Registry, split_monster, template_regex};
pub use text::LineCutter;
