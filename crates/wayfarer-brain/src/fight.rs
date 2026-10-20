//! Fight (docs/brain.md "Fight here"): attack what is here, once, and let auto-combat run. It
//! never leaves the room. One `a <word>` per target: no attack command goes while engaged (the
//! executor's guard), since a repeat restarts the swing timer (`kb/pitfalls.md` 31, 41). The
//! fight ends with `*Combat Off*` or a kill, or lapses unannounced (pitfall 42, the session's
//! quiet rule); then it looks again and picks the next. The monster rules keep it off some
//! monsters ([`target`]).
//!
//! A fight can be a stalemate: the target's armour takes every blow (`glances off its
//! armour`) and it never dies. After [`STALEMATE_ROUNDS`] rounds of the character's swings with
//! no damage dealt, at least one of them a glance, Fight gives the target up: the bot leaves
//! as Flee does and the monster becomes *never fight* for the session.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;

use crate::brain::Atlas;
use crate::executor::{Action, Expect, Outcome, Refused};
use crate::modes::{Mode, Presence, Step, View};
use crate::policy::{attack_word, target};
use wayfarer_world::MonsterRules;

/// How many attacks on one monster may go unanswered (no `*Combat Engaged*`, no refusal)
/// before it is left alone for the session: a missed line is tried again, a refusal the
/// executor does not know is not tried for ever.
pub const ATTACK_TRIES: u32 = 2;
/// Rounds of the character's own swings in a row that did no damage, one of them a glance at
/// least, before the fight is a stalemate.
pub const STALEMATE_ROUNDS: u32 = 4;
/// The character's swings of one round come together (a prompt after each); a swing this long
/// after the round's first starts the next round. Rounds are 5 s apart (`kb/combat.md`).
pub const ROUND_GAP: Duration = Duration::from_secs(2);

/// The character's swings at the target in the round going on.
#[derive(Debug, Clone, Copy)]
struct Round {
    started: Instant,
    damaged: bool,
    glanced: bool,
}

/// The rounds without damage against the target, so far.
#[derive(Debug, Default)]
struct Stall {
    round: Option<Round>,
    rounds: u32,
    glanced: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Fight {
    /// The monster attacked, until its fight ends.
    target: Option<String>,
    /// Look before choosing again: after a kill, `*Combat Off*` or a refused target.
    look: bool,
    /// Monsters the game refused as targets (not here, the weapon does nothing), or whose
    /// attacks went unanswered [`ATTACK_TRIES`] times: left alone for the rest of the session.
    skip: BTreeSet<String>,
    /// The attacks on each monster that timed out.
    timeouts: BTreeMap<String, u32>,
    /// Flee could not get the character out of a monster to flee from: it is fought if it
    /// attacks.
    cornered: bool,
    stall: Stall,
    /// A stalemate with this monster, for the bot to act on.
    stalemate: Option<String>,
}

impl Mode for Fight {
    fn wants(&self, v: &View) -> Option<String> {
        if v.engaged {
            return Some("in combat".to_owned());
        }
        if self.look {
            return Some("looking for the next".to_owned());
        }
        self.pick(v.atlas, v.rules, v.presence)
            .map(|t| format!("{t} is here"))
    }

    fn step(&mut self, v: &View) -> Step {
        if v.engaged {
            let t = self.target.as_deref().unwrap_or("in combat");
            return Step::wait(format!("Fight: {t}"));
        }
        if self.look {
            // `look`, not an empty line: an empty line shows the room only in statline FULL
            // modes (`kb/screen-parsing.md`).
            return Step::send(
                Action::new("look", Expect::RoomSeen),
                "Fight: looking for the next",
            );
        }
        match self.pick(v.atlas, v.rules, v.presence) {
            Some(t) => {
                let t = t.to_owned();
                self.target = Some(t.clone());
                self.stall = Stall::default();
                Step::send(
                    Action::new(format!("a {}", attack_word(&t)), Expect::Engaged),
                    format!("Fight: {t}"),
                )
            }
            None => Step::wait("Fight: nothing to fight"),
        }
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            GameEvent::CombatOff | GameEvent::ExpGain { .. } => {
                self.target = None;
                self.look = true;
                self.stall = Stall::default();
            }
            GameEvent::YouDoNotSee { .. } | GameEvent::NoEffect => self.refused(),
            GameEvent::YouKilled => {
                self.target = None;
                self.look = false;
                self.stall = Stall::default();
            }
            GameEvent::YouHit { target, .. } => self.swing(target, true, false, now),
            GameEvent::YouMiss { target } => self.swing(target, false, false, now),
            GameEvent::SwingGlanced { target } => self.swing(target, false, true, now),
            _ => {}
        }
    }

