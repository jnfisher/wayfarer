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
    party_heals: Vec<SpellDef>,
}

/// A member of the caster's party with it, for the choice of a heal (the caster itself has
/// `name` None).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hurt<'a> {
    /// What to cast it at (the in-game first name), None for the caster.
    pub name: Option<&'a str>,
    pub hp: i32,
    pub missing: i64,
    /// The line it is healed below.
    pub line: i32,
    /// Below its flee line: healed first.
    pub fleeing: bool,
}

/// A heal to cast for the party: on one member, or the party heal.
#[derive(Clone, Debug, PartialEq)]
pub enum PartyCast<'a> {
    On(&'a SpellDef, String),
    Party(&'a SpellDef),
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
                SpellKind::PartyHeal => book.party_heals.push(def),
            }
        }
        book
    }

    #[cfg(test)]
    pub fn from_defs(defs: Vec<SpellDef>) -> Spellbook {
        let mut book = Spellbook::default();
        for d in defs {
            match d.kind {
                SpellKind::Attack => book.attacks.push(d),
                SpellKind::Heal => book.heals.push(d),
                SpellKind::PartyHeal => book.party_heals.push(d),
            }
        }
        book
    }

    pub fn has_attack(&self) -> bool {
        !self.attacks.is_empty()
    }

    /// A heal it can cast on itself.
    pub fn has_heal(&self) -> bool {
        self.heals.iter().any(|s| s.target != 0)
    }

    /// A heal it can cast on another player, or on the party.
    pub fn heals_others(&self) -> bool {
        !self.party_heals.is_empty() || self.heals.iter().any(|s| matches!(s.target, 0 | 2 | 6))
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
        Self::cheapest_cover(
            self.heals.iter().filter(|s| s.target != 0),
            level,
            mana,
            missing,
        )
    }

    /// The same, of the heals that can be cast on another player (target 0, 2 or 6).
    pub fn heal_other(&self, level: i64, mana: i64, missing: i64) -> Option<&SpellDef> {
        Self::cheapest_cover(
            self.heals.iter().filter(|s| matches!(s.target, 0 | 2 | 6)),
            level,
            mana,
            missing,
        )
    }

    /// The strongest party heal that can be paid for.
    pub fn party_heal(&self, level: i64, mana: i64) -> Option<&SpellDef> {
        self.party_heals
            .iter()
            .filter(|s| s.mana <= mana)
            .max_by(|a, b| a.average(level).total_cmp(&b.average(level)))
    }

    /// What to cast for the party (`docs/plan/10-party-play.md` §8): nothing unless another
    /// member is under its line. A member under its flee line gets the single heal first;
    /// otherwise, with `party` on, the party heal when two or more are under their lines or
    /// when what it heals over all of them (each up to what it is missing) beats the single
    /// heal on the worst; else the single heal on the other member furthest under its line.
    /// The caster's own heal stays its self-heal's.
    pub fn for_party<'b>(
        &'b self,
        level: i64,
        mana: i64,
        hurt: &[Hurt<'_>],
        party: bool,
    ) -> Option<PartyCast<'b>> {
        let under: Vec<&Hurt> = hurt.iter().filter(|h| h.hp < h.line).collect();
        let others = || under.iter().filter(|h| h.name.is_some());
        let worst = others().max_by_key(|h| (h.fleeing, h.line - h.hp))?;
        let single = |h: &Hurt| {
            self.heal_other(level, mana, h.missing)
                .map(|s| PartyCast::On(s, h.name.expect("another").to_owned()))
        };
        if worst.fleeing {
            return single(worst);
        }
        if party && let Some(p) = self.party_heal(level, mana) {
            let p_avg = p.average(level);
            let healed: f64 = under.iter().map(|h| p_avg.min(h.missing as f64)).sum();
            let one = self
                .heal_other(level, mana, worst.missing)
                .map_or(0.0, |s| s.average(level).min(worst.missing as f64));
            if under.len() >= 2 || healed > one {
                return Some(PartyCast::Party(p));
            }
        }
        single(worst)
    }

    fn cheapest_cover<'b>(
        heals: impl Iterator<Item = &'b SpellDef> + Clone,
        level: i64,
        mana: i64,
        missing: i64,
    ) -> Option<&'b SpellDef> {
        let affordable = || heals.clone().filter(|s| s.mana <= mana);
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

    fn priest() -> Spellbook {
        Spellbook::from_defs(vec![
            SpellDef::flat("mihe", "minor healing", SpellKind::Heal, 2, 2, 8),
            SpellDef::flat("rain", "healing rain", SpellKind::PartyHeal, 5, 7, 11),
        ])
    }

    fn hurt(name: Option<&str>, hp: i32, missing: i64) -> Hurt<'_> {
        Hurt {
            name,
            hp,
            missing,
            line: 50,
            fleeing: false,
        }
    }

    fn short(c: Option<PartyCast>) -> Option<(String, Option<String>)> {
        c.map(|c| match c {
            PartyCast::On(s, who) => (s.short.clone(), Some(who)),
            PartyCast::Party(s) => (s.short.clone(), None),
        })
    }

    #[test]
    fn one_hurt_member_gets_the_single_heal_two_the_rain() {
        let b = priest();
        let one = [hurt(Some("lauren"), 40, 4), hurt(Some("faye"), 90, 1)];
        assert_eq!(
            short(b.for_party(5, 20, &one, true)),
            Some(("mihe".into(), Some("lauren".into())))
        );
        let two = [hurt(Some("lauren"), 40, 4), hurt(Some("faye"), 45, 5)];
        assert_eq!(
            short(b.for_party(5, 20, &two, true)),
            Some(("rain".into(), None))
        );
        // Party heals off: the worst one gets the single heal.
        assert_eq!(
            short(b.for_party(5, 20, &two, false)),
            Some(("mihe".into(), Some("lauren".into())))
        );
    }

    #[test]
    fn the_rain_for_one_when_it_heals_more() {
        // 20 missing: the rain heals 9, the single heal 5.
        let b = priest();
        let one = [hurt(Some("lauren"), 30, 20)];
        assert_eq!(
            short(b.for_party(5, 20, &one, true)),
            Some(("rain".into(), None))
        );
    }

    #[test]
    fn a_member_under_its_flee_line_comes_first_and_alone() {
        let b = priest();
        let mut low = hurt(Some("faye"), 20, 30);
        low.fleeing = true;
        let hurt = [hurt(Some("lauren"), 40, 4), low, hurt(None, 30, 10)];
        assert_eq!(
            short(b.for_party(5, 20, &hurt, true)),
            Some(("mihe".into(), Some("faye".into())))
        );
    }

    #[test]
    fn only_the_caster_hurt_is_its_own_heals_business() {
        let b = priest();
        let me = [hurt(None, 20, 30), hurt(Some("lauren"), 90, 1)];
        assert_eq!(b.for_party(5, 20, &me, true), None);
        // Counted for the rain, though, with another hurt member.
        let both = [hurt(None, 20, 30), hurt(Some("lauren"), 40, 4)];
        assert_eq!(
            short(b.for_party(5, 20, &both, true)),
            Some(("rain".into(), None))
        );
    }

    #[test]
    fn a_heal_only_for_others_is_no_self_heal() {
        let b = Spellbook::from_defs(vec![{
            let mut d = SpellDef::flat("anno", "annointed hands", SpellKind::Heal, 1, 2, 4);
            d.target = 0;
            d
        }]);
        assert!(!b.has_heal() && b.heals_others());
        assert!(b.heal(5, 9, 3).is_none());
        assert_eq!(b.heal_other(5, 9, 3).unwrap().short, "anno");
    }

    #[test]
    fn a_book_without_spells_casts_nothing() {
        let b = Spellbook::default();
        assert!(!b.has_attack() && !b.has_heal());
        assert!(b.attack(1, 99).is_none() && b.heal(1, 99, 10).is_none());
    }
}
