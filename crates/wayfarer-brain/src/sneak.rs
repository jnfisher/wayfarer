//! Sneaking (C-048, docs/brain.md "Sneaking"): with `movement.sneak` on, the bot keeps the
//! character sneaking. Before a move it makes itself (a leader, a loop, Recover's walks; never
//! Flee) it sends `sneak` until an attempt does not say it failed; a follower, which the game
//! moves, sneaks again after each fight. An attempt can fail silently (LJNCMDA.C:2900-2921), so
//! the moves tell: a move made while sneaking prints `Sneaking...` before the game rolls for it
//! (LJNCMDB.C:5207-5211, LJNUTIL.C:2811-2816), and a failed roll ends the sneak silently
//! (LJNUTIL.C:2909-2926). A move without `Sneaking...` means the sneak had already ended (the
//! move before failed, or something else ended it): the bot sneaks again before the next move. Other ends of the belief:
//! the failure the character noticed, `You are no longer sneaking.`, `You make a sound as you
//! enter the room!`, a fight, a cast, a rest, a death. No attempt goes with a monster in the
//! room (no sneaking with monsters about; a fight comes first). After [`TRIES`] failures in a
//! row it moves on unsneaked until the next fight. `You may not sneak right now!` (just
//! attacked) waits a round.

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::{GameEvent, Rest};
use wayfarer_world::Key;

use crate::executor::{Action, Confirmed, Expect, Outcome, Refused};
use crate::modes::{Mode, Step, View};

/// Failed attempts in a row before the bot moves on unsneaked.
pub const TRIES: u32 = 5;
/// How long `You may not sneak right now!` holds the next attempt: about a round.
const NOT_NOW: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
pub(crate) struct Sneak {
    /// Believed sneaking: an attempt that did not say it failed, and nothing since that ends it.
    on: bool,
    /// Attempts in a row that failed.
    fails: u32,
    /// [`TRIES`] attempts failed: moving on unsneaked until the next fight.
    gave_up: bool,
    /// Said that it moves on unsneaked.
    said: bool,
    /// No attempt before this.
    wait_until: Option<Instant>,
    sent_at: Option<Instant>,
    /// The game said the sneak ended: a walk in progress stops to sneak again.
    broken: bool,
    /// The room the character was last known in, and whether `Sneaking...` has shown since
    /// it got there (the next move's sign).
    room: Option<Key>,
    heard: bool,
}

impl Sneak {
    /// Whether the player wants the character sneaking (`movement.sneak`).
    pub fn enabled(v: &View) -> bool {
        v.settings.bool("movement.sneak")
    }

    /// Believed sneaking.
    pub fn on(&self) -> bool {
        self.on
    }

    fn may_try(&self, v: &View) -> bool {
        Self::enabled(v)
            && !self.on
            && !self.gave_up
            && self.wait_until.is_none_or(|t| v.now >= t)
            && v.presence.here.is_empty()
    }

    /// The bot is about to move the character itself: sneak first.
    pub fn before_move(&self, v: &View) -> bool {
        self.may_try(v) && !v.engaged && v.presence.attackers.is_empty()
    }

    /// It moves on unsneaked, said once: the narration, the first time.
    pub fn unsneaked(&mut self) -> Option<String> {
        (self.gave_up && !std::mem::replace(&mut self.said, true))
            .then(|| format!("Sneak: {TRIES} attempts failed; moving on unsneaked"))
    }

    /// The game said the sneak ended since last asked.
    pub fn take_broken(&mut self) -> bool {
        std::mem::take(&mut self.broken)
    }

    /// When the wait after `You may not sneak right now!` ends.
    pub fn wake(&self, now: Instant) -> Option<Instant> {
        self.wait_until.filter(|&t| t > now)
    }

    /// The character is in `room` now. A move made while believed sneaking that showed no
    /// `Sneaking...` means the sneak had already ended: counted as a failed attempt.
    pub fn arrived(&mut self, room: Option<Key>) {
        let Some(room) = room else {
            return;
        };
        if self.room == Some(room) {
            return;
        }
        if self.room.is_some() && self.on && !self.heard {
            self.off();
            self.failed();
        }
        self.room = Some(room);
        self.heard = false;
    }

    fn failed(&mut self) {
        self.fails += 1;
        if self.fails >= TRIES {
            self.gave_up = true;
            self.said = false;
            self.fails = 0;
        }
    }

    fn off(&mut self) {
        if self.on {
            self.broken = true;
        }
        self.on = false;
    }
}

