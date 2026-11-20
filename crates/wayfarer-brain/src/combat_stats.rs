//! The character's combat numbers (C-050, docs/brain.md "Combat statistics"), in the shape of
//! MegaMud's Combat tab: its own blows by kind with their damage, its attack spells, the damage
//! it does in a round, and the monsters block. On a Quicksilver 1.20 server (C-065) also its
//! openings and weaves, extra damage after a normal hit that is no blow of its own, the weaves
//! resisted, the kills a spell made, and the blows it took for a ward it guards. They build up over every fight, whoever drives,
//! until the player resets them; the [`Ledger`](crate::stats::Ledger) keeps them in the store.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{AttackRow, CombatStats, DamageRow};

/// A round's own lines arrive together, and rounds are 5 s apart (`kb/combat.md` "The round");
/// a line this long after the round's first starts the next.
pub const ROUND_SPAN: Duration = Duration::from_millis(2500);

/// How many and how hard: one kind of damage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Damage {
    pub count: u64,
    pub min: i64,
    pub max: i64,
    pub sum: i64,
}

impl Damage {
    fn add(&mut self, d: i64) {
        if self.count == 0 {
            self.min = d;
            self.max = d;
        } else {
            self.min = self.min.min(d);
            self.max = self.max.max(d);
        }
        self.count += 1;
        self.sum += d;
    }

    fn row(&self, kind: &str, swings: u64) -> DamageRow {
        DamageRow {
            kind: kind.to_owned(),
            count: self.count,
            per_mille: per_mille(self.count, swings),
            min: self.min,
            max: self.max,
            avg_tenths: if self.count == 0 {
                0
            } else {
                // Rounded to the nearest tenth.
                (self.sum * 20 / i64::try_from(self.count).unwrap_or(i64::MAX) + 1) / 2
            },
        }
    }
}

/// `n` of `of` in thousandths, rounded down; 0 of nothing.
fn per_mille(n: u64, of: u64) -> u32 {
    n.saturating_mul(1000)
        .checked_div(of)
        .map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX))
}

/// How often each amount of damage came (C-123): every value kept, so the page can bin them as
/// it likes. Damage is a few hundred values at most.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hist(pub BTreeMap<i64, u64>);

impl Hist {
    pub fn add(&mut self, d: i64) {
        *self.0.entry(d).or_default() += 1;
    }
}

/// One attack's tries and outcomes (C-123): the swing, the backstab, or an attack spell.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Attack {
    /// Did damage; crits among them.
    pub hits: u64,
    pub crits: u64,
    /// Did nothing; dodges among them.
    pub misses: u64,
    pub dodged: u64,
    /// Landed on armour that took it all.
    pub glances: u64,
    /// A cast resisted, or failed in a fight.
    pub resisted: u64,
    /// The hits' damage.
    pub damage: Damage,
    #[serde(default)]
    pub hist: Hist,
    /// A cast's mana, from the `spells` screen; 0 for a blow or a spell not on it.
    #[serde(default)]
    pub mana: i64,
}

impl Attack {
    fn tries(&self) -> u64 {
        self.hits + self.misses + self.glances + self.resisted
    }

    fn hit(&mut self, damage: i64, crit: bool) {
        self.hits += 1;
        if crit {
            self.crits += 1;
        }
        self.damage.add(damage);
        self.hist.add(damage);
    }

    fn miss(&mut self, dodged: bool) {
        self.misses += 1;
        if dodged {
            self.dodged += 1;
        }
    }

    fn row(&self, key: &str) -> AttackRow {
        let (kind, name) = match key.strip_prefix(CAST_KEY) {
            Some(name) => ("cast", Some(name.to_owned())),
            None => (key, None),
        };
        let d = self.damage.row(kind, 0);
        // Casts cost mana, hits and resists alike; a failed cast costs it too.
        let mana = u64::try_from(self.mana.max(0)).unwrap_or(0) * (self.hits + self.resisted);
        AttackRow {
            kind: kind.to_owned(),
            name,
            tries: self.tries(),
            hits: self.hits,
            crits: self.crits,
            misses: self.misses,
            dodged: self.dodged,
            glances: self.glances,
            resisted: self.resisted,
            min: d.min,
            max: d.max,
            avg_tenths: d.avg_tenths,
            mana,
            per_mana_hundredths: i64::try_from(mana)
                .ok()
                .filter(|m| *m > 0)
                .map_or(0, |m| self.damage.sum * 100 / m),
        }
    }
}

