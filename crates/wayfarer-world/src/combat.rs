//! The combat model: one character's blows against one monster, from the game's formulas
//! (`kb/combat.md` "Attack modes" and "How a fight runs"; card C-057). Solo play only: no
//! party damage, no monster turns.
//!
//! A [`Fighter`] is the character for one blow (`move_player_to_fighter`, LJNCOMBT.C:338-943),
//! a [`Defender`] the monster (`move_monster_to_fighter`, :1015-1204), a [`Blow`] one blow's
//! numbers for a [`Move`] (`calculate_attack`, :1226-1427) and a [`Fight`] the rounds of the
//! auto-combat blow loop (:2640-2830), with the chance the monster is dead after each one,
//! worked out exactly over the damage dealt so far. All arithmetic is the C's: integers,
//! division truncating toward zero, every intermediate kept as the C keeps it.
//!
//! No I/O: the caller hands in records parsed from the pack's `raw` JSON with
//! [`ItemRecord::from_raw`], [`MonsterRecord::from_raw`], [`ClassRecord::from_raw`] and
//! [`RaceRecord::from_raw`].
//!
//! Left out: casting (the ×1.1 for a blow after a cast in the same round), monster turns,
//! party damage, backstab, the martial-arts moves, Protective Shield, Shadow Stealth,
//! blindness and light, the knocked-down player, Quickness and Slowness, the slowed weapon,
//! rank, the alignment defences, and the abilities of items only carried in the pack.

use serde_json::Value;

use crate::Error;

/// Whether `genrdn(lo, hi)` can return `hi`, so that it rolls `lo..=hi`; otherwise
/// `lo..hi`. The KB has it unproven either way (`kb/combat.md` "Does `genrdn(lo, hi)`
/// include `hi`?"); John chose "included" on 2026-10-26, and the recovered source's own
/// comment at LJNCOMBT.C:1373 reads it so. Every roll in this module goes through
/// [`roll_count`], so flipping this constant is the whole change if it is settled otherwise.
pub const ROLL_TOP_INCLUDED: bool = true;

/// The rounds [`Fight::expected_rounds`] counts before it stops.
pub const ROUND_CAP: usize = 20;

/// The rounds [`Fight`] reports blows and kill chances for.
pub const ROUNDS_REPORTED: usize = 8;

/// The most blows the loop strikes in one round (`rounds <= 5`, LJNCOMBT.C:2716).
pub const MAX_BLOWS_PER_ROUND: u32 = 6;

/// Every blow costs at least this much energy (LJNCOMBT.C:936-938).
pub const MIN_BLOW_COST: i64 = 200;

/// `maxenergy` of every stock character.
pub const STOCK_MAX_ENERGY: i64 = 1000;

/// A knocked-down monster's DV loss (`smashpenalty`, LJNGLOBL.C:184-187).
pub const SMASH_PENALTY: i64 = 35;

/// The chance of the fight outlasting [`ROUND_CAP`] at which [`Fight::rounds_capped`] is set:
/// one fight in a thousand.
pub const CAP_TAIL: f64 = 1e-3;

/// `type` of an armour item (`ITEM_ARMOUR`, MAJORMUD.H:339).
pub const ITEM_ARMOUR: i64 = 0;

/// The ability numbers this module reads (`ABL_`, MAJORMUD.H:562-680).
pub mod ability {
    /// "Alter Defence Value".
    pub const DEFENCE: i64 = 2;
    /// "Alter Damage (Attacking)": the most damage, for a character.
    pub const DAMAGE: i64 = 4;
    /// "Alter Damage Resistance Value", in tenths.
    pub const DAMAGE_RESIST: i64 = 7;
    /// "Alter AV (Bless)".
    pub const AV_BLESS: i64 = 22;
    /// "Dodge".
    pub const DODGE: i64 = 34;
    /// "Alter Critical Hit Chance".
    pub const CRITICAL: i64 = 58;
    /// "Alter Energy Usage", a percentage.
    pub const ENERGY_USE: i64 = 87;
    /// "Defence Modifier": added to the hit modifier of whoever attacks its holder.
    pub const DEFENCE_MOD: i64 = 104;
    /// "Alter AV (Curse)".
    pub const AV_CURSE: i64 = 105;
    /// "3rd Alter Attack Value".
    pub const AV_THIRD: i64 = 106;
}

/// How many values `genrdn(lo, hi)` can return, from `lo` up.
pub fn roll_count(lo: i64, hi: i64) -> i64 {
    (hi - lo + i64::from(ROLL_TOP_INCLUDED)).max(1)
}

