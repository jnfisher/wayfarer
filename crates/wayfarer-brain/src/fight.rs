//! Fight (docs/brain.md "Fight here"): attack what is here, once, and let auto-combat run. It
//! never leaves the room. One `a <word>` per target: no attack command goes while engaged (the
//! executor's guard), since a repeat restarts the swing timer (`kb/pitfalls.md` 31, 41). The
//! fight ends with `*Combat Off*` or a kill, or lapses unannounced (pitfall 42, the session's
//! quiet rule); then it looks again and picks the next.

use std::collections::{BTreeMap, BTreeSet};

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;

use crate::executor::{Action, Expect, Outcome, Refused};
use crate::modes::{Mode, Step, View};
use crate::policy::{attack_word, target};

/// How many attacks on one monster may go unanswered (no `*Combat Engaged*`, no refusal)
/// before it is left alone for the session: a missed line is tried again, a refusal the
/// executor does not know is not tried for ever.
pub const ATTACK_TRIES: u32 = 2;

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
}

impl Mode for Fight {
    fn wants(&self, v: &View) -> Option<String> {
        if v.engaged {
            return Some("in combat".to_owned());
        }
        if self.look {
            return Some("looking for the next".to_owned());
        }
        let p = v.presence;
        target(&v.atlas.monsters, &p.here, &p.attackers, &self.skip).map(|t| format!("{t} is here"))
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
        let p = v.presence;
        match target(&v.atlas.monsters, &p.here, &p.attackers, &self.skip) {
            Some(t) => {
                self.target = Some(t.to_owned());
                Step::send(
                    Action::new(format!("a {}", attack_word(t)), Expect::Engaged),
                    format!("Fight: {t}"),
                )
            }
            None => Step::wait("Fight: nothing to fight"),
        }
    }

    fn on_event(&mut self, ev: &GameEvent, _now: Instant) {
        match ev {
            GameEvent::CombatOff | GameEvent::ExpGain { .. } => {
                self.target = None;
                self.look = true;
            }
            GameEvent::YouDoNotSee { .. } | GameEvent::NoEffect => self.refused(),
            GameEvent::YouKilled => {
                self.target = None;
                self.look = false;
            }
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
    /// The game refused the target: leave it alone for the session, and look again.
    fn refused(&mut self) {
        if let Some(t) = self.target.take() {
            tracing::info!(target = %t, "refused: skipped for the session");
            self.skip.insert(t);
        }
        self.look = true;
    }
}
