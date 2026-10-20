//! Settings (docs/plan/04-settings.md): declared once, in [`CATALOGUE`]; resolved through
//! layers; validated; read and written as TOML that holds only what differs from the layer
//! below. This is the first slice: the declarations, the layers, the validation and the file
//! format, with the brain reading the thresholds it used to have as constants. The forms
//! (web and terminal), presets by class and style, per-character files and the MegaMud import
//! come later and build on this.
//!
//! ```toml
//! [health]
//! flee_below_hp = 35
//!
//! [movement]
//! search_limit = 500
//! ```

use std::collections::BTreeMap;
use std::fmt;

/// What a setting holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A whole number in `min..=max`.
    Int {
        min: i64,
        max: i64,
    },
    Bool,
}

/// A value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// How much a player needs to see it: the essentials page, the standard pages, or folded away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Essential,
    Standard,
    Advanced,
}

/// Where a setting lives in the forms (`docs/plan/04-settings.md` section 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Health,
    Movement,
    Bot,
}

impl Group {
    /// The table a setting of this group is written under in the TOML file.
    pub fn table(self) -> &'static str {
        match self {
            Group::Health => "health",
            Group::Movement => "movement",
            Group::Bot => "bot",
        }
    }
}

/// One setting, declared once. Everything the forms, the validation, the storage and the docs
/// need comes from this.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// `group.name`: the group's table and the name inside it in the TOML file.
    pub key: &'static str,
    /// "Flee below".
    pub label: &'static str,
    /// A sentence: what it does and when to change it.
    pub help: &'static str,
    pub kind: Kind,
    pub default: Value,
    /// The unit shown beside the number: `%`, `minutes`, `tries`; empty for a switch.
    pub unit: &'static str,
    pub tier: Tier,
    pub group: Group,
    /// MegaMud's name for it, for the import and for players who know MegaMud.
    pub megamud: Option<&'static str>,
}

/// Every setting. A test fails if one lacks help or a default inside its range.
pub const CATALOGUE: &[Setting] = &[
    Setting {
        key: "health.flee_below_hp",
        label: "Flee below",
        help: "Leave the fight and run to a room with no spawns when HP falls below this per cent of the maximum.",
        kind: Kind::Int { min: 0, max: 90 },
        default: Value::Int(30),
        unit: "%",
        tier: Tier::Essential,
        group: Group::Health,
        megamud: Some("HP Run If Below"),
    },
    Setting {
        key: "health.rest_below_hp",
        label: "Rest below",
        help: "Between fights, rest when HP is below this per cent of the maximum (never in a room that spawns monsters).",
        kind: Kind::Int { min: 10, max: 99 },
        default: Value::Int(80),
        unit: "%",
        tier: Tier::Essential,
        group: Group::Health,
        megamud: Some("HP Rest If Below"),
    },
    Setting {
        key: "health.rest_until_hp",
        label: "Rest until",
        help: "Keep resting until HP is back to this per cent of the maximum. A little under 100, so it does not wait a whole regeneration tick for a few points.",
        kind: Kind::Int { min: 20, max: 100 },
        default: Value::Int(95),
        unit: "%",
        tier: Tier::Essential,
        group: Group::Health,
        megamud: Some("HP Rest Until"),
    },
    Setting {
        key: "movement.search_limit",
        label: "Search for a hidden exit up to",
        help: "How many times to search for a hidden exit on the route before giving it up. Each search is a dice roll (the higher the perception, the likelier), so a character with low Int, Will and Charm needs many; give up only when the exit is not there at all.",
        kind: Kind::Int { min: 1, max: 1000 },
        default: Value::Int(300),
        unit: "tries",
        tier: Tier::Standard,
        group: Group::Movement,
        megamud: Some("Search each room"),
    },
    Setting {
        key: "bot.passed_by_minutes",
        label: "Leave a passed-by stop alone for",
        help: "A loop stop the bot could not get to, or left because of a monster to keep away from, is not tried again for this long.",
        kind: Kind::Int { min: 1, max: 120 },
        default: Value::Int(5),
        unit: "minutes",
        tier: Tier::Advanced,
        group: Group::Bot,
        megamud: None,
    },
];

/// The setting declared for `key`.
pub fn setting(key: &str) -> Option<&'static Setting> {
    CATALOGUE.iter().find(|s| s.key == key)
}

/// A layer, least specific first (`docs/plan/04-settings.md` section 2). Class presets sit
/// between the default and "my defaults" and arrive with the presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Default,
    /// My defaults, for all characters.
    Mine,
    /// This server.
    Server,
    /// This character.
    Character,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Layer::Default => "the default",
            Layer::Mine => "my defaults",
            Layer::Server => "this server",
            Layer::Character => "this character",
        })
    }
}

