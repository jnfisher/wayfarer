//! Keyed doors and gates (C-130): a Door or Gate whose `exitpar4` names a key is routed
//! through when the traveller holds that key, whatever its bash odds, and is bashed as before
//! without it. Against the pack built from `tests/fixtures/world` (`common::pack_path`): the
//! Rancid Sewer's gate east of 9/873 (`(2, -200, 0, 976)`, the skeleton key) on the way from the
//! Rhudaur Thieves' Guild to the Shadow Fist Temple, and the slums' Western Door (1/1098,
//! `(2, -60, 0, 569)`, the rusty iron key).

mod common;

use wayfarer_world::{Gate, Key, RoomGraph, Traveller, WorldPack, route};

const THIEVES_GUILD: Key = (2, 2561);
const MEDITATION_CHAMBER: Key = (9, 930);
const SEWER_BEFORE_GATE: Key = (9, 873);
const SEWER_PAST_GATE: Key = (9, 909);
const SKELETON_KEY: i64 = 976;
const SLUM_STREET: Key = (1, 1098);
const WESTERN_ROAD: Key = (1, 2339);
const RUSTY_IRON_KEY: i64 = 569;

fn graph() -> RoomGraph {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    RoomGraph::load(&pack).unwrap()
}

/// A character of strength `strength` with no picklocks, holding `keys`; every hidden exit
/// known (the guild's exit north is one), as a loop's path would name them.
fn traveller(strength: i64, keys: &[i64]) -> Traveller {
    let mut t = Traveller {
        strength: Some(strength),
        picklocks: Some(0),
        ..Traveller::default()
    };
    t.keys.extend(keys);
    t.allow.insert(wayfarer_world::Category::Hidden);
    t
}

#[test]
fn a_keyed_gate_is_a_key_gate_in_the_graph() {
    let g = graph();
    let e = g.edge(SEWER_BEFORE_GATE, 2).expect("the gate east");
    assert_eq!(e.gate, Gate::Key { item: SKELETON_KEY });
    assert!(e.door.is_some(), "still a door for the lock state");
    // A plain door keeps no gate.
    let plain = g.edge((9, 911), 2).expect("the door east of 9/911");
    assert_eq!(plain.gate, Gate::None);
}

#[test]
fn a_keyed_gate_is_routed_through_with_its_key() {
    let g = graph();
    let t = traveller(100, &[SKELETON_KEY]);
    let r = route(&g, THIEVES_GUILD, MEDITATION_CHAMBER, &t).expect("a route with the key");
    assert!(
        r.iter()
            .any(|s| s.from == SEWER_BEFORE_GATE && s.to == SEWER_PAST_GATE),
        "{r:?}"
    );
    assert_eq!(r.last().unwrap().to, MEDITATION_CHAMBER);
    // Certain: one door's worth, not a bash at strength minus 200.
    let e = g.edge(SEWER_BEFORE_GATE, 2).unwrap();
    assert_eq!(t.lock_odds(e), Some(100));
}

#[test]
fn a_keyed_gate_without_its_key_is_not_passable() {
    let g = graph();
    let t = traveller(100, &[]);
    let e = g.edge(SEWER_BEFORE_GATE, 2).unwrap();
    assert_eq!(t.lock_odds(e), Some(0));
    assert!(!t.can_take(e));
    assert!(route(&g, THIEVES_GUILD, MEDITATION_CHAMBER, &t).is_none());
    // Allowed every key as a category (the "anyone" of a reason): through, at the bash price.
    let mut anyone = traveller(100, &[]);
    anyone.allow.insert(wayfarer_world::Category::Key);
    assert_eq!(anyone.lock_odds(e), Some(0));
    assert!(anyone.can_take(e));
}

#[test]
fn a_keyed_door_a_strong_character_bashes_without_the_key() {
    let g = graph();
    let e = g.edge(SLUM_STREET, 3).expect("the Western Door");
    assert_eq!(
        e.gate,
        Gate::Key {
            item: RUSTY_IRON_KEY
        }
    );
    // Strength 130 against `exitpar2` -60: a bash lands 70 times in 100.
    let strong = traveller(130, &[]);
    assert_eq!(strong.lock_odds(e), Some(70));
    assert!(strong.can_take(e));
    assert!(route(&g, SLUM_STREET, WESTERN_ROAD, &strong).is_some());
    // Too weak, and no key: not a way.
    let weak = traveller(50, &[]);
    assert_eq!(weak.lock_odds(e), Some(0));
    assert!(!weak.can_take(e));
    // Weak, with the key: certain.
    let keyed = traveller(50, &[RUSTY_IRON_KEY]);
    assert_eq!(keyed.lock_odds(e), Some(100));
    assert!(keyed.can_take(e));
}

#[test]
fn a_keyed_gate_without_its_key_is_reached_by_buying_the_key() {
    use std::collections::{BTreeMap, BTreeSet};
    use wayfarer_world::{ExitItem, exit_items, route_buying};
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let g = RoomGraph::load(&pack).unwrap();
    let wanted: BTreeSet<i64> = g
        .all_edges()
        .iter()
        .filter_map(|e| match e.gate {
            Gate::Item { item } | Gate::Key { item } => Some(item),
            _ => None,
        })
        .collect();
    let items: BTreeMap<i64, ExitItem> = exit_items::load(&pack, &wanted).unwrap();
    let key = &items[&SKELETON_KEY];
    assert_eq!(key.name, "skeleton key");
    assert!(key.shops.contains(&THIEVES_GUILD), "{:?}", key.shops);
    let t = traveller(130, &[]);
    let (route, buy) = route_buying(
        &g,
        SEWER_BEFORE_GATE,
        MEDITATION_CHAMBER,
        &t,
        &items,
        |_| true,
    )
    .expect("a route by way of the guild");
    let buy = buy.expect("the key to buy");
    assert_eq!((buy.item, buy.shop), (SKELETON_KEY, THIEVES_GUILD));
    assert_eq!(route[0].from, SEWER_BEFORE_GATE);
    // Back up the sewer to the guild first, the gate after the purchase.
    assert_eq!((route[0].command.as_str(), route[0].to), ("n", (9, 872)));
    assert!(
        route
            .iter()
            .skip(buy.at)
            .any(|s| s.from == SEWER_BEFORE_GATE && s.to == SEWER_PAST_GATE),
        "{route:?}"
    );
}
