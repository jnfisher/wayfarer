//! The monster rules (docs/brain.md "Monster rules"): *ignore* and *avoid* by name, in two
//! layers: wayfarer's defaults ([`DEFAULT_RULES`], with MegaMud's monster list behind them) and
//! what the player set (`set_monster_rule`, `wayfarer brain --ignore`), which may also switch a
//! default *off*; the player's are kept with the bot's record. The bot makes no rule itself: a
//! stalemate used to (C-096). The old *never fight* and *flee from* are read as *ignore*
//! (C-073).
//! And where the monsters to avoid were last seen, for the router.

use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_proto::{MonsterRuleEntry, MonsterRuleKind, RuleSource};
use wayfarer_store::RuleRecord;
use wayfarer_world::{DEFAULT_RULES, Key, MonsterRule, MonsterRules};

/// How long a sighting of a monster to avoid keeps its room avoided.
pub const AVOID_MEMORY: Duration = Duration::from_secs(30 * 60);

/// Where a rule the bot keeps came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Session,
    Player,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Session => "session",
            Source::Player => "player",
        }
    }
}

fn rule_str(r: MonsterRule) -> &'static str {
    match r {
        MonsterRule::Ignore => "ignore",
        MonsterRule::Avoid => "avoid",
        MonsterRule::First => "first",
        MonsterRule::Off => "off",
    }
}

/// A stored rule: the old `never_fight` and `flee_from` are *ignore* now (C-073).
fn parse_rule(s: &str) -> Option<MonsterRule> {
    Some(match s {
        "ignore" | "never_fight" | "flee_from" => MonsterRule::Ignore,
        "avoid" => MonsterRule::Avoid,
        "first" => MonsterRule::First,
        "off" => MonsterRule::Off,
        _ => return None,
    })
}

/// The wire's rule.
pub fn wire_rule(r: MonsterRule) -> MonsterRuleKind {
    match r {
        MonsterRule::Ignore => MonsterRuleKind::Ignore,
        MonsterRule::Avoid => MonsterRuleKind::Avoid,
        MonsterRule::First => MonsterRuleKind::First,
        MonsterRule::Off => MonsterRuleKind::Off,
    }
}

/// A rule from the wire.
pub fn from_wire(r: MonsterRuleKind) -> MonsterRule {
    match r {
        MonsterRuleKind::Ignore => MonsterRule::Ignore,
        MonsterRuleKind::Avoid => MonsterRule::Avoid,
        MonsterRuleKind::First => MonsterRule::First,
        MonsterRuleKind::Off => MonsterRule::Off,
    }
}

/// A few words for a rule: "ignore", "avoid", "attack first", "off".
pub fn describe(r: MonsterRule) -> &'static str {
    match r {
        MonsterRule::Ignore => "ignore",
        MonsterRule::Avoid => "avoid",
        MonsterRule::First => "attack first",
        MonsterRule::Off => "off",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    pattern: String,
    rule: MonsterRule,
    source: Source,
    /// When it was made, Unix seconds.
    at: i64,
}

/// A session's monster rules: the defaults, then what it learned, then the player's, each
/// layer over the one before for the same pattern.
#[derive(Debug, Clone)]
pub(crate) struct RuleBook {
    entries: Vec<Entry>,
    rules: MonsterRules,
    /// The bosses seen this session, by lower-cased name, with their name and hours back: each
    /// is *attack first* by default (C-075), listed on the wire so the page can switch it off.
    bosses: std::collections::BTreeMap<String, (String, Option<i64>)>,
}

impl Default for RuleBook {
    fn default() -> Self {
        RuleBook::new(&[])
    }
}

impl RuleBook {
    /// The defaults with the player's rules the store kept (rows it cannot read are left
    /// out). A stalemate's rule kept by an older brain is left out too: those last the session
    /// only (C-091).
    pub fn new(stored: &[RuleRecord]) -> Self {
        let entries = stored
            .iter()
            .filter_map(|r| {
                let source = match r.source.as_str() {
                    "player" => Source::Player,
                    _ => return None,
                };
                Some(Entry {
                    pattern: r.pattern.clone(),
                    rule: parse_rule(&r.rule)?,
                    source,
                    at: r.at,
                })
            })
            .collect();
        let mut book = RuleBook {
            entries,
            rules: MonsterRules::default(),
            bosses: std::collections::BTreeMap::new(),
        };
        book.compile();
        book
    }

