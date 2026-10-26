//! The character's combat numbers (C-050, docs/brain.md "Combat statistics"), in the shape of
//! MegaMud's Combat tab: its own blows by kind with their damage, its attack spells, the damage
//! it does in a round, and the monsters block. They build up over every fight, whoever drives,
//! until the player resets them; the [`Ledger`](crate::stats::Ledger) keeps them in the store.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{CombatStats, DamageRow};

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
}

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
            round: None,
            dirty: false,
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
        let t = &mut self.tally;
        match ev {
            GameEvent::YouHit {
                damage,
                crit,
                backstab,
                ..
            } => {
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
                t.misses += 1;
                if *dodged {
                    t.dodged += 1;
                }
                self.in_round(0, now);
            }
            GameEvent::SwingGlanced { .. } => {
                t.glances += 1;
                self.in_round(0, now);
            }
            GameEvent::SpellCast { damage, .. } => {
                t.cast.add(*damage);
                self.in_round(*damage, now);
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
    pub fn sent(&mut self, line: &str) {
        if crate::commands::classify(line).cmd == Some(crate::commands::Cmd::Backstab) {
            self.tally.backstab_tries += 1;
            self.dirty = true;
        }
    }

    pub fn kill(&mut self) {
        self.tally.kills += 1;
        self.dirty = true;
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
        })
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
        c.sent("bs rat");
        c.sent("attack rat");
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
}
