//! The bot's policies (docs/plan/03-engine.md section 4): pure functions of the state, the
//! world pack and the constants below, with no clock and nothing sent.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use wayfarer_world::{
    ExitType, Gate, Key, LockState, MonsterRule, MonsterRules, Monsters, Traveller, distances,
    huntable,
};

use crate::brain::Atlas;

// The thresholds of docs/plan/04-settings.md section 3 at their defaults. The brain reads the
// settings `health.flee_below_hp`, `health.rest_below_hp` and `health.rest_until_hp`
// (`wayfarer-settings`); these are what it falls back to without a world pack, and the
// defaults those settings declare.

/// Recover takes control below this much HP (per cent of the maximum).
pub const REST_BELOW: i32 = 60;
/// Recover rests until this much HP.
pub const REST_UNTIL: i32 = 85;
/// Flee takes control below this much HP (per cent of the maximum). A plain percentage: the
/// plan's "twice the hardest hit this fight" was dropped (John, 2026-10-19). `wayfarer brain
/// --flee-below` overrides it for the live check only.
pub const FLEE_BELOW: i32 = 25;
/// Rooms Flee runs through while followed before it turns and fights: the default of
/// `health.flee_rooms`.
pub const FLEE_HOPS: u32 = 6;
/// How long Flee waits at the prompt after a hop for a monster to follow
/// (`kb/navigation.md` "Leaving a room": a follower comes in straight after).
pub const FLEE_WATCH: Duration = Duration::from_millis(1500);
/// How far Recover walks to rest out of a room that spawns monsters.
pub const QUIET_WITHIN: u32 = 5;
/// `rest` sent this many times without a `(Resting)` prompt: wait before trying again.
pub const REST_TRIES: u32 = 3;
/// How long Recover waits after `You are too sick to rest!` (a poison tick is 30 s) or a
/// `rest` that would not take.
pub const REST_WAIT: Duration = Duration::from_secs(30);
/// Auto-combat can end without `*Combat Off*` (`kb/pitfalls.md` 42): this long with no combat
/// line (two 5 s rounds and some) and the character counts as out of combat. The same goes for
/// a combat state the brain has not seen yet.
pub const COMBAT_QUIET: Duration = Duration::from_secs(12);

/// HP as a whole per cent of the maximum; `None` until both are known.
pub fn hp_percent(hp: Option<i32>, hp_max: Option<i32>) -> Option<i32> {
    match (hp, hp_max) {
        (Some(hp), Some(max)) if max > 0 => Some((i64::from(hp) * 100 / i64::from(max)) as i32),
        _ => None,
    }
}

/// The word an attack names a monster by: its last (`large giant rat` is `a rat`).
pub fn attack_word(name: &str) -> &str {
    name.split_whitespace().last().unwrap_or(name)
}

/// A monster the bot fights: the records its name finds (adjectives stripped) are all worth
/// hunting, evil and not an NPC (`kb/pitfalls.md` 1: skip a name any of whose records isn't).
pub fn hostile(monsters: &Monsters, name: &str) -> bool {
    let records = monsters.by_name(name);
    !records.is_empty() && records.into_iter().all(huntable)
}

/// Whether the name seen (`fat giant rat`) is one of the monsters `names` (`giant rat`).
pub fn is_one_of(monsters: &Monsters, seen: &str, names: &[String]) -> bool {
    monsters
        .by_name(seen)
        .iter()
        .any(|m| names.iter().any(|n| n.eq_ignore_ascii_case(&m.name)))
}

/// What to attack: a hostile monster that is attacking the character first, else the first
/// hostile one listed here; never one on the skip list. The monster rules (`rules`): one the
/// bot must *never fight* is attacked only when it attacks, and one to *flee from* only when
/// it attacks and the bot is `cornered` (there was no way out).
pub fn target<'a>(
    monsters: &Monsters,
    rules: &MonsterRules,
    here: &'a [String],
    attackers: &'a [String],
    skip: &BTreeSet<String>,
    cornered: bool,
) -> Option<&'a str> {
    let fightable = |n: &&str| !skip.contains(*n) && hostile(monsters, n);
    let attacker = attackers
        .iter()
        .map(String::as_str)
        .filter(fightable)
        .find(|n| cornered || rules.rule_for(n) != Some(MonsterRule::FleeFrom));
    attacker.or_else(|| {
        here.iter().map(String::as_str).filter(fightable).find(|n| {
            !matches!(
                rules.rule_for(n),
                Some(MonsterRule::NeverFight | MonsterRule::FleeFrom)
            )
        })
    })
}

/// The first monster here the bot must *flee from*, if any.
pub fn flee_from<'a>(rules: &MonsterRules, here: &'a [String]) -> Option<&'a str> {
    here.iter()
        .map(String::as_str)
        .find(|n| rules.rule_for(n) == Some(MonsterRule::FleeFrom))
}

/// Monsters come to the room: the spawn pass makes some there, or it holds a monster worth
/// hunting. A room the belief is not sure of counts as one (docs/brain.md "Fight here").
pub fn spawns_here(atlas: &Atlas, room: Option<Key>, certain: bool) -> bool {
    let Some(node) = room.filter(|_| certain).and_then(|k| atlas.graph.node(k)) else {
        return true;
    };
    node.spawns || atlas.monsters.hunt_room(&node.room)
}

/// Where to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestAt {
    Here,
    /// This room spawns; this one, within [`QUIET_WITHIN`] steps, does not.
    Walk(Key),
    /// This room spawns and no quiet room is near.
    HereAnyway,
}

