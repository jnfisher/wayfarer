//! Loot (docs/brain.md "Coins"): pick up the coins left in the room (after a kill, say), the
//! kinds the settings want, as many as the load allows. It takes the character only when
//! nothing is attacking and nothing is in flight; Flee comes before it, Recover and Fight after.
//!
//! The weight of coins and the words for a load are the game's (`kb/economy.md` "Encumbrance"):
//! three coins of a type weigh one unit, each type counted on its own; the load in per cent of
//! the capacity the game reports is None to 16, Light over 16, Medium over 33, Heavy over 66.
//! The capacity already holds the race's share (a dwarf's +20%), so using it as the game gives
//! it needs no race rule.

use std::collections::BTreeMap;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;

use crate::executor::{Action, Expect, Outcome};
use crate::modes::{Mode, Step, View};

/// `get N <coin>` takes 1 to 99998 (`kb/economy.md`).
const MOST_AT_ONCE: u64 = 99_998;
/// Tries at one coin in one room view before it is left alone: a pickup that is answered with
/// no `You picked up` line is not tried for ever.
const TRIES: u32 = 2;

/// The coins the bot takes, largest first, by the word `get` takes. Copper is never taken.
pub const COINS: [&str; 4] = ["runic", "platinum", "gold", "silver"];

/// The first word of a coin's name (`gold crowns` is `gold`).
pub fn coin_word(name: &str) -> &str {
    name.split_whitespace().next().unwrap_or(name)
}

/// The weight units of `count` coins of one type.
pub fn units(count: u64) -> u64 {
    count / 3
}

/// The game's word for a load of `pct` per cent of the capacity.
pub fn level_word(pct: u64) -> &'static str {
    match pct {
        0..=16 => "None",
        17..=33 => "Light",
        34..=66 => "Medium",
        _ => "Heavy",
    }
}

/// The most per cent of the capacity a `cash.max_encumbrance` word allows.
pub fn limit_pct(word: &str) -> u64 {
    match word {
        "light" => 33,
        "medium" => 66,
        _ => 100,
    }
}

/// How many more coins of a type the character may pick up: it holds `have` of them, carries
/// `current` of `max` weight units, and the load may reach `limit` per cent (the game works
/// the percent out by truncation: `current * 100 / max`).
pub fn room_for(have: u64, current: u64, max: u64, limit: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    // The heaviest load that still counts as `limit` per cent.
    let ceiling = ((limit + 1) * max).saturating_sub(1) / 100;
    let spare = ceiling.saturating_sub(current);
    // `units(have + n) - units(have) <= spare`.
    (3 * (units(have) + spare) + 2).saturating_sub(have)
}

#[derive(Debug, Default)]
pub(crate) struct Loot {
    /// `get` sent for a coin since the room was last shown.
    tries: BTreeMap<&'static str, u32>,
}

impl Loot {
    /// What to pick up: the coin's word and how many, the first kind that is wanted, on the
    /// floor, not tried to the limit and fits the load.
    fn next(&self, v: &View) -> Option<(&'static str, u64)> {
        if v.engaged || !v.presence.attackers.is_empty() {
            return None;
        }
        for word in COINS {
            if !v.wants_coin(word) || self.tries.get(word).copied().unwrap_or(0) >= TRIES {
                continue;
            }
            let on_floor = v
                .presence
                .floor
                .iter()
                .find(|(name, _)| coin_word(name) == word)
                .map_or(0, |(_, n)| *n);
            if on_floor == 0 {
                continue;
            }
            let n = match v.load {
                Some((current, max)) => on_floor.min(room_for(
                    v.carried(word),
                    current,
                    max,
                    limit_pct(v.settings.choice("cash.max_encumbrance")),
                )),
                // The load is not known (no `i` yet): take them all; the game has its limit.
                None => on_floor,
            };
            if n > 0 {
                return Some((word, n.min(MOST_AT_ONCE)));
            }
        }
        None
    }

    /// The room is shown again: the tries start over.
    pub fn room_changed(&mut self) {
        self.tries.clear();
    }
}

impl Mode for Loot {
    fn wants(&self, v: &View) -> Option<String> {
        self.next(v).map(|(coin, n)| format!("{n} {coin} here"))
    }

    fn step(&mut self, v: &View) -> Step {
        match self.next(v) {
            Some((coin, n)) => {
                *self.tries.entry(coin).or_default() += 1;
                Step::send(
                    Action::new(format!("get {n} {coin}"), Expect::Prompt),
                    format!("Loot: picking up {n} {coin}"),
                )
            }
            None => Step::wait("Loot: nothing to pick up"),
        }
    }

    fn on_event(&mut self, ev: &GameEvent, _now: Instant) {
        match ev {
            GameEvent::RoomTitle { .. } => self.room_changed(),
            // The game says the load is too much: not for any coin, this view.
            GameEvent::CannotCarry => {
                for coin in COINS {
                    self.tries.insert(coin, TRIES);
                }
            }
            _ => {}
        }
    }

    fn outcome(&mut self, _action: &Action, _outcome: Outcome) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_for_a_load_are_the_games() {
        for (pct, word) in [
            (0, "None"),
            (16, "None"),
            (17, "Light"),
            (33, "Light"),
            (34, "Medium"),
            (66, "Medium"),
            (67, "Heavy"),
        ] {
            assert_eq!(level_word(pct), word, "{pct}%");
        }
    }

    #[test]
    fn coins_weigh_a_unit_a_three_each_type_on_its_own() {
        assert_eq!(
            [units(0), units(2), units(3), units(8), units(9)],
            [0, 0, 1, 2, 3]
        );
    }

    #[test]
    fn how_many_coins_fit_under_a_limit() {
        // Capacity 1000: Medium may reach 66% = 669 units (669 * 100 / 1000 = 66).
        // Holding 600 units and no gold, 69 more units fit: 3 * 69 + 2 = 209 coins.
        assert_eq!(room_for(0, 600, 1000, 66), 209);
        // Already holding 1 gold (no weight yet): one fewer.
        assert_eq!(room_for(1, 600, 1000, 66), 208);
        // At the ceiling, only the coins that still weigh nothing: up to two in a type.
        assert_eq!(room_for(0, 669, 1000, 66), 2);
        assert_eq!(room_for(2, 669, 1000, 66), 0);
        // Over the limit already: only the coins that weigh nothing.
        assert_eq!(room_for(0, 800, 1000, 66), 2);
        // Light is 33%: 339 units.
        assert_eq!(room_for(0, 330, 1000, 33), 3 * 9 + 2);
    }

    #[test]
    fn a_roomier_capacity_takes_more() {
        // A dwarf's capacity is 20% larger, so the same load is a smaller share of it.
        let human = room_for(0, 500, 1000, 66);
        let dwarf = room_for(0, 500, 1200, 66);
        assert!(dwarf > human, "{dwarf} {human}");
    }

    #[test]
    fn the_coin_word_is_the_first_word() {
        assert_eq!(coin_word("gold crowns"), "gold");
        assert_eq!(coin_word("runic coin"), "runic");
    }
}
