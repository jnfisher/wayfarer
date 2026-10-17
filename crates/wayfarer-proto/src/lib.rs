//! Wire messages between the gateway, the brain and the UIs (docs/plan/02-architecture.md,
//! "The wire protocol"). No I/O.
//!
//! Every message is a JSON object whose `"type"` field names it. The gateway sends
//! [`ToClient`] and receives [`ToGateway`]. `docs/protocol.md` lists them with an example
//! each; the TypeScript for the web UI is generated into `ui/src/proto/` ([`export_typescript`]).
//!
//! M0 has the hello, session, terminal and input families. Adding a variant later bumps
//! nothing; removing or changing one bumps [`PROTOCOL`].

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The protocol version both sides send in the handshake. A mismatch is refused.
pub const PROTOCOL: u32 = 1;

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
    Refused { reason: String },
    /// The full list of sessions, sent when it changes.
    SessionList { sessions: Vec<SessionInfo> },
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
    Lagged { session: SessionId, missed: u64 },
    /// Something the client sent could not be handled; the connection stays up.
    Error { message: String },
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
    ];

    /// This message's `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::InputLine { .. } => "input_line",
            Self::InputKeys { .. } => "input_keys",
            Self::HistoryRequest { .. } => "history_request",
            Self::Subscribe { .. } => "subscribe",
        }
    }

    pub fn from_json(text: &str) -> Result<Self, DecodeError> {
        decode(text, Self::TYPES)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("wire messages always serialize")
    }
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