/// Rest here if the room is quiet, else in the nearest quiet room within [`QUIET_WITHIN`]
/// steps (the lowest key among the nearest), else here anyway.
pub fn rest_at(atlas: &Atlas, room: Option<Key>, certain: bool, t: &Traveller) -> RestAt {
    if !spawns_here(atlas, room, certain) {
        return RestAt::Here;
    }
    let Some(from) = room else {
        return RestAt::HereAnyway;
    };
    let mut near: Vec<(u32, Key)> = distances(&atlas.graph, from, QUIET_WITHIN, t)
        .into_iter()
        .filter(|&(k, d)| d > 0 && !spawns_here(atlas, Some(k), true))
        .map(|(k, d)| (d, k))
        .collect();
    near.sort_unstable();
    near.first()
        .map_or(RestAt::HereAnyway, |&(_, k)| RestAt::Walk(k))
}

/// `room.flags`: a protected room (`RF_PROTECTED`, MAJORMUD.H:60). Entering one is refused
/// while in combat ("You may not enter that room while in combat.", LJNUTIL.C:2575).
const PROTECTED: i64 = 0x01;

/// An exit Flee may leave by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub dir: u8,
    pub to: Key,
    /// A door or gate, with its state as last known (open or closed, never locked).
    pub door: Option<LockState>,
    /// Nothing spawns in the room it leads to.
    pub quiet: bool,
}

/// The exits to flee by from `room`, best first: plain exits to rooms that spawn nothing,
/// then plain exits to rooms that spawn, then doors and gates (quiet first). Within each, a
/// room that is not protected comes first (a protected one refuses a character in combat),
/// then the direction's order. Never one in `blocked` (refused this session), a gated exit, a
/// Text, Hidden, Action or other exit that needs more than a direction, or a door known (or,
/// unseen, packed) locked: `doors` holds the states seen this session, over the pack's. Empty
/// when the belief is not sure of the room.
pub fn flee_exits(
    atlas: &Atlas,
    room: Option<Key>,
    certain: bool,
    blocked: &BTreeSet<(Key, u8)>,
    doors: &BTreeMap<(Key, u8), LockState>,
) -> Vec<Candidate> {
    let Some(from) = room.filter(|_| certain) else {
        return Vec::new();
    };
    let mut ranked: Vec<((u8, bool, u8), Candidate)> = atlas
        .graph
        .edges(from)
        .iter()
        .filter(|e| e.gate == Gate::None && !blocked.contains(&(from, e.dir)))
        .filter_map(|e| {
            let door = match e.kind {
                ExitType::Normal
                | ExitType::BlockGuard
                | ExitType::MapChange
                | ExitType::Ticket
                | ExitType::UserCount
                | ExitType::Delay => None,
                ExitType::Door | ExitType::Gate => {
                    match doors.get(&(from, e.dir)).copied().or(e.door) {
                        Some(LockState::Locked) | None => return None,
                        state => state,
                    }
                }
                _ => return None,
            };
            let to = atlas.graph.node(e.to)?;
            let quiet = !spawns_here(atlas, Some(e.to), true);
            let tier = match (door.is_some(), quiet) {
                (false, true) => 0,
                (false, false) => 1,
                (true, true) => 2,
                (true, false) => 3,
            };
            let protected = to.room.flags & PROTECTED != 0;
            let c = Candidate {
                dir: e.dir,
                to: e.to,
                door,
                quiet,
            };
            Some(((tier, protected, e.dir), c))
        })
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hp_percent_floors_and_needs_the_maximum() {
        assert_eq!(hp_percent(Some(28), Some(35)), Some(80));
        assert_eq!(hp_percent(Some(27), Some(35)), Some(77));
        assert_eq!(hp_percent(Some(-5), Some(35)), Some(-14));
        assert_eq!(hp_percent(Some(28), None), None);
        assert_eq!(hp_percent(None, Some(35)), None);
        assert_eq!(hp_percent(Some(1), Some(0)), None);
    }

    #[test]
    fn the_rules_keep_some_monsters_from_being_attacked() {
        let mk = |n: i64, name: &str| wayfarer_world::pack::Monster {
            number: n,
            name: name.into(),
            level: 1,
            exp: 9,
            max_hp: 5,
            mon_type: 6,
            gen_index: 1,
            alignment: 2,
            raw: String::new(),
        };
        let monsters = Monsters::new(vec![mk(1, "giant rat"), mk(2, "kobold"), mk(3, "orc")]);
        let rules = MonsterRules::new([
            ("giant rat", MonsterRule::NeverFight),
            ("orc", MonsterRule::FleeFrom),
            ("kobold", MonsterRule::Avoid),
        ]);
        let none = BTreeSet::new();
        let s = |v: &[&str]| v.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
        let here = s(&["fat giant rat", "orc", "kobold"]);
        // Never fight and flee from are passed over; avoid is fought.
        assert_eq!(
            target(&monsters, &rules, &here, &[], &none, false),
            Some("kobold")
        );
        // A never-fight monster that attacks is fought back; a flee-from one only cornered.
        let rat = s(&["fat giant rat"]);
        assert_eq!(
            target(&monsters, &rules, &here[..1], &rat, &none, false),
            Some("fat giant rat")
        );
        let orc = s(&["orc"]);
        assert_eq!(
            target(&monsters, &rules, &here[1..2], &orc, &none, false),
            None
        );
        assert_eq!(
            target(&monsters, &rules, &here[1..2], &orc, &none, true),
            Some("orc")
        );
        assert_eq!(flee_from(&rules, &here), Some("orc"));
        assert_eq!(flee_from(&rules, &rat), None);
    }

    #[test]
    fn the_attack_word_is_the_last() {
        assert_eq!(attack_word("large giant rat"), "rat");
        assert_eq!(attack_word("kobold"), "kobold");
    }
}
