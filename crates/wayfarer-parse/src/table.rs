//! The prototype's static table, embedded as data: `patterns.py export` output.
//!
//! The export sorts the static patterns by category and name; ties between equally specific
//! patterns are decided by their order in `patterns.STATIC`, so that order is embedded too.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::Deserialize;

const PATTERNS_JSON: &str = include_str!("../../../tests/fixtures/parse-golden/patterns.json");
const STATIC_ORDER: &str = include_str!("../../../tests/fixtures/parse-golden/static-order.txt");

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
