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
//! row it moves on unsneaked until the next fight. `You may not sneak right now!` is the game
//! still counting an attacker, or a monster in the room that could attack (`can_sneak`), which
//! a kill clears within a round or two: it waits [`NOT_NOW`], or only until the next room
//! (C-122; a follower that was refused right after a kill went five rooms unsneaked).

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::{GameEvent, Rest};

use crate::executor::{Action, Confirmed, Expect, Outcome, Refused};
use crate::modes::{Mode, Presence, Step, View};

/// Failed attempts in a row before the bot moves on unsneaked.
pub const TRIES: u32 = 5;
/// How long `You may not sneak right now!` holds the next attempt (C-122): the game refuses
/// while it still counts an attacker or a monster that could attack, which a kill clears
/// within a round or two; a party on the move is rooms away in five seconds, so two.
pub const NOT_NOW: Duration = Duration::from_secs(2);

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
    /// Whether `Sneaking...` has shown since the last move (the next move's sign).
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
        Self::enabled(v) && self.clear_to_try(&v.presence.here, v.now)
    }

    /// Not sneaking, not given up, no wait after a refusal, and no monster in the room: an
    /// attempt with one there fails, so the bot never makes one.
    fn clear_to_try(&self, here: &[String], now: Instant) -> bool {
        !self.on && !self.gave_up && self.wait_until.is_none_or(|t| now >= t) && here.is_empty()
    }

    /// A walk under way that set off unsneaked (a monster in the room, the wait after `You may
    /// not sneak right now!`) stops where the character can sneak (C-135): the first room
    /// with no monster in it and nothing attacking, so the bot sneaks there and walks on. Before,
    /// nothing stopped such a walk, and it went unsneaked to its end (Penny, 46 steps back
    /// from a rest after one refusal).
    pub fn due(&self, presence: &Presence, now: Instant) -> bool {
        self.clear_to_try(&presence.here, now) && presence.attackers.is_empty()
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

    /// The character took a move (a direction that went). A move believed sneaking that showed
    /// no `Sneaking...` means the sneak had ended: a failed attempt. A move also ends the wait
    /// after a refusal (C-122). Driven by the move itself, not the room key, so it works where
    /// the belief does not know the room (terse corridors, a maze): C-133, Penny the Gypsy
    /// walked a long way unsneaked while her position was unknown, since the old room-change
    /// check never fired.
    pub fn moved(&mut self) {
        if self.on && !self.heard {
            self.off();
            self.failed();
        }
        self.heard = false;
        self.wait_until = None;
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
            // Every cast ends sneaking (`kb/spells.md`), a blessing's as much as an attack's.
            GameEvent::SpellCast { .. }
            | GameEvent::YouCast { .. }
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

    /// C-122: a refusal holds the next attempt two seconds, and not past the next room.
    #[test]
    fn a_refusal_waits_two_seconds_or_until_the_next_room() {
        let now = Instant::now();
        let mut s = Sneak::default();
        s.on_event(&GameEvent::SneakRefused, now);
        assert_eq!(NOT_NOW, Duration::from_secs(2));
        assert_eq!(s.wake(now), Some(now + NOT_NOW));
        assert_eq!(s.wake(now + NOT_NOW), None);
        // The party moved on: the monster or the attack the game counted is behind.
        s.on_event(&GameEvent::SneakRefused, now);
        s.moved();
        assert_eq!(s.wake(now), None);
        // No move: the wait still holds.
        s.on_event(&GameEvent::SneakRefused, now);
        assert_eq!(s.wake(now), Some(now + NOT_NOW));
    }

    #[test]
    fn a_move_without_sneaking_shown_means_the_sneak_failed() {
        let now = Instant::now();
        let mut s = Sneak::default();
        s.on_event(&GameEvent::SneakAttempt { failed: false }, now);
        // The move shows `Sneaking...`: still on.
        s.on_event(&GameEvent::StillSneaking, now);
        s.moved();
        assert!(s.on() && !s.take_broken());
        // The next move shows nothing: the move before it failed, or something ended the sneak.
        // No room key is needed: this holds even where the belief is lost (C-133).
        s.moved();
        assert!(!s.on() && s.take_broken());
        assert_eq!(s.fails, 1);
    }

    /// C-135: a blessing ends the sneak as any cast does, and says so, so a walk stops to sneak.
    #[test]
    fn a_cast_that_does_no_damage_ends_the_sneak() {
        let now = Instant::now();
        let mut s = Sneak::default();
        s.on_event(&GameEvent::SneakAttempt { failed: false }, now);
        s.on_event(
            &GameEvent::YouCast {
                spell: "blur".into(),
            },
            now,
        );
        assert!(!s.on() && s.take_broken());
    }

    /// C-135: due where an attempt may go: not with a monster here or attacking, not in the
    /// wait after a refusal, not when sneaking or given up.
    #[test]
    fn due_only_in_a_clear_room_with_nothing_holding_the_attempt() {
        let now = Instant::now();
        let mut s = Sneak::default();
        let clear = Presence::default();
        assert!(s.due(&clear, now));
        let mut monster = Presence::default();
        monster.here.push("violet spore".into());
        assert!(!s.due(&monster, now));
        let mut attacked = Presence::default();
        attacked.attackers.push("thug".into());
        assert!(!s.due(&attacked, now));
        // Refused: not until the wait ends, or the next move.
        s.on_event(&GameEvent::SneakRefused, now);
        assert!(!s.due(&clear, now));
        assert!(s.due(&clear, now + NOT_NOW));
        s.moved();
        assert!(s.due(&clear, now));
        s.on_event(&GameEvent::SneakAttempt { failed: false }, now);
        assert!(!s.due(&clear, now));
        let mut s = Sneak {
            gave_up: true,
            ..Sneak::default()
        };
        assert!(!s.due(&clear, now));
        s.gave_up = false;
        assert!(s.due(&clear, now));
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
        s.on_event(
            &GameEvent::AttackedBy {
                name: "rat".into(),
                blow: None,
            },
            now,
        );
        assert!(!s.gave_up);
    }
}
