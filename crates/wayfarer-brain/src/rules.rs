//! The monster rules (docs/brain.md "Monster rules"): *never fight*, *flee from* and *avoid*
//! by name, in three layers: wayfarer's defaults ([`DEFAULT_RULES`]), what the bot learned this
//! session (a stalemate makes a monster *never fight*) and what the player set
//! (`set_monster_rule`, `wayfarer brain --flee-from`). The last two are kept with the bot's
//! record. And where the monsters to avoid were last seen, for the router.

use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_proto::{MonsterRuleEntry, MonsterRuleKind, RuleSource};
use wayfarer_store::RuleRecord;
use wayfarer_world::{DEFAULT_RULES, Key, MonsterRule, MonsterRules};

/// How long a sighting of a monster to avoid or flee from keeps its room avoided.
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
        MonsterRule::NeverFight => "never_fight",
        MonsterRule::FleeFrom => "flee_from",
        MonsterRule::Avoid => "avoid",
    }
}

fn parse_rule(s: &str) -> Option<MonsterRule> {
    Some(match s {
        "never_fight" => MonsterRule::NeverFight,
        "flee_from" => MonsterRule::FleeFrom,
        "avoid" => MonsterRule::Avoid,
        _ => return None,
    })
}

/// The wire's rule.
pub fn wire_rule(r: MonsterRule) -> MonsterRuleKind {
    match r {
        MonsterRule::NeverFight => MonsterRuleKind::NeverFight,
        MonsterRule::FleeFrom => MonsterRuleKind::FleeFrom,
        MonsterRule::Avoid => MonsterRuleKind::Avoid,
    }
}

/// A rule from the wire.
pub fn from_wire(r: MonsterRuleKind) -> MonsterRule {
    match r {
        MonsterRuleKind::NeverFight => MonsterRule::NeverFight,
        MonsterRuleKind::FleeFrom => MonsterRule::FleeFrom,
        MonsterRuleKind::Avoid => MonsterRule::Avoid,
    }
}

/// A few words for a rule: "never fight", "flee from", "avoid".
pub fn describe(r: MonsterRule) -> &'static str {
    match r {
        MonsterRule::NeverFight => "never fight",
        MonsterRule::FleeFrom => "flee from",
        MonsterRule::Avoid => "avoid",
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
}

impl Default for RuleBook {
    fn default() -> Self {
        RuleBook::new(&[])
    }
}

impl RuleBook {
    /// The defaults with the rules the store kept (rows it cannot read are left out).
    pub fn new(stored: &[RuleRecord]) -> Self {
        let entries = stored
            .iter()
            .filter_map(|r| {
                let source = match r.source.as_str() {
                    "session" => Source::Session,
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

    /// A stalemate with `name`: *never fight* it from now on, unless a rule already keeps the
    /// bot from attacking it. Whether that added a rule.
    pub fn learn(&mut self, name: &str, at: i64) -> bool {
        if matches!(
            self.rule_for(name),
            Some(MonsterRule::NeverFight | MonsterRule::FleeFrom)
        ) {
            return false;
        }
        let pattern = name.trim().to_lowercase();
        self.entries
            .retain(|e| !(e.source == Source::Session && e.pattern.eq_ignore_ascii_case(&pattern)));
        self.entries.push(Entry {
            pattern,
            rule: MonsterRule::NeverFight,
            source: Source::Session,
            at,
        });
        self.compile();
        true
    }

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

    /// What the store keeps: the session's rules and the player's.
    pub fn stored(&self) -> Vec<RuleRecord> {
        self.entries
            .iter()
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
        let defaults = DEFAULT_RULES.iter().map(|&(p, r, _)| MonsterRuleEntry {
            pattern: p.to_owned(),
            rule: wire_rule(r),
            source: RuleSource::Default,
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
                })
        };
        defaults
            .chain(kept(Source::Session))
            .chain(kept(Source::Player))
            .collect()
    }
}

/// A sighting: where a monster to avoid or flee from was last seen, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sighting {
    room: Key,
    at: Instant,
    /// The name as the room showed it.
    name: String,
}

/// Where the monsters to avoid (or flee from) were last seen this session, for
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
            if matches!(
                rules.rule_for(name),
                Some(MonsterRule::Avoid | MonsterRule::FleeFrom)
            ) {
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
    fn layers_go_defaults_then_the_session_then_the_player() {
        let mut book = RuleBook::default();
        assert_eq!(
            book.rule_for("gigantic black ooze"),
            Some(MonsterRule::FleeFrom)
        );
        // Already kept off: a stalemate adds nothing.
        assert!(!book.learn("gigantic black ooze", 10));
        assert!(book.learn("Stone Golem", 10));
        assert!(!book.learn("stone golem", 11));
        assert_eq!(book.rule_for("stone golem"), Some(MonsterRule::NeverFight));
        // The player's rule for the same pattern wins over the session's and the default.
        assert!(book.set("stone golem", Some(MonsterRule::FleeFrom), 12));
        assert_eq!(book.rule_for("stone golem"), Some(MonsterRule::FleeFrom));
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
            Some(MonsterRule::FleeFrom)
        );
        let wire: Vec<(String, RuleSource)> = book
            .wire()
            .into_iter()
            .map(|e| (e.pattern, e.source))
            .collect();
        assert_eq!(
            wire,
            [
                ("gigantic black ooze".to_owned(), RuleSource::Default),
                ("stone golem".to_owned(), RuleSource::Session),
                ("stone golem".to_owned(), RuleSource::Player),
            ]
        );
        // Through the store and back.
        let again = RuleBook::new(&book.stored());
        assert_eq!(again.wire(), book.wire());
        assert_eq!(again.stored(), book.stored());
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
