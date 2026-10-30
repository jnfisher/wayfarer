//! The gateway: telnet sessions, output log, logon scripts, fan-out to UIs
//! (docs/plan/02-architecture.md).
//!
//! [`Gateway`] holds one session per character in the config. Each connected session is a
//! tokio task that owns the socket: it runs the bytes through [`wayfarer_telnet::Telnet`],
//! captures them raw, logs the decoded text as numbered chunks ([`log`]) and publishes them to
//! any number of subscribers, and writes input one command at a time. It runs the server's
//! logon script ([`logon`]), follows the text to know where the character is, reconnects a
//! lost session with backoff and logs every character off cleanly on shutdown. [`serve`] puts
//! it on the network (C-006). The web UI's Accounts panel edits the config through it
//! ([`Gateway::save_accounts`], [`accounts`]).

pub mod accounts;
pub mod config;
mod gateway;
pub mod log;
pub mod logon;
pub mod replay;
mod server;
mod session;

pub use config::{Changes, Character, Config, ConfigError, Credentials, Login, Paths, Server};
pub use gateway::{
    DEFAULT_EMAIL, DEFAULT_GENDER, Gateway, GatewayError, MAX_CHARACTERS, Options, stop_signals,
};
pub use log::{History, HistoryQuery};
pub use logon::Script;
pub use server::{
    Hub, ServerError, ServerOptions, bind, create_token, read_token, serve, token_for_bind,
};
pub use session::{
    Connect, Connecting, Origin, Output, Recorded, Sent, Signup, StateChange, Stream, Tcp,
};
pub use wayfarer_proto::{Chunk, SessionInfo, SessionState};