/// The key of the swing in [`Tally::attacks`]; the backstab's is [`BACKSTAB_KEY`], a spell's
/// [`CAST_KEY`] and its name.
pub const SWING_KEY: &str = "swing";
pub const BACKSTAB_KEY: &str = "backstab";
pub const CAST_KEY: &str = "cast:";

/// A spell on the character's `spells` screen, for the casts it sends (C-123).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownSpell {
    pub short: String,
    pub name: String,
    pub mana: i64,
    /// An attack or area attack spell: only those count a failure in a fight as a miss.
    pub attack: bool,
}

/// How long a `bs` or a `cast` waits for the line that says how it came out.
pub const OUTCOME_WAIT: Duration = Duration::from_secs(10);

/// The numbers since the last reset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tally {
    /// Unix seconds of the last reset (or of the first count).
    pub since: i64,
    pub hit: Damage,
    pub crit: Damage,
    pub backstab: Damage,
    /// Backstabs that were also critical (counted in `backstab`).
    pub backstab_crits: u64,
    /// `bs` / `backstab` commands sent, the bot's or typed.
    pub backstab_tries: u64,
    pub glances: u64,
    /// Misses, dodges among them.
    pub misses: u64,
    pub dodged: u64,
    pub cast: Damage,
    /// Attack casts that failed or were resisted.
    pub cast_misses: u64,
    pub round: Damage,
    pub kills: u64,
    pub flees: u64,
    pub lowest_hp: Option<i64>,
    pub lowest_mana: Option<i64>,
    /// A Thief's openings (1.20): extra damage after a normal hit, not a blow.
    #[serde(default)]
    pub openings: Damage,
    /// A hybrid's weaves that landed (1.20), 0 damage among them: spell damage, not a blow.
    #[serde(default)]
    pub weaves: Damage,
    /// Weaves the target resisted: no damage.
    #[serde(default)]
    pub weaves_resisted: u64,
    /// Kills whose last own line was a spell: a cast or a weave.
    #[serde(default)]
    pub spell_kills: u64,
    /// Monster blows the character took for the ward it guards (1.20 `guard`).
    #[serde(default)]
    pub intercepts: u64,
    /// By monster name: the character's normal hits on it and its weaves into it. A monster
    /// that takes many hits and no weave from a weaver who weaves elsewhere is likely immune to
    /// its spells (a weave never picks a spell the target is immune to).
    #[serde(default)]
    pub weave_targets: BTreeMap<String, (u64, u64)>,
    /// Each attack's outcomes (C-123), by [`SWING_KEY`], [`BACKSTAB_KEY`] or [`CAST_KEY`] and
    /// the spell's name.
    #[serde(default)]
    pub attacks: BTreeMap<String, Attack>,
    /// The damage of every round, and of every blow or cast that did damage (C-123).
    #[serde(default)]
    pub round_hist: Hist,
    #[serde(default)]
    pub all_hist: Hist,
    /// The character's last own line was a spell (a cast or a weave): a death now is a spell
    /// kill. Not kept.
    #[serde(skip)]
    last_spell: bool,
}

/// Normal hits on a monster with no weave into it, from a weaver who has woven elsewhere,
/// before it is listed as likely immune to the weaver's spells.
pub const NO_WEAVE_HITS: u64 = 10;

impl Tally {
    fn landed(&self) -> u64 {
        self.hit.count + self.crit.count + self.backstab.count + self.glances
    }

    fn swings(&self) -> u64 {
        self.landed() + self.misses
    }

    fn is_empty(&self) -> bool {
        self.swings() == 0
            && self.cast.count == 0
            && self.openings.count == 0
            && self.weaves.count == 0
            && self.weaves_resisted == 0
            && self.intercepts == 0
            && self.cast_misses == 0
            && self.backstab_tries == 0
            && self.kills == 0
            && self.flees == 0
    }
}

/// The round in progress: its first line's time and the damage so far.
#[derive(Debug, Clone, Copy)]
struct Round {
    start: Instant,
    damage: i64,
}

/// The tally and the round in progress; `dirty` until the store has it.
#[derive(Debug, Default)]
pub struct CombatTally {
    tally: Tally,
    round: Option<Round>,
    dirty: bool,
    /// The spells the `spells` screen listed, for the casts sent.
    spells: Vec<KnownSpell>,
    /// A `bs` sent: its first blow is the backstab, whatever the line says.
    backstab_sent: Option<Instant>,
    /// The attack spell last sent, for a resist that does not name it.
    cast_sent: Option<(String, Instant)>,
}

