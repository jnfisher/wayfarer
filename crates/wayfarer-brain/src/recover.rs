//! Recover (docs/brain.md "Fight here"): rest when HP is low, out of a room that spawns
//! monsters if a quiet one is near (`kb/pitfalls.md` 20, 24), until HP is back; then walk back
//! to where the bot was started. A monster that attacks wakes it: Fight has the character until
//! that fight is over, then Recover carries on. It does not walk back into a room where a
//! monster to avoid or flee from was last seen (`rules.rs`): it waits until that is forgotten.

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::{GameEvent, Rest};
use wayfarer_world::Key;

use crate::executor::{Action, Expect, Outcome};
use crate::modes::{Mode, Step, View};
use crate::policy::{REST_TRIES, REST_WAIT, RestAt, rest_at};

/// Walks to a quiet room, and walks back, tried before giving up on each (a fight can stop a
/// walk on the way).
const WALKS: u32 = 2;
/// Heals cast on the way to a rest, at most: more than this and the mana is better spent
/// resting it back with the HP.
const HEALS: u32 = 6;
/// One general spell goes per 5-s round (`kb/spells.md`).
const HEAL_GAP: Duration = Duration::from_secs(6);

#[derive(Debug, Default)]
pub(crate) struct Recover {
    /// Where the bot was started: Fight happens there. Unknown at `start` when the position
    /// was (a brain just attached): the first room the bot then believes it is in.
    home: Option<Key>,
    /// Took control, and has not finished: rested to [`REST_UNTIL`] and back home.
    active: bool,
    walks_out: u32,
    walks_back: u32,
    /// `rest` sent since the last `(Resting)` prompt.
    rests: u32,
    /// Don't send `rest` before this (too sick, or it would not take).
    wait_until: Option<Instant>,
    /// Resting in a room that spawns after the walks out were stopped has been warned of.
    warned: bool,
    /// Heals cast since Recover took control, and when the next may go.
    heals: u32,
    heal_ready: Option<Instant>,
}

impl Recover {
    pub fn new(home: Option<Key>) -> Recover {
        Recover {
            home,
            ..Recover::default()
        }
    }

    /// Home, if it was not known at `start` and is now: the believed room, unless a walk
    /// out to rest may have left it. Without a home Recover would never walk back.
    pub fn find_home(&mut self, v: &View) -> Option<Key> {
        if self.home.is_some() || v.room.is_none() || (self.active && self.walks_out > 0) {
            return None;
        }
        self.home = v.room;
        self.home
    }

    fn away(&self, v: &View) -> bool {
        self.home.is_some() && v.room != self.home
    }

    fn low(v: &View) -> bool {
        v.hp.is_some_and(|hp| hp < v.rest_below()) || v.poisoned || v.mana_low()
    }

    fn rested(v: &View) -> bool {
        !v.hp.is_some_and(|hp| hp < v.rest_until()) && !v.poisoned && v.mana_restored()
    }

    /// A heal on the character, between fights: while HP is below the rest line, a heal
    /// spell and its mana are there, and not too many have gone. Resting waits for it.
    fn heal(&mut self, v: &View) -> Option<Step> {
        if v.resting || v.poisoned || self.heals >= HEALS || v.hp? >= v.rest_below() {
            return None;
        }
        let spell = v.heal_spell()?;
        if let Some(t) = self.heal_ready.filter(|&t| v.now < t) {
            let secs = (t - v.now).as_secs() + 1;
            return Some(Step::wait(format!(
                "Recover: HP {}%, the next heal in {secs} s",
                v.hp?
            )));
        }
        self.heals += 1;
        self.heal_ready = Some(v.now + HEAL_GAP);
        Some(Step::send(
            Action::new(spell.short.clone(), Expect::Prompt),
            format!("Recover: HP {}%, casting {}", v.hp?, spell.name),
        ))
    }

    fn done(&mut self) {
        let home = self.home;
        *self = Recover::new(home);
    }
}

impl Mode for Recover {
    fn wants(&self, v: &View) -> Option<String> {
        // A fight comes first: Fight takes the character while engaged or attacked.
        if v.engaged || !v.presence.attackers.is_empty() {
            return None;
        }
        if Self::low(v) {
            return Some(match v.hp {
                Some(hp) if hp < v.rest_below() => format!("HP {hp}%"),
                _ if v.poisoned => "poisoned".to_owned(),
                _ => format!("mana {}%", v.mana_percent().unwrap_or(0)),
            });
        }
        let back = self.away(v) && self.walks_back < WALKS && self.shunned(v).is_none();
        if self.active && (!Self::rested(v) || back) {
            return Some("not finished".to_owned());
        }
        None
    }