/// Why a value or a file was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No such setting.
    Unknown(String),
    /// The wrong kind of value: "flee_below_hp wants a whole number".
    Type { key: String, wants: &'static str },
    /// Outside its range.
    Range {
        key: String,
        value: i64,
        min: i64,
        max: i64,
    },
    /// The HP thresholds out of order (flee < rest below < rest until).
    Order(String),
    /// The file is not TOML, or not the shape settings take.
    Parse(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unknown(k) => write!(f, "no setting {k:?}"),
            Error::Type { key, wants } => write!(f, "{key} wants {wants}"),
            Error::Range {
                key,
                value,
                min,
                max,
            } => write!(f, "{key} is {value}, outside {min} to {max}"),
            Error::Order(why) => write!(f, "{why}"),
            Error::Parse(why) => write!(f, "settings file: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// A set of values from one layer: only what differs from the layers below it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    values: BTreeMap<&'static str, Value>,
}

impl Overrides {
    /// Set `key` to `value`, checked against its declaration.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), Error> {
        let s = setting(key).ok_or_else(|| Error::Unknown(key.to_owned()))?;
        check(s, value)?;
        self.values.insert(s.key, value);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.values.get(key).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Read a settings file: one table per group, `name = value` inside.
    pub fn parse(text: &str) -> Result<Overrides, Error> {
        let table: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| Error::Parse(e.message().to_owned()))?;
        let mut out = Overrides::default();
        for (group, body) in table {
            let toml::Value::Table(body) = body else {
                return Err(Error::Parse(format!("{group}: expected a table")));
            };
            for (name, v) in body {
                let key = format!("{group}.{name}");
                let value = match v {
                    toml::Value::Integer(n) => Value::Int(n),
                    toml::Value::Boolean(b) => Value::Bool(b),
                    _ => {
                        let wants = setting(&key).map_or("a number or a switch", |s| wants(s.kind));
                        return Err(Error::Type { key, wants });
                    }
                };
                out.set(&key, value)?;
            }
        }
        Ok(out)
    }

    /// The file text: only these values, grouped, with the label as a comment.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        for group in [Group::Health, Group::Movement, Group::Bot] {
            let mut rows = CATALOGUE
                .iter()
                .filter(|s| s.group == group)
                .filter_map(|s| self.values.get(s.key).map(|v| (s, v)))
                .peekable();
            if rows.peek().is_none() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("[{}]\n", group.table()));
            for (s, v) in rows {
                let name = s.key.split_once('.').map_or(s.key, |(_, n)| n);
                out.push_str(&format!("{name} = {v}  # {}\n", s.label));
            }
        }
        out
    }
}

fn wants(kind: Kind) -> &'static str {
    match kind {
        Kind::Int { .. } => "a whole number",
        Kind::Bool => "true or false",
    }
}

fn check(s: &Setting, v: Value) -> Result<(), Error> {
    match (s.kind, v) {
        (Kind::Int { min, max }, Value::Int(n)) => {
            if (min..=max).contains(&n) {
                Ok(())
            } else {
                Err(Error::Range {
                    key: s.key.to_owned(),
                    value: n,
                    min,
                    max,
                })
            }
        }
        (Kind::Bool, Value::Bool(_)) => Ok(()),
        (kind, _) => Err(Error::Type {
            key: s.key.to_owned(),
            wants: wants(kind),
        }),
    }
}

/// The values in force: every setting resolved through the layers, with where each came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    values: BTreeMap<&'static str, (Value, Layer)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            values: CATALOGUE
                .iter()
                .map(|s| (s.key, (s.default, Layer::Default)))
                .collect(),
        }
    }
}

impl Settings {
    /// The layers resolved, in the order given (least specific first); refused if the result
    /// puts the HP thresholds out of order.
    pub fn resolve(layers: &[(Layer, &Overrides)]) -> Result<Settings, Error> {
        let mut s = Settings::default();
        for &(layer, over) in layers {
            for (&key, &v) in &over.values {
                s.values.insert(key, (v, layer));
            }
        }
        s.check_order()?;
        Ok(s)
    }

    /// The value of a whole-number setting. A key that is not declared, or not a number, is a
    /// programming error: the catalogue is the only source of keys.
    pub fn int(&self, key: &str) -> i64 {
        match self.values.get(key) {
            Some((Value::Int(n), _)) => *n,
            _ => panic!("no whole-number setting {key:?}"),
        }
    }

    pub fn bool(&self, key: &str) -> bool {
        match self.values.get(key) {
            Some((Value::Bool(b), _)) => *b,
            _ => panic!("no switch setting {key:?}"),
        }
    }

    /// Which layer the value of `key` came from ("the default", "this character").
    pub fn source(&self, key: &str) -> Option<Layer> {
        self.values.get(key).map(|&(_, l)| l)
    }