impl CombatTally {
    /// From the store's JSON; a fresh tally from `now` when there is none or it does not read.
    pub fn load(json: Option<&str>, now: i64) -> Self {
        let tally = json
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or(Tally {
                since: now,
                ..Tally::default()
            });
        Self {
            tally,
            ..Self::default()
        }
    }

    /// The spells on the character's `spells` screen.
    pub fn know_spells(&mut self, spells: Vec<KnownSpell>) {
        self.spells = spells;
    }

    fn spell_by_name(&self, name: &str) -> Option<&KnownSpell> {
        self.spells
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// The attack a cast of `spell` counts in, with the spell's mana when it is known.
    fn cast_attack(&mut self, spell: &str) -> &mut Attack {
        let name = spell.trim().to_lowercase();
        let mana = self.spell_by_name(&name).map_or(0, |s| s.mana);
        let a = self
            .tally
            .attacks
            .entry(format!("{CAST_KEY}{name}"))
            .or_default();
        if mana > 0 {
            a.mana = mana;
        }
        a
    }

    /// The attack of the blow just seen. A blow that did damage says whether it was a backstab
    /// (`surprise`); a miss or a glance does not, so it is the backstab's when a `bs` is waiting
    /// for its first blow. A plain hit while one waits means the stab became a plain attack.
    fn blow_key(&mut self, hit: Option<bool>, now: Instant) -> &'static str {
        let waiting = self
            .backstab_sent
            .take()
            .is_some_and(|at| now.duration_since(at) <= OUTCOME_WAIT);
        match hit {
            Some(true) => BACKSTAB_KEY,
            Some(false) => SWING_KEY,
            None if waiting => BACKSTAB_KEY,
            None => SWING_KEY,
        }
    }

    #[cfg(test)]
    pub fn tally(&self) -> &Tally {
        &self.tally
    }