/// The chance that `genrdn(lo, hi) < t`.
fn p_below(lo: i64, hi: i64, t: i64) -> f64 {
    let n = roll_count(lo, hi);
    (t - lo).clamp(0, n) as f64 / n as f64
}

/// The chance that `genrdn(lo, hi) > t`.
fn p_above(lo: i64, hi: i64, t: i64) -> f64 {
    let n = roll_count(lo, hi);
    (n - (t + 1 - lo).clamp(0, n)) as f64 / n as f64
}

// ---------------------------------------------------------------------------------------
// Records

/// One ability slot with something in it: (ability number, value).
pub type Ability = (i64, i64);

fn field<'a>(v: &'a Value, table: &str, name: &str) -> Result<&'a Value, Error> {
    v.get(name)
        .ok_or_else(|| Error::Format(format!("{table}: raw record without {name:?}")))
}

fn int(v: &Value, table: &str, name: &str) -> Result<i64, Error> {
    field(v, table, name)?
        .as_i64()
        .ok_or_else(|| Error::Format(format!("{table}: {name:?} is not an integer")))
}

fn text(v: &Value, table: &str, name: &str) -> Result<String, Error> {
    Ok(field(v, table, name)?
        .as_str()
        .unwrap_or_default()
        .to_owned())
}

fn ints(v: &Value, table: &str, name: &str) -> Result<Vec<i64>, Error> {
    field(v, table, name)?
        .as_array()
        .ok_or_else(|| Error::Format(format!("{table}: {name:?} is not a list")))?
        .iter()
        .map(|x| {
            x.as_i64()
                .ok_or_else(|| Error::Format(format!("{table}: {name:?} holds a non-integer")))
        })
        .collect()
}

/// The slots of `abilities` with an ability in them, paired with `abilvals`. A slot whose
/// ability is 0 is empty whatever its value (the Magebane's unused values are junk).
fn abilities(v: &Value, table: &str) -> Result<Vec<Ability>, Error> {
    let numbers = ints(v, table, "abilities")?;
    let values = ints(v, table, "abilvals")?;
    Ok(numbers
        .iter()
        .zip(values.iter().chain(std::iter::repeat(&0)))
        .filter(|(a, _)| **a != 0)
        .map(|(a, v)| (*a, *v))
        .collect())
}

fn parse(raw: &str, table: &str) -> Result<Value, Error> {
    serde_json::from_str(raw).map_err(|e| Error::Format(format!("{table}: raw record: {e}")))
}

/// An item record (WCCITEMS), the fields combat reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemRecord {
    pub number: i64,
    pub name: String,
    /// `ITEM_ARMOUR` 0, `ITEM_WEAPON` 1, ...
    pub item_type: i64,
    pub mindmg: i64,
    pub maxdmg: i64,
    /// The weapon's speed figure: bigger is slower.
    pub speed: i64,
    /// A fixed energy cost added on top of the speed's.
    pub energy: i64,
    pub strreq: i64,
    pub accuracy: i64,
    pub ac: i64,
    pub dr: i64,
    pub armtype: i64,
    pub worn: i64,
    pub weight: i64,
    pub abilities: Vec<Ability>,
}

impl ItemRecord {
    pub fn from_raw(raw: &str) -> Result<ItemRecord, Error> {
        ItemRecord::from_value(&parse(raw, "items")?)
    }

    pub fn from_value(v: &Value) -> Result<ItemRecord, Error> {
        let t = "items";
        Ok(ItemRecord {
            number: int(v, t, "number")?,
            name: text(v, t, "name")?,
            item_type: int(v, t, "type")?,
            mindmg: int(v, t, "mindmg")?,
            maxdmg: int(v, t, "maxdmg")?,
            speed: int(v, t, "speed")?,
            energy: int(v, t, "energy")?,
            strreq: int(v, t, "strreq")?,
            accuracy: int(v, t, "accuracy")?,
            ac: int(v, t, "ac")?,
            dr: int(v, t, "dr")?,
            armtype: int(v, t, "armtype")?,
            worn: int(v, t, "worn")?,
            weight: int(v, t, "weight")?,
            abilities: abilities(v, t)?,
        })
    }
}

