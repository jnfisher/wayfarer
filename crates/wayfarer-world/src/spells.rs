//! The spells a player class can cast, from the pack: which are attacks and which heal, what
//! they cost and what they do at a level (`kb/spells.md`, "Spell record" and "Casting").

use std::collections::HashMap;

use serde_json::Value;

use crate::Error;
use crate::pack::{Spell, WorldPack};
use crate::qs120;

/// Ability numbers in a spell's `abilities` (`kb/spells.md`).
const ABILITY_DAMAGE: i64 = 17;
const ABILITY_HEAL: i64 = 18;
/// Descriptive Message: its value in `abilvals` is a message whose texts are the line shown
/// when the spell ends, nothing, and the line its target sees when it is cast.
const ABILITY_DESCRIPTIVE: i64 = 115;
/// Remove Spell: its value is a spell the cast removes (a regen tier removes the others).
const ABILITY_REMOVE_SPELL: i64 = 122;
/// Item abilities: One Time Cast (its value the spell `use` casts), Auto Use (used by hand, the
/// first one ends the list: what follows is the weapon's on-hit cast), Minimum Level.
const ABILITY_ONE_TIME_CAST: i64 = 43;
const ABILITY_AUTO_USE: i64 = 114;
const ABILITY_MIN_LEVEL: i64 = 135;
/// Item types (`kb/game-data.md`): armour (worn) and weapon.
const ITEM_ARMOUR: i64 = 0;
const ITEM_WEAPON: i64 = 1;
/// The game's worn locations, by `worn` (LJNTABLE.C:273): what `i` shows after a worn item.
const WORN_LOCATIONS: [&str; 20] = [
    "Nowhere", "Worn", "Head", "Hands", "Finger", "Feet", "Arms", "Back", "Neck", "Legs", "Waist",
    "Torso", "Off-Hand", "Finger", "Wrist", "Ears", "Worn", "Wrist", "Eyes", "Face",
];
/// The off-hand location: never worn with a two-handed weapon in hand.
const WORN_OFFHAND: i64 = 12;

/// A lasting spell a character can keep on itself or its party (C-047): a general spell (type
/// 3) with a duration, cast on the caster (target 1, 2, 6) or its party (10, 13).
#[derive(Clone, Debug, PartialEq)]
pub struct BuffDef {
    pub name: String,
    pub short: String,
    pub level: i64,
    pub mana: i64,
    pub magery: i64,
    pub magery_level: i64,
    pub target: i64,
    /// `duration`, `durinc`, `durlvls`, `durlvl`, `levelcap`.
    duration: (i64, i64, i64, i64),
    level_cap: i64,
    /// The line shown when it ends ("The effects of bless wear off!"), when it has one with no
    /// `%s` in it.
    pub wear_off: Option<String>,
    /// The line its target sees when it is cast ("You feel lucky!"), likewise; for a 1.20
    /// regen tier, the line the status screen shows while it is on.
    pub felt: Option<String>,
    /// The spell's number.
    pub number: i64,
    /// The spells a cast of it removes (ability 122).
    pub removes: Vec<i64>,
    /// On a Quicksilver 1.20 server, a spell of the regen line: its family and what it adds to
    /// mana regeneration.
    pub regen: Option<RegenTier>,
    /// A buff item's (C-109): the item whose `use` casts it, in place of a cast.
    pub item: Option<BuffItem>,
}

/// An item of unlimited uses whose `use` casts a lasting spell (C-109): wielded (a weapon) or
/// worn (anything else) to be used (LJNCMDB.C:191-214).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuffItem {
    pub number: i64,
    pub name: String,
    pub place: ItemPlace,
    /// The classes that may use it, by number; empty: any.
    pub classes: Vec<i64>,
    /// Its Minimum Level, 0 for none.
    pub min_level: i64,
}

/// Where a buff item has to be to be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemPlace {
    /// Wielded: `i` shows it `(Weapon Hand)`, or `(Two handed)` for a two-handed one.
    Weapon { two_handed: bool },
    /// Worn at this location, as `i` shows it (`Off-Hand`, `Neck` ...).
    Worn { location: String, off_hand: bool },
}

impl ItemPlace {
    /// Whether `slot`, the place `i` shows after a carried item, is this one.
    pub fn is(&self, slot: &str) -> bool {
        match self {
            ItemPlace::Weapon { .. } => is_weapon_slot(slot),
            ItemPlace::Worn { location, .. } => slot == location,
        }
    }
}

/// Whether `slot`, as `i` shows it, is the weapon hand.
pub fn is_weapon_slot(slot: &str) -> bool {
    slot == "Weapon Hand" || slot == "Two handed"
}