    fn outcome(&mut self, action: &Action, outcome: Outcome) {
        match (action.expect, outcome) {
            (Expect::RoomSeen, _) => self.look = false,
            (Expect::Engaged, Outcome::Refused(Refused::Target)) => self.refused(),
            (Expect::Engaged, Outcome::TimedOut) => {
                if let Some(t) = self.target.take() {
                    let n = self.timeouts.entry(t.clone()).or_default();
                    *n += 1;
                    if *n >= ATTACK_TRIES {
                        tracing::info!(target = %t, "unanswered: skipped for the session");
                        self.skip.insert(t);
                    }
                }
                self.look = true;
            }
            _ => {}
        }
    }
}

impl Fight {
    fn pick<'a>(
        &self,
        atlas: &Atlas,
        rules: &MonsterRules,
        presence: &'a Presence,
    ) -> Option<&'a str> {
        target(
            &atlas.monsters,
            rules,
            &presence.here,
            &presence.attackers,
            &self.skip,
            self.cornered,
        )
    }

    /// Whether it would take the character with `presence` in the room: a monster it would
    /// attack, or anything attacking.
    pub fn would_fight(&self, atlas: &Atlas, rules: &MonsterRules, presence: &Presence) -> bool {
        !presence.attackers.is_empty() || self.pick(atlas, rules, presence).is_some()
    }

    /// Flee could not leave a monster to flee from (`true`): fight it if it attacks.
    pub fn set_cornered(&mut self, cornered: bool) {
        self.cornered = cornered;
    }

    /// The game refused the target: leave it alone for the session, and look again.
    fn refused(&mut self) {
        if let Some(t) = self.target.take() {
            tracing::info!(target = %t, "refused: skipped for the session");
            self.skip.insert(t);
        }
        self.look = true;
        self.stall = Stall::default();
    }

    /// One of the character's swings at `at`: counted if it is at the target.
    fn swing(&mut self, at: &str, damaged: bool, glanced: bool, now: Instant) {
        let Some(t) = &self.target else {
            return;
        };
        let (a, b) = (at.to_lowercase(), t.to_lowercase());
        if !(a == b || a.contains(&b) || b.contains(&a)) {
            return;
        }
        if self
            .stall
            .round
            .is_some_and(|r| now >= r.started + ROUND_GAP)
        {
            self.close_round();
        }
        let round = self.stall.round.get_or_insert(Round {
            started: now,
            damaged: false,
            glanced: false,
        });
        round.damaged |= damaged;
        round.glanced |= glanced;
    }

    /// The round is over: count it, and call the stalemate when it is the last one needed.
    fn close_round(&mut self) {
        let Some(round) = self.stall.round.take() else {
            return;
        };
        if round.damaged {
            self.stall = Stall::default();
            return;
        }
        self.stall.rounds += 1;
        self.stall.glanced |= round.glanced;
        if self.stall.rounds >= STALEMATE_ROUNDS && self.stall.glanced {
            let t = self.target.take().expect("swings at a target");
            tracing::info!(target = %t, rounds = self.stall.rounds, "stalemate");
            self.stall = Stall::default();
            self.stalemate = Some(t);
        }
    }

    /// The bot's tick: a round whose swings are all in is counted now, not at the next one.
    pub fn settle(&mut self, now: Instant) {
        if self
            .stall
            .round
            .is_some_and(|r| now >= r.started + ROUND_GAP)
        {
            self.close_round();
        }
    }

    /// When the round going on is over, for the clock to wake the bot.
    pub fn wake(&self) -> Option<Instant> {
        self.stall.round.map(|r| r.started + ROUND_GAP)
    }

    /// The monster a stalemate gave up, once.
    pub fn take_stalemate(&mut self) -> Option<String> {
        self.stalemate.take()
    }
}
