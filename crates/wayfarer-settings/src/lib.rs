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
    /// One of these words.
    Choice(&'static [&'static str]),
}

/// A value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Choice(&'static str),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Choice(c) => write!(f, "\"{c}\""),
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
        default: Value::Int(25),
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
        default: Value::Int(60),
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
        default: Value::Int(85),
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
    Setting {
        key: "bot.style",
        label: "Style",
        help: "How much risk the bot takes: Careful flees early and rests to nearly full, Bold fights on to a lower line and rests less; Balanced is the default. A style sets the HP lines below, and a line you set yourself wins over it.",
        kind: Kind::Choice(&["careful", "balanced", "bold"]),
        default: Value::Choice("balanced"),
        unit: "",
        tier: Tier::Essential,
        group: Group::Bot,
        megamud: None,
    },
    Setting {
        key: "bot.leg_tries",
        label: "Tries to reach a stop",
        help: "Walks to one loop stop that end somewhere else, with no fight between, before the stop is passed by for now.",
        kind: Kind::Int { min: 1, max: 10 },
        default: Value::Int(3),
        unit: "tries",
        tier: Tier::Advanced,
        group: Group::Bot,
        megamud: None,
    },
    Setting {
        key: "bot.give_ups",
        label: "Stops passed by before the bot stops itself",
        help: "Loop stops passed by in a row, with none reached and nothing killed between, before the bot stops and says the loop cannot go round.",
        kind: Kind::Int { min: 1, max: 20 },
        default: Value::Int(3),
        unit: "stops",
        tier: Tier::Advanced,
        group: Group::Bot,
        megamud: None,
    },
    Setting {
        key: "bot.stop_quiet_seconds",
        label: "Quiet time that clears a stop",
        help: "At a stop where only monsters the bot will not attack are left, the stop counts as cleared once your own combat has been quiet this long.",
        kind: Kind::Int { min: 5, max: 120 },
        default: Value::Int(20),
        unit: "seconds",
        tier: Tier::Advanced,
        group: Group::Bot,
        megamud: None,
    },
];

/// The setting declared for `key`.
pub fn setting(key: &str) -> Option<&'static Setting> {
    CATALOGUE.iter().find(|s| s.key == key)
}

/// What a style sets, under every layer: the three HP lines. Balanced sets nothing (the
/// defaults are Balanced: flee below 25, rest below 60 to 85). Careful flees at 30 and rests
/// below 70 to 90; Bold flees at 20 and rests below 50 to 80 (John, 2026-10-20: every style
/// fights on to a lower line than the first draft's). Each keeps flee < rest below <= rest
/// until.
pub fn preset(style: &str) -> Overrides {
    let lines: &[(&str, i64)] = match style {
        "careful" => &[
            ("health.flee_below_hp", 30),
            ("health.rest_below_hp", 70),
            ("health.rest_until_hp", 90),
        ],
        "bold" => &[
            ("health.flee_below_hp", 20),
            ("health.rest_below_hp", 50),
            ("health.rest_until_hp", 80),
        ],
        _ => &[],
    };
    let mut over = Overrides::default();
    for &(key, v) in lines {
        over.set(key, Value::Int(v))
            .expect("a preset is within its ranges");
    }
    over
}

/// A settings file that could not be used: which, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError {
    pub path: std::path::PathBuf,
    pub error: Error,
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.error)
    }
}

impl std::error::Error for FileError {}

/// The settings files in a directory (the data directory): `settings.toml` for all
/// characters, `servers/<host>_<port>.toml` for one server and `characters/<character id>.toml`
/// for one character, each holding only what differs from the layer below it. A file that is
/// not there is an empty layer; one that is there and not valid is an error, never a silent
/// default.
#[derive(Debug, Clone)]
pub struct SettingsDir {
    dir: std::path::PathBuf,
}

/// An id fit to be a file name: letters, digits, `-` and `_`.
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The file name of a server (`host:port` as the gateway names it): the colon becomes `_`, so
/// `127.0.0.1:2323` is `127.0.0.1_2323`. `None` for a name that is not safe as a file name.
fn server_file(server: &str) -> Option<String> {
    let name = server.replace(':', "_");
    let safe = !name.is_empty()
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    safe.then_some(name)
}

impl SettingsDir {
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The files that make up a character's settings, least specific first, whether or not
    /// they exist.
    pub fn files(&self, server: &str, character: &str) -> Vec<(Layer, std::path::PathBuf)> {
        let mut files = vec![(Layer::Mine, self.dir.join("settings.toml"))];
        if let Some(server) = server_file(server) {
            let name = format!("{server}.toml");
            files.push((Layer::Server, self.dir.join("servers").join(name)));
        }
        if safe_id(character) {
            let name = format!("{character}.toml");
            files.push((Layer::Character, self.dir.join("characters").join(name)));
        }
        files
    }