    /// Flee < rest below < rest until, so the bot never flees into a rest it would not take,
    /// nor rests to a line it already counts as rested.
    fn check_order(&self) -> Result<(), Error> {
        let (flee, below, until) = (
            self.int("health.flee_below_hp"),
            self.int("health.rest_below_hp"),
            self.int("health.rest_until_hp"),
        );
        if flee >= below || below > until {
            return Err(Error::Order(format!(
                "the HP lines must run flee below ({flee}) < rest below ({below}) <= rest until ({until})"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_has_help_a_default_in_range_and_a_unique_key() {
        let mut seen = std::collections::BTreeSet::new();
        for s in CATALOGUE {
            assert!(seen.insert(s.key), "{} is declared twice", s.key);
            assert!(
                !s.label.is_empty() && s.help.len() > 20,
                "{} lacks help",
                s.key
            );
            assert!(
                s.help.ends_with('.'),
                "{}: help is a sentence ending in a full stop",
                s.key
            );
            assert!(
                check(s, s.default).is_ok(),
                "{} default is out of range",
                s.key
            );
            let (group, name) = s.key.split_once('.').expect("group.name");
            assert_eq!(
                group,
                s.group.table(),
                "{} is filed under the wrong group",
                s.key
            );
            assert!(!name.is_empty());
            match s.kind {
                Kind::Int { .. } => assert!(!s.unit.is_empty(), "{} has no unit", s.key),
                Kind::Bool => assert!(s.unit.is_empty()),
            }
        }
    }

    #[test]
    fn the_defaults_resolve_and_say_where_they_came_from() {
        let s = Settings::default();
        assert_eq!(s.int("health.flee_below_hp"), 30);
        assert_eq!(s.int("health.rest_below_hp"), 80);
        assert_eq!(s.int("health.rest_until_hp"), 95);
        assert_eq!(s.int("movement.search_limit"), 300);
        assert_eq!(s.int("bot.passed_by_minutes"), 5);
        assert_eq!(s.source("health.flee_below_hp"), Some(Layer::Default));
        assert_eq!(Settings::resolve(&[]).unwrap(), s);
    }

    #[test]
    fn the_most_specific_layer_wins() {
        let mine = Overrides::parse("[health]\nflee_below_hp = 40\nrest_below_hp = 85\n").unwrap();
        let character = Overrides::parse("[health]\nflee_below_hp = 25\n").unwrap();
        let s = Settings::resolve(&[(Layer::Mine, &mine), (Layer::Character, &character)]).unwrap();
        assert_eq!(s.int("health.flee_below_hp"), 25);
        assert_eq!(s.source("health.flee_below_hp"), Some(Layer::Character));
        assert_eq!(s.int("health.rest_below_hp"), 85);
        assert_eq!(s.source("health.rest_below_hp"), Some(Layer::Mine));
        assert_eq!(s.source("health.rest_until_hp"), Some(Layer::Default));
    }

    #[test]
    fn a_file_is_checked_against_the_declarations() {
        assert_eq!(
            Overrides::parse("[health]\nflee_below = 3\n").unwrap_err(),
            Error::Unknown("health.flee_below".into())
        );
        assert!(matches!(
            Overrides::parse("[health]\nflee_below_hp = 95\n").unwrap_err(),
            Error::Range {
                value: 95,
                max: 90,
                ..
            }
        ));
        assert!(matches!(
            Overrides::parse("[health]\nflee_below_hp = \"low\"\n").unwrap_err(),
            Error::Type { .. }
        ));
        assert!(matches!(
            Overrides::parse("flee_below_hp = 3\n").unwrap_err(),
            Error::Parse(_)
        ));
        assert!(matches!(
            Overrides::parse("[health\n").unwrap_err(),
            Error::Parse(_)
        ));
    }

    #[test]
    fn the_hp_lines_must_run_in_order() {
        let bad = Overrides::parse("[health]\nflee_below_hp = 85\n").unwrap();
        assert!(matches!(
            Settings::resolve(&[(Layer::Mine, &bad)]),
            Err(Error::Order(_))
        ));
        let bad = Overrides::parse("[health]\nrest_below_hp = 98\nrest_until_hp = 90\n").unwrap();
        assert!(matches!(
            Settings::resolve(&[(Layer::Character, &bad)]),
            Err(Error::Order(_))
        ));
        // Equal rest lines are fine: rested the moment it is not below.
        let ok = Overrides::parse("[health]\nrest_below_hp = 90\nrest_until_hp = 90\n").unwrap();
        assert!(Settings::resolve(&[(Layer::Character, &ok)]).is_ok());
    }

    #[test]
    fn a_file_holds_only_what_differs_and_round_trips() {
        let mut o = Overrides::default();
        o.set("movement.search_limit", Value::Int(500)).unwrap();
        o.set("health.flee_below_hp", Value::Int(35)).unwrap();
        let text = o.to_toml();
        assert_eq!(
            text,
            "[health]\nflee_below_hp = 35  # Flee below\n\n[movement]\nsearch_limit = 500  # Search for a hidden exit up to\n"
        );
        assert_eq!(Overrides::parse(&text).unwrap(), o);
        assert_eq!(Overrides::default().to_toml(), "");
        assert!(Overrides::parse("").unwrap().is_empty());
    }
}
