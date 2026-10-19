//! The room graph, routing, the position belief, the map layout and spawn pools, against the
//! pack built from `tests/fixtures/world` (`common::pack_path`).
//!
//! Each test loads the graph (about 50 ms here). The first test process to run builds the
//! pack (2-3 s) and the others reuse it, so one of these tests may be slow in a fresh target
//! directory.

mod common;

use std::collections::BTreeSet;
use std::time::Instant;

use wayfarer_world::{
    Belief, Category, DirSet, ExitType, Key, LockState, Monsters, RoomGraph, Traveller, WorldPack,
    huntable, layout, route,
};

fn graph() -> (WorldPack, RoomGraph) {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let t = Instant::now();
    let g = RoomGraph::load(&pack).unwrap();
    let took = t.elapsed();
    eprintln!("RoomGraph::load: {took:?}");
    // About 50 ms (this crate is optimised in test builds too); the card's bound is 500 ms.
    assert!(took.as_millis() < 500, "RoomGraph::load took {took:?}");
    (pack, g)
}

fn dirs(names: &[&str]) -> DirSet {
    DirSet::from_names(names.iter().copied()).unwrap()
}

fn commands(r: &[wayfarer_world::Step]) -> Vec<&str> {
    r.iter().map(|s| s.command.as_str()).collect()
}

const VILLAGE_ENTRANCE: Key = (1, 2140);
const ARENA: Key = (1, 2150);
const NEWHAVEN_DOCKS: Key = (1, 2149);
const SMALL_PIER: Key = (1, 2335);

#[test]
fn graph_loads_every_room_and_exit() {
    let (pack, g) = graph();
    assert_eq!(g.len(), 26_720);
    assert_eq!(g.edge_count(), 62_352);
    assert_eq!(g.dangling, 0);
    // Every exit type name in the pack is known (load fails on one that isn't), and the
    // counts by type equal the pack's.
    let mut kinds = std::collections::BTreeMap::new();
    for e in g.all_edges() {
        *kinds.entry(e.kind.name()).or_insert(0) += 1;
    }
    let exits = pack.all_exits().unwrap();
    let mut want = std::collections::BTreeMap::new();
    for (_, e) in &exits {
        *want.entry(e.exit_type.as_str()).or_insert(0) += 1;
    }
    assert_eq!(kinds, want);
    assert_eq!(kinds.len(), 22);

    // The Arena's north door, locked in the export, and its listed exits.
    let door = g.edge(ARENA, 0).unwrap();
    assert_eq!(
        (door.kind, door.door, door.to),
        (ExitType::Door, Some(LockState::Locked), (1, 2152))
    );
    assert!(g.exits_fit(ARENA, dirs(&["north", "up"])));
    assert!(g.node(ARENA).unwrap().spawns);
    assert!(!g.node(VILLAGE_ENTRANCE).unwrap().spawns);
}