/// The off-hand location as `i` shows it.
pub const OFF_HAND: &str = "Off-Hand";

/// A regen-line spell's place in its family (`docs/kb/quicksilver-1.20.md` "Spells").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegenTier {
    pub family: qs120::Family,
    /// Added to Alter Mana Regeneration, per cent.
    pub regen_pct: i64,
}

impl BuffDef {
    /// How many 5-s rounds it lasts cast at `level` (`kb/spells.md` "Lasting spells"):
    /// `duration + durinc*(L/durlvls)`, `L` held to the level cap, at least `durlvl*L`; gear
    /// that alters spell durations is not counted.
    pub fn rounds(&self, level: i64) -> i64 {
        let (duration, inc, lvls, per_level) = self.duration;
        let l = if self.level_cap > 0 {
            level.min(self.level_cap)
        } else {
            level
        };
        let rounds = duration + if lvls > 0 { inc * (l / lvls) } else { 0 };
        rounds.max(per_level * l)
    }

    /// Cast at the party (no target word), not at the caster.
    pub fn on_party(&self) -> bool {
        matches!(self.target, 10 | 13)
    }

    /// A buff for tests: `rounds` long at any level.
    pub fn flat(short: &str, name: &str, mana: i64, rounds: i64) -> BuffDef {
        BuffDef {
            name: name.to_owned(),
            short: short.to_owned(),
            level: 1,
            mana,
            magery: 1,
            magery_level: 1,
            target: 2,
            duration: (rounds, 0, 1, 0),
            level_cap: 0,
            wear_off: Some(format!("The effects of {name} wear off!")),
            felt: None,
            number: 0,
            removes: Vec::new(),
            regen: None,
            item: None,
        }
    }