    /// The settings for `character` on `server`: the files' layers over the style preset.
    pub fn resolve(&self, server: &str, character: &str) -> Result<Settings, FileError> {
        let files = self.files(server, character);
        let mut layers = Vec::new();
        for (layer, path) in &files {
            let text = match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(FileError {
                        path: path.clone(),
                        error: Error::Parse(e.to_string()),
                    });
                }
            };
            let over = Overrides::parse(&text).map_err(|error| FileError {
                path: path.clone(),
                error,
            })?;
            layers.push((*layer, over));
        }
        let refs: Vec<(Layer, &Overrides)> = layers.iter().map(|(l, o)| (*l, o)).collect();
        // An order error belongs to the files together: name the most specific one.
        Settings::resolve(&refs).map_err(|error| FileError {
            path: files.last().map(|(_, p)| p.clone()).unwrap_or_default(),
            error,
        })
    }
}

/// A layer, least specific first (`docs/plan/04-settings.md` section 2). Class presets would sit
/// beside the style preset, between the default and "my defaults".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Default,
    /// The style preset (`bot.style`: careful, balanced or bold).
    Style,
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
            Layer::Style => "the style",
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
    /// A word that is not one of the options.
    Choice {
        key: String,
        value: String,
        options: &'static [&'static str],
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
            Error::Choice {
                key,
                value,
                options,
            } => write!(f, "{key} is {value:?}, not one of {}", options.join(", ")),
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
                    toml::Value::String(word) => match setting(&key).map(|s| s.kind) {
                        Some(Kind::Choice(options)) => {
                            match options.iter().find(|o| o.eq_ignore_ascii_case(word.trim())) {
                                Some(&o) => Value::Choice(o),
                                None => {
                                    return Err(Error::Choice {
                                        key,
                                        value: word,
                                        options,
                                    });
                                }
                            }
                        }
                        Some(kind) => {
                            return Err(Error::Type {
                                key,
                                wants: wants(kind),
                            });
                        }
                        None => return Err(Error::Unknown(key)),
                    },
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
        Kind::Choice(_) => "one of its words in quotes",
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
        (Kind::Choice(options), Value::Choice(word)) if options.contains(&word) => Ok(()),
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
        // The style is the most specific one named; its preset sits under every layer given.
        let style = layers
            .iter()
            .rev()
            .find_map(|(_, over)| match over.get("bot.style") {
                Some(Value::Choice(c)) => Some(c),
                _ => None,
            })
            .unwrap_or("balanced");
        for (&key, &v) in &preset(style).values {
            s.values.insert(key, (v, Layer::Style));
        }
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

    /// The word of a choice setting.
    pub fn choice(&self, key: &str) -> &'static str {
        match self.values.get(key) {
            Some((Value::Choice(c), _)) => c,
            _ => panic!("no choice setting {key:?}"),
        }
    }

    /// The value of `key`, whatever its kind.
    pub fn value(&self, key: &str) -> Option<Value> {
        self.values.get(key).map(|&(v, _)| v)
    }

    /// Every setting in catalogue order: its declaration, its value and the layer it came from.
    pub fn rows(&self) -> impl Iterator<Item = (&'static Setting, Value, Layer)> + '_ {
        CATALOGUE.iter().map(|s| {
            let (v, l) = self.values[s.key];
            (s, v, l)
        })
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
                Kind::Bool | Kind::Choice(_) => assert!(s.unit.is_empty()),
            }
        }
    }

    #[test]
    fn the_defaults_resolve_and_say_where_they_came_from() {
        let s = Settings::default();
        assert_eq!(s.int("health.flee_below_hp"), 25);
        assert_eq!(s.int("health.rest_below_hp"), 60);
        assert_eq!(s.int("health.rest_until_hp"), 85);
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

    #[test]
    fn a_style_sets_the_hp_lines_and_says_so() {
        for style in ["careful", "balanced", "bold"] {
            let over = Overrides::parse(&format!("[bot]\nstyle = \"{style}\"\n")).unwrap();
            let s = Settings::resolve(&[(Layer::Mine, &over)]).unwrap();
            assert_eq!(s.choice("bot.style"), style);
            let lines = (
                s.int("health.flee_below_hp"),
                s.int("health.rest_below_hp"),
                s.int("health.rest_until_hp"),
            );
            let want = match style {
                "careful" => (30, 70, 90),
                "bold" => (20, 50, 80),
                _ => (25, 60, 85),
            };
            assert_eq!(lines, want, "{style}");
            let from = s.source("health.flee_below_hp");
            assert_eq!(
                from,
                Some(if style == "balanced" {
                    Layer::Default
                } else {
                    Layer::Style
                })
            );
        }
    }

    #[test]
    fn a_line_you_set_wins_over_the_style_and_the_most_specific_style_wins() {
        let mine =
            Overrides::parse("[bot]\nstyle = \"bold\"\n[health]\nrest_until_hp = 95\n").unwrap();
        let character = Overrides::parse("[health]\nflee_below_hp = 25\n").unwrap();
        let s = Settings::resolve(&[(Layer::Mine, &mine), (Layer::Character, &character)]).unwrap();
        // Bold: rest below 50; the character's flee line and my rest-until beat the preset.
        assert_eq!(s.int("health.flee_below_hp"), 25);
        assert_eq!(s.source("health.flee_below_hp"), Some(Layer::Character));
        assert_eq!(s.int("health.rest_below_hp"), 50);
        assert_eq!(s.source("health.rest_below_hp"), Some(Layer::Style));
        assert_eq!(s.int("health.rest_until_hp"), 95);
        assert_eq!(s.source("health.rest_until_hp"), Some(Layer::Mine));
        // A more specific layer names another style: that one's preset applies.
        let careful = Overrides::parse("[bot]\nstyle = \"careful\"\n").unwrap();
        let s = Settings::resolve(&[(Layer::Mine, &mine), (Layer::Character, &careful)]).unwrap();
        assert_eq!(s.choice("bot.style"), "careful");
        assert_eq!(s.int("health.flee_below_hp"), 30);
    }

    #[test]
    fn a_style_that_is_not_one_is_refused_with_the_options() {
        let err = Overrides::parse("[bot]\nstyle = \"reckless\"\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "bot.style is \"reckless\", not one of careful, balanced, bold"
        );
        assert!(matches!(
            Overrides::parse("[bot]\nstyle = 3\n").unwrap_err(),
            Error::Type { .. }
        ));
        assert!(matches!(
            Overrides::parse("[health]\nflee_below_hp = \"bold\"\n").unwrap_err(),
            Error::Type { .. }
        ));
        // Any case, written back in lower case.
        let o = Overrides::parse("[bot]\nstyle = \" Bold \"\n").unwrap();
        assert!(o.to_toml().contains("style = \"bold\""), "{}", o.to_toml());
    }

    #[test]
    fn a_style_whose_lines_clash_with_a_set_line_is_refused() {
        // Bold rests below 50; rest until 40 is below that.
        let o =
            Overrides::parse("[bot]\nstyle = \"bold\"\n[health]\nrest_until_hp = 40\n").unwrap();
        assert!(matches!(
            Settings::resolve(&[(Layer::Character, &o)]),
            Err(Error::Order(_))
        ));
    }

    #[test]
    fn the_files_of_a_directory_are_the_layers_of_a_character() {
        let dir = tempfile::tempdir().unwrap();
        let w = |path: &str, text: &str| {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let files = SettingsDir::new(dir.path());
        // No files: the defaults.
        assert_eq!(
            files.resolve("silvermere", "thorgrim").unwrap(),
            Settings::default()
        );
        w(
            "settings.toml",
            "[bot]\nstyle = \"bold\"\n[movement]\nsearch_limit = 400\n",
        );
        w(
            "servers/silvermere.toml",
            "[movement]\nsearch_limit = 500\n",
        );
        w("characters/thorgrim.toml", "[health]\nflee_below_hp = 25\n");
        let s = files.resolve("silvermere", "thorgrim").unwrap();
        assert_eq!(s.int("movement.search_limit"), 500);
        assert_eq!(s.source("movement.search_limit"), Some(Layer::Server));
        assert_eq!(s.int("health.flee_below_hp"), 25);
        assert_eq!(s.source("health.flee_below_hp"), Some(Layer::Character));
        assert_eq!(s.choice("bot.style"), "bold");
        // Another character on the same server has the server's layer and not Thorgrim's.
        let other = files.resolve("silvermere", "waydeva").unwrap();
        assert_eq!(other.int("health.flee_below_hp"), 20);
        assert_eq!(other.int("movement.search_limit"), 500);
        // Another server has only my defaults.
        let elsewhere = files.resolve("elsewhere", "thorgrim").unwrap();
        assert_eq!(elsewhere.int("movement.search_limit"), 400);
        // A server is named as the gateway names it, `host:port`: the colon is `_` in the file.
        w(
            "servers/127.0.0.1_2323.toml",
            "[movement]\nsearch_limit = 600\n",
        );
        let local = files.resolve("127.0.0.1:2323", "thorgrim").unwrap();
        assert_eq!(local.int("movement.search_limit"), 600);
        assert_eq!(local.source("movement.search_limit"), Some(Layer::Server));
    }

    #[test]
    fn a_file_that_is_not_valid_names_itself_and_ids_cannot_leave_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("characters")).unwrap();
        std::fs::write(
            dir.path().join("characters/thorgrim.toml"),
            "[health]\nflee_below_hp = 95\n",
        )
        .unwrap();
        let files = SettingsDir::new(dir.path());
        let err = files.resolve("silvermere", "thorgrim").unwrap_err();
        assert!(err.path.ends_with("characters/thorgrim.toml"), "{err}");
        assert!(
            err.to_string().contains("health.flee_below_hp is 95"),
            "{err}"
        );
        // An id with a slash or dots names no file.
        assert_eq!(files.files("../x", "../../etc/passwd").len(), 1);
        assert_eq!(files.files("a/b", "c d").len(), 1);
        assert_eq!(files.files(".hidden", "c").len(), 2);
    }
}
