use std::collections::BTreeMap;

use serde::Serialize;

/// One field of an event: the text of a named group, a number (prompt values, `msgnum`), the
/// monster names a death text belongs to, or nothing (a prompt without a resting state).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Int(i64),
    Text(String),
    List(Vec<String>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.to_owned())
    }
}

/// An event's fields by name, as `patterns.py` reports them.
pub type Fields = BTreeMap<String, Value>;

/// One thing the game said: a named event with its fields. Every line yields at least one
/// event; a line nothing recognises is `unknown` (category `unclassified`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Event {
    /// The event name, as in `patterns.py`: `you_hit`, `prompt`, `room_title`, `room_exits`,
    /// a generated role such as `mon_hits_you`, or `wccmsg` for any other game message.
    pub name: String,
    /// The category: `combat`, `room`, `prompt`, `message`, `unclassified`, ...
    pub cat: String,
    pub fields: Fields,
    /// The text the event was read from: the line (stripped, joined when wrapped), or for a
    /// prompt the prompt's own text.
    pub line: String,
    /// The chunk (`Parser::feed` call) in which the line was completed, or, for a prompt seen
    /// at the end of a chunk, that chunk.
    pub seq: u64,
    /// The index of the line in the session's text, counting from 0 (lines as
    /// `patterns.raw_to_lines` cuts them).
    pub line_no: u64,
}

impl Event {
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.fields.get(name).and_then(Value::as_str)
    }
}