/// A known-monster record (WCCKNMSR), the fields combat reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonsterRecord {
    pub number: i64,
    pub name: String,
    pub level: i64,
    pub ac: i64,
    pub dr: i64,
    pub maxhp: i64,
    pub magicres: i64,
    /// The item numbers of its weapon and the one item it wears, 0 for none: their abilities
    /// count as the monster's (`get_monster_ability_value`, LJNABIL.C:681-810).
    pub weapon: i64,
    pub wornitem: i64,
    pub abilities: Vec<Ability>,
}

impl MonsterRecord {
    pub fn from_raw(raw: &str) -> Result<MonsterRecord, Error> {
        MonsterRecord::from_value(&parse(raw, "monsters")?)
    }

    pub fn from_value(v: &Value) -> Result<MonsterRecord, Error> {
        let t = "monsters";
        Ok(MonsterRecord {
            number: int(v, t, "number")?,
            name: text(v, t, "name")?,
            level: int(v, t, "level")?,
            ac: int(v, t, "ac")?,
            dr: int(v, t, "dr")?,
            maxhp: int(v, t, "maxhp")?,
            magicres: int(v, t, "magicres")?,
            weapon: int(v, t, "weapon")?,
            wornitem: int(v, t, "wornitem")?,
            abilities: abilities(v, t)?,
        })
    }
}

/// A class record (WCCCLASS), the fields combat reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassRecord {
    pub number: i64,
    pub name: String,
    /// The class's combat value (Warrior 6, Witchunter 7, Mage 3).
    pub combat: i64,
    pub abilities: Vec<Ability>,
}

impl ClassRecord {
    pub fn from_raw(raw: &str) -> Result<ClassRecord, Error> {
        ClassRecord::from_value(&parse(raw, "classes")?)
    }

    pub fn from_value(v: &Value) -> Result<ClassRecord, Error> {
        let t = "classes";
        Ok(ClassRecord {
            number: int(v, t, "number")?,
            name: text(v, t, "name")?,
            combat: int(v, t, "combat")?,
            abilities: abilities(v, t)?,
        })
    }
}

/// A race record (WCCRACE), the fields combat reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceRecord {
    pub number: i64,
    pub name: String,
    pub abilities: Vec<Ability>,
}

impl RaceRecord {
    pub fn from_raw(raw: &str) -> Result<RaceRecord, Error> {
        RaceRecord::from_value(&parse(raw, "races")?)
    }

    pub fn from_value(v: &Value) -> Result<RaceRecord, Error> {
        let t = "races";
        Ok(RaceRecord {
            number: int(v, t, "number")?,
            name: text(v, t, "name")?,
            abilities: abilities(v, t)?,
        })
    }
}

// ---------------------------------------------------------------------------------------
// The character

/// A lasting effect on the character: what a spell or a potion adds. 0 is "nothing". `av`
/// counts as an Alter AV ability (the best of all of them counts, not the sum) and
/// `energy_pct` as an Alter Energy Usage one; `str` and `agi` change the stats before
/// anything is worked out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buff {
    pub av: i64,
    pub crit: i64,
    pub max_damage: i64,
    pub str: i64,
    pub agi: i64,
    pub energy_pct: i64,
}

/// The character's own statistics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub str: i64,
    pub int: i64,
    pub agi: i64,
    pub cha: i64,
}

/// Everything [`Fighter::new`] needs.
#[derive(Clone, Debug)]
pub struct Character<'a> {
    pub class: &'a ClassRecord,
    pub race: &'a RaceRecord,
    pub level: i64,
    pub stats: Stats,
    /// The wielded weapon; `None` fights with fists (speed 1200, 1-4 damage).
    pub weapon: Option<&'a ItemRecord>,
    pub worn: Vec<&'a ItemRecord>,
    /// Carried weight × 100 / capacity, truncated.
    pub encumbrance: i64,
    pub max_energy: i64,
    pub buffs: Vec<Buff>,
}

/// The bonus totals of `update_dynamic_stats` (LJNABIL.C:1230-1385) that combat reads.
#[derive(Clone, Copy, Debug, Default)]
struct Bonuses {
    /// The best Alter AV value of all sources, if any.
    av: Option<i64>,
    crit: i64,
    damage: i64,
    /// Alter Energy Usage, averaged with what earlier sources gave.
    energy_use: i64,
}

