//! Exits that need a bought item (`exit_items`, `route_buying`), against the pack built from
//! `tests/fixtures/world` (`common::pack_path`): the Homely Hearth's stairs in Silvermere take a
//! room ticket, which its common room sells.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use wayfarer_world::{
    ExitItem, Gate, Key, RoomGraph, Traveller, WorldPack, distances, distances_buying, exit_items,
    route_buying,
};

const BANK: Key = (1, 297);
const STAIRWELL: Key = (1, 313);
const UPSTAIRS: Key = (1, 847);
const BARDS_ROOM: Key = (1, 851);
const ROOM_TICKET: i64 = 924;
const ROPE: i64 = 191;

fn world() -> (RoomGraph, BTreeMap<i64, ExitItem>) {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let graph = RoomGraph::load(&pack).unwrap();
    let wanted: BTreeSet<i64> = graph
        .all_edges()
        .iter()
        .filter_map(|e| match e.gate {
            Gate::Item { item } | Gate::Key { item } => Some(item),
            _ => None,
        })
        .collect();
    let items = exit_items::load(&pack, &wanted).unwrap();
    (graph, items)
}

/// A level 5 character, as the brain plans for one.
fn level_five() -> Traveller {
    Traveller {
        level: Some(5),
        ..Traveller::default()
    }
}

#[test]
fn the_room_ticket_is_sold_in_the_common_room_and_used_up() {
    let (_, items) = world();
    let ticket = &items[&ROOM_TICKET];
    assert_eq!(ticket.name, "room ticket");
    assert!(ticket.used_up());
    assert!(ticket.shops.contains(&(1, 311)), "{:?}", ticket.shops);
    // A rope and grapple is not used up.
    assert!(!items[&ROPE].used_up());
}

#[test]
fn a_walk_upstairs_buys_a_ticket_in_the_common_room_first() {
    let (graph, items) = world();
    let t = level_five();
    let (route, buy) = route_buying(&graph, BANK, BARDS_ROOM, &t, &items, |_| true).unwrap();
    let buy = buy.expect("a ticket to buy");
    assert_eq!((buy.item, buy.name.as_str()), (ROOM_TICKET, "room ticket"));
    assert!(items[&ROOM_TICKET].shops.contains(&buy.shop));
    // The route reaches the shop at `at`, then goes up the stairs.
    assert_eq!(route[buy.at].from, buy.shop);
    let stairs = route
        .iter()
        .position(|s| s.from == STAIRWELL && s.to == UPSTAIRS)
        .expect("up the stairs");
    assert!(stairs >= buy.at, "{route:?}");
    assert_eq!(route.last().unwrap().to, BARDS_ROOM);
}

#[test]
fn nothing_is_bought_with_a_ticket_carried() {
    let (graph, items) = world();
    let mut t = level_five();
    t.items.insert(ROOM_TICKET);
    let (route, buy) = route_buying(&graph, BANK, BARDS_ROOM, &t, &items, |_| true).unwrap();
    assert_eq!(buy, None);
    assert_eq!(route.last().unwrap().to, BARDS_ROOM);
}

#[test]
fn a_walk_that_needs_no_item_buys_nothing() {
    let (graph, items) = world();
    let (route, buy) =
        route_buying(&graph, BANK, (1, 301), &level_five(), &items, |_| true).unwrap();
    assert_eq!(buy, None);
    assert_eq!(route.len(), 1);
}

#[test]
fn rooms_behind_a_bought_item_count_as_reachable() {
    let (graph, items) = world();
    let t = level_five();
    assert!(!distances(&graph, BANK, 10_000, &t).contains_key(&BARDS_ROOM));
    assert!(distances_buying(&graph, BANK, 10_000, &t, &items).contains_key(&BARDS_ROOM));
}