#[test]
fn routes() {
    let (_pack, g) = graph();
    let level1 = Traveller {
        level: Some(1),
        ..Traveller::default()
    };

    // Village Entrance to the Arena and back.
    let there = route(&g, VILLAGE_ENTRANCE, ARENA, &level1).unwrap();
    assert_eq!(commands(&there), ["w", "w", "d"]);
    assert_eq!(there[0].from, VILLAGE_ENTRANCE);
    assert_eq!(there[2].to, ARENA);
    let back = route(&g, ARENA, VILLAGE_ENTRANCE, &level1).unwrap();
    assert_eq!(commands(&back), ["u", "e", "e"]);
    assert_eq!(route(&g, ARENA, ARENA, &level1).unwrap(), []);

    // A Text exit comes out as its phrase (message 8564).
    let skiff = route(&g, NEWHAVEN_DOCKS, SMALL_PIER, &level1).unwrap();
    assert_eq!(commands(&skiff), ["borrow skiff"]);
    assert_eq!((skiff[0].exit_type, skiff[0].dir), (ExitType::Text, 1));
    assert_eq!(
        g.edge(NEWHAVEN_DOCKS, 1).unwrap().phrases,
        ["borrow skiff", "go skiff", "row skiff"]
    );

    // Level exits: the Healer (1:2146 west, levels 1-3) and the Silvermere Docks crossing back
    // to Newhaven (1:33 north, 0-3), the only way back to the island.
    let healer = (1, 2190);
    let road = (1, 2146);
    assert_eq!(commands(&route(&g, road, healer, &level1).unwrap()), ["w"]);
    let level4 = Traveller {
        level: Some(4),
        ..Traveller::default()
    };
    assert!(
        route(&g, road, healer, &level4)
            .is_none_or(|r| r.iter().all(|s| s.exit_type != ExitType::Level))
    );
    let level3 = Traveller {
        level: Some(3),
        ..Traveller::default()
    };
    let home = route(&g, (1, 33), NEWHAVEN_DOCKS, &level3).unwrap();
    assert_eq!(home[0].exit_type, ExitType::Level);
    assert_eq!(route(&g, (1, 33), NEWHAVEN_DOCKS, &level4), None);

    // A blocked edge is avoided: Silvermere Docks 1:27 west to 1:33, or round by River Street.
    assert_eq!(
        commands(&route(&g, (1, 27), (1, 33), &level1).unwrap()),
        ["w"]
    );
    let mut blocked = level1.clone();
    blocked.blocked.insert(((1, 27), 3));
    let around = route(&g, (1, 27), (1, 33), &blocked).unwrap();
    assert_eq!(commands(&around), ["s", "w", "n"]);
    assert_eq!(around.last().unwrap().to, (1, 33));

    // The spawn cost steers around a spawning room when a way without one is one step longer:
    // from the river south of the Silvermere Docks to Newhaven, Docks, the river (1:2342,
    // which spawns) is one step shorter than the docks and the skiff. No Lair in the stock data
    // has a detour of one step, so this is a spawning Normal room.
    let free = Traveller {
        costs: wayfarer_world::Costs { spawn: 0, door: 2 },
        ..level1.clone()
    };
    let river = (1, 2342);
    assert!(g.node(river).unwrap().spawns);
    let short = route(&g, (1, 1813), NEWHAVEN_DOCKS, &free).unwrap();
    assert_eq!(short.len(), 4);
    assert!(short.iter().any(|s| s.to == river));
    let safe = route(&g, (1, 1813), NEWHAVEN_DOCKS, &level1).unwrap();
    assert_eq!(safe.len(), 5);
    assert!(
        safe.iter()
            .all(|s| s.to == NEWHAVEN_DOCKS || !g.node(s.to).unwrap().spawns)
    );
    assert_eq!(commands(&safe), ["s", "w", "n", "n", "n"]);

    // Key exits only when allowed.
    let key = g
        .all_edges()
        .iter()
        .find(|e| e.kind == ExitType::Key)
        .unwrap();
    assert!(!level1.can_take(key));
    let mut keys = level1.clone();
    keys.allow.insert(Category::Key);
    assert!(keys.can_take(key));
}

/// The rooms of this name whose exits, as the export lists them, are `want`.
fn named_with_exits(pack: &WorldPack, name: &str, want: DirSet) -> BTreeSet<Key> {
    pack.rooms_by_name(name)
        .unwrap()
        .into_iter()
        .filter(|r| {
            let shown: DirSet = pack
                .exits(r.map, r.room)
                .unwrap()
                .iter()
                .filter(|e| !matches!(e.exit_type.as_str(), "Hidden" | "Text" | "Remote Action"))
                .map(|e| e.dir)
                .collect();
            shown == want
        })
        .map(|r| (r.map, r.room))
        .collect()
}

#[test]
fn belief() {
    let (pack, g) = graph();

    // River Street with exits east and west: eight rooms. A move west into the one River
    // Street with a south exit collapses it.
    let mut b = Belief::unknown();
    b.observe(
        &g,
        Some("River Street"),
        Some(dirs(&["east", "west"])),
        false,
    );
    let want = named_with_exits(&pack, "River Street", dirs(&["east", "west"]));
    assert_eq!(want.len(), 8);
    assert_eq!(b.candidates(), &want);
    assert!(!b.certain());
    b.moved(&g, "w");
    b.observe(
        &g,
        Some("River Street"),
        Some(dirs(&["south", "east", "west"])),
        false,
    );
    assert!(b.certain());
    assert_eq!(b.best(), Some((1, 19)));

    // Silvermere's five Docks stay five until an exit set tells them apart.
    let docks: BTreeSet<Key> = [(1, 27), (1, 31), (1, 32), (1, 33), (1, 296)].into();
    let mut b = Belief::unknown();
    b.observe(&g, Some("Docks"), None, false);
    assert_eq!(b.candidates(), &docks);
    b.observe(&g, None, None, true);
    assert_eq!(b.candidates(), &docks);
    b.observe(&g, Some("Docks"), Some(dirs(&["north", "south"])), false);
    assert_eq!(b.candidates(), &[(1, 296)].into());

    // Newhaven, Docks has a Text exit south, never listed.
    let mut b = Belief::unknown();
    b.observe(&g, Some("Newhaven, Docks"), Some(dirs(&["north"])), false);
    assert_eq!(b.best(), Some(NEWHAVEN_DOCKS));
    assert!(b.certain());

    // A title lost behind the prompt still advances on the exits.
    b.moved(&g, "n");
    b.observe(&g, None, Some(dirs(&["south", "northwest"])), false);
    assert_eq!(b.candidates(), &[(1, 2148)].into());
}