impl Bonuses {
    /// `update_dynamic_with_ability` (LJNABIL.C:1046-1130) for the abilities combat reads.
    fn apply(&mut self, (ability, value): Ability) {
        match ability {
            ability::AV_BLESS | ability::AV_CURSE | ability::AV_THIRD => {
                self.av = Some(self.av.map_or(value, |b| b.max(value)));
            }
            ability::CRITICAL => self.crit += value,
            ability::DAMAGE => self.damage += value,
            ability::ENERGY_USE => {
                self.energy_use = if self.energy_use != 0 {
                    (self.energy_use + value) / 2
                } else {
                    value
                };
            }
            _ => {}
        }
    }

    /// The sources in the game's order: spells (the buffs), race, class, worn items, the
    /// weapon. The order matters only for Alter Energy Usage.
    fn of(c: &Character) -> Bonuses {
        let mut b = Bonuses::default();
        for buff in &c.buffs {
            for slot in [
                (ability::AV_BLESS, buff.av),
                (ability::CRITICAL, buff.crit),
                (ability::DAMAGE, buff.max_damage),
                (ability::ENERGY_USE, buff.energy_pct),
            ] {
                if slot.1 != 0 {
                    b.apply(slot);
                }
            }
        }
        let items = c.worn.iter().copied().chain(c.weapon);
        let slots = c
            .race
            .abilities
            .iter()
            .chain(&c.class.abilities)
            .chain(items.flat_map(|i| &i.abilities));
        for &slot in slots {
            b.apply(slot);
        }
        b
    }
}

/// The energy one blow costs before the move doubles it (`compute_energy_used`,
/// LJNCOMBT.C:221-250, then :580-588, :839-848, :908 of `move_player_to_fighter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Energy {
    /// `extra + speed*1000/divisor`.
    pub raw: i64,
    /// After the Str shortfall and Alter Energy Usage, before the encumbrance factor.
    pub before_load: i64,
    /// After the encumbrance factor: what the fast-blow test reads.
    pub after_load: i64,
    /// The Str is short of the weapon's `strreq`.
    pub too_weak: bool,
    /// The critical chance a fast blow adds (0 for none).
    pub fast_crit: i64,
}

/// The character for one blow: what `move_player_to_fighter` leaves in the fighter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fighter {
    pub level: i64,
    pub str: i64,
    pub agi: i64,
    pub max_energy: i64,
    pub energy: Energy,
    pub min_damage: i64,
    pub max_damage: i64,
    pub av: i64,
    /// `mabase` (LJNUTIL.C:4994-5002).
    pub mabase: i64,
    /// `max(1, mabase + critbonus)` plus the fast-blow bonus, before the 40 rule.
    pub crit: i64,
}

/// The whole square root (`root` in LJNCOMBT.C:719-725).
fn isqrt(n: i64) -> i64 {
    n.max(0).isqrt()
}

impl Fighter {
    pub fn new(c: &Character) -> Fighter {
        let bonus = Bonuses::of(c);
        let str = c.stats.str + c.buffs.iter().map(|b| b.str).sum::<i64>();
        let agi = c.stats.agi + c.buffs.iter().map(|b| b.agi).sum::<i64>();
        let (int, cha) = (c.stats.int, c.stats.cha);
        let (level, combat, load) = (c.level, c.class.combat, c.encumbrance);

        // Energy (LJNCOMBT.C:221-250).
        let (speed, extra, strreq) = c
            .weapon
            .map_or((1200, 0, 0), |w| (w.speed, w.energy, w.strreq));
        let skill = level * combat + 45;
        let quick = agi + 150;
        let divisor = skill * quick * 1500 / 9000;
        let raw = if skill != 0 && quick != 0 && divisor != 0 {
            extra + speed * 1000 / divisor
        } else {
            50
        };
        let mut cost = raw;
        let too_weak = c.weapon.is_some() && strreq > str;
        if too_weak {
            cost = cost * ((strreq - str) * 3 + 200) / 200;
        }
        // (sic) the C tests an unsigned copy, so a negative percentage gives a nonsense
        // cost there; only positive ones are meant (LJNCOMBT.C:839-848).
        if bonus.energy_use > 0 {
            cost = cost * bonus.energy_use / 100;
        }
        let before_load = cost;
        let after_load = cost * (load / 2 + 75) / 100;

        // Critical chance (LJNUTIL.C:4994-5002, LJNCOMBT.C:401, :892-901).
        let mabase =
            (level / 10 + (int - 50) / 10 + (agi - 50) / 20 + (cha - 50) / 30).clamp(1, 75);
        let mut fast_crit = 0;
        if after_load < MIN_BLOW_COST && load <= 66 && !too_weak {
            let n = (200 - after_load + (agi - 50) / 10).min(20);
            fast_crit = if load < 33 { n } else { n / 2 };
        }
        let crit = (mabase + bonus.crit).max(1) + fast_crit;

        // Damage (LJNCOMBT.C:640-660, :782).
        let (mut min, mut max) = c.weapon.map_or((1, 4), |w| (w.mindmg, w.maxdmg));
        max += (str - 50) / 10;
        if (str - 100) / 10 * 2 > 0 {
            min += (str - 100) / 10 * 2;
        }
        min = min.min(max).max(0);
        max = max.max(0) + bonus.damage;

        // Attack value (LJNCOMBT.C:666-730, :778).
        let armour = c.worn.iter().filter(|i| i.item_type == ITEM_ARMOUR);
        let mut accuracy =
            c.weapon.map_or(0, |w| w.accuracy) + armour.map(|i| i.accuracy).sum::<i64>();
        if accuracy == 0 {
            accuracy = 1;
        }
        if load < 33 {
            accuracy += 15 - load / 10;
        }
        let skill = (combat - 1) * 2 + level / 2 + accuracy / 2;
        let training = (combat - 1) * isqrt(level);
        let av = (skill + training) * 2 + (str - 50) / 3 + (agi - 50) / 6 + bonus.av.unwrap_or(0);

        Fighter {
            level,
            str,
            agi,
            max_energy: c.max_energy,
            energy: Energy {
                raw,
                before_load,
                after_load,
                too_weak,
                fast_crit,
            },
            min_damage: min,
            max_damage: max,
            av,
            mabase,
            crit,
        }
    }

