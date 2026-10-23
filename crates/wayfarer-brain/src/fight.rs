//! Fight (docs/brain.md "Fight here"): attack what is here, once, and let auto-combat run. It
//! never leaves the room. One attack command per target; the fight ends with `*Combat Off*` or a kill, or lapses unannounced (pitfall 42, the session's
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
/// One general spell goes per 5-s round (`kb/spells.md`); a heal in a fight is tried again
/// no sooner than this.
pub const HEAL_GAP: Duration = Duration::from_secs(5);
/// After the game refuses an attack spell (no mana, a spell already cast this round) the
/// fights are fought with weapons for this long.
pub const SPELL_HOLD: Duration = Duration::from_secs(12);

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
    /// The game refused an attack spell; the hold starts at the next step.
    spell_refused: bool,
    /// No attack spells until then.
    spell_hold: Option<Instant>,
    /// No heal in a fight before then.
    heal_ready: Option<Instant>,
    /// A heal was cast in this fight and the `*Combat Off*` it causes has not come yet.
    healed: bool,
    /// The fight with the target was started with an attack spell, which has been cast this
    /// many times at it.
    spell_fight: bool,
    casts: u32,
    /// The monster the casts are counted for: a heal in the middle of the fight does not
    /// start the count again.
    cast_target: Option<String>,
    /// The attack that changes a spell fight into a weapon fight was sent; the `*Combat Off*`
    /// the game may print for the restart has not come yet.
    switched: bool,
    /// The weapon, not the spell, for the rest of this monster's fight.
    weapon_only: bool,
}

impl Mode for Fight {
    fn wants(&self, v: &View) -> Option<String> {
        if v.engaged {
            return Some("in combat".to_owned());
        }
        if self.look {
            return Some("looking for the next".to_owned());
        }
        self.choose(v).map(|t| format!("{t} is here"))
    }