/// A room shown: its title and `Obvious exits`.
type Shown = (&'static str, &'static [&'static str]);

/// The room-walk capture (`tests/fixtures/captures/room-walk/plain.txt`): what was typed, then
/// the title and `Obvious exits` shown, or `None` when the move was refused.
const ROOM_WALK: &[(&str, Option<Shown>)] = &[
    (
        "",
        Some(("Newhaven, Narrow Road", &["north", "east", "west", "down"])),
    ),
    (
        "e",
        Some(("Newhaven, Narrow Path", &["north", "south", "east", "west"])),
    ),
    (
        "e",
        Some((
            "Newhaven, Village Entrance",
            &["north", "south", "west", "southeast"],
        )),
    ),
    (
        "look",
        Some((
            "Newhaven, Village Entrance",
            &["north", "south", "west", "southeast"],
        )),
    ),
    ("n", Some(("Newhaven, Weapons Shop", &["south"]))),
    (
        "s",
        Some((
            "Newhaven, Village Entrance",
            &["north", "south", "west", "southeast"],
        )),
    ),
    (
        "w",
        Some(("Newhaven, Narrow Path", &["north", "south", "east", "west"])),
    ),
    (
        "w",
        Some(("Newhaven, Narrow Road", &["north", "east", "west", "down"])),
    ),
    ("d", Some(("Newhaven, Arena", &["north", "up"]))),
    // "The door is closed!"
    ("n", None),
    (
        "u",
        Some(("Newhaven, Narrow Road", &["north", "east", "west", "down"])),
    ),
    (
        "e",
        Some(("Newhaven, Narrow Path", &["north", "south", "east", "west"])),
    ),
    (
        "e",
        Some((
            "Newhaven, Village Entrance",
            &["north", "south", "west", "southeast"],
        )),
    ),
    (
        "se",
        Some(("Newhaven, Forest Path", &["south", "northwest"])),
    ),
    ("s", Some(("Newhaven, Docks", &["north"]))),
    ("borrow skiff", Some(("Small Pier", &["north", "south"]))),
    ("s", Some(("Docks", &["north", "south"]))),
    ("s", Some(("Docks", &["north", "south", "east", "west"]))),
    ("e", Some(("Docks", &["north", "south", "east", "west"]))),
    (
        "s",
        Some(("River Street", &["north", "south", "east", "west"])),
    ),
    ("e", Some(("River Street", &["east", "west"]))),
];

#[test]
fn belief_follows_the_room_walk_capture() {
    let (_pack, g) = graph();
    let mut b = Belief::unknown();
    let mut t = Traveller::default();
    let mut path = Vec::new();
    for &(cmd, seen) in ROOM_WALK {
        match seen {
            None => t.blocked.extend(b.refused(&g, cmd)),
            Some((title, exits)) => {
                if wayfarer_world::mm::direction(cmd).is_some() || cmd == "borrow skiff" {
                    b.moved(&g, cmd);
                }
                b.observe(&g, Some(title), Some(dirs(exits)), false);
                assert!(b.certain(), "after {cmd:?}: {:?}", b.candidates());
                path.push(b.best().unwrap());
            }
        }
    }
    assert_eq!(t.blocked, [(ARENA, 0)].into());
    assert_eq!(
        path[path.len() - 7..],
        [
            NEWHAVEN_DOCKS,
            SMALL_PIER,
            (1, 296),
            (1, 33),
            (1, 27),
            (1, 26),
            (1, 25)
        ]
    );
}

/// Pitfall 34: the belief is wrong (Forest Path, really the Village Entrance), the route's
/// `nw` is refused with "There is no exit in that direction!"; the edge is marked, and the
/// look that follows relocates.
#[test]
fn a_refused_move_marks_the_edge_and_a_look_relocates() {
    let (_pack, g) = graph();
    let forest_path = (1, 2148);
    let mut b = Belief::at(forest_path);
    let mut t = Traveller::default();
    let r = route(&g, forest_path, VILLAGE_ENTRANCE, &t).unwrap();
    assert_eq!(commands(&r), ["nw"]);
    t.blocked.extend(b.refused(&g, "nw"));
    assert_eq!(t.blocked, [(forest_path, 5)].into());
    assert!(route(&g, forest_path, VILLAGE_ENTRANCE, &t).is_none_or(|r| r[0].dir != 5));
    b.observe(
        &g,
        Some("Newhaven, Village Entrance"),
        Some(dirs(&["north", "south", "west", "southeast"])),
        false,
    );
    assert_eq!(b.candidates(), &[VILLAGE_ENTRANCE].into());
}

#[test]
fn layout_around_the_village_entrance() {
    let (_pack, g) = graph();
    let lay = layout(&g, VILLAGE_ENTRANCE, 15);
    let center = &lay.rooms[0];
    assert_eq!(
        (center.key, center.x, center.y, center.level),
        (VILLAGE_ENTRANCE, 0, 0, 0)
    );
    // North of the entrance is the Weapons Shop; the Arena is two west and one down.
    let at = |k: Key| lay.rooms.iter().find(|p| p.key == k).unwrap();
    assert_eq!((at((1, 2141)).x, at((1, 2141)).y), (0, -1));
    assert_eq!((at(ARENA).x, at(ARENA).y, at(ARENA).level), (-2, 0, -1));
    // The stock data has two collisions within 15 steps, both where the river meets the
    // island's south shore.
    let collisions: Vec<Key> = lay.collisions().map(|p| p.key).collect();
    assert_eq!(collisions, [(1, 2363), (1, 1816)]);
    // Each room once; segments join placed rooms.
    let keys: BTreeSet<Key> = lay.rooms.iter().map(|p| p.key).collect();
    assert_eq!(keys.len(), lay.rooms.len());
    assert!(!lay.segments.is_empty());
    for s in &lay.segments {
        assert!(keys.contains(&s.from) && keys.contains(&s.to), "{s:?}");
    }
    let door = lay
        .segments
        .iter()
        .find(|s| (s.from, s.to) == (ARENA, (1, 2152)))
        .unwrap();
    assert_eq!(
        (door.dir, door.kind, door.one_way),
        (0, wayfarer_world::SegmentKind::Door, false)
    );
    assert!(
        lay.segments
            .iter()
            .any(|s| s.kind == wayfarer_world::SegmentKind::Text)
    );
}

#[test]
fn spawn_pools_and_huntable_monsters() {
    let (pack, g) = graph();
    let ms = Monsters::load(&pack).unwrap();
    assert_eq!(ms.iter().count(), 1_101);

    // The Arena: giant rats, filthbugs, kobold thieves; all huntable.
    let arena = g.room(ARENA).unwrap();
    let pool = ms.spawn_pool(arena);
    let names: BTreeSet<&str> = pool.iter().map(|m| m.name.as_str()).collect();
    assert!(
        names.contains("giant rat") && names.contains("kobold thief"),
        "{names:?}"
    );
    assert!(pool.iter().all(|m| huntable(m)));
    assert!(ms.hunt_room(arena));

    // The Weapons Shop: its shopkeeper, an NPC.
    let shop = g.room((1, 2141)).unwrap();
    let pool = ms.spawn_pool(shop);
    assert_eq!(pool.len(), 1);
    assert_eq!(pool[0].mon_type, wayfarer_world::monsters::MONTYPE_NPC);
    assert!(!ms.hunt_room(shop));
    assert!(!g.node((1, 2141)).unwrap().spawns);

    let rats = ms.by_name("thin giant rat");
    assert!(rats.iter().any(|m| m.number == 1), "{rats:?}");
    assert!(rats.iter().all(|m| m.name == "giant rat"));
}
