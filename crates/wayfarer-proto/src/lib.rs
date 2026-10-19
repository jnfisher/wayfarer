//! Wire messages between the gateway, the brain and the UIs (docs/plan/02-architecture.md,
//! "The wire protocol"). No I/O.
//!
//! Every message is a JSON object whose `"type"` field names it. The gateway sends
//! [`ToClient`] and receives [`ToGateway`]. `docs/protocol.md` lists them with an example
//! each; the TypeScript for the web UI is generated into `ui/src/proto/` ([`export_typescript`]).
//!
//! M0 has the hello, session, terminal and input families; M1 adds the brain's: events, state
//! and dev (brain → UIs), control (UI → brain) and `sent` (gateway → brain). [`Family`] says
//! who may send what. Adding a variant later bumps nothing; removing or changing one bumps
//! [`PROTOCOL`].

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
    /// Connected, with no logon script running: none is configured, or it stopped and the
    /// player has the session (character creation, say).
    Connected,
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

/// What the brain knows about one character: the wire's `character` snapshot, and the shape a
/// `character_patch` (a JSON merge patch, RFC 7386) applies to. Every field is left out until
/// it is known; in a patch, `null` clears one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct CharacterState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// Hit points; below zero while dying.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp_max: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana_max: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>,
    /// Experience still needed for the next level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp_next: Option<u64>,
    /// How far through the level, as the game shows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp_percent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lives: Option<u32>,
    /// Unspent character points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cp: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<Stats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_exits: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_combat: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resting: Option<bool>,
    /// `poisoned`, `blind`, `held`, `diseased`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statuses: Option<Vec<String>>,
    /// Which form the character's last prompt had (`hp`, `hp_mana`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse: Option<ParseCounts>,
    /// When the last re-sync finished (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
    /// The output `seq` this snapshot reflects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_seq: Option<u64>,
}

/// A character's six stats.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct Stats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intellect: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub willpower: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agility: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charm: Option<u32>,
}

/// How the parser is doing on a character, counted since the brain attached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ParseCounts {
    pub lines: u64,
    /// Lines nothing recognised (`unknown` events).
    pub unclassified: u64,
}

/// The world pack a brain loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct WorldPackInfo {
    pub path: String,
    pub hash: String,
}

/// Who may send a [`ToGateway`] message, and what the gateway does with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Hello, input, history and subscribe: from any client, handled by the gateway.
    Gateway,
    /// From a UI, relayed to the brain: `resync`, `state_request`.
    Control,
    /// From the brain, relayed to the UIs: events, state and dev (`event`, `character`,
    /// `character_patch`, `brain_info`, `notice`, `warning`).
    Brain,
}

impl Family {
    /// Whether a client of this kind may send messages of this family.
    pub fn allowed_from(self, client: ClientKind) -> bool {
        match self {
            Self::Gateway => true,
            Self::Control => client == ClientKind::Ui,
            Self::Brain => client == ClientKind::Brain,
        }
    }
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
    Refused {
        reason: String,
    },
    /// The full list of sessions, sent when it changes.
    SessionList {
        sessions: Vec<SessionInfo>,
    },
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
    Lagged {
        session: SessionId,
        missed: u64,
    },
    /// Something the client sent could not be handled; the connection stays up.
    Error {
        message: String,
    },

