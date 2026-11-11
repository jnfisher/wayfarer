//! Stash (docs/brain.md "Stash currency", C-092, C-093): hide the silver, gold and copper
//! carried in a loop's first stop each time the loop gets there (`cash.stash_on_loop`, on by
//! default), so the coins a round picks up collect in one room out of sight, for the player to
//! take later. Platinum and runic are never hidden. A hidden coin is in the room's hidden list:
//! `get` finds it only after a search reveals it (`kb/economy.md`), so Loot never takes it back.
//!
//! A party leader with the setting on has each member with it hide its own coins too, once
//! for each time the leader's loop reaches its first stop (the coordinator passes the round on
//! as `PartyRole::Follower::stash`), whatever the member's own setting says.
//!
//! Coins hidden in the wrong one of a maze of look-alike rooms are lost to the player, so a
//! stash never hides on a guess (C-093). The leader hides only when its belief is sure of the
//! stop. A follower borrows the leader's sureness: it hides only in the leader's room, with the
//! leader in sight. The counts come from an `i` sent first, not from the last one, because a
//! give or a share since then changed them. A coin still carried after its `hide` is read and
//! hidden again once.

use wayfarer_parse::typed::GameEvent;
use wayfarer_world::Key;

use crate::executor::{Action, Expect, Outcome};
use crate::loot::coin_word;
use crate::modes::{Mode, Step, View};
use crate::walker::key_str;

/// The coins hidden, by the word `hide` takes. Platinum and runic stay carried.
pub const STASHED: [&str; 3] = ["silver", "gold", "copper"];

/// Who vouches for the room the coins go in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sure {
    /// The bot's own belief, sure of the loop's first stop.
    Own,
    /// The party leader's: sure of its first stop, with this follower in that room. The name
    /// is checked against the room's `Also here:`.
    Leader,
}

#[derive(Debug, Default)]
pub(crate) struct Stash {
    /// Hide the coins here, once nothing is to be fought.
    due: Option<Key>,
    sure: Option<Sure>,
    /// `look` sent when it was not sure of the room: the room is shown again, with who is in
    /// it, before the stash gives up on it.
    glanced: bool,
    /// `i` sent before the first `hide`: the counts are the ones carried now.
    read: bool,
    /// The coins hidden so far this time.
    hidden: Vec<&'static str>,
    /// The coins hidden a second time, after the `i` after hiding still showed them.
    again: Vec<&'static str>,
    /// `i` sent after hiding: the coins carried are read again.
    looked: bool,
}

/// Whether a stash may hide coins here: the stop it is due in, the believed room, and whether
/// the belief is sure of it. Unsure, or sure of another room, it may not (C-093).
pub fn sure_of(due: Key, room: Option<Key>, certain: bool) -> bool {
    certain && room == Some(due)
}

impl Stash {
    /// Hide the coins carried in `stop`, the loop's first stop, when next there with nothing
    /// to fight, and only if the belief is sure of it then.
    pub fn begin(&mut self, stop: Key) {
        self.start(stop, Sure::Own);
    }

    /// A follower's stash: hide the coins in the leader's room `room`, which the leader is
    /// sure of, while this follower is there with the leader.
    pub fn begin_following(&mut self, room: Key) {
        self.start(room, Sure::Leader);
    }

    /// A stash is under way: begun and not yet done or given up.
    pub fn active(&self) -> bool {
        self.due.is_some()
    }

    fn start(&mut self, room: Key, sure: Sure) {
        *self = Stash {
            due: Some(room),
            sure: Some(sure),
            ..Stash::default()
        };
    }

    /// Whether this tick's view lets the stash hide coins in its room.
    fn here(&self, v: &View) -> bool {
        let Some(due) = self.due else {
            return false;
        };
        match self.sure {
            Some(Sure::Own) => sure_of(due, v.room, v.certain),
            Some(Sure::Leader) => {
                v.party.stash().is_some_and(|(_, room)| room == due)
                    && v.party
                        .following()
                        .is_some_and(|leader| v.presence.sees_player(leader))
            }
            None => false,
        }
    }

    fn count(v: &View, word: &str) -> u64 {
        v.coins
            .iter()
            .filter(|c| coin_word(&c.coin) == word)
            .map(|c| c.count)
            .sum()
    }

    /// The next coin to hide, and how many: carried, and not hidden yet this time.
    fn next(&self, v: &View) -> Option<(&'static str, u64)> {
        STASHED.into_iter().find_map(|word| {
            if self.hidden.contains(&word) {
                return None;
            }
            let count = Self::count(v, word);
            (count > 0).then_some((word, count))
        })
    }

