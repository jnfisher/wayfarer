//! Stash (docs/brain.md "Stash currency", C-092): hide the silver, gold and copper carried in
//! a loop's first stop each time the loop gets there (`cash.stash_on_loop`, on by default), so
//! the coins a round picks up collect in one room out of sight, for the player to take later.
//! Platinum and runic are never hidden. A hidden coin is in the room's hidden list: `get`
//! finds it only after a search reveals it (`kb/economy.md`), so Loot never takes it back.
//!
//! A party leader with the setting on has each member with it hide its own coins too, once
//! for each time the leader's loop reaches its first stop (the coordinator passes the round on
//! as `PartyRole::Follower::stash`), whatever the member's own setting says.

use wayfarer_parse::typed::GameEvent;
use wayfarer_world::Key;

use crate::executor::{Action, Expect, Outcome};
use crate::loot::coin_word;
use crate::modes::{Mode, Step, View};

/// The coins hidden, by the word `hide` takes. Platinum and runic stay carried.
pub const STASHED: [&str; 3] = ["silver", "gold", "copper"];

#[derive(Debug, Default)]
pub(crate) struct Stash {
    /// Hide the coins here, once nothing is to be fought.
    due: Option<Key>,
    /// The coins hidden so far this time.
    hidden: Vec<&'static str>,
    /// `i` sent after hiding: the coins carried are read again.
    looked: bool,
}

impl Stash {
    /// Hide the coins carried in `room` when next there with nothing to fight.
    pub fn begin(&mut self, room: Key) {
        self.due = Some(room);
        self.hidden.clear();
        self.looked = false;
    }

    /// The next coin to hide, and how many: carried, and not hidden yet this time.
    fn next(&self, v: &View) -> Option<(&'static str, u64)> {
        STASHED.into_iter().find_map(|word| {
            if self.hidden.contains(&word) {
                return None;
            }
            let count: u64 = v
                .coins
                .iter()
                .filter(|c| coin_word(&c.coin) == word)
                .map(|c| c.count)
                .sum();
            (count > 0).then_some((word, count))
        })
    }

    /// The bot's tick: a stash due somewhere else is dropped (the loop moved on), and one with
    /// nothing left to do is over.
    pub fn observe(&mut self, v: &View) {
        let Some(room) = self.due else {
            return;
        };
        let done = self.next(v).is_none() && (self.hidden.is_empty() || self.looked);
        if v.room != Some(room) || done {
            self.due = None;
        }
    }
}

impl Mode for Stash {
    fn wants(&self, v: &View) -> Option<String> {
        (self.due.is_some() && self.due == v.room && !v.engaged)
            .then(|| "stashing coins".to_owned())
    }

    fn step(&mut self, v: &View) -> Step {
        if let Some((word, count)) = self.next(v) {
            self.hidden.push(word);
            return Step::send(
                Action::new(format!("hide {count} {word}"), Expect::Prompt),
                format!("Stash: hiding {count} {word} in {}", v.room_name(v.room)),
            )
            .doing("stashing coins");
        }
        if !self.hidden.is_empty() && !self.looked {
            // The coins carried, read again: the hidden ones are gone.
            self.looked = true;
            return Step::send(Action::new("i", Expect::Prompt), "Stash: done")
                .doing("stashing coins");
        }
        self.due = None;
        Step::wait("Stash: nothing to hide")
    }

    fn on_event(&mut self, _ev: &GameEvent, _now: tokio::time::Instant) {}

    fn outcome(&mut self, _action: &Action, _outcome: Outcome) {}
}
