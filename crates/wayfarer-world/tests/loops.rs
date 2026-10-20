//! Generated loops and the MegaMud loop filter, against the pack built from
//! `tests/fixtures/world` (`common::pack_path`). The fixture pack has no MegaMud data, so the
//! filter is checked on made-up loops here and on the real path set in `pack.rs`'s ignored
//! `megamud_paths_verify`.

mod common;

use std::collections::{BTreeSet, HashMap};

use wayfarer_world::loops::{Area, Loop, MmLoop, generate, hunt_rooms, megamud_through, verified};
use wayfarer_world::pack::{MmPath, Verification};
use wayfarer_world::{ExitType, Gate, Key, Monsters, RoomGraph, Traveller, WorldPack};

const VILLAGE_ENTRANCE: Key = (1, 2140);
const ARENA: Key = (1, 2150);
/// North of the Arena through its locked door: a Lair.
const DUNGEON_ENTRANCE: Key = (1, 2152);
/// North of the Dungeon Entrance: a Lair (cave bears).
const SMALL_CAVERN: Key = (1, 2156);
/// The Healer, west of the Narrow Road through a Level exit (1 to 3).
const HEALER: Key = (1, 2190);
/// Newhaven's shops, the Adventurer's Guild and the Healer: NPCs only.
const SHOPS: [Key; 6] = [
    (1, 2141),
    (1, 2142),
    (1, 2144),
    (1, 2145),
    (1, 2147),
    HEALER,
];

const NEWHAVEN: Area = Area {
    anchor: VILLAGE_ENTRANCE,
    radius: 8,
};

fn world() -> (RoomGraph, Monsters) {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    (
        RoomGraph::load(&pack).unwrap(),
        Monsters::load(&pack).unwrap(),
    )
}

fn level(n: i64) -> Traveller {
    Traveller {
        level: Some(n),
        ..Traveller::default()
    }
}

/// A closed walk: each step leaves the room the last one reached, by an exit the traveller may
/// take, from the first stop back to it, passing every stop.
fn assert_closes(g: &RoomGraph, l: &Loop, t: &Traveller) {
    let Some(first) = l.steps.first() else {
        assert_eq!(l.rooms.len(), 1, "{l:?}");
        return;
    };
    assert_eq!(first.from, l.rooms[0]);
    assert_eq!(l.steps.last().unwrap().to, l.rooms[0]);
    for w in l.steps.windows(2) {
        assert_eq!(w[0].to, w[1].from, "{l:?}");
    }
    for s in &l.steps {
        let e = g.edge(s.from, s.dir).unwrap();
        assert_eq!(e.to, s.to);
        assert!(t.can_take(e), "{s:?}");
    }
    let passed: BTreeSet<Key> = l.steps.iter().map(|s| s.to).collect();
    assert!(l.rooms.iter().all(|k| passed.contains(k)), "{l:?}");
    assert_eq!(l.length, l.steps.len());
    assert_eq!(l.spawns, l.rooms.len());
}

#[test]
fn the_newhaven_loop_visits_every_hunt_room_and_closes() {
    let (g, ms) = world();
    let t = level(1);
    let hunt = hunt_rooms(&g, &ms, NEWHAVEN, &t);
    let keys: Vec<Key> = hunt.iter().map(|h| h.key).collect();
    assert_eq!(keys, [ARENA, DUNGEON_ENTRANCE, SMALL_CAVERN]);
    let arena = &hunt[0];
    assert!(arena.is_arena && !arena.is_lair, "{arena:?}");
    assert!(arena.monsters.iter().any(|(_, n)| n == "giant rat"));
    assert!(hunt[1].is_lair && hunt[2].is_lair);
    assert_eq!(hunt[2].monsters.len(), 1);
    assert_eq!(hunt[2].monsters[0].1, "cave bear");
    // The shops are within the radius but are no stops.
    let area = NEWHAVEN.rooms(&g, &t);
    assert!(SHOPS.iter().all(|k| area.contains_key(k)));
    assert!(SHOPS.iter().all(|k| !keys.contains(k)));

    let loops = generate(&g, &ms, NEWHAVEN, &t);
    assert_eq!(loops.len(), 1, "{loops:?}");
    let l = &loops[0];
    assert_eq!(l.name, "Newhaven");
    assert_eq!(l.rooms, [ARENA, DUNGEON_ENTRANCE, SMALL_CAVERN]);
    assert_closes(&g, l, &t);
    assert!(l.length < 40, "{}", l.length);
    let commands: Vec<&str> = l.steps.iter().map(|s| s.command.as_str()).collect();
    assert_eq!(commands, ["n", "n", "s", "s"]);
    // Through the Arena's door both ways.
    assert_eq!(l.steps.iter().filter(|s| s.door.is_some()).count(), 1);
}

#[test]
fn without_doors_the_lair_behind_the_arena_is_no_stop() {
    let (g, ms) = world();
    let t = Traveller {
        no_doors: true,
        ..level(1)
    };
    let hunt = hunt_rooms(&g, &ms, NEWHAVEN, &t);
    assert_eq!(hunt.iter().map(|h| h.key).collect::<Vec<_>>(), [ARENA]);
    let loops = generate(&g, &ms, NEWHAVEN, &t);
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].rooms, [ARENA]);
    assert!(loops[0].steps.is_empty());
}

