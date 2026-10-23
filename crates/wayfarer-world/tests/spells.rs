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

#[test]
fn healing_rain_is_a_party_heal_and_acid_rain_is_no_heal() {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let spells = Spells::load(&pack).unwrap();
    // Found by the record (type 3, target 13 Full Party Area, a heal), not the word `rain`.
    let rain = spells.find("Priest", "rain", "healing rain").unwrap();
    assert_eq!((rain.kind, rain.target), (SpellKind::PartyHeal, 13));
    assert!(spells.find("Druid", "rain", "acid rain").is_none());
    // Minor healing can be cast on another player (target 2, Self or Single).
    assert_eq!(
        spells
            .find("Cleric", "mihe", "minor healing")
            .unwrap()
            .target,
        2
    );
}

#[test]
fn lasting_spells_carry_their_duration_and_wear_off_line() {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let spells = Spells::load(&pack).unwrap();
    // bless: 40 rounds and 5 a level (one level a step), up to level 20.
    let bless = spells.buff("Cleric", "bles", "bless").unwrap();
    assert_eq!(bless.mana, 4);
    assert_eq!(bless.rounds(1), 45);
    assert_eq!(bless.rounds(10), 90);
    assert_eq!(bless.rounds(40), bless.rounds(20));
    assert_eq!(
        bless.wear_off.as_deref(),
        Some("The effects of bless wear off!")
    );
    assert_eq!(bless.felt.as_deref(), Some("You feel lucky!"));
    assert!(!bless.on_party());
    // holy aura is cast on the party.
    let aura = spells.buff("Priest", "aura", "holy aura").unwrap();
    assert!(aura.on_party());
    assert_eq!(
        aura.wear_off.as_deref(),
        Some("You no longer feel safe from evil!")
    );
    // barkskin is the Druids' (magery 3), not a Cleric's.
    assert!(spells.buff("Druid", "skin", "barkskin").is_some());
    assert!(spells.buff("Cleric", "skin", "barkskin").is_none());
    // An attack is no lasting spell.
    assert!(spells.buff("Mage", "mmis", "magic missile").is_none());
}