    fn compile(&mut self) {
        let layer = |s: Source| {
            self.entries
                .iter()
                .filter(move |e| e.source == s)
                .map(|e| (e.pattern.as_str(), e.rule))
        };
        let defaults = DEFAULT_RULES.iter().map(|&(p, r, _)| (p, r));
        self.rules = MonsterRules::new(
            defaults
                .chain(layer(Source::Session))
                .chain(layer(Source::Player)),
        );
    }

    pub fn rules(&self) -> &MonsterRules {
        &self.rules
    }

    pub fn rule_for(&self, name: &str) -> Option<MonsterRule> {
        self.rules.rule_for(name)
    }

    /// A stalemate with `name`: *ignore* it from now on, unless a rule already does. Whether
    /// The player's rule for `pattern`: set, changed, or removed (`None`). Whether anything
    /// changed.
    pub fn set(&mut self, pattern: &str, rule: Option<MonsterRule>, at: i64) -> bool {
        let pattern = pattern.trim();
        let i = self
            .entries
            .iter()
            .position(|e| e.source == Source::Player && e.pattern.eq_ignore_ascii_case(pattern));
        match (i, rule) {
            (None, None) => return false,
            (Some(i), None) => {
                self.entries.remove(i);
            }
            (Some(i), Some(r)) if self.entries[i].rule == r => return false,
            (Some(i), Some(r)) => self.entries[i].rule = r,
            (None, Some(r)) => self.entries.push(Entry {
                pattern: pattern.to_owned(),
                rule: r,
                source: Source::Player,
                at,
            }),
        }
        self.compile();
        true
    }

    /// A boss was seen (`name` as the game data has it, back `hours` after a kill): its default,
    /// *attack first*, is listed from now on. Whether that is new.
    pub fn saw_boss(&mut self, name: &str, hours: Option<i64>) -> bool {
        let key = name.trim().to_lowercase();
        if self.bosses.contains_key(&key) {
            return false;
        }
        self.bosses.insert(key, (name.trim().to_owned(), hours));
        true
    }

    /// Remove the top rule for `pattern` (the page's `x`): the player's own (an `off` among
    /// them, which puts back what it switched off), else a learned one, else the default is
    /// switched off by a player `off`. Whether anything changed.
    pub fn remove(&mut self, pattern: &str, at: i64) -> bool {
        let pattern = pattern.trim();
        let find = |s: Source| {
            self.entries
                .iter()
                .position(|e| e.source == s && e.pattern.eq_ignore_ascii_case(pattern))
        };
        if let Some(i) = find(Source::Player).or_else(|| find(Source::Session)) {
            self.entries.remove(i);
            self.compile();
            return true;
        }
        let default = DEFAULT_RULES
            .iter()
            .any(|(p, _, _)| p.eq_ignore_ascii_case(pattern))
            || self.bosses.contains_key(&pattern.to_lowercase());
        default && self.set(pattern, Some(MonsterRule::Off), at)
    }

    /// What the store keeps: the player's rules. A stalemate's *ignore* lasts the session
    /// only (C-091): a monster given up on a bad run is fought again after a restart.
    pub fn stored(&self) -> Vec<RuleRecord> {
        self.entries
            .iter()
            .filter(|e| e.source == Source::Player)
            .map(|e| RuleRecord {
                pattern: e.pattern.clone(),
                rule: rule_str(e.rule).to_owned(),
                source: e.source.as_str().to_owned(),
                at: e.at,
            })
            .collect()
    }

    /// The wire's `monster_rules`: the defaults, then the session's, then the player's.
    pub fn wire(&self) -> Vec<MonsterRuleEntry> {
        let defaults = DEFAULT_RULES.iter().map(|&(p, r, why)| MonsterRuleEntry {
            pattern: p.to_owned(),
            rule: wire_rule(r),
            source: RuleSource::Default,
            reason: Some(why.to_owned()),
        });
        let bosses = self.bosses.values().map(|(name, hours)| MonsterRuleEntry {
            pattern: name.to_lowercase(),
            rule: MonsterRuleKind::First,
            source: RuleSource::Default,
            reason: Some(match hours {
                Some(h) => format!("a boss: one alive at a time, back {h} h after a kill"),
                None => "a boss: one alive at a time".to_owned(),
            }),
        });
        let kept = |s: Source| {
            self.entries
                .iter()
                .filter(move |e| e.source == s)
                .map(|e| MonsterRuleEntry {
                    pattern: e.pattern.clone(),
                    rule: wire_rule(e.rule),
                    source: match e.source {
                        Source::Session => RuleSource::Session,
                        Source::Player => RuleSource::Player,
                    },
                    reason: None,
                })
        };
        defaults
            .chain(bosses)
            .chain(kept(Source::Session))
            .chain(kept(Source::Player))
            .collect()
    }
}

