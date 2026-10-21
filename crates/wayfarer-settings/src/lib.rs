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
    Spells,
    Cash,
    Combat,
}

impl Group {
    /// The table a setting of this group is written under in the TOML file.
    pub fn table(self) -> &'static str {
        match self {
            Group::Health => "health",
            Group::Movement => "movement",
            Group::Bot => "bot",
            Group::Spells => "spells",
            Group::Cash => "cash",
            Group::Combat => "combat",
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

/// The commands that start a fight, by the game's command table (`commands.rs`, kind Attack):
/// a test in the brain checks every word here is one.
pub const ATTACK_COMMANDS: &[&str] = &[
    "a", "attack", "bash", "allout", "force", "bs", "backstab", "kil", "kill", "pu", "punch",
];

/// The groups, in the order a file lists them.
const GROUPS: [Group; 6] = [
    Group::Health,
    Group::Movement,
    Group::Bot,
    Group::Spells,
    Group::Cash,
    Group::Combat,
];

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
    Setting {
        key: "health.rest_mana_below",
        label: "Rest for mana below",
        help: "A caster with attack spells rests between fights when mana is below this per cent of the maximum (never in a room that spawns monsters, and not before it has finished with the HP).",
        kind: Kind::Int { min: 0, max: 90 },
        default: Value::Int(30),
        unit: "%",
        tier: Tier::Essential,
        group: Group::Health,
        megamud: Some("Mana Rest If Below"),
    },
    Setting {
        key: "health.rest_mana_until",
        label: "Rest for mana until",
        help: "Keep resting until mana is back to this per cent of the maximum.",
        kind: Kind::Int { min: 20, max: 100 },
        default: Value::Int(90),
        unit: "%",
        tier: Tier::Essential,
        group: Group::Health,
        megamud: Some("Mana Rest Until"),
    },
    Setting {
        key: "spells.attack",
        label: "Attack spells",
        help: "Auto: a caster fights with its best attack spell it can pay for, and with weapons when the mana is short. Off: weapons only.",
        kind: Kind::Choice(&["auto", "off"]),
        default: Value::Choice("auto"),
        unit: "",
        tier: Tier::Essential,
        group: Group::Spells,
        megamud: None,
    },
    Setting {
        key: "spells.attack_mana_above",
        label: "Cast attack spells while mana is above",
        help: "Below this per cent of the maximum mana a fight is fought with weapons, keeping what is left for healing.",
        kind: Kind::Int { min: 0, max: 90 },
        default: Value::Int(15),
        unit: "%",
        tier: Tier::Advanced,
        group: Group::Spells,
        megamud: None,
    },
    Setting {
        key: "spells.heal",
        label: "Healing spells",
        help: "Auto: a character with a healing spell casts it on itself between fights, before it rests, and in a fight below the line next. Off: it only rests.",
        kind: Kind::Choice(&["auto", "off"]),
        default: Value::Choice("auto"),
        unit: "",
        tier: Tier::Essential,
        group: Group::Spells,
        megamud: None,
    },
    Setting {
        key: "combat.attack_command",
        label: "Attack with",
        help: "The command that starts a fight, followed by the monster's name: a (attack), bash (aa, allout, force), bs (backstab: needs sneaking), kil (killblow, also spelt kill) or pu (punch), or their longer spellings. It is checked against the game's own command table, so only an attack command is accepted.",
        kind: Kind::Choice(ATTACK_COMMANDS),
        default: Value::Choice("a"),
        unit: "",
        tier: Tier::Essential,
        group: Group::Combat,
        megamud: Some("Attack"),
    },
    Setting {
        key: "cash.silver",
        label: "Pick up silver",
        help: "Pick up silver nobles left in the room (after a kill, say) when not in a fight.",
        kind: Kind::Bool,
        default: Value::Bool(false),
        unit: "",
        tier: Tier::Essential,
        group: Group::Cash,
        megamud: Some("WantSilver"),
    },
    Setting {
        key: "cash.gold",
        label: "Pick up gold",
        help: "Pick up gold crowns left in the room when not in a fight.",
        kind: Kind::Bool,
        default: Value::Bool(false),
        unit: "",
        tier: Tier::Essential,
        group: Group::Cash,
        megamud: Some("WantGold"),
    },
    Setting {
        key: "cash.platinum",
        label: "Pick up platinum",
        help: "Pick up platinum pieces left in the room when not in a fight.",
        kind: Kind::Bool,
        default: Value::Bool(true),
        unit: "",
        tier: Tier::Essential,
        group: Group::Cash,
        megamud: Some("WantPlat"),
    },
    Setting {
        key: "cash.runic",
        label: "Pick up runic coins",
        help: "Pick up runic coins left in the room when not in a fight.",
        kind: Kind::Bool,
        default: Value::Bool(true),
        unit: "",
        tier: Tier::Essential,
        group: Group::Cash,
        megamud: Some("WantRunic"),
    },
    Setting {
        key: "cash.max_encumbrance",
        label: "Carry coins up to",
        help: "The heaviest the character may get from picking up coins, by the game's own words for the load: light (up to 33%), medium (up to 66%) or heavy (no limit but the game's). With medium, coins that would make the character Heavy are left, or only as many as fit are taken. A character with more capacity (a dwarf's +20%) is counted by the capacity the game reports.",
        kind: Kind::Choice(&["light", "medium", "heavy"]),
        default: Value::Choice("medium"),
        unit: "",
        tier: Tier::Essential,
        group: Group::Cash,
        megamud: Some("DontBeHeavy"),
    },
    Setting {
        key: "spells.heal_in_combat_below_hp",
        label: "Heal in a fight below",
        help: "In a fight, cast a healing spell when HP is below this per cent of the maximum. Casting ends the swing for that round, so keep it close to the flee line; below the flee line it never happens, since fleeing comes first.",
        kind: Kind::Int { min: 10, max: 90 },
        default: Value::Int(50),
        unit: "%",
        tier: Tier::Advanced,
        group: Group::Spells,
        megamud: None,
    },
];

/// The setting declared for `key`.
pub fn setting(key: &str) -> Option<&'static Setting> {
    CATALOGUE.iter().find(|s| s.key == key)
}

