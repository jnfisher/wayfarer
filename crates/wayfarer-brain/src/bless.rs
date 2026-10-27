//! Blessings (C-047, docs/brain.md "Blessings"): the lasting spells the player ticked for the
//! character (bless, barkskin, holy aura) are kept on. With the mana to spare
//! (`spells.bless_mana_above`), Bless casts each one the bot does not believe is on: between
//! fights ahead of a rest (the party's too) unless the character is under its own rest line,
//! and in a fight after a heal (`spells.bless_in_combat`, C-056). One is on from a cast that did not fail for as long as it lasts at the
//! character's level (`BuffDef::rounds`), or until its wear-off line; off at a death. A cast
//! that fails is tried again after a round; [`FAILS`] in a row hold that spell for [`HOLD`]
//! (it may be on already and not renewable). A cast is done at the first prompt after it, which
//! another player's line may bring before the cast's own: its failure line coming within the
//! round after still counts. Between fights, while one is believed on, the status screen is
//! looked at every `spells.bless_check_seconds`: it lists the line of every spell on the
//! character after its last row, and one believed on that it does not show is off.

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
/// The status screen's spell list for a sysop with debug on, in place of the spells' lines.
const SPELLS_LISTED: &str = "You have the following spell cast upon you:";

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
    /// The last cast taken as on (its short name, when it went), for a failure line that comes
    /// after the prompt that ended it.
    settled: Option<(String, Instant)>,
    /// The character's level when it last cast: how long a spell lasts.
    level: i64,
    /// How often the status screen is looked at (`spells.bless_check_seconds`); zero: never.
    check_every: Duration,
    /// When a status screen last came, or a cast went (as good as a look).
    checked_at: Option<Instant>,
    /// The lines of the status screen coming in, up to its prompt.
    screen: Option<Vec<String>>,
    /// Spells the last screen did not show, for the cast's narration.
    unseen: Vec<String>,
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
    /// mana under the blessing's line (`spells.bless_mana_above`).
    fn due<'a>(&'a self, v: &View) -> Option<&'a Kept> {
        let (mana, max) = v.mana?;
        let line = max * i64::from(v.bless_mana_above()) / 100;
        self.kept.iter().find(|k| {
            k.on_until.is_none_or(|t| v.now >= t)
                && k.retry_at.is_none_or(|t| v.now >= t)
                && mana >= k.def.mana
                && mana - k.def.mana >= line
        })
    }

    /// Look at the status screen this often from now on; zero: never.
    pub fn set_check(&mut self, every: Duration) {
        self.check_every = every;
    }

    /// When the status screen is next looked at: one is believed on, and none came for
    /// `check_every`.
    fn check_at(&self, now: Instant) -> Option<Instant> {
        if self.check_every.is_zero()
            || !self
                .kept
                .iter()
                .any(|k| k.on_until.is_some_and(|t| now < t))
        {
            return None;
        }
        Some(self.checked_at.map_or(now, |t| t + self.check_every))
    }

    /// When a spell comes due, may be tried again or is looked for on the status screen, for
    /// the clock to wake the bot.
    pub fn wake(&self, now: Instant) -> Option<Instant> {
        self.kept
            .iter()
            .flat_map(|k| [k.on_until, k.retry_at])
            .chain([self.check_at(now)])
            .flatten()
            .filter(|&t| t > now)
            .min()
    }

    /// The status screen came to its prompt with `seen` after its rows: a spell believed on
    /// that has a line of its own and is not among them is off. A sysop with debug on gets a
    /// list of the spells instead of their lines (`kb/screen-patterns.md` "Status screens"),
    /// which says nothing here.
    fn screen_seen(&mut self, seen: &[String], now: Instant) {
        if self.casting.is_some() || seen.iter().any(|l| l == SPELLS_LISTED) {
            return;
        }
        for k in &mut self.kept {
            let Some(felt) = k.def.felt.as_deref() else {
                continue;
            };
            if k.on_until.is_some_and(|t| now < t) && !seen.iter().any(|l| l == felt) {
                k.on_until = None;
                self.unseen.push(k.def.short.clone());
            }
        }
    }

    fn turn_on(&mut self, short: &str, at: Instant) {
        let level = self.level.max(1);
        if let Some(k) = self.kept.iter_mut().find(|k| k.def.short == short) {
            let rounds = k.def.rounds(level);
            k.on_until = Some(at + ROUND * u32::try_from(rounds.max(1)).unwrap_or(1));
            k.fails = 0;
            k.retry_at = None;
        }
        self.checked_at = Some(at);
    }

    /// The cast of `short` that went at `at` failed: tried again after a round, or held.
    fn failed(&mut self, short: &str, at: Instant) {
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
    }
}