/// A sighting: where a monster to avoid was last seen, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sighting {
    room: Key,
    at: Instant,
    /// The name as the room showed it.
    name: String,
}

/// Where the monsters to avoid were last seen this session, for
/// [`AVOID_MEMORY`]: their rooms are costly to walk through.
#[derive(Debug, Default)]
pub(crate) struct Sightings {
    /// By lower-cased name.
    last: HashMap<String, Sighting>,
}

impl Sightings {
    /// The room `room` shows `here`: the monsters to avoid among them were last seen here, and
    /// those last seen here that it no longer shows are not. Whether the rooms to avoid changed.
    pub fn seen(&mut self, rules: &MonsterRules, room: Key, here: &[String], now: Instant) -> bool {
        let before = self.rooms(now);
        let names: BTreeSet<String> = here.iter().map(|n| n.to_lowercase()).collect();
        self.last
            .retain(|name, s| s.room != room || names.contains(name));
        for name in here {
            if rules.rule_for(name) == Some(MonsterRule::Avoid) {
                self.last.insert(
                    name.to_lowercase(),
                    Sighting {
                        room,
                        at: now,
                        name: name.clone(),
                    },
                );
            }
        }
        self.rooms(now) != before
    }

    /// `name` (a monster a stalemate gave up) was in `room` now: kept out of as a monster to
    /// avoid's room is.
    pub fn mark(&mut self, room: Key, name: &str, now: Instant) {
        self.last.insert(
            name.to_lowercase(),
            Sighting {
                room,
                at: now,
                name: name.to_owned(),
            },
        );
    }

    /// Forget the sightings older than [`AVOID_MEMORY`]. Whether any went.
    pub fn expire(&mut self, now: Instant) -> bool {
        let n = self.last.len();
        self.last.retain(|_, s| now < s.at + AVOID_MEMORY);
        self.last.len() != n
    }

    /// The rooms to avoid now.
    pub fn rooms(&self, now: Instant) -> BTreeSet<Key> {
        self.last
            .values()
            .filter(|s| now < s.at + AVOID_MEMORY)
            .map(|s| s.room)
            .collect()
    }

    /// A monster to avoid that was last seen in `room`, if one was (the first by name).
    pub fn at(&self, room: Key, now: Instant) -> Option<&str> {
        let mut here: Vec<&Sighting> = self
            .last
            .values()
            .filter(|s| s.room == room && now < s.at + AVOID_MEMORY)
            .collect();
        here.sort_by(|a, b| a.name.cmp(&b.name));
        here.first().map(|s| s.name.as_str())
    }