    /// Whether casting it removes `other` (1.20: a regen tier removes every other tier of its
    /// family, and the cast that removed one ends there).
    pub fn replaces(&self, other: &BuffDef) -> bool {
        other.number != 0 && self.removes.contains(&other.number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellKind {
    /// Auto-combat damage to one monster (type 0, target 0, 4 or 8).
    Attack,
    /// A heal on one player (type 3, target 0, 1, 2 or 6): the caster for 1, 2 and 6, another
    /// player for 0, 2 and 6 (`target` says which).
    Heal,
    /// A heal on the caster's whole party (type 3, target 10 or 13: healing rain, divine mercy).
    PartyHeal,
    /// Instant damage to every monster in the room (type 0, target 12, Full Attack Area:
    /// fireball, swarm; `kb/aoe-grinding.md` §1), C-105.
    Area,
}

/// A spell a player can learn, with what the choice between spells needs.
#[derive(Clone, Debug, PartialEq)]
pub struct SpellDef {
    pub name: String,
    pub short: String,
    pub kind: SpellKind,
    /// The character level the spell needs.
    pub level: i64,
    pub mana: i64,
    pub magery: i64,
    pub magery_level: i64,
    /// The record's target (`kb/spells.md` "Spell record": 0 Single, 1 Self, 2 Self or Single,
    /// 6 Any, 13 Full Party Area, ...).
    pub target: i64,
    min: (i64, i64, i64),
    max: (i64, i64, i64),
    level_cap: i64,
    /// How the line the caster sees for each cast starts, the spell's message up to its first
    /// `%` ("The fireball explodes, "), when that is long enough to know it by: an area spell's
    /// casts are counted by it (C-105).
    pub cast_line: Option<String>,
}

impl SpellDef {
    /// A spell with the given range, for tests and callers that do not read a pack: damage or
    /// heal from `lo` to `hi` at any level.
    pub fn flat(short: &str, name: &str, kind: SpellKind, mana: i64, lo: i64, hi: i64) -> SpellDef {
        SpellDef {
            name: name.to_owned(),
            short: short.to_owned(),
            kind,
            level: 1,
            mana,
            magery: 1,
            magery_level: 1,
            target: match kind {
                SpellKind::Attack => 0,
                SpellKind::Heal => 2,
                SpellKind::PartyHeal => 13,
                SpellKind::Area => 12,
            },
            min: (lo, 0, 0),
            max: (hi, 0, 0),
            level_cap: 0,
            cast_line: None,
        }
    }

    /// The same, with the start of the caster's line for each cast.
    pub fn with_cast_line(mut self, line: &str) -> SpellDef {
        self.cast_line = Some(line.to_owned());
        self
    }

    /// Whether `line` is the caster's line for a cast of this spell.
    pub fn is_cast_line(&self, line: &str) -> bool {
        self.cast_line
            .as_deref()
            .is_some_and(|start| line.trim_start().starts_with(start))
    }

    /// The middle of the spell's range for a caster of `level`: the range is uniform from
    /// `minval + mininc*L/minlvls` to `maxval + maxinc*L/maxlvls`, `L` held to the level cap,
    /// a minimum above the maximum lowered to it.
    pub fn average(&self, level: i64) -> f64 {
        let hi = amount_at(self.max, level, self.level_cap);
        let lo = amount_at(self.min, level, self.level_cap).min(hi);
        (lo + hi) as f64 / 2.0
    }
}

/// One bound of a spell's range for a caster of `level`: `base + inc*L/lvls`, `L` held to
/// `cap` (0: no cap).
fn amount_at((base, inc, lvls): (i64, i64, i64), level: i64, cap: i64) -> i64 {
    let l = if cap > 0 { level.min(cap) } else { level };
    if lvls > 0 {
        base + inc * l / lvls
    } else {
        base
    }
}

/// The start of a caster's message that knows the cast: up to the first `%`, past a leading
/// "You cast %s" with the spell's name put in.
fn cast_line_of(msg: &str, name: &str) -> String {
    let msg = msg.trim();
    let msg = match msg.strip_prefix("You cast %s") {
        Some(rest) => format!("You cast {name}{rest}"),
        None => msg.to_owned(),
    };
    msg.split('%').next().unwrap_or("").trim().to_owned()
}

/// A lasting spell a character can keep on itself or its party (C-047): a general spell with a
/// duration, on the caster (target 1, 2, 6) or its party (10, 13).
fn is_lasting(s: &Spell, v: &Value) -> bool {
    s.spell_type == 3
        && v.get("duration").and_then(Value::as_i64).unwrap_or(0) > 0
        && matches!(s.target, 1 | 2 | 6 | 10 | 13)
}

/// The record's list `k` (`abilities`, `abilvals`, `classes`).
fn ints(v: &Value, k: &str) -> Vec<i64> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

/// The lasting spell `s` (record `v`) as a blessing; `regen`, its place in 1.20's regen line.
fn buff_def(
    pack: &WorldPack,
    s: &Spell,
    v: &Value,
    regen: Option<&qs120::RegenSpell>,
) -> Result<BuffDef, Error> {
    let int = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
    let (abilities, abilvals) = (ints(v, "abilities"), ints(v, "abilvals"));
    let message = abilities
        .iter()
        .position(|&a| a == ABILITY_DESCRIPTIVE)
        .and_then(|i| abilvals.get(i).copied())
        .map(|n| pack.message(n))
        .transpose()?
        .flatten();
    let plain = |t: &str| {
        let t = t.trim();
        (!t.is_empty() && !t.contains('%')).then(|| t.to_owned())
    };
    let removes: Vec<i64> = abilities
        .iter()
        .zip(&abilvals)
        .filter(|(a, _)| **a == ABILITY_REMOVE_SPELL)
        .map(|(_, &v)| v)
        .filter(|&v| v > 0)
        .collect();
    // The new tiers have their family's texts; mana flux, nature tap, serenity and profane
    // link keep their stock ones.
    let texts = regen
        .filter(|r| r.number >= 6000)
        .and_then(|r| qs120::texts_of(r.family));
    Ok(BuffDef {
        name: s.name.clone(),
        short: s.short_name.clone(),
        level: s.level,
        mana: s.mana,
        magery: int("magery"),
        magery_level: int("magelvl"),
        target: s.target,
        duration: (
            int("duration"),
            int("durinc"),
            int("durlvls"),
            int("durlvl"),
        ),
        level_cap: int("levelcap"),
        wear_off: texts
            .map(|t| t.ends.clone())
            .or_else(|| message.as_ref().and_then(|m| plain(&m[0]))),
        // What the status screen shows while it is on (bless.rs reads it there).
        felt: texts
            .map(|t| t.status.clone())
            .or_else(|| message.as_ref().and_then(|m| plain(&m[2]))),
        number: s.number,
        removes,
        regen: regen.map(|r| RegenTier {
            family: r.family,
            regen_pct: r.regen_pct,
        }),
        item: None,
    })
}

/// The pack's buff items (C-109): weapons and worn items of unlimited charges whose `use`
/// casts a lasting spell, each as the blessing it gives, keyed by the item's name.
fn buff_items(pack: &WorldPack) -> Result<Vec<BuffDef>, Error> {
    let mut out = Vec::new();
    for item in pack
        .items_of_type(ITEM_ARMOUR)?
        .into_iter()
        .chain(pack.items_of_type(ITEM_WEAPON)?)
    {
        let Ok(v) = serde_json::from_str::<Value>(&item.raw) else {
            continue;
        };
        let int = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
        let worn = int("worn");
        if int("charges") >= 0 || (item.item_type == ITEM_ARMOUR && worn <= 0) {
            continue;
        }
        let (abilities, abilvals) = (ints(&v, "abilities"), ints(&v, "abilvals"));
        let pairs: Vec<(i64, i64)> = abilities.into_iter().zip(abilvals).collect();
        // Used by hand, the first Auto Use ends the list.
        let by_hand = pairs.iter().take_while(|(a, _)| *a != ABILITY_AUTO_USE);
        let Some(spell) = by_hand
            .clone()
            .find(|(a, _)| *a == ABILITY_ONE_TIME_CAST)
            .map(|&(_, n)| pack.spell(n))
            .transpose()?
            .flatten()
        else {
            continue;
        };
        let Ok(sv) = serde_json::from_str::<Value>(&spell.raw) else {
            continue;
        };
        if !is_lasting(&spell, &sv) {
            continue;
        }
        let place = if item.item_type == ITEM_WEAPON {
            ItemPlace::Weapon {
                two_handed: matches!(int("wpntype"), 1 | 3),
            }
        } else {
            let Some(location) = usize::try_from(worn)
                .ok()
                .and_then(|w| WORN_LOCATIONS.get(w))
            else {
                continue;
            };
            ItemPlace::Worn {
                location: (*location).to_owned(),
                off_hand: worn == WORN_OFFHAND,
            }
        };
        let mut def = buff_def(pack, &spell, &sv, None)?;
        def.name = format!("{} ({})", spell.name, item.name);
        def.short = item.name.clone();
        // On only from its own use: its lines may be another spell's (sword major bless has
        // bless's "You feel lucky!", which chant and bless from the party bring too).
        def.felt = None;
        def.item = Some(BuffItem {
            number: item.number,
            name: item.name.clone(),
            place,
            classes: ints(&v, "classes").into_iter().filter(|&c| c > 0).collect(),
            min_level: pairs
                .iter()
                .find(|(a, _)| *a == ABILITY_MIN_LEVEL)
                .map_or(0, |&(_, l)| l),
        });
        out.push(def);
    }
    Ok(out)
}

/// The player spells of the pack, and each class's magery.
#[derive(Clone, Debug, Default)]
pub struct Spells {
    spells: Vec<SpellDef>,
    /// The lasting spells a caster can keep on itself or its party.
    buffs: Vec<BuffDef>,
    /// Class name → (magery, magery level); 0 for a class that does not cast.
    classes: HashMap<String, (i64, i64)>,
    /// The buff items (C-109), keyed by the item's name.
    items: Vec<BuffDef>,
    /// Class name → number, for an item's class list.
    class_numbers: HashMap<String, i64>,
}

impl Spells {
    /// The pack's spells, as stock 1.11p has them.
    pub fn load(pack: &WorldPack) -> Result<Spells, Error> {
        Self::from_rows(pack, pack.spells()?, false)
    }

    /// The spells as a Quicksilver 1.20 server has them ([`qs120::spells`]): Spell Mod v2.1,
    /// protective aura and dark binding, the regen line; no monster copy.
    pub fn load_qs120(pack: &WorldPack) -> Result<Spells, Error> {
        Self::from_rows(pack, qs120::spells(pack)?, true)
    }

    fn from_rows(pack: &WorldPack, rows: Vec<Spell>, qs120: bool) -> Result<Spells, Error> {
        let mut spells = Vec::new();
        let mut buffs = Vec::new();
        for s in rows {
            let Ok(v) = serde_json::from_str::<Value>(&s.raw) else {
                continue;
            };
            let int = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
            let magery = int("magery");
            // Spells of no magery are the monsters' and the items'.
            if magery == 0 || s.short_name.is_empty() {
                continue;
            }
            // The spell's first ability is what it is. A heal that goes with a buff
            // (righteousness, blood ritual), one that works over time (regeneration, way of the
            // troll: a `duration` above 0), a resurrection (restoration: no amount) and one that
            // takes from the target (a negative amount) are not heals to cast between blows.
            let first = v
                .get("abilities")
                .and_then(Value::as_array)
                .and_then(|l| l.first())
                .and_then(Value::as_i64);
            let min = (int("minval"), int("mininc"), int("minlvls"));
            let max = (int("maxval"), int("maxinc"), int("maxlvls"));
            // The amount is judged at the level the spell is first cast: 1.20's maelstrom and
            // comet grow from a base of 0 (`min 0, +2 a level`), ember and prismatic blast from
            // below it, while restoration (level 0, 30 a level) and the item spells have none.
            let instant = int("duration") == 0 && amount_at(max, int("level"), int("levelcap")) > 0;
            if is_lasting(&s, &v) {
                // A regen-line spell (1.20 only): its lines are 1.20's own, not the pack's.
                let regen = qs120.then(|| qs120::regen_spell(s.number)).flatten();
                buffs.push(buff_def(pack, &s, &v, regen)?);
                continue;
            }
            let kind = match (s.spell_type, s.target) {
                (0, 0 | 4 | 8) if first == Some(ABILITY_DAMAGE) && instant => SpellKind::Attack,
                (3, 0 | 1 | 2 | 6) if first == Some(ABILITY_HEAL) && instant => SpellKind::Heal,
                // Found by what it does, never by its word: `rain` is also the Druid's acid
                // rain, a room attack (`docs/plan/10-party-play.md` §8).
                (3, 10 | 13) if first == Some(ABILITY_HEAL) && instant => SpellKind::PartyHeal,
                (0, 12) if first == Some(ABILITY_DAMAGE) && instant => SpellKind::Area,
                _ => continue,
            };
            // The caster's line, up to the first `%`: "The fireball explodes, %scausing %d
            // damage!" is known by "The fireball explodes,"; "You cast %s on %sthe room" by
            // "You cast swarm on", the name put in.
            let cast_line = match (kind, v.get("msgnum").and_then(Value::as_i64)) {
                (SpellKind::Area, Some(n)) if n > 0 => pack
                    .message(n)?
                    .map(|m| cast_line_of(&m[0], &s.name))
                    .filter(|l| l.len() >= 8),
                _ => None,
            };
            spells.push(SpellDef {
                name: s.name,
                short: s.short_name,
                kind,
                level: s.level,
                mana: s.mana,
                magery,
                magery_level: int("magelvl"),
                target: s.target,
                min,
                max,
                level_cap: int("levelcap"),
                cast_line,
            });
        }
        let classes = pack
            .class_magery()?
            .into_iter()
            .map(|(n, m, l)| (n, (m, l)))
            .collect();
        let class_numbers = pack
            .classes()?
            .into_iter()
            .map(|(n, name)| (name, n))
            .collect();
        Ok(Spells {
            spells,
            buffs,
            classes,
            items: buff_items(pack)?,
            class_numbers,
        })
    }

    /// Whether the class casts at all.
    pub fn casts(&self, class: &str) -> bool {
        self.classes.get(class).is_some_and(|&(m, _)| m > 0)
    }

    /// The lasting spell the `spells` screen listed by this short name and name, if the class
    /// can use it.
    pub fn buff(&self, class: &str, short: &str, name: &str) -> Option<&BuffDef> {
        let &(magery, magery_level) = self.classes.get(class)?;
        self.buffs.iter().find(|s| {
            s.short == short
                && s.name == name
                && s.magery == magery
                && s.magery_level <= magery_level
        })
    }

    /// The blessing the carried item `name` gives (C-109), if it is a buff item a character of
    /// `class` at `level` may use: its class list has the class, its Minimum Level is reached,
    /// and its spell's magery, if any, is the class's at a high enough magery level.
    pub fn item_buff(&self, class: &str, level: i64, name: &str) -> Option<&BuffDef> {
        let def = self
            .items
            .iter()
            .find(|d| d.short.eq_ignore_ascii_case(name))?;
        let item = def.item.as_ref()?;
        if !item.classes.is_empty()
            && !self
                .class_numbers
                .get(class)
                .is_some_and(|n| item.classes.contains(n))
        {
            return None;
        }
        if level < item.min_level {
            return None;
        }
        if def.magery != 0 {
            let &(magery, magery_level) = self.classes.get(class)?;
            if magery != def.magery || magery_level < def.magery_level {
                return None;
            }
        }
        Some(def)
    }

    /// Every buff item of the pack (C-109).
    pub fn buff_items(&self) -> &[BuffDef] {
        &self.items
    }

    /// The spell the `spells` screen listed by this short name and name, if it is an attack or
    /// a heal the class can use.
    pub fn find(&self, class: &str, short: &str, name: &str) -> Option<&SpellDef> {
        let &(magery, magery_level) = self.classes.get(class)?;
        self.spells.iter().find(|s| {
            s.short == short
                && s.name == name
                && s.magery == magery
                && s.magery_level <= magery_level
        })
    }
}