    // From the brain, relayed to the UIs subscribed to `session` (to every UI when there is
    // none). `ToGateway` says what each one means.
    Event {
        session: SessionId,
        seq: u64,
        line_no: u64,
        name: String,
        cat: String,
        #[ts(type = "Record<string, unknown>")]
        fields: serde_json::Map<String, serde_json::Value>,
        line: String,
    },
    Character {
        session: SessionId,
        state: Box<CharacterState>,
    },
    CharacterPatch {
        session: SessionId,
        #[ts(type = "unknown")]
        patch: serde_json::Value,
    },
    BrainInfo {
        build: String,
        started_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        world_pack: Option<WorldPackInfo>,
    },
    Notice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
    },
    Warning {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
    },

    // From a UI, relayed to the brain.
    Resync {
        session: SessionId,
    },
    StateRequest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
    },

    /// To the brain only: a line went to the game, typed by a client or sent by a logon or
    /// log-off step; `seq` is the session's latest output `seq` at that moment. A password is
    /// `********`.
    Sent {
        session: SessionId,
        seq: u64,
        text: String,
    },
    /// To UIs only: whether a brain is attached. Sent right after `welcome` and whenever it
    /// changes.
    BrainState {
        attached: bool,
    },
    /// To a UI only: it fell behind and `missed` relayed brain messages were dropped. Recover
    /// with a `state_request`.
    BrainLagged {
        missed: u64,
    },
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

    /// UI → brain: re-sync the character (`st`, `exp`, `i`, look).
    Resync { session: SessionId },
    /// UI → brain: send a fresh `character` snapshot (of one session, or of all) and a
    /// `brain_info`.
    StateRequest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
    },

    /// Brain → UIs: one parsed event, as `wayfarer-parse` produced it, with the output `seq`
    /// its line came from. The gateway never reads `fields`.
    Event {
        session: SessionId,
        seq: u64,
        line_no: u64,
        name: String,
        cat: String,
        #[ts(type = "Record<string, unknown>")]
        fields: serde_json::Map<String, serde_json::Value>,
        line: String,
    },
    /// Brain → UIs: the whole character snapshot.
    Character {
        session: SessionId,
        state: Box<CharacterState>,
    },
    /// Brain → UIs: a JSON merge patch (RFC 7386) to apply to the last snapshot. The gateway
    /// never reads it.
    CharacterPatch {
        session: SessionId,
        #[ts(type = "unknown")]
        patch: serde_json::Value,
    },
    /// Brain → UIs: who the brain is. `started_at` is RFC 3339.
    BrainInfo {
        build: String,
        started_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        world_pack: Option<WorldPackInfo>,
    },
    /// Brain → UIs: something the brain did or saw, worth a line in the dev panel.
    Notice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
    },
    /// Brain → UIs: something wrong but not fatal.
    Warning {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        session: Option<SessionId>,
        text: String,
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
        "event",
        "character",
        "character_patch",
        "brain_info",
        "notice",
        "warning",
        "resync",
        "state_request",
        "sent",
        "brain_state",
        "brain_lagged",
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
            Self::Event { .. } => "event",
            Self::Character { .. } => "character",
            Self::CharacterPatch { .. } => "character_patch",
            Self::BrainInfo { .. } => "brain_info",
            Self::Notice { .. } => "notice",
            Self::Warning { .. } => "warning",
            Self::Resync { .. } => "resync",
            Self::StateRequest { .. } => "state_request",
            Self::Sent { .. } => "sent",
            Self::BrainState { .. } => "brain_state",
            Self::BrainLagged { .. } => "brain_lagged",
        }
    }

    /// The session this message is about, if it names one.
    pub fn session(&self) -> Option<&str> {
        match self {
            Self::SessionState { session, .. }
            | Self::Output { session, .. }
            | Self::History { session, .. }
            | Self::Lagged { session, .. }
            | Self::Event { session, .. }
            | Self::Character { session, .. }
            | Self::CharacterPatch { session, .. }
            | Self::Resync { session }
            | Self::Sent { session, .. } => Some(session),
            Self::Notice { session, .. }
            | Self::Warning { session, .. }
            | Self::StateRequest { session } => session.as_deref(),
            Self::Welcome { .. }
            | Self::Refused { .. }
            | Self::SessionList { .. }
            | Self::Error { .. }
            | Self::BrainInfo { .. }
            | Self::BrainState { .. }
            | Self::BrainLagged { .. } => None,
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
        "resync",
        "state_request",
        "event",
        "character",
        "character_patch",
        "brain_info",
        "notice",
        "warning",
    ];

    /// This message's `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::InputLine { .. } => "input_line",
            Self::InputKeys { .. } => "input_keys",
            Self::HistoryRequest { .. } => "history_request",
            Self::Subscribe { .. } => "subscribe",
            Self::Resync { .. } => "resync",
            Self::StateRequest { .. } => "state_request",
            Self::Event { .. } => "event",
            Self::Character { .. } => "character",
            Self::CharacterPatch { .. } => "character_patch",
            Self::BrainInfo { .. } => "brain_info",
            Self::Notice { .. } => "notice",
            Self::Warning { .. } => "warning",
        }
    }

    /// Who may send this message, and whether the gateway relays it.
    pub fn family(&self) -> Family {
        match self {
            Self::Hello { .. }
            | Self::InputLine { .. }
            | Self::InputKeys { .. }
            | Self::HistoryRequest { .. }
            | Self::Subscribe { .. } => Family::Gateway,
            Self::Resync { .. } | Self::StateRequest { .. } => Family::Control,
            Self::Event { .. }
            | Self::Character { .. }
            | Self::CharacterPatch { .. }
            | Self::BrainInfo { .. }
            | Self::Notice { .. }
            | Self::Warning { .. } => Family::Brain,
        }
    }

    /// A [`Family::Control`] or [`Family::Brain`] message as the gateway relays it, unchanged;
    /// `None` for a message the gateway handles itself.
    pub fn into_relayed(self) -> Option<ToClient> {
        Some(match self {
            Self::Resync { session } => ToClient::Resync { session },
            Self::StateRequest { session } => ToClient::StateRequest { session },
            Self::Event {
                session,
                seq,
                line_no,
                name,
                cat,
                fields,
                line,
            } => ToClient::Event {
                session,
                seq,
                line_no,
                name,
                cat,
                fields,
                line,
            },
            Self::Character { session, state } => ToClient::Character { session, state },
            Self::CharacterPatch { session, patch } => ToClient::CharacterPatch { session, patch },
            Self::BrainInfo {
                build,
                started_at,
                world_pack,
            } => ToClient::BrainInfo {
                build,
                started_at,
                world_pack,
            },
            Self::Notice { session, text } => ToClient::Notice { session, text },
            Self::Warning { session, text } => ToClient::Warning { session, text },
            Self::Hello { .. }
            | Self::InputLine { .. }
            | Self::InputKeys { .. }
            | Self::HistoryRequest { .. }
            | Self::Subscribe { .. } => return None,
        })
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
