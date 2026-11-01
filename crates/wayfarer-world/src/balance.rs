//! Quicksilver 1.20's numbers for the models (C-069, `docs/kb/quicksilver-1.20.md` "Numbers"
//! and "Models"): the server's `[balance]` as frozen in 1.20.0 and the per-class damage knobs,
//! read from `data/quicksilver-1.20/balance.toml` and `class.toml`, and the formulas of the
//! handoff's sections 5 and 6 that use them. Every function here is 1.20's; 1.11p's numbers are
//! the combat model's own (`combat.rs`), and [`Balance::stock`] gives 1.11p's equivalents where
//! a caller wants one value for both.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use toml::Value;

const BALANCE_TOML: &str = include_str!("../data/quicksilver-1.20/balance.toml");
const CLASS_TOML: &str = include_str!("../data/quicksilver-1.20/class.toml");

/// A per cent as an exact fraction: `num / den` per cent. Applied to a whole number the way the
/// server applies it, truncating once at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pct {
    pub num: i64,
    pub den: i64,
}

impl Pct {
    pub const HUNDRED: Pct = Pct { num: 100, den: 1 };

    pub fn whole(p: i64) -> Pct {
        Pct { num: p, den: 1 }
    }

    /// `x × pct / 100`, truncated.
    pub fn apply(self, x: i64) -> i64 {
        x * self.num / (self.den * 100)
    }

    /// As a number of per cent, for display and expected values.
    pub fn value(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

/// A class knob: one value at every level, or `[level, %]` anchors with straight lines
/// between them and flat outside them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Knob {
    Fixed(i64),
    Anchors(Vec<(i64, i64)>),
}

impl Knob {
    /// The knob at `level`, exactly.
    pub fn at(&self, level: i64) -> Pct {
        let anchors = match self {
            Knob::Fixed(v) => return Pct::whole(*v),
            Knob::Anchors(a) => a,
        };
        let Some(&(first_level, first)) = anchors.first() else {
            return Pct::HUNDRED;
        };
        if level <= first_level {
            return Pct::whole(first);
        }
        for w in anchors.windows(2) {
            let ((l0, v0), (l1, v1)) = (w[0], w[1]);
            if level <= l1 && l1 > l0 {
                let den = l1 - l0;
                return Pct {
                    num: v0 * den + (v1 - v0) * (level - l0),
                    den,
                };
            }
        }
        Pct::whole(anchors.last().map_or(100, |a| a.1))
    }

    /// Whether the value never falls as level rises.
    pub fn never_falls(&self) -> bool {
        match self {
            Knob::Fixed(_) => true,
            Knob::Anchors(a) => a.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 <= w[1].1),
        }
    }

    fn from(v: &Value) -> Option<Knob> {
        if let Some(n) = v.as_integer() {
            return Some(Knob::Fixed(n));
        }
        let anchors = v
            .as_array()?
            .iter()
            .map(|p| {
                let p = p.as_array()?;
                Some((p.first()?.as_integer()?, p.get(1)?.as_integer()?))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Knob::Anchors(anchors))
    }
}

/// The three class knobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKnob {
    Melee,
    Spell,
    Weave,
}

impl ClassKnob {
    fn key(self) -> &'static str {
        match self {
            ClassKnob::Melee => "class_melee_pct",
            ClassKnob::Spell => "class_spell_pct",
            ClassKnob::Weave => "class_weave_pct",
        }
    }
}

/// A race's regeneration bonuses (1.20): added to Alter Healing Rate and Alter Mana
/// Regeneration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RaceRegen {
    pub heal: i64,
    pub mana: i64,
}