    /// For the store: the closed rounds only (a round in progress is counted when it closes).
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.tally).expect("a tally serializes")
    }

    /// Whether there is something the store does not have yet.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn saved(&mut self) {
        self.dirty = false;
    }

    /// Zero everything, from `now` (unix seconds).
    pub fn reset(&mut self, now: i64) {
        self.tally = Tally {
            since: now,
            ..Tally::default()
        };
        self.round = None;
        self.backstab_sent = None;
        self.cast_sent = None;
        self.dirty = true;
    }

    /// When the round in progress is over.
    pub fn round_ends(&self) -> Option<Instant> {
        self.round.map(|r| r.start + ROUND_SPAN)
    }

    /// Close the round in progress if its time is up.
    pub fn close_round_if_due(&mut self, now: Instant) {
        if let Some(r) = self.round
            && now.duration_since(r.start) >= ROUND_SPAN
        {
            self.tally.round.add(r.damage);
            self.tally.round_hist.add(r.damage);
            self.round = None;
            self.dirty = true;
        }
    }

    /// One of the character's own attack lines, with its damage, in the round of `now`.
    fn in_round(&mut self, damage: i64, now: Instant) {
        self.close_round_if_due(now);
        match &mut self.round {
            Some(r) => r.damage += damage,
            None => {
                self.round = Some(Round { start: now, damage });
            }
        }
        self.dirty = true;
    }

    /// A game event the character saw.
    pub fn event(&mut self, ev: &GameEvent, in_combat: bool, now: Instant) {
        self.attack_event(ev, in_combat, now);
        let t = &mut self.tally;
        match ev {
            GameEvent::YouHit {
                target,
                damage,
                crit,
                backstab,
            } => {
                t.last_spell = false;
                t.all_hist.add(*damage);
                if !*backstab {
                    t.weave_targets.entry(target.to_lowercase()).or_default().0 += 1;
                }
                if *backstab {
                    t.backstab.add(*damage);
                    if *crit {
                        t.backstab_crits += 1;
                    }
                } else if *crit {
                    t.crit.add(*damage);
                } else {
                    t.hit.add(*damage);
                }
                self.in_round(*damage, now);
            }
            GameEvent::YouMiss { dodged, .. } => {
                t.last_spell = false;
                t.misses += 1;
                if *dodged {
                    t.dodged += 1;
                }
                self.in_round(0, now);
            }
            GameEvent::SwingGlanced { .. } => {
                t.last_spell = false;
                t.glances += 1;
                self.in_round(0, now);
            }
            GameEvent::SpellCast { damage, .. } => {
                t.last_spell = true;
                t.all_hist.add(*damage);
                t.cast.add(*damage);
                self.in_round(*damage, now);
            }
            // The character's own opening (1.20): damage on top of the hit before it, in the
            // round, but no blow and no crit of its own.
            GameEvent::Opening {
                who: None,
                target: Some(_),
                damage,
                ..
            } => {
                t.last_spell = false;
                t.openings.add(*damage);
                self.in_round(*damage, now);
            }
            // The character's own weave (1.20): spell damage after a hit, or one resist roll
            // for the whole proc and no damage.
            GameEvent::Weave {
                who: None,
                target: Some(target),
                damage,
                resisted,
                ..
            } => {
                t.last_spell = true;
                t.weave_targets.entry(target.to_lowercase()).or_default().1 += 1;
                if *resisted {
                    t.weaves_resisted += 1;
                } else {
                    t.weaves.add(*damage);
                }
                self.in_round(*damage, now);
            }
            // The character intercepted a blow meant for its ward (1.20 `guard`).
            GameEvent::Intercept { guard: None, .. } => {
                t.intercepts += 1;
                self.dirty = true;
            }
            // A failed cast is an attack cast's miss only in a fight: out of one it is a heal
            // or a blessing.
            GameEvent::CastFailed { .. } | GameEvent::CastResisted if in_combat => {
                t.cast_misses += 1;
                self.in_round(0, now);
            }
            _ => {}
        }
    }

    /// The attack a blow or cast belongs to, and how it came out (C-123).
    fn attack_event(&mut self, ev: &GameEvent, in_combat: bool, now: Instant) {
        match ev {
            GameEvent::YouHit {
                damage,
                crit,
                backstab,
                ..
            } => {
                let key = self.blow_key(Some(*backstab), now);
                let a = self.tally.attacks.entry(key.to_owned()).or_default();
                a.hit(*damage, *crit);
            }
            GameEvent::YouMiss { dodged, .. } => {
                let key = self.blow_key(None, now);
                self.tally
                    .attacks
                    .entry(key.to_owned())
                    .or_default()
                    .miss(*dodged);
            }
            GameEvent::SwingGlanced { .. } => {
                let key = self.blow_key(None, now);
                self.tally
                    .attacks
                    .entry(key.to_owned())
                    .or_default()
                    .glances += 1;
            }
            GameEvent::SpellCast { spell, damage, .. } => {
                self.cast_sent = None;
                self.cast_attack(spell).hit(*damage, false);
            }
            GameEvent::CastFailed { spell } if in_combat => {
                self.cast_sent = None;
                // Only an attack spell's failure is a miss: a heal's or a blessing's is not.
                if self.spells.is_empty()
                    || self.spell_by_name(spell.trim()).is_some_and(|s| s.attack)
                {
                    self.cast_attack(spell).resisted += 1;
                }
            }
            GameEvent::CastResisted if in_combat => {
                let name = self
                    .cast_sent
                    .take()
                    .filter(|(_, at)| now.duration_since(*at) <= OUTCOME_WAIT)
                    .map_or_else(|| "a spell".to_owned(), |(n, _)| n);
                self.cast_attack(&name).resisted += 1;
            }
            _ => {}
        }
    }

    /// The character's HP and mana now, for the lowest seen.
    pub fn vitals(&mut self, hp: Option<i64>, mana: Option<i64>) {
        let t = &mut self.tally;
        let lower = |low: &mut Option<i64>, v: Option<i64>| match (*low, v) {
            (_, None) => false,
            (Some(l), Some(v)) if v >= l => false,
            (_, v) => {
                *low = v;
                true
            }
        };
        if lower(&mut t.lowest_hp, hp) | lower(&mut t.lowest_mana, mana) {
            self.dirty = true;
        }
    }

    /// A line went to the game, the bot's or typed: a backstab try counts.
    /// Its first blow is the backstab; an attack spell's cast waits for its outcome.
    pub fn sent(&mut self, line: &str, now: Instant) {
        match crate::commands::classify(line).cmd {
            Some(crate::commands::Cmd::Backstab) => {
                self.tally.backstab_tries += 1;
                self.backstab_sent = Some(now);
                self.dirty = true;
            }
            Some(crate::commands::Cmd::Cast) => {
                let short = line.split_whitespace().nth(1).unwrap_or("");
                if let Some(s) = self
                    .spells
                    .iter()
                    .find(|s| s.attack && s.short.eq_ignore_ascii_case(short))
                {
                    self.cast_sent = Some((s.name.to_lowercase(), now));
                }
            }
            _ => {}
        }
    }

    /// A monster died; a spell kill when the character's last own line was a spell.
    pub fn kill(&mut self) {
        self.tally.kills += 1;
        if std::mem::take(&mut self.tally.last_spell) {
            self.tally.spell_kills += 1;
        }
        self.dirty = true;
    }

    /// The monsters likely immune to the character's spells: [`NO_WEAVE_HITS`] normal hits or
    /// more and no weave, from a character that has woven into something else.
    fn no_weave(&self) -> Vec<String> {
        let t = &self.tally;
        if t.weaves.count + t.weaves_resisted == 0 {
            return Vec::new();
        }
        t.weave_targets
            .iter()
            .filter(|(_, (hits, weaves))| *hits >= NO_WEAVE_HITS && *weaves == 0)
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn flee(&mut self) {
        self.tally.flees += 1;
        self.dirty = true;
    }

    /// For the `stats` message: `None` when nothing has been counted since the reset.
    pub fn to_wire(&self) -> Option<CombatStats> {
        let t = &self.tally;
        if t.is_empty() {
            return None;
        }
        let swings = t.swings();
        let landed = t.landed();
        let rows = [
            ("hit", &t.hit),
            ("crit", &t.crit),
            ("backstab", &t.backstab),
            ("cast", &t.cast),
        ]
        .into_iter()
        .filter(|(_, d)| d.count > 0)
        .map(|(k, d)| d.row(k, swings))
        // Openings and weaves come after a normal hit: their share is of those.
        .chain(
            [("opening", &t.openings), ("weave", &t.weaves)]
                .into_iter()
                .filter(|(_, d)| d.count > 0)
                .map(|(k, d)| d.row(k, t.hit.count + t.crit.count)),
        )
        .collect();
        Some(CombatStats {
            since: crate::stats::rfc3339_at(t.since),
            swings,
            accuracy_per_mille: per_mille(landed, swings),
            crit_rate_per_mille: per_mille(t.crit.count + t.backstab_crits, landed),
            rows,
            misses: t.misses,
            dodged: t.dodged,
            glances: t.glances,
            backstab_tries: t.backstab_tries,
            backstab_crits: t.backstab_crits,
            cast_misses: t.cast_misses,
            cast_accuracy_per_mille: per_mille(t.cast.count, t.cast.count + t.cast_misses),
            round: (t.round.count > 0).then(|| t.round.row("round", 0)),
            kills: t.kills,
            flees: t.flees,
            lowest_hp: t.lowest_hp,
            lowest_mana: t.lowest_mana,
            weaves_resisted: t.weaves_resisted,
            spell_kills: t.spell_kills,
            intercepts: t.intercepts,
            no_weave: self.no_weave(),
            attacks: self.attack_rows(),
        })
    }

    /// The attacks tried at all: the swing, the backstab, then the spells by name.
    fn attack_rows(&self) -> Vec<AttackRow> {
        let a = &self.tally.attacks;
        [SWING_KEY, BACKSTAB_KEY]
            .into_iter()
            .filter_map(|k| a.get(k).map(|x| (k, x)))
            .chain(
                a.iter()
                    .filter(|(k, _)| k.starts_with(CAST_KEY))
                    .map(|(k, x)| (k.as_str(), x)),
            )
            .filter(|(_, x)| x.tries() > 0)
            .map(|(k, x)| x.row(k))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(damage: i64) -> GameEvent {
        GameEvent::YouHit {
            target: "rat".into(),
            damage,
            crit: false,
            backstab: false,
        }
    }

    fn miss(dodged: bool) -> GameEvent {
        GameEvent::YouMiss {
            target: "rat".into(),
            dodged,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn each_kind_of_blow_counts_with_its_damage() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 1000);
        c.event(&hit(4), true, t0);
        c.event(&hit(10), true, t0);
        c.event(
            &GameEvent::YouHit {
                target: "rat".into(),
                damage: 34,
                crit: true,
                backstab: false,
            },
            true,
            t0,
        );
        c.event(
            &GameEvent::YouHit {
                target: "rat".into(),
                damage: 90,
                crit: true,
                backstab: true,
            },
            true,
            t0,
        );
        c.event(&miss(false), true, t0);
        c.event(&miss(true), true, t0);
        c.event(
            &GameEvent::SwingGlanced {
                target: "ooze".into(),
            },
            true,
            t0,
        );
        let t = c.tally();
        assert_eq!(
            (t.hit.count, t.hit.min, t.hit.max, t.hit.sum),
            (2, 4, 10, 14)
        );
        assert_eq!((t.crit.count, t.crit.max), (1, 34));
        assert_eq!((t.backstab.count, t.backstab_crits), (1, 1));
        assert_eq!((t.misses, t.dodged, t.glances), (2, 1, 1));
        let w = c.to_wire().unwrap();
        // 7 swings, 5 landed; 2 crits (one a backstab's) of the 5.
        assert_eq!(w.swings, 7);
        assert_eq!(w.accuracy_per_mille, 714);
        assert_eq!(w.crit_rate_per_mille, 400);
        let kinds: Vec<_> = w.rows.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(kinds, ["hit", "crit", "backstab"]);
        assert_eq!(w.rows[0].avg_tenths, 70);
        assert_eq!(w.rows[0].per_mille, 285);
    }

    #[tokio::test(start_paused = true)]
    async fn rounds_are_the_lines_that_come_together() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        // Round 1: two blows at once; round 2, 5 s on: a miss only; round 3: a blow and a cast.
        c.event(&hit(5), true, t0);
        c.event(&hit(7), true, t0 + Duration::from_millis(30));
        c.event(&miss(false), true, t0 + Duration::from_secs(5));
        c.event(&hit(20), true, t0 + Duration::from_secs(10));
        c.event(
            &GameEvent::SpellCast {
                spell: "mmis".into(),
                target: Some("rat".into()),
                damage: 8,
            },
            true,
            t0 + Duration::from_secs(10),
        );
        // The third closes when its time is up.
        assert_eq!(c.tally().round.count, 2);
        c.close_round_if_due(t0 + Duration::from_secs(13));
        let r = c.tally().round;
        assert_eq!((r.count, r.min, r.max, r.sum), (3, 0, 28, 40));
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_cast_misses_only_in_a_fight() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.event(
            &GameEvent::CastFailed {
                spell: "bless".into(),
            },
            false,
            t0,
        );
        assert_eq!(c.tally().cast_misses, 0);
        c.event(&GameEvent::CastResisted, true, t0);
        c.event(
            &GameEvent::SpellCast {
                spell: "mmis".into(),
                target: None,
                damage: 12,
            },
            true,
            t0,
        );
        let w = c.to_wire().unwrap();
        assert_eq!(w.cast_misses, 1);
        assert_eq!(w.cast_accuracy_per_mille, 500);
    }

    #[tokio::test(start_paused = true)]
    async fn the_tally_survives_the_store_and_resets_to_zero() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.event(&hit(9), true, t0);
        c.sent("bs rat", t0);
        c.sent("attack rat", t0);
        c.kill();
        c.flee();
        c.vitals(Some(40), Some(10));
        c.vitals(Some(55), Some(4));
        c.close_round_if_due(t0 + ROUND_SPAN);
        assert!(c.dirty());
        let back = CombatTally::load(Some(&c.to_json()), 99);
        assert_eq!(back.tally(), c.tally());
        let t = back.tally();
        assert_eq!((t.backstab_tries, t.kills, t.flees), (1, 1, 1));
        assert_eq!((t.lowest_hp, t.lowest_mana), (Some(40), Some(4)));
        c.reset(500);
        assert_eq!(c.tally().since, 500);
        assert!(c.to_wire().is_none(), "nothing counted since the reset");
    }

    fn opening(who: Option<&str>, damage: i64) -> GameEvent {
        GameEvent::Opening {
            who: who.map(Into::into),
            target: Some("giant rat".into()),
            verb: "slash".into(),
            damage,
        }
    }

    fn weave(target: &str, damage: i64, resisted: bool) -> GameEvent {
        GameEvent::Weave {
            who: None,
            target: Some(target.into()),
            spell: "meteor swarm".into(),
            damage,
            resisted,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_opening_is_damage_but_no_blow() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.event(&hit(7), true, t0);
        c.event(&opening(None, 31), true, t0);
        // Another Thief's opening is not this character's.
        c.event(&opening(Some("Thornwyn"), 23), true, t0);
        c.close_round_if_due(t0 + ROUND_SPAN);
        let t = c.tally();
        assert_eq!((t.openings.count, t.openings.sum), (1, 31));
        assert_eq!((t.hit.count, t.crit.count), (1, 0));
        assert_eq!(t.round.sum, 38, "the round's damage has the opening");
        let w = c.to_wire().unwrap();
        assert_eq!(w.swings, 1, "an opening is no swing");
        assert_eq!(w.crit_rate_per_mille, 0);
        let opening = w.rows.iter().find(|r| r.kind == "opening").unwrap();
        assert_eq!(
            (opening.count, opening.per_mille),
            (1, 1000),
            "one per normal hit"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn weaves_resists_and_a_spell_kill() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.event(&hit(10), true, t0);
        c.event(&weave("giant rat", 75, false), true, t0);
        c.event(&hit(10), true, t0 + Duration::from_secs(5));
        c.event(
            &weave("giant rat", 0, false),
            true,
            t0 + Duration::from_secs(5),
        );
        c.event(&hit(10), true, t0 + Duration::from_secs(10));
        c.event(
            &weave("giant rat", 0, true),
            true,
            t0 + Duration::from_secs(10),
        );
        c.event(&hit(10), true, t0 + Duration::from_secs(15));
        c.event(
            &weave("giant rat", 40, false),
            true,
            t0 + Duration::from_secs(15),
        );
        c.kill();
        // A kill by a blow is no spell kill.
        c.event(&hit(10), true, t0 + Duration::from_secs(20));
        c.kill();
        let t = c.tally();
        assert_eq!((t.weaves.count, t.weaves.sum, t.weaves.min), (3, 115, 0));
        assert_eq!(t.weaves_resisted, 1);
        assert_eq!((t.kills, t.spell_kills), (2, 1));
        let w = c.to_wire().unwrap();
        assert_eq!(w.swings, 5, "weaves are no swings");
        assert_eq!((w.weaves_resisted, w.spell_kills), (1, 1));
        let row = w.rows.iter().find(|r| r.kind == "weave").unwrap();
        assert_eq!((row.count, row.per_mille), (3, 600));
    }

    #[tokio::test(start_paused = true)]
    async fn the_round_goes_on_after_a_stab() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.event(
            &GameEvent::YouHit {
                target: "giant rat".into(),
                damage: 140,
                crit: true,
                backstab: true,
            },
            true,
            t0,
        );
        c.event(&hit(8), true, t0 + Duration::from_millis(20));
        c.event(
            &GameEvent::YouHit {
                target: "giant rat".into(),
                damage: 30,
                crit: true,
                backstab: false,
            },
            true,
            t0 + Duration::from_millis(40),
        );
        c.close_round_if_due(t0 + ROUND_SPAN);
        let t = c.tally();
        assert_eq!((t.backstab.count, t.backstab_crits), (1, 1));
        assert_eq!((t.hit.count, t.crit.count), (1, 1));
        assert_eq!((t.round.count, t.round.sum), (1, 178));
    }

    #[tokio::test(start_paused = true)]
    async fn intercepts_count_for_the_guard_only() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.event(
            &GameEvent::Intercept {
                guard: None,
                monster: "orc".into(),
                ward: Some("Mirelle".into()),
            },
            true,
            t0,
        );
        // The ward's view and the room's: not this character's blows.
        c.event(
            &GameEvent::Intercept {
                guard: Some("Brannoc".into()),
                monster: "orc".into(),
                ward: None,
            },
            true,
            t0,
        );
        c.event(
            &GameEvent::Intercept {
                guard: Some("Brannoc".into()),
                monster: "orc".into(),
                ward: Some("Mirelle".into()),
            },
            true,
            t0,
        );
        assert_eq!(c.tally().intercepts, 1);
        assert_eq!(c.to_wire().unwrap().intercepts, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_monster_never_woven_into_is_listed() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        for _ in 0..NO_WEAVE_HITS {
            c.event(
                &GameEvent::YouHit {
                    target: "Ghost".into(),
                    damage: 5,
                    crit: false,
                    backstab: false,
                },
                true,
                t0,
            );
        }
        assert!(
            c.to_wire().unwrap().no_weave.is_empty(),
            "no weave anywhere yet: not a weaver"
        );
        c.event(&hit(5), true, t0);
        c.event(&weave("rat", 12, false), true, t0);
        assert_eq!(c.to_wire().unwrap().no_weave, ["ghost"]);
    }

    fn spells() -> Vec<KnownSpell> {
        let s = |short: &str, name: &str, mana, attack| KnownSpell {
            short: short.into(),
            name: name.into(),
            mana,
            attack,
        };
        vec![
            s("godw", "god's wrath", 14, true),
            s("mihe", "minor healing", 2, false),
        ]
    }

    fn row<'a>(w: &'a CombatStats, kind: &str) -> &'a AttackRow {
        w.attacks
            .iter()
            .find(|r| r.kind == kind || r.name.as_deref() == Some(kind))
            .unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn a_backstab_counts_its_misses_and_glances() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        // A stab that lands, one that misses, one the armour takes, and one that became a
        // plain attack (the monster was already fighting).
        c.sent("bs rat", t0);
        c.event(
            &GameEvent::YouHit {
                target: "rat".into(),
                damage: 120,
                crit: true,
                backstab: true,
            },
            true,
            t0,
        );
        c.event(&hit(6), true, t0);
        c.sent("bs rat", t0 + Duration::from_secs(20));
        c.event(&miss(true), true, t0 + Duration::from_secs(21));
        c.event(&miss(false), true, t0 + Duration::from_secs(21));
        c.sent("bs rat", t0 + Duration::from_secs(40));
        c.event(
            &GameEvent::SwingGlanced {
                target: "rat".into(),
            },
            true,
            t0 + Duration::from_secs(41),
        );
        c.sent("bs rat", t0 + Duration::from_secs(60));
        c.event(&hit(9), true, t0 + Duration::from_secs(61));
        // A bs long ago has no say over a miss now.
        c.sent("bs rat", t0 + Duration::from_secs(80));
        c.event(&miss(false), true, t0 + Duration::from_secs(95));
        let w = c.to_wire().unwrap();
        let bs = row(&w, "backstab");
        assert_eq!(
            (
                bs.tries, bs.hits, bs.crits, bs.misses, bs.dodged, bs.glances
            ),
            (3, 1, 1, 1, 1, 1)
        );
        assert_eq!((bs.min, bs.max), (120, 120));
        let sw = row(&w, "swing");
        assert_eq!((sw.tries, sw.hits, sw.misses), (4, 2, 2));
        assert_eq!(w.attacks[0].kind, "swing", "the swing first");
    }

    #[tokio::test(start_paused = true)]
    async fn a_spell_counts_its_resists_and_its_damage_per_mana() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.know_spells(spells());
        let cast = |damage| GameEvent::SpellCast {
            spell: "god's wrath".into(),
            target: Some("orc".into()),
            damage,
        };
        c.sent("c godw orc", t0);
        c.event(&cast(30), true, t0);
        c.sent("c godw orc", t0);
        c.event(&cast(40), true, t0);
        c.sent("c godw orc", t0);
        c.event(&GameEvent::CastResisted, true, t0);
        c.event(
            &GameEvent::CastFailed {
                spell: "god's wrath".into(),
            },
            true,
            t0,
        );
        // A heal failing in a fight is no attack's miss.
        c.event(
            &GameEvent::CastFailed {
                spell: "minor healing".into(),
            },
            true,
            t0,
        );
        let w = c.to_wire().unwrap();
        let g = row(&w, "god's wrath");
        assert_eq!(g.kind, "cast");
        assert_eq!((g.tries, g.hits, g.resisted), (4, 2, 2));
        // 70 damage for 4 casts of 14 mana: 1.25 a mana.
        assert_eq!((g.mana, g.per_mana_hundredths), (56, 125));
        assert_eq!(w.attacks.len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn rounds_and_every_hit_go_in_the_histograms() {
        let t0 = Instant::now();
        let mut c = CombatTally::load(None, 0);
        c.event(&hit(5), true, t0);
        c.event(&hit(5), true, t0);
        c.event(&hit(8), true, t0 + Duration::from_secs(5));
        c.close_round_if_due(t0 + Duration::from_secs(10));
        let t = c.tally();
        assert_eq!(t.all_hist.0, BTreeMap::from([(5, 2), (8, 1)]));
        assert_eq!(t.round_hist.0, BTreeMap::from([(10, 1), (8, 1)]));
        assert_eq!(t.attacks[SWING_KEY].hist.0.get(&5), Some(&2));
        let back = CombatTally::load(Some(&c.to_json()), 0);
        assert_eq!(back.tally(), t, "kept in the store");
    }

    #[test]
    fn a_tally_kept_before_1_20_loads() {
        let old = r#"{"since":5,"hit":{"count":1,"min":4,"max":4,"sum":4},
            "crit":{"count":0,"min":0,"max":0,"sum":0},
            "backstab":{"count":0,"min":0,"max":0,"sum":0},"backstab_crits":0,
            "backstab_tries":0,"glances":0,"misses":0,"dodged":0,
            "cast":{"count":0,"min":0,"max":0,"sum":0},"cast_misses":0,
            "round":{"count":0,"min":0,"max":0,"sum":0},"kills":3,"flees":0,
            "lowest_hp":null,"lowest_mana":null}"#;
        let c = CombatTally::load(Some(old), 99);
        let t = c.tally();
        assert_eq!((t.since, t.hit.count, t.kills), (5, 1, 3));
        assert_eq!((t.openings.count, t.weaves.count, t.intercepts), (0, 0, 0));
    }
}