impl Mode for Bless {
    fn wants(&self, v: &View) -> Option<String> {
        // In a fight only with `spells.bless_in_combat` (C-056, after a heal: `Fight::may_bless`);
        // a rest, the party's too, gives way to it (`Recover::yields_to_bless`).
        let fighting = v.engaged || !v.presence.attackers.is_empty();
        if fighting && !v.settings.bool("spells.bless_in_combat") {
            return None;
        }
        if let Some(k) = self.due(v) {
            return Some(format!("{} is off", k.def.name));
        }
        // The status screen only between fights.
        (!fighting && self.check_at(v.now).is_some_and(|t| v.now >= t))
            .then(|| "checking the blessings are on".to_owned())
    }

    fn step(&mut self, v: &View) -> Step {
        let Some(k) = self.due(v) else {
            if self.check_at(v.now).is_some_and(|t| v.now >= t) {
                self.checked_at = Some(v.now);
                return Step::send(
                    Action::new("st", Expect::Prompt),
                    "Blessing: looking at the status screen",
                )
                .doing("checking the blessings are on");
            }
            return Step::wait("Blessing: nothing to cast");
        };
        let (short, name) = (k.def.short.clone(), k.def.name.clone());
        let narration = if self.unseen.contains(&short) {
            format!("Blessing: {name} is not on the status screen; casting it")
        } else {
            format!("Blessing: casting {name}")
        };
        self.unseen.clear();
        self.casting = Some((short.clone(), v.now, false));
        self.settled = None;
        self.level = v.level;
        // A spell on the caster or the party takes no target word.
        Step::send(Action::new(short, Expect::Prompt), narration).doing(format!("casting {name}"))
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            GameEvent::CastFailed { spell } if self.casting.is_none() => {
                // The cast taken as on, failed after all: by name, so another spell's failure
                // (Fight's) is not taken for it.
                if let Some((short, at)) = self.settled.take() {
                    let named = self
                        .kept
                        .iter()
                        .any(|k| k.def.short == short && k.def.name.eq_ignore_ascii_case(spell));
                    if named && now < at + ROUND {
                        self.failed(&short, at);
                    } else {
                        self.settled = Some((short, at));
                    }
                }
            }
            GameEvent::CastFailed { .. } | GameEvent::NoMana | GameEvent::AlreadyCast => {
                if let Some((_, _, failed)) = &mut self.casting {
                    *failed = true;
                }
            }
            GameEvent::YouKilled => {
                for k in &mut self.kept {
                    k.on_until = None;
                }
            }
            // The status screen's first row: its spell lines follow its last, up to the prompt.
            GameEvent::StatusName { .. } => {
                self.screen = Some(Vec::new());
                self.checked_at = Some(now);
            }
            GameEvent::Prompt { .. } => {
                if let Some(seen) = self.screen.take() {
                    self.screen_seen(&seen, now);
                }
            }
            _ => {
                let Some(line) = crate::modes::line_of(ev) else {
                    return;
                };
                let line = line.trim();
                if let Some(screen) = &mut self.screen {
                    screen.push(line.to_owned());
                }
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
            self.failed(&short, at);
        } else {
            self.turn_on(&short, at);
            self.settled = Some((short, at));
        }
    }
}