/// The numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Balance {
    pub backstab_energy_pct: i64,
    pub backstab_stealth_div: i64,
    pub backstab_level_pct: i64,
    pub backstab_crit_max_pct: i64,
    pub opportunist_pct: i64,
    pub opportunist_level_pct: i64,
    pub weave_pct: i64,
    pub weave_pct_magery1: i64,
    pub weave_budget_per_level: i64,
    pub weave_power_min_pct: i64,
    pub weave_power_max_pct: i64,
    pub weave_stat_bonus_pct: i64,
    pub spell_stat_permille: i64,
    pub spell_stat_knee: i64,
    pub rest_mana_mult: i64,
    pub meditate_mana_mult: i64,
    pub crit_knee: i64,
    pub mr_stat_knee: i64,
    pub dodge_agi_knee: i64,
    pub aoe_resist_cut_max_pct: i64,
    pub aoe_resist_cut_div: i64,
    pub guard_chance_base: i64,
    pub guard_chance_per_combat: i64,
    pub guard_chance_max: i64,
    pub scalecap: i64,
    /// front, mid, back, solo.
    pub rank_weights: [i64; 4],
    pub race_regen: BTreeMap<String, RaceRegen>,
    /// Class name → knob name → knob.
    classes: BTreeMap<String, BTreeMap<String, Knob>>,
}