    /// What one blow of `mv` costs (LJNCOMBT.C:909-939).
    pub fn cost(&self, mv: Move) -> i64 {
        let cost = match mv {
            Move::Attack => self.energy.after_load,
            Move::Bash => self.energy.after_load * 2,
            Move::Smash => self.max_energy,
        };
        cost.max(MIN_BLOW_COST).min(self.max_energy)
    }
}

// ---------------------------------------------------------------------------------------
// The monster

/// The monster for one blow: what `move_monster_to_fighter` leaves in the defender.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Defender {
    pub level: i64,
    pub magicres: i64,
    pub maxhp: i64,
    /// `ac` + Alter Defence Value.
    pub dv: i64,
    /// `dr*10` + Alter Damage Resistance, in tenths; a tenth of it comes off every blow.
    pub dr: i64,
    pub dodge: i64,
    pub defmod: i64,
}

impl Defender {
    /// `items` are the monster's weapon and worn item, when it has them (`weapon` and
    /// `wornitem` on the record): their abilities add to the record's.
    pub fn new(m: &MonsterRecord, items: &[&ItemRecord]) -> Defender {
        let total = |a: i64| -> i64 {
            m.abilities
                .iter()
                .chain(items.iter().flat_map(|i| &i.abilities))
                .filter(|(n, _)| *n == a)
                .map(|(_, v)| v)
                .sum()
        };
        Defender {
            level: m.level,
            magicres: m.magicres,
            maxhp: m.maxhp,
            dv: m.ac + total(ability::DEFENCE),
            dr: m.dr * 10 + total(ability::DAMAGE_RESIST),
            dodge: total(ability::DODGE),
            defmod: total(ability::DEFENCE_MOD),
        }
    }
}

// ---------------------------------------------------------------------------------------
// One blow

/// The attack moves the planner compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Move {
    Attack,
    Bash,
    Smash,
}

impl Move {
    pub const ALL: [Move; 3] = [Move::Attack, Move::Bash, Move::Smash];

    /// (hit modifier, damage bonus %, damage multiplier) (LJNCOMBT.C:1254-1290, :1386-1391).
    fn modifiers(self) -> (i64, i64, i64) {
        match self {
            Move::Attack => (0, 0, 1),
            Move::Bash => (-15, 10, 3),
            Move::Smash => (-25, 20, 5),
        }
    }
}

/// The hit chance in percent, as the C works it out (LJNCOMBT.C:1316-1342).
pub fn hit_chance(av: i64, hitmod: i64, dv: i64) -> i64 {
    let power = (av + hitmod) * (av + hitmod) / 14 / 10;
    let hit = if power != 0 { 100 - dv * dv / power } else { 5 };
    hit.clamp(10, 99)
}

/// The monster's chance to dodge a landed blow, in percent (LJNCOMBT.C:1378-1386).
pub fn dodge_chance(dodge: i64, av: i64) -> i64 {
    let chance = if av > 8 { dodge * 10 / (av / 8) } else { 0 };
    chance.min(95)
}

