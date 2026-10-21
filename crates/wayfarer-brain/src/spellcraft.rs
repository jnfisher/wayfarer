//! What a caster knows and which spell to cast: the spellbook the `spells` screen listed,
//! matched to the pack's attacks and heals, and the choices over it (`kb/spells.md`).

use wayfarer_world::{SpellDef, SpellKind, Spells};

/// One row of the `spells` screen: level, mana, short name, name.
pub type SpellRow = (i64, i64, String, String);

/// The attack and heal spells a character can cast now, with the mana the game itself listed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spellbook {
    attacks: Vec<SpellDef>,
    heals: Vec<SpellDef>,
}

impl Spellbook {
    /// The rows of the `spells` screen matched to the pack; a spell the pack does not know as
    /// an attack or a heal of the class (buffs, light, area spells) is left out.
    pub fn learn(spells: &Spells, class: &str, rows: &[SpellRow]) -> Spellbook {
        let mut book = Spellbook::default();
        for (_, mana, short, name) in rows {
            let Some(def) = spells.find(class, short, name) else {
                continue;
            };
            let mut def = def.clone();
            def.mana = *mana;
            match def.kind {
                SpellKind::Attack => book.attacks.push(def),
                SpellKind::Heal => book.heals.push(def),
            }
        }
        book
    }

    #[cfg(test)]
    pub fn from_defs(defs: Vec<SpellDef>) -> Spellbook {
        let (attacks, heals) = defs.into_iter().partition(|d| d.kind == SpellKind::Attack);
        Spellbook { attacks, heals }
    }

    pub fn has_attack(&self) -> bool {
        !self.attacks.is_empty()
    }

    pub fn has_heal(&self) -> bool {
        !self.heals.is_empty()
    }

    /// The attack to cast with `mana` at `level`: the one that does the most damage of those
    /// that can be paid for, the cheaper of two that do the same.
    pub fn attack(&self, level: i64, mana: i64) -> Option<&SpellDef> {
        self.attacks
            .iter()
            .filter(|s| s.mana <= mana)
            .max_by(|a, b| {
                a.average(level)
                    .total_cmp(&b.average(level))
                    .then(b.mana.cmp(&a.mana))
            })
    }

    /// The heal to cast with `mana` at `level` when `missing` hit points are gone: the
    /// cheapest that heals them all, or the strongest there is when none does.
    pub fn heal(&self, level: i64, mana: i64, missing: i64) -> Option<&SpellDef> {
        let affordable = || self.heals.iter().filter(|s| s.mana <= mana);
        affordable()
            .filter(|s| s.average(level) >= missing as f64)
            .min_by_key(|s| s.mana)
            .or_else(|| affordable().max_by(|a, b| a.average(level).total_cmp(&b.average(level))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Spellbook {
        Spellbook::from_defs(vec![
            SpellDef::flat("mmis", "magic missile", SpellKind::Attack, 1, 4, 12),
            SpellDef::flat("lbolt", "lightning bolt", SpellKind::Attack, 6, 20, 40),
            SpellDef::flat("mihe", "minor healing", SpellKind::Heal, 2, 2, 8),
            SpellDef::flat("mend", "mend", SpellKind::Heal, 6, 15, 25),
        ])
    }

    #[test]
    fn the_strongest_attack_that_can_be_paid_for() {
        let b = book();
        assert_eq!(b.attack(5, 20).unwrap().short, "lbolt");
        // Five mana pays for the missile only.
        assert_eq!(b.attack(5, 5).unwrap().short, "mmis");
        assert!(b.attack(5, 0).is_none());
    }

    #[test]
    fn the_cheapest_heal_that_covers_the_damage() {
        let b = book();
        // 4 missing: minor healing (average 5) covers it.
        assert_eq!(b.heal(5, 20, 4).unwrap().short, "mihe");
        // 12 missing: only mend (20) does.
        assert_eq!(b.heal(5, 20, 12).unwrap().short, "mend");
        // 40 missing: none covers it, the strongest goes.
        assert_eq!(b.heal(5, 20, 40).unwrap().short, "mend");
        // Short of mana for mend, the small heal is better than none.
        assert_eq!(b.heal(5, 3, 12).unwrap().short, "mihe");
        assert!(b.heal(5, 1, 12).is_none());
    }

    #[test]
    fn a_book_without_spells_casts_nothing() {
        let b = Spellbook::default();
        assert!(!b.has_attack() && !b.has_heal());
        assert!(b.attack(1, 99).is_none() && b.heal(1, 99, 10).is_none());
    }
}
