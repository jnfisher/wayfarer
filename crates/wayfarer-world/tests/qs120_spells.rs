//! Quicksilver 1.20's spells (C-066) over the fixtures' pack: the 1.20 view replaces the
//! Mage, Priest and Druid spells, adds the regen line and leaves the monster copies out; the
//! 1.11p view is the pack's, untouched. The regen scrolls are sold at their shops.

mod common;

use wayfarer_world::qs120::{self, Family, MONSTER_COPIES};
use wayfarer_world::spellshop::{Learner, load, load_qs120, plan};
use wayfarer_world::{SpellKind, Spells, WorldPack};

fn pack() -> WorldPack {
    WorldPack::open(common::pack_path()).unwrap()
}

#[test]
fn the_1_20_view_replaces_the_stock_spells_and_adds_the_regen_line() {
    let pack = pack();
    let stock = Spells::load(&pack).unwrap();
    let qs = Spells::load_qs120(&pack).unwrap();
    // Magic missile: 4-12 flat on 1.11p, 6+L to 15+L capped at 9 on 1.20.
    let mm = stock.find("Mage", "mmis", "magic missile").unwrap();
    assert_eq!(mm.average(5), 8.0);
    let mm = qs.find("Mage", "mmis", "magic missile").unwrap();
    assert_eq!(mm.kind, SpellKind::Attack);
    assert_eq!(mm.average(5), (11 + 20) as f64 / 2.0);
    assert_eq!(mm.average(40), mm.average(9), "capped at 9");
    // The regen line: a level-20 Mage keeps arcane attunement up; it is no stock spell.
    let aatt = qs.buff("Mage", "aatt", "arcane attunement").unwrap();
    assert_eq!((aatt.level, aatt.mana, aatt.rounds(20)), (20, 8, 150));
    let tier = aatt.regen.unwrap();
    assert_eq!((tier.family, tier.regen_pct), (Family::Attunement, 100));
    assert_eq!(
        aatt.wear_off.as_deref(),
        Some("Your attunement to the mana fades.")
    );
    // What the status screen shows while it is on.
    assert_eq!(
        aatt.felt.as_deref(),
        Some("You are attuned to the flow of mana.")
    );
    assert!(stock.buff("Mage", "aatt", "arcane attunement").is_none());
    // Mana flux: level 16 on 1.11p, a fixed +150% at 35 on 1.20, which replaces the tiers.
    let flux = qs.buff("Mage", "flux", "mana flux").unwrap();
    assert_eq!((flux.level, flux.mana, flux.rounds(50)), (35, 12, 180));
    assert_eq!(flux.regen.unwrap().regen_pct, 150);
    let gatt = qs.buff("Mage", "gatt", "greater attunement").unwrap();
    assert!(gatt.replaces(aatt) && gatt.replaces(flux) && aatt.replaces(gatt));
    // Serenity and profane link don't remove each other.
    let serenity = qs.buff("Priest", "nity", "serenity").unwrap();
    let profane = qs.buff("Priest", "prfl", "profane link").unwrap();
    assert!(!serenity.replaces(profane) && !profane.replaces(serenity));
    let ldev = qs.buff("Priest", "ldev", "lesser devotion").unwrap();
    assert!(serenity.replaces(ldev) && ldev.replaces(serenity));
    // A half caster's tier: a Gypsy can use minor attunement, a Paladin no devotion.
    assert!(qs.buff("Gypsy", "mnat", "minor attunement").is_some());
    assert!(qs.buff("Paladin", "mndv", "minor devotion").is_none());
    assert!(qs.buff("Cleric", "mndv", "minor devotion").is_some());
    // The Bard's songs.
    let song = qs.buff("Bard", "reve", "song of reverie").unwrap();
    assert_eq!(song.regen.unwrap().family, Family::Song);
    assert_eq!(
        song.wear_off.as_deref(),
        Some("The melody in your mind fades.")
    );
}

#[test]
fn renamed_spells_and_the_monster_copies() {
    let pack = pack();
    let rows = qs120::spells(&pack).unwrap();
    let by = |n: i64| rows.iter().find(|s| s.number == n).unwrap();
    assert_eq!(by(24).name, "protective aura");
    assert_eq!(by(1056).name, "dark binding");
    assert!(rows.iter().all(|s| !MONSTER_COPIES.contains(&s.number)));
    // The stock pack's view keeps the old names.
    let stock = pack.spells().unwrap();
    assert_eq!(
        stock.iter().find(|s| s.number == 24).unwrap().name,
        "detect magic"
    );
    // The three spells outside the table are capped at 65.
    for n in qs120::CAPPED_ONLY {
        if let Some(s) = rows.iter().find(|s| s.number == n) {
            let v: serde_json::Value = serde_json::from_str(&s.raw).unwrap();
            let cap = v["levelcap"].as_i64().unwrap();
            assert!(cap > 0 && cap <= 65, "{n}: {cap}");
        }
    }
    // Every row of the table is in the view, with the table's level.
    for row in qs120::spell_rows() {
        let s = by(row.number);
        assert_eq!((s.name.as_str(), s.level), (row.name.as_str(), row.level));
    }
}

#[test]
fn the_regen_scrolls_are_sold_at_their_shops() {
    let pack = pack();
    let stock = load(&pack).unwrap();
    let qs = load_qs120(&pack).unwrap();
    assert!(!stock.iter().any(|o| o.spell >= 6000));
    let offer = |spell: &str| {
        qs.iter()
            .find(|o| o.spell_name == spell)
            .unwrap_or_else(|| panic!("{spell} is sold"))
    };
    let latt = offer("lesser attunement");
    assert_eq!(
        (latt.scroll.as_str(), latt.shop, latt.cost, latt.currency),
        ("scroll of lesser attunement", 48, 20, 2)
    );
    let rapture = offer("song of rapture");
    assert_eq!((rapture.cost, rapture.currency, rapture.shop), (50, 3, 93));
    // The shops' markups as the pack has them agree with the handoff's.
    for shop in qs120::regen_shops() {
        if let Some(o) = qs.iter().find(|o| o.shop == shop.shop) {
            assert_eq!(o.markup, shop.markup_pct, "{}", shop.name);
            assert!(!o.rooms.is_empty(), "{} has rooms", shop.name);
        }
    }
    // A Gypsy is offered minor attunement; a Paladin no devotion.
    let reach = |rooms: &[wayfarer_world::Key]| rooms.first().map(|&k| (1, k));
    let gypsy = Learner {
        magery: 1,
        magery_level: 2,
        level: 5,
        charm: None,
    };
    let items = plan(&qs, gypsy, &[], 10_000, reach);
    assert!(items.iter().any(|i| i.spell == "minor attunement"));
    let paladin = Learner {
        magery: 2,
        magery_level: 1,
        level: 50,
        charm: None,
    };
    let items = plan(&qs, paladin, &[], 10_000, reach);
    assert!(
        !items.iter().any(|i| i.spell.contains("devotion")),
        "{items:?}"
    );
    // Item 121 renamed, where a shop sells it.
    assert!(!qs.iter().any(|o| o.scroll == "scroll of detect magic"));
}