/// How a class fights, for its HP lines: a heavily armoured fighter takes hits at low HP; a
/// lightly armoured skirmisher or hybrid leaves earlier; a robe caster, with the fewest hit
/// points, earliest (`docs/plan/04-settings.md` section 3: "flee below 30 melee, 50 casters").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    /// Warrior, Witchunter, Paladin, Ranger, Cleric: plate or mail, a basher.
    Fighter,
    /// Ninja, Thief, Mystic, Warlock, Missionary, Gypsy, Bard: light armour, or a mix of
    /// melee and magic.
    Skirmisher,
    /// Mage, Priest, Druid: robes and spells.
    Caster,
}

/// The kind of a class, by the name the status screen gives (any case). A name that is not a
/// class of the stock game is a Fighter, the plainest.
pub fn class_kind(class: &str) -> ClassKind {
    match class.trim().to_ascii_lowercase().as_str() {
        "ninja" | "thief" | "mystic" | "warlock" | "missionary" | "gypsy" | "bard" => {
            ClassKind::Skirmisher
        }
        "mage" | "priest" | "druid" => ClassKind::Caster,
        _ => ClassKind::Fighter,
    }
}

/// The three HP lines for a class and a style: (flee below, rest below, rest until). The
/// Balanced lines of a Fighter are 25 / 60 / 85, a Skirmisher's 35 / 65 / 85 and a Caster's
/// 45 / 70 / 90 (John, 2026-10-20: bolder than the first draft; the class gaps are the plan's
/// "casters flee earlier", inferred). Careful adds 5 / 10 / 5 to a class's lines and Bold takes
/// them off, so a Fighter's Careful is 30 / 70 / 90 and Bold 20 / 50 / 80. Each is held within
/// its setting's range and keeps flee < rest below <= rest until.
pub fn hp_lines(class: Option<&str>, style: &str) -> (i64, i64, i64) {
    let (flee, below, until) = match class.map_or(ClassKind::Fighter, class_kind) {
        ClassKind::Fighter => (25, 60, 85),
        ClassKind::Skirmisher => (35, 65, 85),
        ClassKind::Caster => (45, 70, 90),
    };
    let (df, db, du) = match style {
        "careful" => (5, 10, 5),
        "bold" => (-5, -10, -5),
        _ => (0, 0, 0),
    };
    let clamp = |key: &str, v: i64| match setting(key).map(|s| s.kind) {
        Some(Kind::Int { min, max }) => v.clamp(min, max),
        _ => v,
    };
    let flee = clamp("health.flee_below_hp", flee + df);
    let below = clamp("health.rest_below_hp", below + db).max(flee + 5);
    let until = clamp("health.rest_until_hp", until + du).max(below);
    (flee, below, until)
}

