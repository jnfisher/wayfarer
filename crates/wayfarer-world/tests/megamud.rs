//! MegaMud's loops as tasks see them, and what its paths know about exits, against the pack
//! built from `tests/fixtures/world` with the nine paths in `tests/fixtures/megamud`
//! (`common::pack_path`).

mod common;

use std::collections::{BTreeSet, HashSet};

use wayfarer_world::loops::{MmLoop, hunt_rooms, megamud_loops};
use wayfarer_world::{
    Area, ExitType, Key, Monsters, PathKnowledge, RoomGraph, Traveller, WorldPack,
};

/// The Chieftain's Large Wooden Room: a boss lair the Wooden Passages loop never enters.
const CHIEFTAIN: Key = (1, 1865);
/// Where `Mwodloop.mp` starts.
const WOODEN_START: Key = (1, 1875);
/// The Hidden Keep: a hidden exit north, searched for by `Ftrehkep.mp`.
const HIDDEN_KEEP: Key = (9, 724);
/// On the Keep Wall: `climb tree` up into the keep.
const KEEP_TREE: Key = (9, 735);
const NORTH: u8 = 0;
const UP: u8 = 8;

struct Mm {
    pack: WorldPack,
    graph: RoomGraph,
    monsters: Monsters,
    loops: Vec<MmLoop>,
}

fn mm() -> Mm {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let graph = RoomGraph::load(&pack).unwrap();
    let monsters = Monsters::load(&pack).unwrap();
    let loops = megamud_loops(&pack, &graph, &monsters).unwrap();
    Mm {
        pack,
        graph,
        monsters,
        loops,
    }
}

fn find<'a>(m: &'a Mm, file: &str) -> &'a MmLoop {
    m.loops
        .iter()
        .find(|l| l.path.file == file)
        .unwrap_or_else(|| panic!("{file} not among {:?}", m.loops.len()))
}

#[test]
fn the_fixture_has_eleven_paths_and_no_waypoints() {
    let m = mm();
    assert_eq!(m.pack.paths().unwrap().len(), 11);
    assert!(m.pack.waypoints().unwrap().is_empty());
    // The eight loops all verify; Ftrehkep.mp, Hkepleoq.mp and Leoqhkep.mp go from one place
    // to another.
    let files: Vec<&str> = m.loops.iter().map(|l| l.path.file.as_str()).collect();
    assert_eq!(
        files,
        [
            "Bosploop.mp",
            "CRSTLOOP.mp",
            "HKEPLOOP.mp",
            "Mwodloop.mp",
            "NARNLOO2.mp",
            "NARNLOO3.mp",
            "Narnloop.mp",
            "VLEFLOOP.mp"
        ]
    );
}

#[test]
fn the_wooden_passages_stops_are_its_hunt_rooms_in_path_order() {
    let m = mm();
    let l = find(&m, "Mwodloop.mp");
    assert_eq!(l.name(), "Wooden Passages");
    assert_eq!(l.rooms.first(), Some(&WOODEN_START));
    let stops = l.stops();
    assert_eq!(stops.first(), Some(&WOODEN_START));
    assert!(stops.len() > 30, "{}", stops.len());
    // None repeated, though the walk passes some rooms twice.
    let once: HashSet<Key> = stops.iter().copied().collect();
    assert_eq!(once.len(), stops.len());
    assert!(l.rooms.len() > stops.len());
    // In the order the walk first passes them.
    let first_at = |k: &Key| l.rooms.iter().position(|r| r == k).unwrap();
    assert!(stops.windows(2).all(|w| first_at(&w[0]) < first_at(&w[1])));
    // Every one a Wooden Passage that spawns something worth hunting; never the Chieftain's.
    let area = Area {
        anchor: WOODEN_START,
        radius: 30,
    };
    let hunt: BTreeSet<Key> = hunt_rooms(&m.graph, &m.monsters, area, &Traveller::anyone())
        .iter()
        .map(|h| h.key)
        .collect();
    for k in &stops {
        let name = &m.graph.room(*k).unwrap().name;
        assert!(name.starts_with("Wooden Passage"), "{k:?} {name}");
        assert!(hunt.contains(k), "{k:?}");
    }
    assert!(!l.rooms.contains(&CHIEFTAIN));
    assert!(!stops.contains(&CHIEFTAIN));
    assert_eq!(l.not_runnable(), None);
}