    /// When the next sighting is forgotten.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.last.values().map(|s| s.at + AVOID_MEMORY).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_go_defaults_then_the_player() {
        let mut book = RuleBook::default();
        assert_eq!(
            book.rule_for("gigantic black ooze"),
            Some(MonsterRule::Ignore)
        );
        // MegaMud's: the gem guardians are not worth a fight (C-096).
        assert_eq!(book.rule_for("ruby guardian"), Some(MonsterRule::Ignore));
        // The player's rule wins over the default.
        assert!(book.set("stone golem", Some(MonsterRule::Avoid), 12));
        assert_eq!(book.rule_for("stone golem"), Some(MonsterRule::Avoid));
        assert!(book.set("Gigantic Black Ooze", Some(MonsterRule::Avoid), 13));
        assert_eq!(
            book.rule_for("gigantic black ooze"),
            Some(MonsterRule::Avoid)
        );
        assert!(!book.set("gigantic black ooze", Some(MonsterRule::Avoid), 14));
        assert!(book.set("gigantic black ooze", None, 15));
        assert!(!book.set("gigantic black ooze", None, 15));
        assert_eq!(
            book.rule_for("gigantic black ooze"),
            Some(MonsterRule::Ignore)
        );
        let ours = |b: &RuleBook| -> Vec<(String, RuleSource)> {
            b.wire()
                .into_iter()
                .filter(|e| e.pattern == "gigantic black ooze" || e.pattern == "stone golem")
                .map(|e| (e.pattern, e.source))
                .collect()
        };
        assert_eq!(
            ours(&book),
            [
                ("gigantic black ooze".to_owned(), RuleSource::Default),
                ("stone golem".to_owned(), RuleSource::Player),
            ]
        );
        // Through the store and back: the player's rule is kept.
        let again = RuleBook::new(&book.stored());
        assert_eq!(again.stored(), book.stored());
        let kept = ours(&again);
        assert_eq!(
            kept,
            [
                ("gigantic black ooze".to_owned(), RuleSource::Default),
                ("stone golem".to_owned(), RuleSource::Player),
            ]
        );
    }

    #[test]
    fn removing_takes_the_top_rule_off_and_a_default_is_switched_off_and_back() {
        let mut book = RuleBook::default();
        assert!(book.set("stone golem", Some(MonsterRule::Avoid), 11));
        assert!(book.remove("Stone Golem", 12));
        assert_eq!(book.rule_for("stone golem"), None);
        assert!(!book.remove("stone golem", 14));
        // A default: switched off by a player `off`, kept in the store.
        assert!(book.remove("gigantic black ooze", 15));
        assert_eq!(book.rule_for("gigantic black ooze"), None);
        let again = RuleBook::new(&book.stored());
        assert_eq!(again.rule_for("gigantic black ooze"), None);
        assert!(
            again
                .wire()
                .iter()
                .any(|e| e.pattern == "gigantic black ooze"
                    && e.rule == MonsterRuleKind::Off
                    && e.source == RuleSource::Player)
        );
        // Removing the `off` puts the default back.
        assert!(book.remove("gigantic black ooze", 16));
        assert_eq!(
            book.rule_for("gigantic black ooze"),
            Some(MonsterRule::Ignore)
        );
    }

    #[test]
    fn the_old_kinds_in_the_store_load_as_ignore_and_a_kept_stalemate_is_dropped() {
        let stored = |pattern: &str, rule: &str, source: &str| RuleRecord {
            pattern: pattern.into(),
            rule: rule.into(),
            source: source.into(),
            at: 1,
        };
        let book = RuleBook::new(&[
            stored("stone golem", "never_fight", "session"),
            stored("fat orc", "flee_from", "player"),
            stored("kobold", "avoid", "player"),
        ]);
        // A stalemate's rule an older brain kept is dropped: those last the session (C-091).
        assert_eq!(book.rule_for("stone golem"), None);
        assert_eq!(book.rule_for("fat orc"), Some(MonsterRule::Ignore));
        assert_eq!(book.rule_for("kobold"), Some(MonsterRule::Avoid));
        let rules: Vec<String> = book.stored().into_iter().map(|r| r.rule).collect();
        assert_eq!(rules, ["ignore", "avoid"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_sighting_is_where_the_monster_was_last_seen_for_half_an_hour() {
        let rules = MonsterRules::new([("orc", MonsterRule::Avoid)]);
        let mut s = Sightings::default();
        let t0 = Instant::now();
        let (a, b) = ((1, 1), (1, 2));
        assert!(s.seen(&rules, a, &["fat orc".into(), "rat".into()], t0));
        assert_eq!(s.rooms(t0), [a].into());
        assert_eq!(s.at(a, t0), Some("fat orc"));
        // Seen again elsewhere: that is where it is now.
        assert!(s.seen(&rules, b, &["fat orc".into()], t0));
        assert_eq!(s.rooms(t0), [b].into());
        // Its room shown without it: gone from there.
        assert!(s.seen(&rules, b, &[], t0));
        assert!(s.rooms(t0).is_empty());
        s.seen(&rules, a, &["fat orc".into()], t0);
        assert_eq!(s.next_expiry(), Some(t0 + AVOID_MEMORY));
        assert!(!s.expire(t0 + AVOID_MEMORY - Duration::from_secs(1)));
        assert!(s.rooms(t0 + AVOID_MEMORY).is_empty());
        assert!(s.expire(t0 + AVOID_MEMORY));
        assert_eq!(s.next_expiry(), None);
    }
}
