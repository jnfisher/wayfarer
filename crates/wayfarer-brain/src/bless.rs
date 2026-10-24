//! Blessings (C-047, docs/brain.md "Blessings"): the lasting spells the player ticked for the
//! character (bless, barkskin, holy aura) are kept on. Between fights, with nothing attacking
//! and with the mana to spare, Bless casts each one the bot does not believe is on, ahead of a
//! rest (the party's too) unless the character is under its own rest line. One is on from a cast that did not fail for as long as it lasts at the
//! character's level (`BuffDef::rounds`), or until its wear-off line; off at a death. A cast
//! that fails is tried again after a round; [`FAILS`] in a row hold that spell for [`HOLD`]
//! (it may be on already and not renewable).

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_world::BuffDef;

use crate::executor::{Action, Expect, Outcome};
use crate::modes::{Mode, Step, View};

/// A round: one general spell goes per round (`kb/spells.md`).
const ROUND: Duration = Duration::from_secs(5);
/// Failed casts in a row before the spell is held.
const FAILS: u32 = 3;
/// How long a spell that keeps failing is left alone.
const HOLD: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
struct Kept {
    def: BuffDef,
    /// On until then, as far as the bot knows.
    on_until: Option<Instant>,
    /// Not cast again before this (a failure, or held).
    retry_at: Option<Instant>,
    fails: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Bless {
    kept: Vec<Kept>,
    /// The spell being cast (its short name), when it went, and whether the game said it
    /// failed.
    casting: Option<(String, Instant, bool)>,
    /// The character's level when it last cast: how long a spell lasts.
    level: i64,
}

impl Bless {
    /// The spells to keep on from now on; what is believed of those kept before stays.
    pub fn set(&mut self, defs: Vec<BuffDef>) {
        let old = std::mem::take(&mut self.kept);
        self.kept = defs
            .into_iter()
            .map(|def| {
                old.iter()
                    .find(|k| k.def.short == def.short)
                    .map(|k| Kept {
                        def: def.clone(),
                        ..k.clone()
                    })
                    .unwrap_or(Kept {
                        def,
                        on_until: None,
                        retry_at: None,
                        fails: 0,
                    })
            })
            .collect();
    }

    /// Whether the spell `short` is believed on at `now`.
    pub fn is_on(&self, short: &str, now: Instant) -> bool {
        self.kept
            .iter()
            .any(|k| k.def.short == short && k.on_until.is_some_and(|t| now < t))
    }

    /// The first spell to cast now, if any: off, not held, and paid for without taking the
    /// mana under the rest line.
    fn due<'a>(&'a self, v: &View) -> Option<&'a Kept> {
        let (mana, max) = v.mana?;
        let line = max * i64::from(v.rest_mana_below()) / 100;
        self.kept.iter().find(|k| {
            k.on_until.is_none_or(|t| v.now >= t)
                && k.retry_at.is_none_or(|t| v.now >= t)
                && mana >= k.def.mana
                && mana - k.def.mana >= line
        })
    }

    /// When a spell comes due or may be tried again, for the clock to wake the bot.
    pub fn wake(&self, now: Instant) -> Option<Instant> {
        self.kept
            .iter()
            .flat_map(|k| [k.on_until, k.retry_at])
            .flatten()
            .filter(|&t| t > now)
            .min()
    }

    fn turn_on(&mut self, short: &str, at: Instant) {
        let level = self.level.max(1);
        if let Some(k) = self.kept.iter_mut().find(|k| k.def.short == short) {
            let rounds = k.def.rounds(level);
            k.on_until = Some(at + ROUND * u32::try_from(rounds.max(1)).unwrap_or(1));
            k.fails = 0;
            k.retry_at = None;
        }
    }
}

impl Mode for Bless {
    fn wants(&self, v: &View) -> Option<String> {
        // Not in a fight; a rest, the party's too, gives way to it (`Recover::yields_to_bless`).
        if v.engaged || !v.presence.attackers.is_empty() {
            return None;
        }
        self.due(v).map(|k| format!("{} is off", k.def.name))
    }

    fn step(&mut self, v: &View) -> Step {
        let Some(k) = self.due(v) else {
            return Step::wait("Blessing: nothing to cast");
        };
        let (short, name) = (k.def.short.clone(), k.def.name.clone());
        self.casting = Some((short.clone(), v.now, false));
        self.level = v.level;
        // A spell on the caster or the party takes no target word.
        Step::send(
            Action::new(short, Expect::Prompt),
            format!("Blessing: casting {name}"),
        )
        .doing(format!("casting {name}"))
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            GameEvent::CastFailed | GameEvent::NoMana | GameEvent::AlreadyCast => {
                if let Some((_, _, failed)) = &mut self.casting {
                    *failed = true;
                }
            }
            GameEvent::YouKilled => {
                for k in &mut self.kept {
                    k.on_until = None;
                }
            }
            _ => {
                let Some(line) = crate::modes::line_of(ev) else {
                    return;
                };
                let line = line.trim();
                for k in &mut self.kept {
                    if k.def.wear_off.as_deref() == Some(line) {
                        k.on_until = None;
                        k.retry_at = None;
                        k.fails = 0;
                    }
                }
                // The line its target sees: on, whoever cast it.
                if let Some(short) = self
                    .kept
                    .iter()
                    .find(|k| {
                        k.def.felt.as_deref() == Some(line) && k.on_until.is_none_or(|t| now >= t)
                    })
                    .map(|k| k.def.short.clone())
                {
                    self.turn_on(&short, now);
                }
            }
        }
    }

    fn outcome(&mut self, _action: &Action, outcome: Outcome) {
        let Some((short, at, failed)) = self.casting.take() else {
            return;
        };
        if failed || outcome == Outcome::TimedOut {
            if let Some(k) = self.kept.iter_mut().find(|k| k.def.short == short) {
                k.fails += 1;
                k.on_until = None;
                k.retry_at = Some(if k.fails >= FAILS {
                    k.fails = 0;
                    at + HOLD
                } else {
                    at + ROUND
                });
            }
        } else {
            self.turn_on(&short, at);
        }
    }
}