impl Balance {
    /// Quicksilver 1.20.0's, from the embedded data.
    pub fn qs120() -> &'static Balance {
        static B: OnceLock<Balance> = OnceLock::new();
        B.get_or_init(|| {
            Balance::parse(BALANCE_TOML, CLASS_TOML).expect("the 1.20 balance data reads")
        })
    }

    /// Stock 1.11p's equivalents, where it has one: the crit knee at 40, no stat knees, no
    /// openings or weaves, a rest that gives no extra mana, every class knob 100.
    pub fn stock() -> Balance {
        Balance {
            backstab_energy_pct: 100,
            backstab_stealth_div: 10,
            backstab_level_pct: 100,
            backstab_crit_max_pct: 100,
            opportunist_pct: 0,
            opportunist_level_pct: 0,
            weave_pct: 0,
            weave_pct_magery1: 0,
            weave_budget_per_level: 0,
            weave_power_min_pct: 0,
            weave_power_max_pct: 0,
            weave_stat_bonus_pct: 0,
            spell_stat_permille: 0,
            spell_stat_knee: i64::MAX,
            rest_mana_mult: 1,
            meditate_mana_mult: 3,
            crit_knee: 40,
            mr_stat_knee: i64::MAX,
            dodge_agi_knee: i64::MAX,
            aoe_resist_cut_max_pct: 0,
            aoe_resist_cut_div: 1,
            guard_chance_base: 0,
            guard_chance_per_combat: 0,
            guard_chance_max: 0,
            scalecap: i64::MAX,
            rank_weights: [2, 2, 2, 2],
            race_regen: BTreeMap::new(),
            classes: BTreeMap::new(),
        }
    }

    /// The data's text to the numbers; `Err` names what is missing or wrong.
    pub fn parse(balance: &str, class: &str) -> Result<Balance, String> {
        let b: toml::Table = balance.parse().map_err(|e| format!("balance.toml: {e}"))?;
        let int = |k: &str| {
            b.get(k)
                .and_then(Value::as_integer)
                .ok_or_else(|| format!("balance.toml: no whole number {k:?}"))
        };
        let ranks = b
            .get("rank_weights")
            .and_then(Value::as_table)
            .ok_or("balance.toml: no [rank_weights]")?;
        let rank = |k: &str| {
            ranks
                .get(k)
                .and_then(Value::as_integer)
                .ok_or_else(|| format!("balance.toml: no rank weight {k:?}"))
        };
        let race_regen = b
            .get("race_regen")
            .and_then(Value::as_table)
            .map(|t| {
                t.iter()
                    .map(|(race, v)| {
                        let get = |k: &str| v.get(k).and_then(Value::as_integer).unwrap_or(0);
                        (
                            race.clone(),
                            RaceRegen {
                                heal: get("heal"),
                                mana: get("mana"),
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let c: toml::Table = class.parse().map_err(|e| format!("class.toml: {e}"))?;
        let mut classes = BTreeMap::new();
        for (name, knobs) in &c {
            let knobs = knobs
                .as_table()
                .ok_or_else(|| format!("class.toml: [{name}] is not a table"))?;
            let mut out = BTreeMap::new();
            for (k, v) in knobs {
                let knob = Knob::from(v)
                    .ok_or_else(|| format!("class.toml: {name}.{k} is not a number or anchors"))?;
                out.insert(k.clone(), knob);
            }
            classes.insert(name.clone(), out);
        }
        Ok(Balance {
            backstab_energy_pct: int("backstab_energy_pct")?,
            backstab_stealth_div: int("backstab_stealth_div")?,
            backstab_level_pct: int("backstab_level_pct")?,
            backstab_crit_max_pct: int("backstab_crit_max_pct")?,
            opportunist_pct: int("opportunist_pct")?,
            opportunist_level_pct: int("opportunist_level_pct")?,
            weave_pct: int("weave_pct")?,
            weave_pct_magery1: int("weave_pct_magery1")?,
            weave_budget_per_level: int("weave_budget_per_level")?,
            weave_power_min_pct: int("weave_power_min_pct")?,
            weave_power_max_pct: int("weave_power_max_pct")?,
            weave_stat_bonus_pct: int("weave_stat_bonus_pct")?,
            spell_stat_permille: int("spell_stat_permille")?,
            spell_stat_knee: int("spell_stat_knee")?,
            rest_mana_mult: int("rest_mana_mult")?,
            meditate_mana_mult: int("meditate_mana_mult")?,
            crit_knee: int("crit_knee")?,
            mr_stat_knee: int("mr_stat_knee")?,
            dodge_agi_knee: int("dodge_agi_knee")?,
            aoe_resist_cut_max_pct: int("aoe_resist_cut_max_pct")?,
            aoe_resist_cut_div: int("aoe_resist_cut_div")?,
            guard_chance_base: int("guard_chance_base")?,
            guard_chance_per_combat: int("guard_chance_per_combat")?,
            guard_chance_max: int("guard_chance_max")?,
            scalecap: int("scalecap")?,
            rank_weights: [rank("front")?, rank("mid")?, rank("back")?, rank("solo")?],
            race_regen,
            classes,
        })
    }

    /// A class knob at a level: 100 for a class or knob the data leaves out.
    pub fn class(&self, class: &str, knob: ClassKnob, level: i64) -> Pct {
        self.classes
            .get(class)
            .and_then(|k| k.get(knob.key()))
            .map_or(Pct::HUNDRED, |k| k.at(level))
    }

    /// Every class knob as the data has it, by class and knob name.
    pub fn knobs(&self) -> impl Iterator<Item = (&str, &str, &Knob)> {
        self.classes.iter().flat_map(|(c, ks)| {
            ks.iter()
                .map(move |(k, knob)| (c.as_str(), k.as_str(), knob))
        })
    }

    /// A race's regeneration bonuses.
    pub fn race(&self, race: &str) -> RaceRegen {
        self.race_regen.get(race).copied().unwrap_or_default()
    }

    /// The level the level-scaled parts of the stab, the opening and the weave use.
    pub fn capped(&self, level: i64) -> i64 {
        level.min(self.scalecap)
    }

    /// A critical chance after the knee: past it a point counts a third.
    pub fn crit(&self, crit: i64) -> i64 {
        knee(crit, self.crit_knee)
    }

    /// A player's magic-resistance stat part, `(Int + 3 × Wil) / 4`, past the knee a third
    /// (gear and spells are added after).
    pub fn mr_stat(&self, int: i64, wil: i64) -> i64 {
        knee((int + 3 * wil) / 4, self.mr_stat_knee)
    }

    /// A player's dodge term from Agility, `(Agi − 50) / 3`, Agility past the knee a third.
    pub fn dodge_agi(&self, agi: i64) -> i64 {
        (knee(agi, self.dodge_agi_knee) - 50) / 3
    }

    /// A player damage spell's amount after the casting stat: `+0.5%` a point over 50, the stat
    /// past 100 a third; `half` for a weave, which takes half the bonus.
    pub fn casting_stat(&self, amount: i64, stat: i64, half: bool) -> i64 {
        let over = (knee(stat, self.spell_stat_knee) - 50).max(0) * self.spell_stat_permille;
        let over = if half {
            over * self.weave_stat_bonus_pct / 100
        } else {
            over
        };
        amount * (1000 + over) / 1000
    }

    /// The energy of the stab, from the energy of a normal blow with the same weapon, at most
    /// the round's.
    pub fn stab_energy(&self, normal: i64, round: i64) -> i64 {
        (normal * self.backstab_energy_pct / 100).min(round)
    }

    /// The opener: the stab and the normal blows left in the same round.
    pub fn opener_blows(&self, normal: i64, round: i64) -> i64 {
        if normal <= 0 {
            return 0;
        }
        (round - self.stab_energy(normal, round)) / normal
    }

    /// A stab's damage from its rolled base (the weapon's roll and the stab's own figures):
    /// `stealth / 4` more, then `× (100 + 2L)%` with L capped.
    pub fn stab_damage(&self, base: i64, stealth: i64, level: i64) -> i64 {
        let l = self.capped(level);
        (base + stealth / self.backstab_stealth_div) * (100 + self.backstab_level_pct * l / 100)
            / 100
    }

    /// A critical stab's range: from the stab's maximum to `backstab_crit_max_pct`% of it.
    pub fn stab_crit_range(&self, max: i64) -> (i64, i64) {
        (max, max * self.backstab_crit_max_pct / 100)
    }

    /// An opening's expected damage (it never crits): `(2L + stealth/4 + the mean of the
    /// backstab bonus) × (100 + L/2)% − DR/10`, L capped, at least 0.
    pub fn opening_damage(
        &self,
        level: i64,
        stealth: i64,
        bs_min: i64,
        bs_max: i64,
        dr: i64,
    ) -> f64 {
        let l = self.capped(level) as f64;
        let base =
            2.0 * l + (stealth / self.backstab_stealth_div) as f64 + (bs_min + bs_max) as f64 / 2.0;
        let mult = 100.0 + l * self.opportunist_level_pct as f64 / 100.0;
        (base * mult / 100.0 - (dr / 10) as f64).max(0.0)
    }

    /// A Thief's expected extra damage a normal hit on a surviving target: the opening's
    /// chance of [`Balance::opening_damage`].
    pub fn opening_per_hit(
        &self,
        level: i64,
        stealth: i64,
        bs_min: i64,
        bs_max: i64,
        dr: i64,
    ) -> f64 {
        self.opportunist_pct as f64 / 100.0
            * self.opening_damage(level, stealth, bs_min, bs_max, dr)
    }

    /// A weave's chance a normal hit, by the class's magery level (1: Paladin, Ranger), 0 for a
    /// class that never weaves.
    pub fn weave_chance(&self, class: &str) -> i64 {
        match class {
            "Cleric" | "Missionary" | "Warlock" | "Gypsy" | "Bard" => self.weave_pct,
            "Paladin" | "Ranger" => self.weave_pct_magery1,
            _ => 0,
        }
    }

    /// A weave's expected damage before the target's resistances: the spell's mean volley
    /// `volley` (a Bard: 0, its budget), the power its budget leaves (`3 × L` held, between 25%
    /// and 100% of the spell's strength), half the casting-stat bonus, then the class's spell
    /// and weave knobs.
    pub fn weave_damage(&self, class: &str, level: i64, volley: f64, stat: i64) -> f64 {
        let budget = (self.weave_budget_per_level * self.capped(level)) as f64;
        let raw = if class == "Bard" || volley <= 0.0 {
            budget
        } else {
            let lo = volley * self.weave_power_min_pct as f64 / 100.0;
            let hi = volley * self.weave_power_max_pct as f64 / 100.0;
            budget.clamp(lo, hi)
        };
        let bonus = self.casting_stat(1000, stat, true) as f64 / 1000.0;
        let spell = self.class(class, ClassKnob::Spell, level).value() / 100.0;
        let weave = self.class(class, ClassKnob::Weave, level).value() / 100.0;
        raw * bonus * spell * weave
    }

    /// A weaver's expected extra damage a normal hit on a surviving target.
    pub fn weave_per_hit(&self, class: &str, level: i64, volley: f64, stat: i64) -> f64 {
        self.weave_chance(class) as f64 / 100.0 * self.weave_damage(class, level, volley, stat)
    }

    /// An area spell's expected share on a monster of magic resistance `mr` that resists with
    /// chance `p_resist`: a resisting monster's share is cut by `min(25, MR/4)`%.
    pub fn area_share(&self, share: f64, mr: i64, p_resist: f64) -> f64 {
        let cut = (mr / self.aoe_resist_cut_div).clamp(0, self.aoe_resist_cut_max_pct) as f64;
        share * (1.0 - p_resist.clamp(0.0, 0.98) * cut / 100.0)
    }

    /// What an "inflict damage on armour" spell (ability 17) does to a player of magic
    /// resistance `mr` (1.20's fix): under 50 more, not less; from 50, or with Anti-Magic, as
    /// 1.11p (half).
    pub fn armour_spell_damage(&self, amount: i64, mr: i64, anti_magic: bool) -> i64 {
        if mr >= 50 || anti_magic {
            amount / 2
        } else {
            amount + amount * (50 - mr) / 100
        }
    }

    /// A guard's chance to take a blow meant for its ward, `combat` its class's raw combat
    /// rating: `25 + 5 × (combat − 2)`, at most 95.
    pub fn guard_chance(&self, combat: i64) -> i64 {
        (self.guard_chance_base + self.guard_chance_per_combat * (combat - 2).max(0))
            .min(self.guard_chance_max)
    }
}

/// Past `knee` a point counts a third.
fn knee(x: i64, knee: i64) -> i64 {
    if x > knee { (x - knee) / 3 + knee } else { x }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_reads_and_no_class_value_falls() {
        let b = Balance::qs120();
        assert_eq!(
            (b.crit_knee, b.scalecap, b.backstab_energy_pct),
            (50, 60, 150)
        );
        assert_eq!(b.rank_weights, [3, 2, 1, 2]);
        assert_eq!(b.race("Human"), RaceRegen { heal: 15, mana: 15 });
        assert_eq!(b.race("Gaunt One"), RaceRegen { heal: 0, mana: 25 });
        assert_eq!(b.race("Kang"), RaceRegen::default());
        let mut n = 0;
        for (class, knob, k) in b.knobs() {
            assert!(k.never_falls(), "{class}.{knob} falls: {k:?}");
            n += 1;
        }
        assert_eq!(n, 19, "the knobs of handoff 7.1");
    }

    #[test]
    fn the_class_tables_at_their_anchors_and_between() {
        let b = Balance::qs120();
        let at = |c: &str, k: ClassKnob, l: i64| b.class(c, k, l).value();
        // Fixed values, and a class or knob not listed.
        assert_eq!(at("Witchunter", ClassKnob::Melee, 1), 114.0);
        assert_eq!(at("Witchunter", ClassKnob::Melee, 99), 114.0);
        assert_eq!(at("Warrior", ClassKnob::Melee, 30), 100.0);
        assert_eq!(at("Druid", ClassKnob::Spell, 50), 100.0);
        assert_eq!(at("Priest", ClassKnob::Spell, 10), 67.0);
        // Anchors: flat before the first, on the line between, flat after the last.
        assert_eq!(at("Ninja", ClassKnob::Melee, 10), 77.0);
        assert_eq!(at("Ninja", ClassKnob::Melee, 40), 77.0);
        assert_eq!(at("Ninja", ClassKnob::Melee, 50), 86.5);
        assert_eq!(at("Ninja", ClassKnob::Melee, 60), 96.0);
        assert_eq!(at("Ninja", ClassKnob::Melee, 75), 96.0);
        // The Gypsy's weave: 59 at 35, 95 at 50 (three anchors).
        assert_eq!(at("Gypsy", ClassKnob::Weave, 35), 59.0);
        assert_eq!(at("Gypsy", ClassKnob::Weave, 50), 95.0);
        assert_eq!(at("Gypsy", ClassKnob::Weave, 60), 122.0);
        // Exact fractions, applied as whole numbers at the end.
        let p = b.class("Mage", ClassKnob::Spell, 41);
        assert_eq!(
            p,
            Pct {
                num: 87 * 20 + 6,
                den: 20
            }
        );
        assert_eq!(p.apply(1000), 1000 * 1746 / 2000);
    }

    #[test]
    fn a_falling_anchor_list_is_seen() {
        let k = Knob::Anchors(vec![(30, 95), (40, 71)]);
        assert!(!k.never_falls());
        assert!(Knob::Anchors(vec![(40, 77), (60, 96)]).never_falls());
    }

    #[test]
    fn the_handoffs_worked_numbers() {
        let b = Balance::qs120();
        // A 400-energy normal blow: a 600 stab and one more blow in the round.
        assert_eq!(b.stab_energy(400, 1000), 600);
        assert_eq!(b.opener_blows(400, 1000), 1);
        // A slow weapon's stab still takes the whole round.
        assert_eq!(b.stab_energy(700, 1000), 1000);
        assert_eq!(b.opener_blows(700, 1000), 0);
        // A Warrior's guard chance is 45%.
        assert_eq!(b.guard_chance(6), 45);
        assert_eq!(b.guard_chance(30), 95);
        // The casting stat: +25% at 100, +30% at 130, +50.5% at 255; a weave takes half.
        assert_eq!(b.casting_stat(1000, 100, false), 1250);
        assert_eq!(b.casting_stat(1000, 130, false), 1300);
        assert_eq!(b.casting_stat(1000, 255, false), 1505);
        assert_eq!(b.casting_stat(1000, 40, false), 1000);
        assert_eq!(b.casting_stat(1000, 100, true), 1125);
        // Crit 80 counts as 60; 1.11p's knee is 40.
        assert_eq!(b.crit(80), 60);
        assert_eq!(b.crit(50), 50);
        assert_eq!(Balance::stock().crit(80), 53);
        // MR 0 takes 150% of an armour-piercing nuke, MR 30 120%; 50 up as 1.11p.
        assert_eq!(b.armour_spell_damage(100, 0, false), 150);
        assert_eq!(b.armour_spell_damage(100, 30, false), 120);
        assert_eq!(b.armour_spell_damage(100, 60, false), 50);
        assert_eq!(b.armour_spell_damage(100, 0, true), 50);
        // The stat knees.
        assert_eq!(b.mr_stat(110, 110), 110);
        assert_eq!(b.mr_stat(140, 140), 120);
        assert_eq!(b.dodge_agi(150), 33);
        assert_eq!(b.dodge_agi(180), 36);
        // Area spells: MR 100 cuts a resisting monster's share by 25%; the resist chance is
        // capped at 98%.
        assert_eq!(b.area_share(100.0, 100, 1.0), 75.5);
        assert_eq!(b.area_share(100.0, 40, 0.5), 95.0);
    }

    #[test]
    fn the_stab_and_the_opening_stop_growing_at_60() {
        let b = Balance::qs120();
        // Base 20, stealth 40 (+10), level 30: × 160%.
        assert_eq!(b.stab_damage(20, 40, 30), 48);
        assert_eq!(b.stab_damage(20, 40, 60), b.stab_damage(20, 40, 61));
        assert_eq!(b.stab_damage(20, 40, 60), 66);
        assert_eq!(b.stab_crit_range(90), (90, 180));
        // Level 20, stealth 40, backstab 5-15, DR 2 (a tenth: 0): (40 + 10 + 10) × 110%.
        assert_eq!(b.opening_damage(20, 40, 5, 15, 2), 66.0);
        assert!((b.opening_per_hit(20, 40, 5, 15, 2) - 6.6).abs() < 1e-9);
        assert_eq!(
            b.opening_damage(60, 0, 0, 0, 0),
            b.opening_damage(65, 0, 0, 0, 0)
        );
        // Stock 1.11p has no openings.
        assert_eq!(Balance::stock().opening_per_hit(20, 40, 5, 15, 2), 0.0);
    }

    #[test]
    fn weaves() {
        let b = Balance::qs120();
        assert_eq!(b.weave_chance("Gypsy"), 20);
        assert_eq!(b.weave_chance("Ranger"), 10);
        assert_eq!(b.weave_chance("Mage"), 0);
        // A level-20 Warlock with a 100-damage volley and Int 50: the budget, 60, sits between
        // 25 and 100; × the Warlock's weave knob at 20 (64%).
        let d = b.weave_damage("Warlock", 20, 100.0, 50);
        assert!((d - 60.0 * 0.64).abs() < 1e-9, "{d}");
        // A small volley is held to its strength.
        let d = b.weave_damage("Warlock", 20, 40.0, 50);
        assert!((d - 40.0 * 0.64).abs() < 1e-9, "{d}");
        // The budget stops at 60.
        assert_eq!(
            b.weave_damage("Bard", 60, 0.0, 50),
            b.weave_damage("Bard", 70, 0.0, 50)
        );
        assert!((b.weave_per_hit("Warlock", 20, 100.0, 50) - 0.2 * 60.0 * 0.64).abs() < 1e-9);
    }
}
