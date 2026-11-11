//! Fight (docs/brain.md "Fight here"): attack what is here, once, and let auto-combat run. It
//! never leaves the room. One attack command per target; the fight ends with `*Combat Off*` or a kill, or lapses unannounced (pitfall 42, the session's
//! quiet rule); then it looks again and picks the next. The monster rules keep it off some
//! monsters ([`target`]).
//!
//! A fight can be a stalemate: the target's armour takes every blow (`glances off its
//! armour`) and it never dies. After [`STALEMATE_ROUNDS`] rounds of the character's swings with
//! no damage dealt, at least one of them a glance, Fight gives the target up: the bot leaves
//! as Flee does and the monster becomes *ignored* for the session.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;

use crate::brain::Atlas;
use crate::executor::{Action, Expect, Outcome, Refused};
use crate::modes::{Mode, Presence, Step, View};
use crate::policy::{attack_word, hostile, is_first, target};
use crate::spellcraft::PartyCast;
use wayfarer_world::MonsterRules;

/// How many attacks on one monster may go unanswered (no `*Combat Engaged*`, no refusal)
/// before it is left alone for the session: a missed line is tried again, a refusal the
/// executor does not know is not tried for ever.
pub const ATTACK_TRIES: u32 = 2;

/// While blind and out of combat, how often it looks to see whether it can see again.
pub const BLIND_LOOK: std::time::Duration = std::time::Duration::from_secs(10);
/// Rounds of the character's own swings in a row that did no damage, one of them a glance at
/// least, with nobody else damaging the target meanwhile, before the fight is a stalemate:
/// 50 seconds (C-091; four rounds gave up high-AC monsters on a run of bad luck).
pub const STALEMATE_ROUNDS: u32 = 10;
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
    /// attacks went unanswered [`ATTACK_TRIES`] times: left alone while the character stays in
    /// `skip_room`. Never for the session: a name the bot grinds comes back in every room
    /// (Penny, 2026-11-11, blind, wrote off every "nomadic shaman" for the session).
    skip: BTreeSet<String>,
    /// The room the skips were made in; leaving it clears them.
    skip_room: Option<wayfarer_world::Key>,
    /// The character is blind ("You are blind.", a black curse): it cannot see a target, so
    /// the game answers an attack with "You do not see …" or takes it for speech. Nothing is
    /// skipped for that; a room shown again means it sees (C-094).
    blind: bool,
    /// While blind and out of combat, when it looks again to see whether it can.
    blind_look: Option<Instant>,
    /// The attacks on each monster that timed out.
    timeouts: BTreeMap<String, u32>,
    stall: Stall,
    /// A stalemate with this monster, for the bot to act on.
    stalemate: Option<String>,
    /// The monster to attack first it switched to mid-fight (C-075): once per arrival, so a
    /// round that shows it again sends nothing more.
    switched_to: Option<String>,
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
    /// A sneaker with backstabs on: the next fight opens with `bs` (C-048), when sneaking.
    backstab: bool,
    /// Believed sneaking: the backstab goes, and before an attack spell. Not sneaking (the
    /// sneak failed and the bot knows it), the fight opens with the attack command (C-083).
    sneaking: bool,
    /// The fight opened with `bs`; its strike has not come yet.
    opened_bs: bool,
    /// The backstab struck (hit or missed): auto-combat goes on with the attack command, sent
    /// at the next step (C-083).
    bs_struck: bool,
    /// On a passive Go to: nothing is fought, not even what attacks the character.
    passive: bool,
}

