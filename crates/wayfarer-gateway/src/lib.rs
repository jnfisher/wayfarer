//! The gateway: telnet sessions, output log, logon scripts, fan-out to UIs
//! (docs/plan/02-architecture.md).
//!
//! [`Gateway`] holds one session per character in the config. Each connected session is a
//! tokio task that owns the socket: it runs the bytes through [`wayfarer_telnet::Telnet`],
//! captures them raw, logs the decoded text as numbered chunks ([`log`]) and publishes them to
//! any number of subscribers, and writes input one command at a time. [`serve`] puts it on the
//! network (C-006); no logon scripts yet (C-005).

pub mod config;
mod gateway;
pub mod log;
mod server;
mod session;

pub use config::{Character, Config, ConfigError, Credentials, Login, Paths, Server};
pub use gateway::{Gateway, GatewayError, Options};
pub use log::{History, HistoryQuery};
pub use server::{
    ServerError, ServerOptions, bind, create_token, read_token, serve, token_for_bind,
};
pub use session::StateChange;
pub use wayfarer_proto::{Chunk, SessionInfo, SessionState};
