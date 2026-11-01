//! The prototype's static table, embedded as data: `patterns.py export` output.
//!
//! The export sorts the static patterns by category and name; ties between equally specific
//! patterns are decided by their order in `patterns.STATIC`, so that order is embedded too.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::Deserialize;

const PATTERNS_JSON: &str = include_str!("../../../tests/fixtures/parse-golden/patterns.json");
const STATIC_ORDER: &str = include_str!("../../../tests/fixtures/parse-golden/static-order.txt");
/// The lines Quicksilver 1.20 adds (`docs/plan/quicksilver-1.20/handoff.md` section 3), and
/// its regen line's spell texts (section 3.8).
const QS120_JSON: &str = include_str!("../../../tests/fixtures/qs120/patterns-1.20.json");
const QS120_REGEN_JSON: &str =
    include_str!("../../../tests/fixtures/qs120-regen/patterns-1.20.json");

#[derive(Debug, Deserialize)]
pub struct Table {
    #[serde(rename = "static")]
    pub statics: Vec<Static>,
    pub tables: Vec<RowTable>,
    pub join: Vec<Join>,
    pub exit_tail: Vec<String>,
    pub prompt: PromptForms,
    pub roles: Roles,
}

#[derive(Debug, Deserialize)]
pub struct Static {
    pub name: String,
    pub cat: String,
    pub rx: String,
    pub example: String,
    pub seen: String,
    pub prio: i32,
}

/// A static pattern of a rule set other than stock 1.11p: one of Quicksilver 1.20's new lines.
#[derive(Debug, Deserialize)]
pub struct RulesStatic {
    pub name: String,
    pub cat: String,
    pub rx: String,
    pub example: String,
    pub seen: String,
    pub prio: i32,
    /// The rule set whose server prints the line (`"1.20"`). A property of the data, never a
    /// detection signal: the patterns are registered for every server.
    pub rules: String,
}

#[derive(Debug, Deserialize)]
struct RulesTable {
    #[serde(rename = "static")]
    statics: Vec<RulesStatic>,
}

/// Quicksilver 1.20's patterns, in the file's order.
pub fn qs120() -> &'static [RulesStatic] {
    static QS120: OnceLock<Vec<RulesStatic>> = OnceLock::new();
    QS120.get_or_init(|| {
        [QS120_JSON, QS120_REGEN_JSON]
            .into_iter()
            .flat_map(|json| {
                let t: RulesTable =
                    serde_json::from_str(json).expect("embedded patterns-1.20.json parses");
                t.statics
            })
            .collect()
    })
}

#[derive(Debug, Deserialize)]
pub struct RowTable {
    pub opener: String,
    pub row_name: String,
    pub cat: String,
    pub rx: String,
    pub example: String,
    pub max_rows: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Join {
    pub prefix: String,
    pub rule: String,
}

#[derive(Debug, Deserialize)]
pub struct PromptForms {
    pub std: String,
    pub generic: String,
    pub statline_codes: BTreeMap<String, StatlineCode>,
    pub statline_len: usize,
}

#[derive(Debug, Deserialize)]
pub struct StatlineCode {
    pub field: Option<String>,
    pub rx: String,
}

#[derive(Debug, Deserialize)]
pub struct Roles {
    pub args: BTreeMap<String, Vec<String>>,
}

/// The embedded table, static patterns in `patterns.STATIC` order.
pub fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t: Table =
            serde_json::from_str(PATTERNS_JSON).expect("embedded patterns.json parses");
        let order: HashMap<&str, usize> = STATIC_ORDER
            .lines()
            .enumerate()
            .map(|(i, n)| (n, i))
            .collect();
        assert_eq!(
            order.len(),
            t.statics.len(),
            "static-order.txt names every static pattern once"
        );
        t.statics.sort_by_key(|s| order[s.name.as_str()]);
        t
    })
}