#[test]
fn the_bandit_keep_stops_are_in_the_courtyard_and_the_keep() {
    let m = mm();
    let l = find(&m, "HKEPLOOP.mp");
    assert_eq!(l.name(), "Bandit Keep");
    let names: BTreeSet<&str> = l
        .stops()
        .iter()
        .map(|k| m.graph.room(*k).unwrap().name.as_str())
        .collect();
    for want in [
        "Stone-Strewn Courtyard",
        "Courtyard, Entrance Door",
        "Inner Keep Entrance",
        "Hallway, Keep Entrance",
        "Living Quarters",
        "Keep Wall Path",
        "Guard Tower",
    ] {
        assert!(names.contains(want), "{want} not in {names:?}");
    }
    assert!(l.stops().iter().all(|k| k.0 == 9));
}

#[test]
fn a_loop_with_nothing_to_hunt_is_listed_but_not_runnable() {
    let m = mm();
    let l = find(&m, "Bosploop.mp");
    assert_eq!(l.name(), "Oasis Loop");
    assert!(l.stops().is_empty());
    assert!(l.rooms.len() > 10);
    assert_eq!(l.not_runnable(), Some("nothing to hunt on it"));
}

#[test]
fn the_newhaven_loops_stop_at_the_arena_and_the_lairs_behind_it() {
    let m = mm();
    let stops = |f: &str| find(&m, f).stops();
    assert_eq!(stops("Narnloop.mp"), [(1, 2150)]);
    assert_eq!(stops("NARNLOO2.mp"), [(1, 2150), (1, 2152)]);
    assert_eq!(stops("NARNLOO3.mp"), [(1, 2150), (1, 2152), (1, 2156)]);
}

#[test]
fn the_way_into_the_keep_searches_north_and_climbs_a_tree() {
    let m = mm();
    let k = PathKnowledge::load(&m.pack, &m.graph).unwrap();
    // Every path whose walk reached its end.
    assert_eq!(k.len(), 11);
    assert_eq!(
        k.hidden_exits("Ftrehkep.mp"),
        BTreeSet::from([(HIDDEN_KEEP, NORTH)])
    );
    // The way to Leo the Quick's room searches west at 9:819 and back east at 9:864; every
    // verified path's hidden exits together are what a custom loop's traveller may search for.
    const WEST: u8 = 3;
    const EAST: u8 = 2;
    assert_eq!(
        k.hidden_exits("Hkepleoq.mp"),
        BTreeSet::from([((9, 819), WEST)])
    );
    assert_eq!(
        k.hidden_exits("Leoqhkep.mp"),
        BTreeSet::from([((9, 864), EAST)])
    );
    assert!(k.all_hidden_exits().contains(&((9, 819), WEST)));
    let search = k
        .exits("Ftrehkep.mp")
        .iter()
        .find(|e| e.from == HIDDEN_KEEP)
        .unwrap();
    assert_eq!(
        (search.dir, search.kind, search.actions.as_slice()),
        (NORTH, ExitType::Hidden, ["search n".to_owned()].as_slice())
    );
    assert_eq!(k.phrases(KEEP_TREE, UP), ["climb tree"]);
    let climb = k
        .exits("Ftrehkep.mp")
        .iter()
        .find(|e| e.from == KEEP_TREE)
        .unwrap();
    assert_eq!(
        (climb.kind, climb.phrase.as_deref()),
        (ExitType::Text, Some("climb tree"))
    );
    // The loop inside the keep finds no hidden exit itself, and nothing is known of a path
    // not in the pack.
    assert!(k.hidden_exits("HKEPLOOP.mp").is_empty());
    // The Giant Efreetis loop searches twice: `se[search se]`, `nw[search nw]`.
    assert_eq!(
        k.hidden_exits("VLEFLOOP.mp"),
        BTreeSet::from([((16, 2153), 6), ((16, 2158), 5)])
    );
    assert!(k.exits("NOSUCH.mp").is_empty());
}

#[test]
fn a_text_exit_the_pack_names_keeps_its_own_phrase() {
    let m = mm();
    let k = PathKnowledge::load(&m.pack, &m.graph).unwrap();
    let mut g = RoomGraph::load(&m.pack).unwrap();
    // The stock pack's message for the tree has its phrases: nothing to learn.
    assert_eq!(g.learn_phrases(&k), 0);
    let e = g.edge(KEEP_TREE, UP).unwrap();
    assert_eq!(e.command, "climb tree");
}
