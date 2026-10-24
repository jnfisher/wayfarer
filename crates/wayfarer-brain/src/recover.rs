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
use crate::modes::{Healed, Mode, Step, View};
use crate::policy::{REST_TRIES, REST_WAIT, RestAt, rest_at};

/// Walks to a quiet room, and walks back, tried before giving up on each (a fight can stop a
/// walk on the way).
const WALKS: u32 = 2;
/// How long Recover leaves healing alone after the game says the mana is short.
const NO_MANA_PAUSE: Duration = Duration::from_secs(30);
/// One general spell goes per 5-s round (`kb/spells.md`).
const HEAL_GAP: Duration = Duration::from_secs(5);

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
    /// When the next heal may go (a round after the last, or after the mana ran short).
    heal_ready: Option<Instant>,
    /// The walk its last step asked for: out to rest (`true`) or back home.
    last_walk: Option<bool>,
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
        let below = v.own_heal_below();
        if v.poisoned || below == 0 || v.hp? >= below {
            return None;
        }
        let spell = v.heal_spell()?;
        if !v.heal_pays(spell, Healed::Me) {
            return None;
        }
        // Between casts it rests (a cast ends a rest; the next prompt without `(Resting)` has
        // it rest again).
        if self.heal_ready.is_some_and(|t| v.now < t) {
            return None;
        }
        self.heal_ready = Some(v.now + HEAL_GAP);
        Some(
            Step::send(
                Action::new(spell.short.clone(), Expect::Prompt),
                format!("Recover: HP {}%, casting {}", v.hp?, spell.name),
            )
            .doing("healing itself"),
        )
    }

    fn done(&mut self) {
        let home = self.home;
        *self = Recover::new(home);
    }
}

impl Recover {
    /// A party member in the room is hurt and a heal is ready (C-043), between fights: with
    /// something to fight here the heals are Fight's, and taking control would only cast
    /// again in the round Fight just cast in.
    fn heal_due(&self, v: &View) -> bool {
        self.heal_ready.is_none_or(|t| v.now >= t)
            && crate::policy::quiet(&v.atlas.monsters, v.rules, &v.presence.here)
            && v.rest_party_heal().is_some()
    }

    /// Whether a blessing may go ahead of a rest: not before a heal for the party, nor while
    /// the character itself is under its rest line (it rests first). A cast ends the rest;
    /// Recover rests again after it.
    pub fn yields_to_bless(&self, v: &View) -> bool {
        !self.heal_due(v) && !Self::low(v)
    }
}

impl Mode for Recover {
    fn wants(&self, v: &View) -> Option<String> {
        // A fight comes first: Fight takes the character while engaged or attacked.
        if v.engaged || !v.presence.attackers.is_empty() {
            return None;
        }
        if self.heal_due(v) {
            return Some("healing the party".to_owned());
        }
        // Every member of a resting party rests with it (C-042, C-045).
        if let Some(why) = v.party.party_rest()
            && !Self::low(v)
        {
            return Some(why.to_owned());
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
        // Between rests, a heal for a hurt member of the party: it shortens a party rest.
        if self.heal_due(v)
            && let Some(cast) = v.rest_party_heal()
        {
            self.heal_ready = Some(v.now + HEAL_GAP);
            let mut step = crate::fight::cast_step(&cast, "Recover");
            if v.party.party_rest().is_some() {
                step.narration.push_str(" (party rest)");
            }
            return step;
        }
        if self.walks_back > 0 && !Self::rested(v) {
            // Low again after giving up the walk home: a new rest, from here.
            self.done();
        }
        self.active = true;
        let here = v.room_name(v.room);
        if Self::rested(v) {
            // Rested itself, the leader holds the party where the quiet-room rule allows until
            // everyone is (the coordinator lifts it); then it walks back home as after any rest.
            if let Some(why) = v.party.party_rest() {
                if self.walks_out < WALKS
                    && v.party.following().is_none()
                    && let RestAt::Walk(quiet) = rest_at(v.atlas, v.room, v.certain, v.traveller)
                {
                    self.walks_out += 1;
                    self.last_walk = Some(true);
                    return Step::walk(
                        quiet,
                        format!(
                            "Recover: walking the party to {} to rest ({here} spawns)",
                            v.room_name(Some(quiet))
                        ),
                    );
                }
                return Step::wait(format!("Recover: {why}")).doing("waiting for the party");
            }
            if let Some((home, name)) = self.shunned(v) {
                // Still active: it walks back once the sighting is forgotten.
                return Step::wait(format!(
                    "Recover: rested; not walking back to {}: {name} was last seen there",
                    v.room_name(Some(home))
                ));
            }
            // A follower rests where the leader is and goes where it goes.
            if self.away(v) && self.walks_back < WALKS && v.party.following().is_none() {
                self.walks_back += 1;
                self.last_walk = Some(false);
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
        if let Some(step) = self.heal(v) {
            return step;
        }
        if v.resting {
            self.rests = 0;
            return Step::wait(format!("Recover: resting to {}% in {here}", v.rest_until()));
        }
        // Every walk out was stopped on the way (a fight) and a quiet room is still near:
        // resting here goes against the quiet-room rule (`kb/pitfalls.md` 20, 24). Said once.
        let against_the_rule = self.walks_out >= WALKS
            && !self.warned
            && matches!(
                rest_at(v.atlas, v.room, v.certain, v.traveller),
                RestAt::Walk(_)
            );
        if self.walks_out < WALKS && v.party.following().is_none() {
            match rest_at(v.atlas, v.room, v.certain, v.traveller) {
                RestAt::Walk(quiet) => {
                    self.walks_out += 1;
                    self.last_walk = Some(true);
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
        let why = if v.party.party_rest().is_some() && !Self::low(v) {
            "Recover: resting with the party".to_owned()
        } else {
            format!("Recover: resting to {}% in {here}", v.rest_until())
        };
        Step::send(rest, why)
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            // Out of mana: no more heals for a while; the rest brings it back.
            GameEvent::NoMana => self.heal_ready = Some(now + NO_MANA_PAUSE),
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

    /// The walk its last step asked for did not go (the bot sneaks first, C-048): it is not
    /// counted against [`WALKS`].
    pub fn walk_deferred(&mut self) {
        match self.last_walk.take() {
            Some(true) => self.walks_out = self.walks_out.saturating_sub(1),
            Some(false) => self.walks_back = self.walks_back.saturating_sub(1),
            None => {}
        }
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