/// What a class and a style set, under every layer: the three HP lines, where they differ from
/// the defaults (a Balanced Fighter sets nothing). `class` is the status screen's name for the
/// character's class, `None` until it is known.
pub fn preset(class: Option<&str>, style: &str) -> Overrides {
    let (flee, below, until) = hp_lines(class, style);
    let mut over = Overrides::default();
    for (key, v) in [
        ("health.flee_below_hp", flee),
        ("health.rest_below_hp", below),
        ("health.rest_until_hp", until),
    ] {
        if setting(key).map(|s| s.default) != Some(Value::Int(v)) {
            over.set(key, Value::Int(v))
                .expect("a preset is within its ranges");
        }
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

/// A settings file to start from: every setting with its label, help and default, commented
/// out. Uncomment a line to set it for `scope`; what is left commented follows the layers
/// below it (the class and style presets, then the defaults).
pub fn starter_toml(scope: &str) -> String {
    let mut out = format!(
        "# Settings for {scope}. Every setting is commented out: remove the `# ` before one to\n\
         # change it (the [group] lines are already live).\n\
         # What is not set here comes from the layers below (character > server > settings.toml\n\
         # > class and style presets > defaults). `wayfarer settings` shows each value and where it\n\
         # came from. Restart the brain after editing.\n"
    );
    for group in GROUPS {
        // The header is live (an empty table sets nothing), so an uncommented line under it is
        // in the right table.
        out.push_str(&format!("\n[{}]\n", group.table()));
        for s in CATALOGUE.iter().filter(|s| s.group == group) {
            let name = s.key.split_once('.').map_or(s.key, |(_, n)| n);
            let range = match s.kind {
                Kind::Int { min, max } => format!(" ({min} to {max}{})", unit_suffix(s.unit)),
                Kind::Bool => " (true or false)".to_owned(),
                Kind::Choice(words) => format!(" ({})", words.join(", ")),
            };
            out.push_str(&format!("# {}{range}: {}\n", s.label, s.help));
            out.push_str(&format!("# {name} = {}\n", s.default));
        }
    }
    out
}

fn unit_suffix(unit: &str) -> String {
    if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    }
}

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

    /// The most specific file of the call: the character's, else the server's, else
    /// `settings.toml`.
    pub fn most_specific(&self, server: &str, character: &str) -> std::path::PathBuf {
        let files = self.files(server, character);
        let wanted = if !character.is_empty() {
            Layer::Character
        } else if !server.is_empty() {
            Layer::Server
        } else {
            Layer::Mine
        };
        files
            .into_iter()
            .rev()
            .find(|(l, _)| *l == wanted)
            .map_or_else(|| self.dir.join("settings.toml"), |(_, p)| p)
    }

    /// Write the starter file ([`starter_toml`]) for the call's most specific file. An existing
    /// file is left alone unless `force`; `Ok(false)` then. Every line of a starter is a
    /// comment, so a new file changes nothing until a line is uncommented.
    pub fn write_starter(
        &self,
        server: &str,
        character: &str,
        force: bool,
    ) -> Result<(std::path::PathBuf, bool), String> {
        let path = self.most_specific(server, character);
        if path.exists() && !force {
            return Ok((path, false));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let scope = if !character.is_empty() {
            format!("the character {character}")
        } else if !server.is_empty() {
            format!("the server {server}")
        } else {
            "all characters".to_owned()
        };
        std::fs::write(&path, starter_toml(&scope))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok((path, true))
    }

    /// The settings for `character` on `server`: the files' layers over the style preset.
    pub fn resolve(&self, server: &str, character: &str) -> Result<Settings, FileError> {
        self.resolve_for(server, character, None)
    }

    /// The same for a character of `class`.
    pub fn resolve_for(
        &self,
        server: &str,
        character: &str,
        class: Option<&str>,
    ) -> Result<Settings, FileError> {
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
        Settings::resolve_for(class, &refs).map_err(|error| FileError {
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
    /// The class and style presets (`bot.style`: careful, balanced or bold; the class kind).
    Preset,
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
            Layer::Preset => "the class and style",
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
        for group in GROUPS {
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
        Self::resolve_for(None, layers)
    }

    /// The same for a character of `class` (the status screen's name for it; `None` while it is
    /// not known): the class's HP lines are the base the style moves.
    pub fn resolve_for(
        class: Option<&str>,
        layers: &[(Layer, &Overrides)],
    ) -> Result<Settings, Error> {
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
        for (&key, &v) in &preset(class, style).values {
            s.values.insert(key, (v, Layer::Preset));
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
                    Layer::Preset
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
        assert_eq!(s.source("health.rest_below_hp"), Some(Layer::Preset));
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
    fn a_starter_file_lists_every_setting_and_sets_none() {
        let text = starter_toml("all characters");
        for s in CATALOGUE {
            let name = s.key.split_once('.').unwrap().1;
            assert!(
                text.contains(&format!("# {name} = {}", s.default)),
                "{}",
                s.key
            );
        }
        assert!(Overrides::parse(&text).unwrap().is_empty());
        // Uncommented, a line works.
        let on = text.replace("# heal = \"auto\"", "heal = \"off\"");
        assert_eq!(
            Overrides::parse(&on).unwrap().get("spells.heal"),
            Some(Value::Choice("off"))
        );
    }

    #[test]
    fn a_starter_is_written_once_in_the_most_specific_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = SettingsDir::new(tmp.path());
        let (path, wrote) = dir.write_starter("", "waydeva", false).unwrap();
        assert!(wrote && path == tmp.path().join("characters/waydeva.toml"));
        std::fs::write(&path, "[spells]\nheal = \"off\"\n").unwrap();
        // Left alone, then replaced when asked.
        assert!(!dir.write_starter("", "waydeva", false).unwrap().1);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("heal = \"off\"")
        );
        assert!(dir.write_starter("", "waydeva", true).unwrap().1);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("# Settings for the character waydeva")
        );
        let (all, _) = dir.write_starter("", "", false).unwrap();
        assert_eq!(all, tmp.path().join("settings.toml"));
        let (srv, _) = dir.write_starter("127.0.0.1:2323", "", false).unwrap();
        assert_eq!(srv, tmp.path().join("servers/127.0.0.1_2323.toml"));
        // Starters change nothing.
        assert_eq!(
            dir.resolve("127.0.0.1:2323", "waydeva")
                .unwrap()
                .int("health.flee_below_hp"),
            25
        );
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

    #[test]
    fn a_class_is_a_fighter_a_skirmisher_or_a_caster() {
        for c in [
            "Warrior",
            "Witchunter",
            "Paladin",
            "Ranger",
            "Cleric",
            "Nobody",
        ] {
            assert_eq!(class_kind(c), ClassKind::Fighter, "{c}");
        }
        for c in [
            "Ninja",
            "Thief",
            "Mystic",
            "Warlock",
            "Missionary",
            "Gypsy",
            "bard",
        ] {
            assert_eq!(class_kind(c), ClassKind::Skirmisher, "{c}");
        }
        for c in ["Mage", "PRIEST", " druid "] {
            assert_eq!(class_kind(c), ClassKind::Caster, "{c}");
        }
    }

    #[test]
    fn the_hp_lines_follow_the_class_and_the_style_moves_them() {
        let lines = |class, style| hp_lines(class, style);
        // A fighter, or a class not yet known.
        assert_eq!(lines(None, "balanced"), (25, 60, 85));
        assert_eq!(lines(Some("Warrior"), "careful"), (30, 70, 90));
        assert_eq!(lines(Some("Warrior"), "bold"), (20, 50, 80));
        assert_eq!(lines(Some("Thief"), "balanced"), (35, 65, 85));
        assert_eq!(lines(Some("Thief"), "careful"), (40, 75, 90));
        assert_eq!(lines(Some("Thief"), "bold"), (30, 55, 80));
        assert_eq!(lines(Some("Mage"), "balanced"), (45, 70, 90));
        assert_eq!(lines(Some("Mage"), "careful"), (50, 80, 95));
        assert_eq!(lines(Some("Mage"), "bold"), (40, 60, 85));
        // Every combination is in range and in order.
        for class in [None, Some("Warrior"), Some("Thief"), Some("Mage")] {
            for style in ["careful", "balanced", "bold"] {
                let (f, b, u) = lines(class, style);
                assert!((0..=90).contains(&f) && (10..=99).contains(&b) && (20..=100).contains(&u));
                assert!(f < b && b <= u, "{class:?} {style}: {f} {b} {u}");
            }
        }
    }

    #[test]
    fn a_class_moves_the_defaults_and_says_so() {
        // No class: a fighter's, which are the defaults, set by nothing.
        let fighter = Settings::resolve_for(None, &[]).unwrap();
        assert_eq!(fighter, Settings::default());
        assert_eq!(fighter.source("health.flee_below_hp"), Some(Layer::Default));
        // A Mage: the class and style presets set the lines.
        let mage = Settings::resolve_for(Some("Mage"), &[]).unwrap();
        assert_eq!(mage.int("health.flee_below_hp"), 45);
        assert_eq!(mage.int("health.rest_until_hp"), 90);
        assert_eq!(mage.source("health.flee_below_hp"), Some(Layer::Preset));
        // The style moves a Mage's lines from the Mage's, and a file beats both.
        let over =
            Overrides::parse("[bot]\nstyle = \"bold\"\n[health]\nrest_until_hp = 95\n").unwrap();
        let s = Settings::resolve_for(Some("mage"), &[(Layer::Character, &over)]).unwrap();
        assert_eq!(s.int("health.flee_below_hp"), 40);
        assert_eq!(s.int("health.rest_below_hp"), 60);
        assert_eq!(s.int("health.rest_until_hp"), 95);
        assert_eq!(s.source("health.rest_until_hp"), Some(Layer::Character));
    }

    #[test]
    fn a_directory_resolves_for_a_class() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.toml"),
            "[health]\nflee_below_hp = 33\n",
        )
        .unwrap();
        let files = SettingsDir::new(dir.path());
        let mage = files.resolve_for("x", "y", Some("Mage")).unwrap();
        // My file beats the class's line; the class's others stand.
        assert_eq!(mage.int("health.flee_below_hp"), 33);
        assert_eq!(mage.int("health.rest_below_hp"), 70);
        // Without a class it is a fighter's.
        assert_eq!(
            files.resolve("x", "y").unwrap().int("health.rest_below_hp"),
            60
        );
    }
}