    fn step(&mut self, v: &View) -> Step {
        if self.walks_back > 0 && !Self::rested(v) {
            // Low again after giving up the walk home: a new rest, from here.
            self.done();
        }
        self.active = true;
        let here = v.room_name(v.room);
        if Self::rested(v) {
            if let Some((home, name)) = self.shunned(v) {
                // Still active: it walks back once the sighting is forgotten.
                return Step::wait(format!(
                    "Recover: rested; not walking back to {}: {name} was last seen there",
                    v.room_name(Some(home))
                ));
            }
            if self.away(v) && self.walks_back < WALKS {
                self.walks_back += 1;
                let home = self.home.expect("away from home");
                return Step::walk(
                    home,
                    format!(
                        "Recover: rested; walking back to {}",
                        v.room_name(Some(home))
                    ),
                );
            }
            self.done();
            return Step::wait(format!("Recover: rested in {here}"));
        }
        if v.resting {
            self.rests = 0;
            return Step::wait(format!("Recover: resting to {}% in {here}", v.rest_until()));
        }
        if let Some(step) = self.heal(v) {
            return step;
        }
        // Every walk out was stopped on the way (a fight) and a quiet room is still near:
        // resting here goes against the quiet-room rule (`kb/pitfalls.md` 20, 24). Said once.
        let against_the_rule = self.walks_out >= WALKS
            && !self.warned
            && matches!(
                rest_at(v.atlas, v.room, v.certain, v.traveller),
                RestAt::Walk(_)
            );
        if self.walks_out < WALKS {
            match rest_at(v.atlas, v.room, v.certain, v.traveller) {
                RestAt::Walk(quiet) => {
                    self.walks_out += 1;
                    return Step::walk(
                        quiet,
                        format!(
                            "Recover: walking to {} to rest ({here} spawns)",
                            v.room_name(Some(quiet))
                        ),
                    );
                }
                RestAt::HereAnyway => {
                    // Asked once: a later step rests here without asking again.
                    self.walks_out = WALKS;
                }
                RestAt::Here => {}
            }
        }
        if self.wait_until.is_some_and(|t| v.now < t) {
            return Step::wait(format!("Recover: can't rest in {here}; waiting"));
        }
        if self.rests >= REST_TRIES {
            self.rests = 0;
            self.wait_until = Some(v.now + REST_WAIT);
            return Step::wait(format!("Recover: can't rest in {here}; waiting"));
        }
        self.rests += 1;
        let rest = Action::new("rest", Expect::Prompt);
        if against_the_rule {
            self.warned = true;
            return Step::send(
                rest,
                format!(
                    "Recover: resting in {here} though it spawns: {WALKS} walks to a quiet \
                     room were stopped"
                ),
            )
            .warning();
        }
        Step::send(
            rest,
            format!("Recover: resting to {}% in {here}", v.rest_until()),
        )
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            // Out of mana: no more heals this time; the rest brings it back.
            GameEvent::NoMana => self.heals = HEALS,
            GameEvent::TooSick => {
                self.rests = 0;
                self.wait_until = Some(now + REST_WAIT);
            }
            GameEvent::Prompt {
                resting: Some(Rest::Resting | Rest::Meditating),
                ..
            }
            | GameEvent::Resting { .. } => self.rests = 0,
            _ => {}
        }
    }

    fn outcome(&mut self, _action: &Action, _outcome: Outcome) {}
}

impl Recover {
    /// Flee is over: rest to [`REST_UNTIL`] where it got to and walk back home, whatever the
    /// HP (a flee line above [`REST_BELOW`] leaves HP above it).
    pub fn after_flee(&mut self) {
        self.active = true;
    }

    /// Still recovering (it has not finished since it took control).
    pub fn busy(&self) -> bool {
        self.active
    }

    /// Rested, but away from home after [`WALKS`] walks back that did not get there: it
    /// waits where it is.
    pub fn stranded(&self, v: &View) -> bool {
        self.active && Self::rested(v) && self.away(v) && self.walks_back >= WALKS
    }

    /// Rested and away from home, and home is where a monster to avoid was last seen: the
    /// home and the monster.
    pub fn shunned<'a>(&self, v: &'a View) -> Option<(Key, &'a str)> {
        let home = self.home.filter(|_| self.active && self.away(v))?;
        v.shunned(home).map(|name| (home, name))
    }

    /// When a wait ends, for the clock to wake the bot.
    pub fn wake(&self, now: Instant) -> Option<Instant> {
        self.wait_until
            .into_iter()
            .chain(self.heal_ready.filter(|&t| t > now))
            .min()
    }

    /// Where the bot plays from, once known.
    pub fn home(&self) -> Option<Key> {
        self.home
    }

    /// Play from `home` from now on (Task mode: the stop it is at or walking to).
    pub fn set_home(&mut self, home: Key) {
        self.home = Some(home);
    }
}