#[test]
fn a_gated_exit_outside_the_level_is_not_used() {
    let (g, ms) = world();
    // Level 2 passes the Healer's exit (1 to 3); level 5 does not.
    assert!(NEWHAVEN.rooms(&g, &level(2)).contains_key(&HEALER));
    let t = level(5);
    assert!(!NEWHAVEN.rooms(&g, &t).contains_key(&HEALER));

    // Farther out, the river's way back to the island is a Level exit (up to 3): at level 5
    // the river is a part of its own, and no loop over everything closes.
    let wide = Area {
        anchor: VILLAGE_ENTRANCE,
        radius: 16,
    };
    let gated: Vec<_> = g
        .all_edges()
        .iter()
        .filter(|e| e.kind == ExitType::Level && !t.can_take(e))
        .collect();
    assert!(
        gated
            .iter()
            .any(|e| e.from == (1, 2363) && matches!(e.gate, Gate::Level { max: 3, .. }))
    );
    let loops = generate(&g, &ms, wide, &t);
    let names: Vec<&str> = loops.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Newhaven: Newhaven, Arena",
            "Newhaven: The Silver River, Silvermere Docks"
        ]
    );
    for l in &loops {
        assert_closes(&g, l, &t);
        assert!(
            l.steps
                .iter()
                .all(|s| !gated.iter().any(|e| e.from == s.from && e.dir == s.dir)),
            "{l:?}"
        );
    }
    assert_eq!(loops[0].rooms, [ARENA, DUNGEON_ENTRANCE, SMALL_CAVERN]);
    assert_eq!(loops[1].spawns, 15);

    // At level 1 the river and the island are one area: one loop over all 18 rooms.
    let t = level(1);
    let loops = generate(&g, &ms, wide, &t);
    assert_eq!(loops.len(), 1, "{loops:?}");
    assert_eq!(loops[0].name, "Newhaven");
    assert_eq!(loops[0].spawns, 18);
    assert_closes(&g, &loops[0], &t);
}

fn mm_loop(file: &str, walked: bool, closes: bool, rooms: &[Key]) -> MmLoop {
    MmLoop {
        path: MmPath {
            file: file.into(),
            description: String::new(),
            author: String::new(),
            from_id: 1,
            to_id: 1,
            is_loop: true,
            item: String::new(),
            prepath: String::new(),
            nextpath: String::new(),
            verification: Verification {
                start_candidates: 1,
                start: Some(rooms[0]),
                end: Some(*rooms.last().unwrap()),
                walked,
                closes,
                steps_checked: rooms.len() as i64,
                steps_exact: rooms.len() as i64,
                steps_title: 0,
                steps_none: 0,
                steps_wildcard: 0,
                broke_at: (!walked).then_some(1),
                broke_reason: None,
            },
        },
        rooms: rooms.to_vec(),
    }
}

#[test]
fn megamud_loops_through_the_area_are_verified_ones() {
    let (g, _) = world();
    let area: HashMap<Key, u32> = NEWHAVEN.rooms(&g, &level(1));
    let loops = [
        mm_loop("ARENA.mp", true, true, &[ARENA, DUNGEON_ENTRANCE, ARENA]),
        mm_loop("BROKEN.mp", false, false, &[ARENA, DUNGEON_ENTRANCE]),
        mm_loop("OPEN.mp", true, false, &[ARENA, DUNGEON_ENTRANCE]),
        mm_loop("FAR.mp", true, true, &[(1, 1), (1, 2), (1, 1)]),
    ];
    let listed: Vec<&str> = megamud_through(&loops, &area)
        .iter()
        .map(|l| l.path.file.as_str())
        .collect();
    assert_eq!(listed, ["ARENA.mp"]);
    assert!(loops.iter().filter(|l| verified(&l.path)).count() == 2);
}

/// The Hidden Keep in the Darkwood Forest: its east door into the courtyard is locked with
/// `exitpar2` -250 (the 2026-10-20 keep soak).
const HIDDEN_KEEP: Key = (9, 724);

fn strength(s: i64) -> Traveller {
    Traveller {
        strength: Some(s),
        ..Traveller::default()
    }
}

#[test]
fn a_doors_loop_has_no_stop_behind_a_door_the_character_cannot_open() {
    let (g, m) = world();
    let area = Area {
        anchor: HIDDEN_KEEP,
        radius: 8,
    };
    let (weak, strong) = (strength(97), strength(300));
    let stops = |t: &Traveller| -> BTreeSet<Key> {
        generate(&g, &m, area, t)
            .iter()
            .flat_map(|l| l.rooms.clone())
            .collect()
    };
    let reach = area.rooms(&g, &weak);
    let (weak_stops, strong_stops) = (stops(&weak), stops(&strong));
    assert!(!weak_stops.is_empty());
    assert!(
        weak_stops.iter().all(|k| reach.contains_key(k)),
        "{weak_stops:?}"
    );
    // The courtyard: stops with Str 300 that Str 97 cannot get to.
    let courtyard: Vec<&Key> = strong_stops
        .iter()
        .filter(|k| !reach.contains_key(k))
        .collect();
    assert!(!courtyard.is_empty(), "{strong_stops:?}");
}