    /// A coin hidden once and still carried after the `i` that followed: the game refused the
    /// count (a give or a share changed it), so it goes again with the new one, once.
    fn left(&self, v: &View) -> Option<(&'static str, u64)> {
        self.hidden.iter().find_map(|&word| {
            let count = Self::count(v, word);
            (count > 0 && !self.again.contains(&word)).then_some((word, count))
        })
    }

    fn place(&self, v: &View) -> String {
        let name = v.room_name(self.due);
        self.due
            .map_or(name.clone(), |k| format!("{name} ({})", key_str(k)))
    }

    /// The bot's tick: a stash due somewhere else is dropped (the loop moved on).
    pub fn observe(&mut self, v: &View) {
        let Some(due) = self.due else {
            return;
        };
        let gone = match self.sure {
            // Sure of being somewhere else.
            Some(Sure::Own) => v.certain && v.room.is_some_and(|r| r != due),
            // The leader's round is over, or it is somewhere else now.
            Some(Sure::Leader) => !v.party.stash().is_some_and(|(_, room)| room == due),
            None => true,
        };
        if gone {
            self.due = None;
        }
    }
}

impl Mode for Stash {
    fn wants(&self, v: &View) -> Option<String> {
        (self.due.is_some() && !v.engaged).then(|| "stashing coins".to_owned())
    }

    fn step(&mut self, v: &View) -> Step {
        if !self.read && self.next(v).is_none() {
            // Nothing to hide by the last count: not worth an `i` (a give into an empty purse
            // waits for the next round).
            self.due = None;
            return Step::wait("Stash: nothing to hide");
        }
        if !self.read {
            // Not sure where it is: nothing is hidden this time round. The next round tries
            // again; coins in the wrong look-alike would be lost to the player.
            if !self.here(v) && !self.glanced {
                self.glanced = true;
                return Step::send(
                    Action::new("look", Expect::Prompt),
                    "Stash: looking where it is",
                )
                .doing("stashing coins");
            }
            if !self.here(v) {
                let place = self.place(v);
                self.due = None;
                return Step::wait(format!("Stash: not sure this is {place}, not hiding"));
            }
            self.read = true;
            let place = self.place(v);
            return Step::send(
                Action::new("i", Expect::Prompt),
                format!("Stash: counting the coins to hide in {place}"),
            )
            .doing("stashing coins");
        }
        if let Some((word, count)) = self.next(v) {
            self.hidden.push(word);
            let place = self.place(v);
            return Step::send(
                Action::new(format!("hide {count} {word}"), Expect::Prompt),
                format!("Stash: hiding {count} {word} in {place}"),
            )
            .doing("stashing coins");
        }
        if !self.hidden.is_empty() && !self.looked {
            // The coins carried, read again: the hidden ones are gone.
            self.looked = true;
            return Step::send(Action::new("i", Expect::Prompt), "Stash: checking")
                .doing("stashing coins");
        }
        if let Some((word, count)) = self.left(v) {
            self.again.push(word);
            self.looked = false;
            return Step::send(
                Action::new(format!("hide {count} {word}"), Expect::Prompt),
                format!("Stash: {count} {word} still carried, hiding them again"),
            )
            .doing("stashing coins");
        }
        self.due = None;
        let still: Vec<String> = self
            .hidden
            .iter()
            .filter_map(|&w| {
                let n = Self::count(v, w);
                (n > 0).then(|| format!("{n} {w}"))
            })
            .collect();
        if self.hidden.is_empty() {
            Step::wait("Stash: nothing to hide")
        } else if still.is_empty() {
            Step::wait("Stash: done")
        } else {
            Step::wait(format!(
                "Stash: done, but {} would not hide",
                still.join(", ")
            ))
        }
    }

    fn on_event(&mut self, _ev: &GameEvent, _now: tokio::time::Instant) {}

    fn outcome(&mut self, _action: &Action, _outcome: Outcome) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coins_are_hidden_only_where_the_belief_is_sure_of_the_stop() {
        let stop = (15, 12);
        assert!(sure_of(stop, Some(stop), true));
        // One of a maze of look-alikes: the best guess is the stop, but it is a guess.
        assert!(!sure_of(stop, Some(stop), false));
        // Sure, of another room.
        assert!(!sure_of(stop, Some((15, 5)), true));
        assert!(!sure_of(stop, None, false));
    }
}
