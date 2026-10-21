//! The spell catalogue read from the fixtures' pack.

mod common;

use wayfarer_world::{SpellKind, Spells, WorldPack};

#[test]
fn a_mage_has_magic_missile_and_a_warrior_casts_nothing() {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let spells = Spells::load(&pack).unwrap();
    assert!(spells.casts("Mage"));
    assert!(!spells.casts("Warrior"));
    let mmis = spells.find("Mage", "mmis", "magic missile").unwrap();
    assert_eq!(mmis.kind, SpellKind::Attack);
    assert_eq!(mmis.mana, 1);
    // 4 to 12 at level 1 (no increase): the middle is 8.
    assert_eq!(mmis.average(1), 8.0);
    // The rat's copy of the spell is no player's, and a Warrior has no magery for it.
    assert!(spells.find("Warrior", "mmis", "magic missile").is_none());
}

#[test]
fn minor_healing_is_a_heal_that_grows_with_the_level() {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let spells = Spells::load(&pack).unwrap();
    // Magery 2 is the clerical group; a Cleric (2, level 2) can use it, a Mage (1) cannot.
    let heal = spells.find("Cleric", "mihe", "minor healing").unwrap();
    assert_eq!(heal.kind, SpellKind::Heal);
    assert!(spells.find("Mage", "mihe", "minor healing").is_none());
    assert!(heal.average(10) > heal.average(1), "{heal:?}");
    // Held to the level cap (10): level 40 heals no more than level 10.
    assert_eq!(heal.average(40), heal.average(10));
}

#[test]
fn buffs_that_heal_heals_over_time_and_resurrection_are_not_heals_to_cast() {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let spells = Spells::load(&pack).unwrap();
    // righteousness and blood ritual (a buff first), regeneration (over time), restoration
    // (a resurrection); minor healing and godheal are.
    for (class, short, name) in [
        ("Cleric", "rite", "righteousness"),
        ("Cleric", "blud", "blood ritual"),
        ("Druid", "rgen", "regeneration"),
        ("Cleric", "rsto", "restoration"),
    ] {
        assert!(spells.find(class, short, name).is_none(), "{name}");
    }
    assert!(spells.find("Cleric", "mihe", "minor healing").is_some());
}

#[test]
fn a_flat_spell_averages_its_range() {
    let s = wayfarer_world::SpellDef::flat("x", "x", SpellKind::Attack, 2, 4, 10);
    assert_eq!(s.average(30), 7.0);
}