/// A name as the game shows it (`lauren` is Lauren).
fn capitalized(name: &str) -> String {
    let mut c = name.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// The command and narration for a party heal (`mihe lauren`, `rain`).
pub(crate) fn cast_step(cast: &PartyCast, mode: &str) -> Step {
    match cast {
        PartyCast::On(s, who) => Step::send(
            Action::new(format!("{} {who}", s.short), Expect::Prompt),
            format!("{mode}: casting {} on {who}", s.name),
        )
        .doing(format!("healing {}", capitalized(who))),
        PartyCast::Party(s) => Step::send(
            Action::new(s.short.clone(), Expect::Prompt),
            format!("{mode}: casting {} on the party", s.name),
        )
        .doing(format!("casting {}", s.name)),
    }
}

impl Mode for Fight {
    fn wants(&self, v: &View) -> Option<String> {
        if self.passive {
            return None;
        }
        if v.engaged {
            return Some("in combat".to_owned());
        }
        if self.blind {
            // Nothing to aim at: it waits to see again, looking now and then.
            return (!v.presence.here.is_empty() || self.blind_look.is_none_or(|t| v.now >= t))
                .then(|| "blind".to_owned());
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
            if let Some(step) = self.heal_mates(v) {
                return step;
            }
            if let Some(step) = self.after_backstab(v) {
                return step;
            }
            if let Some(step) = self.weapon_now(v) {
                return step;
            }
            if let Some(step) = self.switch_to_first(v) {
                return step;
            }
            if self.blind {
                return Step::wait("Fight: blind, swinging at what attacks");
            }
            let t = self.target.as_deref().unwrap_or("in combat");
            return Step::wait(format!("Fight: {t}"));
        }
        if self.blind {
            if self.blind_look.is_none_or(|t| v.now >= t) {
                self.blind_look = Some(v.now + BLIND_LOOK);
                return Step::send(
                    Action::new("look", Expect::RoomSeen),
                    "Fight: blind, looking whether it can see",
                );
            }
            return Step::wait("Fight: blind, waiting to see again");
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
                let spell = if (self.backstab && self.sneaking)
                    || self.weapon_only
                    || self.spell_hold.is_some_and(|t| v.now < t)
                {
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
                    // Any sneaker may try it: only a backstab weapon or bare hands backstab,
                    // any other weapon strikes a normal blow (`kb/classes/ninja.md`).
                    // Only while believed sneaking: a sneak that failed opens with the attack
                    // command (C-083).
                    None if self.backstab && self.sneaking => {
                        self.opened_bs = true;
                        self.bs_struck = false;
                        Step::send(
                            Action::new(format!("bs {}", attack_word(&t)), Expect::Engaged),
                            format!("Fight: {t} (backstab)"),
                        )
                    }
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
                self.opened_bs = false;
                self.bs_struck = false;
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
            // Blind: the refusals that came before it knew (Lexi, 2026-11-11: "You do not see
            // shaman here!", then "You are blind.") were the blindness too.
            GameEvent::Blind => {
                if !self.blind {
                    tracing::info!(unskipped = ?self.skip, "blind: nothing is skipped until it sees again");
                }
                self.blind = true;
                self.skip.clear();
                self.timeouts.clear();
            }
            // A room shown: it sees again.
            GameEvent::RoomTitle { .. } | GameEvent::AlsoHere { .. } => {
                self.blind = false;
                self.blind_look = None;
            }
            // A cast at the target, hit or missed: counted for the limit.
            GameEvent::SpellCast { .. } | GameEvent::CastFailed { .. } if self.spell_fight => {
                self.casts += 1;
            }
            GameEvent::YouKilled => {
                self.opened_bs = false;
                self.bs_struck = false;
                self.weapon_only = false;
                self.spell_fight = false;
                self.cast_target = None;
                self.target = None;
                self.look = false;
                self.stall = Stall::default();
            }
            GameEvent::YouHit { target, .. } => {
                self.bs_struck |= self.opened_bs;
                self.swing(target, true, false, now);
            }
            // 1.20: an opening or a weave that did damage is damage dealt in the round.
            GameEvent::Opening {
                who: None,
                target: Some(target),
                damage,
                ..
            }
            | GameEvent::Weave {
                who: None,
                target: Some(target),
                damage,
                resisted: false,
                ..
            } if *damage > 0 => self.swing(target, true, false, now),
            GameEvent::YouMiss { target, .. } => {
                self.bs_struck |= self.opened_bs;
                self.swing(target, false, false, now);
            }
            GameEvent::SwingGlanced { target } => self.swing(target, false, true, now),
            // Someone else damaged the target (a party member's blow or spell): the monster can
            // be hurt, so no stalemate yet (C-091).
            // An attack the game took for speech (`You say "bash shaman"`): a blind character's
            // command that names what it cannot see.
            GameEvent::Other(e)
                if e.name == "you_say"
                    && self.target.as_deref().is_some_and(|t| {
                        let msg = e.text("msg").unwrap_or_default().to_lowercase();
                        t.split_whitespace()
                            .last()
                            .is_some_and(|w| msg.ends_with(&w.to_lowercase()))
                    }) =>
            {
                self.blind = true;
                self.target = None;
                self.look = false;
            }
            GameEvent::Other(e) => {
                let hurt = match e.name.as_str() {
                    "other_hits" => e.text("target"),
                    "spell_cast_room" if e.text("dmg").is_some_and(|d| d != "0") => {
                        e.text("target")
                    }
                    _ => None,
                };
                if let (Some(hurt), Some(t)) = (hurt, &self.target) {
                    let (a, b) = (hurt.to_lowercase(), t.to_lowercase());
                    if a == b || a.contains(&b) || b.contains(&a) {
                        self.stall = Stall::default();
                    }
                }
            }
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
            // Blind: the attack went nowhere because it cannot see; that says nothing about
            // the monster.
            (Expect::Engaged, Outcome::TimedOut) if self.blind => {
                self.target = None;
                self.stall = Stall::default();
            }
            (Expect::Engaged, Outcome::TimedOut) => {
                if let Some(t) = self.target.take() {
                    let n = self.timeouts.entry(t.clone()).or_default();
                    *n += 1;
                    if *n >= ATTACK_TRIES {
                        tracing::info!(target = %t, "unanswered: skipped in this room");
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
        Some(
            Step::send(
                Action::new(spell.short.clone(), Expect::Prompt),
                format!("Fight: HP {}%, casting {}", v.hp?, spell.name),
            )
            .doing("healing itself"),
        )
    }

    /// Whether a blessing may be cast in this fight now (C-056): no heal is due, its own or a
    /// party member's, and no spell of ours went this round (one general spell a round).
    pub fn may_bless(&self, v: &View) -> bool {
        if self.heal_ready.is_some_and(|t| v.now < t) {
            return false;
        }
        let own =
            v.hp.is_some_and(|hp| hp < v.heal_in_combat_below() && v.heal_spell().is_some());
        let mates = v
            .party_heal(v.member_heal_in_combat(), v.heal_in_combat_below())
            .is_some();
        !own && !mates
    }

    /// A blessing is cast in the fight: the `*Combat Off*` it causes is ours, as after a heal
    /// (the same monster is attacked again at once), and no heal goes this round.
    pub fn cast_mid_fight(&mut self, now: Instant) {
        self.healed = true;
        self.heal_ready = Some(now + HEAL_GAP);
    }

    /// A monster to attack first (a boss, by default) is here while the character fights
    /// another: attack it at once, once for each time it comes (C-075). Thrag's wolves came
    /// first on the Thrag run, attacking, and the party fought them with Thrag in the room.
    fn switch_to_first(&mut self, v: &View) -> Option<Step> {
        if self
            .target
            .as_deref()
            .is_some_and(|t| is_first(&v.atlas.monsters, v.rules, t))
        {
            return None;
        }
        let first = v
            .presence
            .here
            .iter()
            .chain(&v.presence.attackers)
            .map(String::as_str)
            .filter(|n| !self.skip.contains(*n) && hostile(&v.atlas.monsters, n))
            .find(|n| is_first(&v.atlas.monsters, v.rules, n));
        let Some(first) = first else {
            self.switched_to = None;
            return None;
        };
        if self.switched_to.as_deref() == Some(first) {
            return None;
        }
        let first = first.to_owned();
        let from = self.target.replace(first.clone());
        self.switched_to = Some(first.clone());
        self.stall = Stall::default();
        self.spell_fight = false;
        let attack = format!("{} {}", v.attack_command(), attack_word(&first));
        let why = match from {
            Some(from) => format!("Fight: {first} is here: switching from {from}"),
            None => format!("Fight: {first} is here: switching to it"),
        };
        Some(Step::send(Action::new(attack, Expect::Engaged), why))
    }

    /// A heal for another member of the party in the room, or the party heal (C-043); the fight
    /// goes on at once after the cast, as after its own heal.
    fn heal_mates(&mut self, v: &View) -> Option<Step> {
        if self.heal_ready.is_some_and(|t| v.now < t) {
            return None;
        }
        let cast = v.party_heal(v.member_heal_in_combat(), v.heal_in_combat_below())?;
        self.heal_ready = Some(v.now + HEAL_GAP);
        self.healed = true;
        Some(cast_step(&cast, "Fight"))
    }

    /// The spell fight is over: the spell has been cast as often as
    /// `spells.max_casts_per_monster` allows at this monster, or the mana has come down to
    /// `spells.attack_mana_above` (the game casts the spell every round on its own, so the line
    /// checked when the fight opened is not enough). The attack command is sent at it, which
    /// carries on the fight with the weapon.
    /// The backstab's round is over (C-083): auto-combat repeats a backstab only while hidden,
    /// so the fight goes on with the attack command (`bash`, `smash`, `a` …), sent once.
    fn after_backstab(&mut self, v: &View) -> Option<Step> {
        if !(self.opened_bs && self.bs_struck) {
            return None;
        }
        self.opened_bs = false;
        self.bs_struck = false;
        let t = self.target.clone()?;
        let attack = format!("{} {}", v.attack_command(), attack_word(&t));
        Some(Step::send(
            Action::new(attack, Expect::Engaged),
            format!("Fight: backstab done, now {}", v.attack_command()),
        ))
    }

    fn weapon_now(&mut self, v: &View) -> Option<Step> {
        let t = self.target.clone().unwrap_or_else(|| "it".to_owned());
        let why = if spell_fight_is_over(v.max_casts(), self.spell_fight, self.casts, self.switched)
        {
            format!("{} casts at {t}", self.casts)
        } else if self.spell_fight && !self.switched && v.attack_mana_short() {
            format!("mana {}%", v.mana_percent().unwrap_or(0))
        } else {
            return None;
        };
        self.switched = true;
        self.weapon_only = true;
        self.spell_fight = false;
        let attack = format!("{} {}", v.attack_command(), attack_word(&t));
        Some(Step::send(
            Action::new(attack, Expect::Engaged),
            format!("Fight: {why}, now the weapon"),
        ))
    }

    /// The monster to go for: one to attack first, if one is here; else the party leader's, when
    /// this follower is with it and nothing attacks it (exp is shared only among those fighting the monster that dies,
    /// `kb/grouping.md` §9) and its own rules would fight it; else its own pick.
    fn choose<'a>(&self, v: &View<'a>) -> Option<&'a str> {
        // A monster to attack first (a boss) comes before the leader's target too (C-075).
        if let Some(own) = self.pick(v.atlas, v.rules, v.presence)
            && is_first(&v.atlas.monsters, v.rules, own)
        {
            return Some(own);
        }
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
        )
    }

    /// Whether it would take the character with `presence` in the room: a monster it would
    /// attack, or anything attacking.
    pub fn would_fight(&self, atlas: &Atlas, rules: &MonsterRules, presence: &Presence) -> bool {
        !presence.attackers.is_empty() || self.pick(atlas, rules, presence).is_some()
    }

    /// The bot's tick: in another room than the skips were made in, they go (a monster
    /// refused here is fought in the next room).
    pub fn in_room(&mut self, room: Option<wayfarer_world::Key>) {
        if room != self.skip_room {
            if !self.skip.is_empty() {
                tracing::info!(skipped = ?self.skip, "left the room: the skips go");
            }
            self.skip.clear();
            self.timeouts.clear();
            self.skip_room = room;
        }
    }

    /// On a passive Go to (`true`): fight nothing, not even what attacks the character.
    pub fn set_passive(&mut self, passive: bool) {
        self.passive = passive;
    }

    /// The monster it fights or last went for.
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// Open the next fight with `bs` (a sneaker, C-048); `sneaking`, it goes before a spell.
    pub fn set_backstab(&mut self, backstab: bool, sneaking: bool) {
        self.backstab = backstab;
        self.sneaking = sneaking;
    }

    /// The game refused the target: leave it alone in this room, and look again. Blind, the
    /// refusal is the blindness, not the monster: nothing is skipped.
    fn refused(&mut self) {
        if let Some(t) = self.target.take()
            && !self.blind
        {
            tracing::info!(target = %t, "refused: skipped in this room");
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
    use wayfarer_world::{SpellDef, SpellKind};

    #[test]
    fn a_heal_says_whom_it_heals() {
        let mihe = SpellDef::flat("mihe", "minor healing", SpellKind::Heal, 2, 3, 8);
        let step = cast_step(&PartyCast::On(&mihe, "lauren".into()), "Recover");
        assert_eq!(step.doing.as_deref(), Some("healing Lauren"));
        let rain = SpellDef::flat("rain", "healing rain", SpellKind::PartyHeal, 5, 7, 11);
        let step = cast_step(&PartyCast::Party(&rain), "Fight");
        assert_eq!(step.doing.as_deref(), Some("casting healing rain"));
    }

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