/// The critical chance after the "a third above 40" rule (LJNCOMBT.C:1346-1348).
pub fn crit_after_rule(crit: i64) -> i64 {
    if crit > 40 {
        (crit - 40) / 3 + 40
    } else {
        crit
    }
}

/// One blow's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    pub mv: Move,
    pub cost: i64,
    /// The hit chance in percent: the blow lands when `genrdn(1,100)` is below it.
    pub hit: i64,
    /// The critical chance after the 40 rule; 0 for a bash or a smash.
    pub crit: i64,
    /// The monster's dodge chance in percent; it dodges only with a Dodge above 0.
    pub dodge: i64,
    /// The damage a normal landed blow can do, after the monster's DR and the move's
    /// multiplier, a glance counted as 0.
    pub damage: (i64, i64),
    /// The same for a critical hit, when the blow can crit.
    pub crit_damage: Option<(i64, i64)>,
    /// The chances of the blow landing, critting (of all blows) and being dodged (of all).
    pub p_hit: f64,
    pub p_crit: f64,
    pub p_dodge: f64,
    pub expected_damage: f64,
    /// The blow's damage as a distribution.
    dist: Dist,
}

/// A blow's damage: a point mass at 0 (miss, dodge, glance) and two uniform ranges of
/// positive damage, `mult*v` for `v` in `lo..=hi`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dist {
    zero: f64,
    mult: i64,
    ranges: [Range; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Range {
    /// The total chance of the range.
    weight: f64,
    lo: i64,
    hi: i64,
}

impl Range {
    const EMPTY: Range = Range {
        weight: 0.0,
        lo: 1,
        hi: 0,
    };

    fn mean(&self, mult: i64) -> f64 {
        if self.weight == 0.0 {
            0.0
        } else {
            self.weight * mult as f64 * (self.lo + self.hi) as f64 / 2.0
        }
    }
}

/// The damage roll (`genrdn(0, max-min+1) + min - dr/10`, LJNCOMBT.C:1373-1375) of a blow
/// that lands with chance `weight`: its positive part, and the chance it does nothing.
fn damage_roll(weight: f64, min: i64, max: i64, dr: i64) -> (Range, f64) {
    let n = roll_count(0, max - min + 1);
    let (lo, hi) = (min - dr / 10, min - dr / 10 + n - 1);
    let pos_lo = lo.max(1);
    if hi < pos_lo || weight == 0.0 {
        return (Range::EMPTY, weight);
    }
    let positive = weight * (hi - pos_lo + 1) as f64 / n as f64;
    let range = Range {
        weight: positive,
        lo: pos_lo,
        hi,
    };
    (range, weight - positive)
}

impl Blow {
    /// One blow of `mv` against `d`, the monster knocked down or not.
    pub fn new(f: &Fighter, d: &Defender, mv: Move, down: bool) -> Blow {
        let (move_hitmod, bonus, mult) = mv.modifiers();
        let hitmod = move_hitmod + d.defmod;
        let crit = match mv {
            Move::Attack => crit_after_rule(f.crit),
            Move::Bash | Move::Smash => 0,
        };
        let (mut min, mut max) = (f.min_damage, f.max_damage);
        if bonus != 0 {
            min = min * (bonus + 100) / 100;
            max = max * (bonus + 100) / 100;
        }
        let dv = if down {
            (d.dv - SMASH_PENALTY).max(0)
        } else {
            d.dv
        };
        let hit = hit_chance(f.av, hitmod, dv);
        // A negative dodge catches the monster off guard: then the hit chance is 99.
        let off_guard = if d.dodge < 0 {
            p_above(0, 100, d.dodge + 100)
        } else {
            0.0
        };
        let p_hit = off_guard * p_below(1, 100, 99) + (1.0 - off_guard) * p_below(1, 100, hit);
        let pc = p_below(0, 100, crit);
        let dodge = dodge_chance(d.dodge, f.av);
        let pd = if d.dodge > 0 {
            p_below(0, 100, dodge)
        } else {
            0.0
        };

        let normal_range = (min, min.max(max));
        let crit_range = (max * 2, (max * 2).max(max * 4));
        let landed = p_hit * (1.0 - pd);
        let (normal, zero_n) =
            damage_roll(landed * (1.0 - pc), normal_range.0, normal_range.1, d.dr);
        let (crit_part, zero_c) = damage_roll(landed * pc, crit_range.0, crit_range.1, d.dr);
        let zero = (1.0 - p_hit) + p_hit * pd + zero_n + zero_c;
        let dist = Dist {
            zero,
            mult,
            ranges: [normal, crit_part],
        };
        let shown = |(lo, hi): (i64, i64)| {
            let n = roll_count(0, hi - lo + 1);
            let at = |v: i64| ((v - d.dr / 10) * mult).max(0);
            (at(lo), at(lo + n - 1))
        };
        Blow {
            mv,
            cost: f.cost(mv),
            hit,
            crit,
            dodge,
            damage: shown(normal_range),
            crit_damage: (pc > 0.0).then(|| shown(crit_range)),
            p_hit,
            p_crit: p_hit * pc,
            p_dodge: p_hit * pd,
            expected_damage: normal.mean(mult) + crit_part.mean(mult),
            dist,
        }
    }

    /// The chance the blow does any damage.
    fn p_damage(&self) -> f64 {
        1.0 - self.dist.zero
    }
}

// ---------------------------------------------------------------------------------------
// Rounds

/// The blows struck in each of the first `rounds` rounds by a blow costing `cost`: engaging
/// sets the energy to 0, every round adds `max_energy`, blows are struck while the energy
/// left is at least the cost, at most six a round, and what is left carries over.
pub fn blows_per_round(cost: i64, max_energy: i64, rounds: usize) -> Vec<u32> {
    let mut energy = 0;
    (0..rounds)
        .map(|_| {
            energy += max_energy;
            let mut blows = 0;
            while blows < MAX_BLOWS_PER_ROUND && energy >= cost {
                energy -= cost;
                blows += 1;
            }
            blows
        })
        .collect()
}

/// A fight of one move against one monster, round by round.
#[derive(Clone, Debug, PartialEq)]
pub struct Fight {
    /// The first blow (a smash after a knock-down is [`Fight::down_blow`]).
    pub blow: Blow,
    /// The blow against a knocked-down monster, when a smash can knock this one down
    /// (`Str + level > monster level + magicres + 30`, LJNCOMBT.C:3062-3070).
    pub down_blow: Option<Blow>,
    pub blows_per_round: [u32; ROUNDS_REPORTED],
    pub expected_damage_per_blow: f64,
    pub expected_damage_round1: f64,
    pub expected_damage_round2: f64,
    /// `p_kill_by_round[k]`: the chance the monster is dead after round `k + 1`.
    pub p_kill_by_round: [f64; ROUNDS_REPORTED],
    /// The expected rounds to the kill, counting at most [`ROUND_CAP`].
    pub expected_rounds: f64,
    /// The fight may well run past [`ROUND_CAP`] rounds (a chance of at least
    /// [`CAP_TAIL`]), so `expected_rounds` is short.
    pub rounds_capped: bool,
}

/// The chance the monster is still alive is below this: the fight is over.
const DONE: f64 = 1e-15;

/// The damage dealt so far, for a monster still alive: `up[x]` is the chance of `x` damage,
/// `x < maxhp`. Every cell from `reach` up is 0 in all four arrays, so a blow works only
/// on the cells the damage can have reached.
struct Tally {
    up: Vec<f64>,
    /// The same while knocked down (only for a smash that can knock it down).
    down: Vec<f64>,
    /// Scratch: the prefix sums by stride, and the next `up` or `down`.
    prefix: Vec<f64>,
    scratch: Vec<f64>,
    reach: usize,
}

impl Tally {
    fn new(maxhp: usize) -> Tally {
        let mut up = vec![0.0; maxhp];
        up[0] = 1.0;
        Tally {
            up,
            down: vec![0.0; maxhp],
            prefix: vec![0.0; maxhp],
            scratch: vec![0.0; maxhp],
            reach: 1,
        }
    }

    fn alive(&self) -> f64 {
        self.up[..self.reach].iter().sum::<f64>() + self.down[..self.reach].iter().sum::<f64>()
    }
}

impl Dist {
    /// The most damage one blow can add.
    fn most(&self) -> usize {
        let hi = self
            .ranges
            .iter()
            .filter(|r| r.weight > 0.0)
            .map(|r| r.hi)
            .max();
        (hi.unwrap_or(0) * self.mult) as usize
    }
}

/// Adds to `dst[x]`, for every `x` below `len`, the chance of reaching `x` from `src` with
/// `dist`'s positive damage: a sliding window over the prefix sums of `src` by stride `mult`.
fn add_positive(src: &[f64], dst: &mut [f64], prefix: &mut [f64], dist: &Dist, len: usize) {
    let m = dist.mult as usize;
    prefix[..m.min(len)].copy_from_slice(&src[..m.min(len)]);
    for i in m..len {
        prefix[i] = src[i] + prefix[i - m];
    }
    for r in dist.ranges.iter().filter(|r| r.weight > 0.0) {
        let w = r.weight / (r.hi - r.lo + 1) as f64;
        // `src[x - mult*v]` for `v` in `lo..=hi`: the prefix at `x - near` less the one at
        // `x - far`, the latter 0 below index 0.
        let (near, far) = (
            (dist.mult * r.lo) as usize,
            (dist.mult * (r.hi + 1)) as usize,
        );
        for x in near..far.min(len) {
            dst[x] += w * prefix[x - near];
        }
        for x in far..len {
            dst[x] += w * (prefix[x - near] - prefix[x - far]);
        }
    }
}

impl Fight {
    pub fn new(f: &Fighter, d: &Defender, mv: Move) -> Fight {
        let blow = Blow::new(f, d, mv, false);
        let knocks_down = mv == Move::Smash && f.str + f.level > d.level + d.magicres + 30;
        let down_blow = knocks_down.then(|| Blow::new(f, d, mv, true));
        let blows = blows_per_round(blow.cost, f.max_energy, ROUND_CAP);

        // Expected damage, the monster's hit points aside.
        let mut p_down = 0.0;
        let mut round_damage = [0.0; 2];
        for (r, &n) in blows.iter().take(2).enumerate() {
            for _ in 0..n {
                let down = down_blow.map_or(0.0, |b| b.expected_damage);
                round_damage[r] += p_down * down + (1.0 - p_down) * blow.expected_damage;
                if knocks_down {
                    p_down += (1.0 - p_down) * blow.p_damage();
                }
            }
        }

        let maxhp = d.maxhp.max(1) as usize;
        let mut t = Tally::new(maxhp);
        let mut alive_after = Vec::with_capacity(ROUND_CAP);
        let mut alive = 1.0;
        for &n in &blows {
            if alive >= DONE {
                for _ in 0..n {
                    t.strike(&blow, down_blow.as_ref());
                }
                alive = t.alive();
            }
            alive_after.push(alive);
        }

        let mut p_kill_by_round = [0.0; ROUNDS_REPORTED];
        for (p, a) in p_kill_by_round.iter_mut().zip(&alive_after) {
            *p = (1.0 - a).clamp(0.0, 1.0);
        }
        let mut per_round = [0; ROUNDS_REPORTED];
        per_round.copy_from_slice(&blows[..ROUNDS_REPORTED]);
        let expected_rounds = 1.0 + alive_after[..ROUND_CAP - 1].iter().sum::<f64>();
        Fight {
            blow,
            down_blow,
            blows_per_round: per_round,
            expected_damage_per_blow: blow.expected_damage,
            expected_damage_round1: round_damage[0],
            expected_damage_round2: round_damage[1],
            p_kill_by_round,
            expected_rounds,
            rounds_capped: alive_after[ROUND_CAP - 1] >= CAP_TAIL,
        }
    }
}

impl Tally {
    /// Folds one blow in. A knock-down blow sends what it damages to `down`.
    fn strike(&mut self, up_blow: &Blow, down_blow: Option<&Blow>) {
        let most = up_blow
            .dist
            .most()
            .max(down_blow.map_or(0, |b| b.dist.most()));
        let len = (self.reach + most).min(self.up.len());
        self.reach = len;
        let Tally {
            up,
            down,
            prefix,
            scratch,
            ..
        } = self;
        match down_blow {
            None => {
                let zero = up_blow.dist.zero;
                for (s, u) in scratch[..len].iter_mut().zip(&up[..len]) {
                    *s = zero * u;
                }
                add_positive(up, scratch, prefix, &up_blow.dist, len);
                std::mem::swap(up, scratch);
            }
            Some(db) => {
                // From knocked down: everything stays down.
                for (s, v) in scratch[..len].iter_mut().zip(&down[..len]) {
                    *s = db.dist.zero * v;
                }
                add_positive(down, scratch, prefix, &db.dist, len);
                // From up: a damaging blow knocks it down; a blow that does nothing leaves it.
                add_positive(up, scratch, prefix, &up_blow.dist, len);
                std::mem::swap(down, scratch);
                for u in &mut up[..len] {
                    *u *= up_blow.dist.zero;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