    fn step(&mut self, v: &View) -> Step {
        if v.engaged {
            if let Some(step) = self.heal(v) {
                return step;
            }
            if let Some(step) = self.enough_casts(v) {
                return step;
            }
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
        match self.choose(v) {
            Some(t) => {
                let t = t.to_owned();
                self.target = Some(t.clone());
                self.stall = Stall::default();
                if std::mem::take(&mut self.spell_refused) {
                    self.spell_hold = Some(v.now + SPELL_HOLD);
                }
                let spell = if self.weapon_only || self.spell_hold.is_some_and(|t| v.now < t) {
                    None
                } else {
                    v.attack_spell()
                };
                if self.cast_target.as_deref() != Some(t.as_str()) {
                    self.casts = 0;
                    self.weapon_only = false;
                    self.cast_target = Some(t.clone());
                }
                self.spell_fight = spell.is_some();
                match spell {
                    // Cast out of combat, the spell engages and auto-combat casts it every
                    // round while the mana lasts (`kb/spells.md`).
                    Some(s) => Step::send(
                        Action::new(format!("{} {}", s.short, attack_word(&t)), Expect::Engaged),
                        format!("Fight: {t} with {}", s.name),
                    ),
                    None => Step::send(
                        Action::new(
                            format!("{} {}", v.attack_command(), attack_word(&t)),
                            Expect::Engaged,
                        ),
                        format!("Fight: {t}"),
                    ),
                }
            }
            None => Step::wait("Fight: nothing to fight"),
        }
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            // The end of the fight our own heal caused: the monster is still there, so no
            // `look`; the next step attacks it again, before the next round's swings.
            GameEvent::CombatOff if std::mem::take(&mut self.healed) => {
                self.target = None;
                self.stall = Stall::default();
            }
            // The same for the restart when the weapon takes over from the spell.
            GameEvent::CombatOff if std::mem::take(&mut self.switched) => {
                self.target = None;
                self.stall = Stall::default();
            }
            GameEvent::CombatOff | GameEvent::ExpGain { .. } => {
                self.healed = false;
                self.switched = false;
                self.weapon_only = false;
                self.spell_fight = false;
                self.casts = 0;
                self.cast_target = None;
                self.target = None;
                self.look = true;
                self.stall = Stall::default();
            }
            GameEvent::YouDoNotSee { .. } | GameEvent::NoEffect => self.refused(),
            // A cast at the target, hit or missed: counted for the limit.
            GameEvent::SpellCast { .. } | GameEvent::CastFailed if self.spell_fight => {
                self.casts += 1;
            }
            GameEvent::YouKilled => {
                self.weapon_only = false;
                self.spell_fight = false;
                self.cast_target = None;
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
            (Expect::Engaged, Outcome::Confirmed(_)) => {
                self.healed = false;
                self.switched = false;
            }
            (Expect::Engaged, Outcome::Refused(Refused::Target)) => self.refused(),
            (Expect::Engaged, Outcome::Refused(Refused::Spell)) => self.spell_refused = true,
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

/// Whether a fight fought with a spell has had its casts: `max` casts at the monster are set
/// (0 is no limit), the fight is a spell's, `casts` have been made, and the attack that changes
/// it into a weapon fight has not been sent yet.
fn spell_fight_is_over(max: u32, spell_fight: bool, casts: u32, switched: bool) -> bool {
    max > 0 && spell_fight && casts >= max && !switched
}

impl Fight {
    /// A heal on the character in a fight, when HP is below the line and a spell and the
    /// mana for one are there. The cast ends auto-combat (`kb/spells.md`); Fight looks and
    /// engages again.
    fn heal(&mut self, v: &View) -> Option<Step> {
        if v.hp? >= v.heal_in_combat_below() || self.heal_ready.is_some_and(|t| v.now < t) {
            return None;
        }
        let spell = v.heal_spell()?;
        self.heal_ready = Some(v.now + HEAL_GAP);
        self.healed = true;
        Some(Step::send(
            Action::new(spell.short.clone(), Expect::Prompt),
            format!("Fight: HP {}%, casting {}", v.hp?, spell.name),
        ))
    }

    /// The spell has been cast as often as `spells.max_casts_per_monster` allows at this
    /// monster: the attack command is sent at it, which carries on the fight with the weapon.
    fn enough_casts(&mut self, v: &View) -> Option<Step> {
        if !spell_fight_is_over(v.max_casts(), self.spell_fight, self.casts, self.switched) {
            return None;
        }
        self.switched = true;
        self.weapon_only = true;
        self.spell_fight = false;
        let t = self.target.clone().unwrap_or_else(|| "it".to_owned());
        let attack = format!("{} {}", v.attack_command(), attack_word(&t));
        Some(Step::send(
            Action::new(attack, Expect::Engaged),
            format!("Fight: {} casts at {t}, now the weapon", self.casts),
        ))
    }

    /// The monster to go for: the party leader's, when this follower is with it and nothing
    /// attacks it (exp is shared only among those fighting the monster that dies,
    /// `kb/grouping.md` §9) and its own rules would fight it; else its own pick.
    fn choose<'a>(&self, v: &View<'a>) -> Option<&'a str> {
        if v.presence.attackers.is_empty()
            && let Some(lead) = v.party.assist()
            && let Some(name) = v
                .presence
                .here
                .iter()
                .find(|n| n.eq_ignore_ascii_case(lead))
            && target(
                &v.atlas.monsters,
                v.rules,
                std::slice::from_ref(name),
                &[],
                &self.skip,
                self.cornered,
            )
            .is_some()
        {
            return Some(name);
        }
        self.pick(v.atlas, v.rules, v.presence)
    }

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

    /// The monster it fights or last went for.
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spell_fight_is_over_after_the_most_casts_and_only_once() {
        // No limit (0): never.
        assert!(!spell_fight_is_over(0, true, 99, false));
        // Under the limit: not yet; at it: over; the attack sent: not again.
        assert!(!spell_fight_is_over(2, true, 1, false));
        assert!(spell_fight_is_over(2, true, 2, false));
        assert!(spell_fight_is_over(2, true, 5, false));
        assert!(!spell_fight_is_over(2, true, 2, true));
        // A fight fought with the weapon has no casts to count.
        assert!(!spell_fight_is_over(2, false, 9, false));
    }
}