impl Mode for Sneak {
    /// A follower out of a fight, not resting, sneaks again (the game moves it; it never
    /// walks itself while with its leader).
    fn wants(&self, v: &View) -> Option<String> {
        (v.party.following().is_some()
            && v.party.rejoin().is_none()
            && self.may_try(v)
            && !v.engaged
            && !v.resting
            && v.presence.attackers.is_empty()
            && v.party.party_rest().is_none())
        .then(|| "not sneaking".to_owned())
    }

    fn step(&mut self, v: &View) -> Step {
        self.sent_at = Some(v.now);
        let n = self.fails + 1;
        let why = if n > 1 {
            format!("Sneak: sneaking (attempt {n})")
        } else {
            "Sneak: sneaking".to_owned()
        };
        Step::send(Action::new("sneak", Expect::Sneak), why).doing("sneaking")
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            // Not yet sure: the next move says.
            GameEvent::SneakAttempt { failed: false } => self.on = true,
            // Still sneaking when this move began (its own roll comes after).
            GameEvent::StillSneaking => {
                self.on = true;
                self.heard = true;
                self.fails = 0;
            }
            GameEvent::SneakAttempt { failed: true } => {
                self.on = false;
                self.failed();
            }
            GameEvent::SneakRefused => self.wait_until = Some(now + NOT_NOW),
            GameEvent::NotSneaking => self.off(),
            // A fight ends sneaking, and starts the attempts afresh.
            GameEvent::CombatEngaged | GameEvent::AttackedBy { .. } => {
                self.off();
                self.gave_up = false;
                self.fails = 0;
            }
            GameEvent::SpellCast { .. }
            | GameEvent::CastFailed { .. }
            | GameEvent::YouKilled
            | GameEvent::Resting { .. }
            | GameEvent::Prompt {
                resting: Some(Rest::Resting | Rest::Meditating),
                ..
            } => self.off(),
            _ => {}
        }
    }

    fn outcome(&mut self, _action: &Action, outcome: Outcome) {
        match outcome {
            Outcome::Confirmed(Confirmed::Sneak { .. }) => {}
            Outcome::Refused(Refused::NotNow) => {
                self.wait_until = self.sent_at.map(|t| t + NOT_NOW);
            }
            // No answer: counted as a failure.
            _ => self.failed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_game_says_turns_the_belief_on_and_off() {
        let now = Instant::now();
        let mut s = Sneak::default();
        s.on_event(&GameEvent::SneakAttempt { failed: false }, now);
        assert!(s.on());
        // A fight ends it, and the game noticing a move does too; each says the sneak broke.
        s.on_event(&GameEvent::CombatEngaged, now);
        assert!(!s.on() && s.take_broken());
        s.on_event(&GameEvent::StillSneaking, now);
        assert!(s.on());
        s.on_event(&GameEvent::NotSneaking, now);
        assert!(!s.on() && s.take_broken() && !s.take_broken());
        s.on_event(&GameEvent::SneakRefused, now);
        assert_eq!(s.wake(now), Some(now + NOT_NOW));
    }

    #[test]
    fn a_move_without_sneaking_shown_means_the_sneak_failed() {
        let now = Instant::now();
        let (a, b, c): (Key, Key, Key) = ((1, 1), (1, 2), (1, 3));
        let mut s = Sneak::default();
        s.arrived(Some(a));
        s.on_event(&GameEvent::SneakAttempt { failed: false }, now);
        // The move shows `Sneaking...`: still on.
        s.on_event(&GameEvent::StillSneaking, now);
        s.arrived(Some(b));
        assert!(s.on() && !s.take_broken());
        // The next shows nothing: the move before it failed, or something ended the sneak.
        s.arrived(None);
        s.arrived(Some(c));
        assert!(!s.on() && s.take_broken());
        assert_eq!(s.fails, 1);
    }

    #[test]
    fn five_failures_give_up_until_the_next_fight() {
        let now = Instant::now();
        let mut s = Sneak::default();
        for _ in 0..TRIES {
            assert!(!s.gave_up);
            s.on_event(&GameEvent::SneakAttempt { failed: true }, now);
        }
        assert!(s.gave_up);
        assert!(s.unsneaked().is_some());
        assert!(s.unsneaked().is_none(), "said once");
        s.on_event(&GameEvent::AttackedBy { name: "rat".into() }, now);
        assert!(!s.gave_up);
    }
}
