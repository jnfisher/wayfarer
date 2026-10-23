//! The spells a player class can cast, from the pack: which are attacks and which heal, what
//! they cost and what they do at a level (`kb/spells.md`, "Spell record" and "Casting").

use std::collections::HashMap;

use serde_json::Value;

use crate::Error;
use crate::pack::WorldPack;

/// Ability numbers in a spell's `abilities` (`kb/spells.md`).
const ABILITY_DAMAGE: i64 = 17;
const ABILITY_HEAL: i64 = 18;
/// Descriptive Message: its value in `abilvals` is a message whose texts are the line shown
/// when the spell ends, nothing, and the line its target sees when it is cast.
const ABILITY_DESCRIPTIVE: i64 = 115;

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
    /// The line its target sees when it is cast ("You feel lucky!"), likewise.
    pub felt: Option<String>,
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
        }
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
            },
            min: (lo, 0, 0),
            max: (hi, 0, 0),
            level_cap: 0,
        }
    }

    /// The middle of the spell's range for a caster of `level`: the range is uniform from
    /// `minval + mininc*L/minlvls` to `maxval + maxinc*L/maxlvls`, `L` held to the level cap,
    /// a minimum above the maximum lowered to it.
    pub fn average(&self, level: i64) -> f64 {
        let l = if self.level_cap > 0 {
            level.min(self.level_cap)
        } else {
            level
        };
        let at = |(base, inc, lvls): (i64, i64, i64)| {
            if lvls > 0 {
                base + inc * l / lvls
            } else {
                base
            }
        };
        let hi = at(self.max);
        let lo = at(self.min).min(hi);
        (lo + hi) as f64 / 2.0
    }
}

/// The player spells of the pack, and each class's magery.
#[derive(Clone, Debug, Default)]
pub struct Spells {
    spells: Vec<SpellDef>,
    /// The lasting spells a caster can keep on itself or its party.
    buffs: Vec<BuffDef>,
    /// Class name → (magery, magery level); 0 for a class that does not cast.
    classes: HashMap<String, (i64, i64)>,
}

impl Spells {
    pub fn load(pack: &WorldPack) -> Result<Spells, Error> {
        let mut spells = Vec::new();
        let mut buffs = Vec::new();
        for s in pack.spells()? {
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
            let instant = int("duration") == 0 && int("minval") > 0;
            if s.spell_type == 3 && int("duration") > 0 && matches!(s.target, 1 | 2 | 6 | 10 | 13) {
                let ints = |k: &str| -> Vec<i64> {
                    v.get(k)
                        .and_then(Value::as_array)
                        .map(|l| l.iter().filter_map(Value::as_i64).collect())
                        .unwrap_or_default()
                };
                let message = ints("abilities")
                    .iter()
                    .position(|&a| a == ABILITY_DESCRIPTIVE)
                    .and_then(|i| ints("abilvals").get(i).copied())
                    .map(|n| pack.message(n))
                    .transpose()?
                    .flatten();
                let plain = |t: &str| {
                    let t = t.trim();
                    (!t.is_empty() && !t.contains('%')).then(|| t.to_owned())
                };
                buffs.push(BuffDef {
                    name: s.name.clone(),
                    short: s.short_name.clone(),
                    level: s.level,
                    mana: s.mana,
                    magery,
                    magery_level: int("magelvl"),
                    target: s.target,
                    duration: (
                        int("duration"),
                        int("durinc"),
                        int("durlvls"),
                        int("durlvl"),
                    ),
                    level_cap: int("levelcap"),
                    wear_off: message.as_ref().and_then(|m| plain(&m[0])),
                    felt: message.as_ref().and_then(|m| plain(&m[2])),
                });
                continue;
            }
            let kind = match (s.spell_type, s.target) {
                (0, 0 | 4 | 8) if first == Some(ABILITY_DAMAGE) && instant => SpellKind::Attack,
                (3, 0 | 1 | 2 | 6) if first == Some(ABILITY_HEAL) && instant => SpellKind::Heal,
                // Found by what it does, never by its word: `rain` is also the Druid's acid
                // rain, a room attack (`docs/plan/10-party-play.md` §8).
                (3, 10 | 13) if first == Some(ABILITY_HEAL) && instant => SpellKind::PartyHeal,
                _ => continue,
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
                min: (int("minval"), int("mininc"), int("minlvls")),
                max: (int("maxval"), int("maxinc"), int("maxlvls")),
                level_cap: int("levelcap"),
            });
        }
        let classes = pack
            .class_magery()?
            .into_iter()
            .map(|(n, m, l)| (n, (m, l)))
            .collect();
        Ok(Spells {
            spells,
            buffs,
            classes,
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
