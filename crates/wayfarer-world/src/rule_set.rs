//! Which rules a server plays (`docs/kb/quicksilver-1.20.md` "Detection"): stock MajorMUD
//! 1.11p, or Quicksilver 1.20, a server that plays 1.11p with rule changes. A connection starts
//! [`RuleSet::Unknown`] and plays as 1.11p until it knows: a wrong guess of 1.11p on a 1.20
//! server only plays a little worse, a wrong guess of 1.20 on a 1.11p server sends commands the
//! server reads as speech and plans spells that don't exist.

use std::fmt;
use std::str::FromStr;

/// The rules a connection's server plays.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RuleSet {
    /// Not known yet: everything behaves as [`RuleSet::Stock`].
    #[default]
    Unknown,
    /// Stock MajorMUD 1.11p.
    Stock,
    /// Quicksilver 1.20.
    Qs120,
}

impl RuleSet {
    /// As the wire and the store spell it: `unknown`, `1.11p`, `1.20`.
    pub fn as_str(self) -> &'static str {
        match self {
            RuleSet::Unknown => "unknown",
            RuleSet::Stock => "1.11p",
            RuleSet::Qs120 => "1.20",
        }
    }

    /// Whether Quicksilver 1.20's behaviour is on: only when 1.20 is known, never while
    /// unknown.
    pub fn qs120(self) -> bool {
        self == RuleSet::Qs120
    }
}

impl fmt::Display for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RuleSet {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unknown" => Ok(RuleSet::Unknown),
            "1.11p" => Ok(RuleSet::Stock),
            "1.20" => Ok(RuleSet::Qs120),
            other => Err(format!("not a rule set: {other:?}")),
        }
    }
}

/// The first line of a 1.20 server's answer to `help 1.20`, after ANSI and trailing white
/// space are stripped. A 1.11p server answers `Sorry, no help is available for that topic!`.
pub const QS120_HELP_TITLE: &str = "Quicksilver 1.20: how the rules differ from 1.11p";

/// The command that asks.
pub const QS120_PROBE: &str = "help 1.20";

/// Spell names only a 1.20 server has: the renamed detect magic and dark plague, and the
/// regen line (`docs/kb/quicksilver-1.20.md` "Spells"). Lower case.
pub const QS120_SPELL_NAMES: [&str; 23] = [
    "protective aura",
    "dark binding",
    "lesser attunement",
    "arcane attunement",
    "greater attunement",
    "minor attunement",
    "modest attunement",
    "steady attunement",
    "deep attunement",
    "lesser devotion",
    "devotion",
    "greater devotion",
    "minor devotion",
    "modest devotion",
    "steady devotion",
    "deep devotion",
    "lesser communion",
    "communion",
    "greater communion",
    "song of reverie",
    "song of rapture",
    "song of stillness",
    "song of transcendence",
];

/// An item name only a 1.20 server has, besides the regen line's scrolls and songsheets:
/// `scroll of protective aura`, which was the scroll of detect magic. Lower case.
pub const QS120_ITEM_NAMES: [&str; 1] = ["scroll of protective aura"];

/// Whether `name` (a spell, a scroll or an item, any case) exists only on a 1.20 server. A
/// scroll or songsheet teaching a regen-line spell counts too (`scroll of lesser attunement`,
/// `songsheet of reverie`).
pub fn qs120_name(name: &str) -> bool {
    let n = name.trim().to_lowercase();
    let n = n.as_str();
    if QS120_SPELL_NAMES.contains(&n) || QS120_ITEM_NAMES.contains(&n) {
        return true;
    }
    if let Some(spell) = n.strip_prefix("scroll of ") {
        return QS120_SPELL_NAMES.contains(&spell);
    }
    if let Some(song) = n.strip_prefix("songsheet of ") {
        return ["reverie", "rapture", "stillness", "transcendence"].contains(&song);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spelled_both_ways() {
        for r in [RuleSet::Unknown, RuleSet::Stock, RuleSet::Qs120] {
            assert_eq!(r.as_str().parse::<RuleSet>(), Ok(r));
        }
        assert!("1.11".parse::<RuleSet>().is_err());
        assert!(RuleSet::Qs120.qs120());
        assert!(!RuleSet::Unknown.qs120());
        assert_eq!(RuleSet::default(), RuleSet::Unknown);
    }

    #[test]
    fn names_only_1_20_has() {
        for n in [
            "protective aura",
            "Dark Binding",
            "devotion",
            "song of transcendence",
            "scroll of protective aura",
            "scroll of deep devotion",
            "songsheet of reverie",
        ] {
            assert!(qs120_name(n), "{n}");
        }
        // stock 1.11p names, the regen line's stock members among them
        for n in [
            "detect magic",
            "dark plague",
            "mana flux",
            "nature tap",
            "serenity",
            "profane link",
            "scroll of mana flux",
            "magic missile",
            "songsheet of pain",
        ] {
            assert!(!qs120_name(n), "{n}");
        }
    }
}
