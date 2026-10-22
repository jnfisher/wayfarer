//! The scrolls shops sell, read from the fixtures' pack (`kb/spells.md` "Learning").

mod common;

use wayfarer_world::WorldPack;
use wayfarer_world::spellshop::{Learner, load, plan};

#[test]
fn the_newhaven_spell_shop_sells_mend_and_a_mage_is_offered_what_it_lacks() {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let offers = load(&pack).unwrap();
    assert!(!offers.is_empty());
    let mend = offers
        .iter()
        .find(|o| o.spell_name == "mend" && o.shop_name == "Newhaven Spell Shop")
        .expect("mend at the Newhaven Spell Shop");
    // 8 gold, no markup, a shop room to walk to.
    assert_eq!((mend.cost, mend.currency, mend.markup), (8, 2, 0));
    assert!(!mend.rooms.is_empty());
    // A mage knowing nothing is offered magic missile, free, and not the priests' mend.
    let mage = Learner {
        magery: 1,
        magery_level: 2,
        level: 3,
        charm: Some(50),
    };
    let items = plan(&offers, mage, &[], 0, |rooms| {
        rooms.first().map(|&k| (1, k))
    });
    let names: Vec<_> = items.iter().map(|i| i.spell.as_str()).collect();
    assert!(names.contains(&"magic missile"), "{names:?}");
    let missile = items.iter().find(|i| i.spell == "magic missile").unwrap();
    assert_eq!(
        (missile.price, missile.usable, missile.affordable),
        (0, true, true)
    );
    // Knowing it removes it.
    let known = ["Magic Missile".to_owned()];
    let items = plan(&offers, mage, &known, 0, |rooms| {
        rooms.first().map(|&k| (1, k))
    });
    assert!(!items.iter().any(|i| i.spell == "magic missile"));
}
