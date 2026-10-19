//! The bot's policies (docs/plan/03-engine.md section 4): pure functions of the state, the
//! world pack and the constants below, with no clock and nothing sent.

use std::collections::BTreeSet;
use std::time::Duration;

use wayfarer_world::{Key, Monsters, Traveller, distances, huntable};

use crate::brain::Atlas;

// The thresholds of docs/plan/04-settings.md section 3 at their defaults. They are constants
// in this slice; the settings replace them in M4.

/// Recover takes control below this much HP (per cent of the maximum).
pub const REST_BELOW: i32 = 80;
/// Recover rests until this much HP.
pub const REST_UNTIL: i32 = 95;
/// Below this the bot stops itself, in place of the Flee mode that is not built yet.
pub const STOP_BELOW: i32 = 30;
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
/// hostile one listed here; never one on the skip list.
pub fn target<'a>(
    monsters: &Monsters,
    here: &'a [String],
    attackers: &'a [String],
    skip: &BTreeSet<String>,
) -> Option<&'a str> {
    attackers
        .iter()
        .chain(here)
        .map(String::as_str)
        .find(|n| !skip.contains(*n) && hostile(monsters, n))
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
    fn the_attack_word_is_the_last() {
        assert_eq!(attack_word("large giant rat"), "rat");
        assert_eq!(attack_word("kobold"), "kobold");
    }
}
