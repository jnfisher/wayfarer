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
    Belief, Category, DirSet, ExitType, Gate, How, Key, LockState, Monsters, RoomGraph, Traveller,
    WorldPack, huntable, layout, route, route_or_search, route_within,
};

fn graph() -> (WorldPack, RoomGraph) {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let t = Instant::now();
    let g = RoomGraph::load(&pack).unwrap();
    let took = t.elapsed();
    eprintln!("RoomGraph::load: {took:?}");
    // About 180 ms, of which some 55 ms read the rooms' command texts (C-052); this crate is
    // optimised in test builds too. The card's bound was 500 ms; a loaded CI runner has taken
    // 565 ms with the whole suite beside it, so the bound is 1000 ms (John, 2026-11-22).
    assert!(took.as_millis() < 1000, "RoomGraph::load took {took:?}");
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
    // The pack's 62,352 exits and 45 ways through rooms' command texts: 40 that teleport
    // (C-052) and 5 that cast a spell that only teleports (C-086: the Dragon's Teeth holes at
    // 2/115, 2/487 and 2/630, and the two organic chambers' `go hole`, 2/2366 and 2/2367); one
    // way an item's script gives, the potion of levitation's at Mossy Cave, Waterfall; and what
    // the monsters answer: 5 Remote Actions (the shadow guards' door, the sphinxes' ways up) and
    // 6 teleports; and 3 room commands behind a quest ability check (the Ancient Darkwood Tree's
    // two portals, and the Ancient Fortress jail cell's `shakabuku`, level 65).
    assert_eq!(g.edge_count(), 62_412);
    // Into players' houses, which have no name: there, but never taken (C-053).
    assert_eq!(g.into_unnamed, 34);
    assert_eq!(g.dangling, 0);
    // Every exit type name in the pack is known (load fails on one that isn't), and the
    // counts by type equal the pack's.
    let mut kinds = std::collections::BTreeMap::new();
    // A monster's answer that opens an exit is a Remote Action too, numbered past the
    // directions as the ways through are: not an exit of the pack.
    for e in g
        .all_edges()
        .iter()
        .filter(|e| !e.kind.is_command() && e.dir < 10)
    {
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

    // By default the route is the shortest (spawn cost 0). A spawn cost steers around a
    // spawning room when a way without one is one step longer:
    // from the river south of the Silvermere Docks to Newhaven, Docks, the river (1:2342,
    // which spawns) is one step shorter than the docks and the skiff. No Lair in the stock data
    // has a detour of one step, so this is a spawning Normal room.
    let free = level1.clone();
    let cautious = Traveller {
        costs: wayfarer_world::Costs {
            spawn: 40,
            ..Default::default()
        },
        ..level1.clone()
    };
    let river = (1, 2342);
    assert!(g.node(river).unwrap().spawns);
    let short = route(&g, (1, 1813), NEWHAVEN_DOCKS, &free).unwrap();
    assert_eq!(short.len(), 4);
    assert!(short.iter().any(|s| s.to == river));
    let safe = route(&g, (1, 1813), NEWHAVEN_DOCKS, &cautious).unwrap();
    assert_eq!(safe.len(), 5);
    assert!(
        safe.iter()
            .all(|s| s.to == NEWHAVEN_DOCKS || !g.node(s.to).unwrap().spawns)
    );
    assert_eq!(commands(&safe), ["s", "w", "n", "n", "n"]);

    // A room to avoid (a monster the bot avoids was last seen there) is gone round when there
    // is another way, however much longer: the river again, by the docks and the skiff.
    let wary = Traveller {
        avoid: [river].into(),
        ..level1.clone()
    };
    let around = route(&g, (1, 1813), NEWHAVEN_DOCKS, &wary).unwrap();
    assert!(
        around.iter().all(|s| s.to != river),
        "{:?}",
        commands(&around)
    );
    assert_eq!(around.len(), 5);
    // And still walked through when it is the only way: from the Arena to the Narrow Path,
    // by the Narrow Road. The destination is never avoided.
    let road = (1, 2146);
    let wary = Traveller {
        avoid: [road].into(),
        ..level1.clone()
    };
    let through = route(&g, ARENA, (1, 2143), &wary).unwrap();
    assert!(
        through.iter().any(|s| s.to == road),
        "{:?}",
        commands(&through)
    );
    assert_eq!(
        commands(&route(&g, ARENA, road, &wary).unwrap()),
        ["u"],
        "the destination itself is not avoided"
    );

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

#[test]
fn a_contradicted_belief_looks_everywhere() {
    // C-085: Lexi in the Dragon's Teeth Hills, 2026-11-10. The brain was sure of 2/482; she
    // was in 2/191, the same name and exits in another copy of the same stretch of maze. A
    // move west went to 2/483 in the belief and to 2/192 in the game (alike again); the next
    // west, to 2/193, showed exits 2/484 lacks.
    let (pack, g) = graph();
    const HILLS: &str = "Dragon's Teeth Hills";
    let mut b = Belief::at((2, 482));
    b.moved(&g, "w");
    assert_eq!(
        b.observe(&g, Some(HILLS), Some(dirs(&["east", "west"])), false),
        How::Followed
    );
    assert_eq!(b.candidates(), &[(2, 483)].into());
    b.moved(&g, "w");
    let seen = dirs(&["south", "east", "northwest"]);
    // Every room of that name and exits in the world, not the nearest copy of the maze.
    assert_eq!(
        b.observe(&g, Some(HILLS), Some(seen), false),
        How::Relocated
    );
    let want = named_with_exits(&pack, HILLS, seen);
    assert!(want.len() > 1 && want.contains(&(2, 193)), "{want:?}");
    assert_eq!(b.candidates(), &want);

    // Without its exits (the title behind a prompt) it is not placed by name and exits: the
    // old widening, near the candidates first, then anywhere by name.
    let mut b = Belief::at((2, 482));
    assert_eq!(b.observe(&g, Some("Docks"), None, false), How::Anywhere);
    assert_eq!(b.candidates().len(), 5);
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

    // No Colliseum room spawns (the spawn pass never visits one), though some have spawn data.
    const COLLISEUM: i64 = 5;
    let colliseums: Vec<_> = g
        .nodes()
        .filter(|n| n.room.room_type == COLLISEUM)
        .collect();
    assert!(colliseums.iter().any(|n| n.room.max_monsters > 0));
    assert!(
        colliseums
            .iter()
            .all(|n| !n.spawns && ms.generated(&n.room).is_empty())
    );

    let rats = ms.by_name("thin giant rat");
    assert!(rats.iter().any(|m| m.number == 1), "{rats:?}");
    assert!(rats.iter().all(|m| m.name == "giant rat"));
}

/// The Hidden Keep in the Darkwood Forest (9:724): east, a Door locked with `exitpar2` -250
/// into the courtyard; north, a Hidden exit to the Keep Wall (the way MegaMud's `Ftrehkep.mp`
/// takes, along the wall and up a tree).
const HIDDEN_KEEP: Key = (9, 724);
const COURTYARD_DOOR: Key = (9, 798);
const KEEP_WALL: Key = (9, 725);

#[test]
fn a_locked_door_is_routed_through_only_by_a_character_who_can_open_it() {
    let (_, g) = graph();
    let t = |strength: Option<i64>, picklocks: Option<i64>| Traveller {
        strength,
        picklocks,
        ..Traveller::default()
    };
    let door = g.edge(HIDDEN_KEEP, 2).unwrap();
    assert_eq!(
        (door.kind, door.door, door.params[1]),
        (ExitType::Door, Some(LockState::Locked), -250)
    );
    // Str 97: 97 - 250 never opens it, and there is no other way in.
    let weak = t(Some(97), None);
    assert_eq!(weak.door_odds(door), Some(0));
    assert_eq!(route(&g, HIDDEN_KEEP, COURTYARD_DOOR, &weak), None);
    // Str 300: one step, the door.
    let strong = t(Some(300), None);
    assert_eq!(strong.door_odds(door), Some(50));
    let r = route(&g, HIDDEN_KEEP, COURTYARD_DOOR, &strong).unwrap();
    assert_eq!(commands(&r), ["e"]);
    assert_eq!(r[0].door, Some(LockState::Locked));
    // Picking counts too, and the better of the two wins.
    assert_eq!(t(Some(97), Some(260)).door_odds(door), Some(10));
    assert_eq!(t(Some(97), Some(0)).door_odds(door), Some(0));
    // Strength unknown: not checked.
    assert_eq!(t(None, None).door_odds(door), None);
    assert!(route(&g, HIDDEN_KEEP, COURTYARD_DOOR, &t(None, None)).is_some());
    // Seen open this session: no odds to it.
    let mut open = weak.clone();
    open.doors.insert((HIDDEN_KEEP, 2), LockState::Open);
    assert_eq!(open.door_odds(door), None);
    assert_eq!(
        commands(&route(&g, HIDDEN_KEEP, COURTYARD_DOOR, &open).unwrap()),
        ["e"]
    );
    // The same door shut but not locked opens with `open`, whatever the strength.
    let mut shut = weak.clone();
    shut.doors.insert((HIDDEN_KEEP, 2), LockState::Closed);
    let r = route(&g, HIDDEN_KEEP, COURTYARD_DOOR, &shut).unwrap();
    assert_eq!(
        (commands(&r), r[0].door),
        (vec!["e"], Some(LockState::Closed))
    );
}

#[test]
fn a_door_the_pack_has_shut_or_open_routes_regardless_of_strength() {
    // The fixture export has no door shut but unlocked (state 1); the Flaming Chamber's east
    // door is open in it, with `exitpar2` -999.
    let (_, g) = graph();
    let door = g.edge((17, 2978), 2).unwrap();
    assert_eq!((door.door, door.params[1]), (Some(LockState::Open), -999));
    let weak = Traveller {
        strength: Some(10),
        ..Traveller::default()
    };
    assert_eq!(weak.lock_odds(door), Some(0));
    assert_eq!(weak.door_odds(door), None);
    let mut shut_edge = door.clone();
    shut_edge.door = Some(LockState::Closed);
    assert_eq!(weak.door_odds(&shut_edge), None);
    assert!(weak.can_take(door) && weak.can_take(&shut_edge));
    let r = route(&g, door.from, door.to, &weak).unwrap();
    assert_eq!(
        (commands(&r), r[0].door),
        (vec!["e"], Some(LockState::Open))
    );
    // Once the game showed it locked, it is left out.
    let mut locked = weak.clone();
    locked
        .doors
        .insert((door.from, door.dir), LockState::Locked);
    assert_eq!(locked.door_odds(door), Some(0));
    assert!(!locked.can_take(door));
}

#[test]
fn a_hidden_exit_known_to_the_traveller_is_routed_through() {
    let (_, g) = graph();
    let north = g.edge(HIDDEN_KEEP, 0).unwrap();
    assert_eq!((north.kind, north.to), (ExitType::Hidden, KEEP_WALL));
    let weak = Traveller {
        strength: Some(97),
        ..Traveller::default()
    };
    assert!(!weak.can_take(north));
    let mut found = weak.clone();
    found.found.insert((HIDDEN_KEEP, 0));
    let r = route(&g, HIDDEN_KEEP, COURTYARD_DOOR, &found).unwrap();
    assert_eq!(r[0].command, "n");
    assert_eq!(r[0].exit_type, ExitType::Hidden);
    eprintln!("into the keep: {:?}", commands(&r));
    // Known but not showing: the same way, at the search's cost.
    let mut known = weak.clone();
    known.searchable.insert((HIDDEN_KEEP, 0));
    let r2 = route(&g, HIDDEN_KEEP, COURTYARD_DOOR, &known).unwrap();
    assert_eq!(commands(&r2), commands(&r));
}

#[test]
fn a_passable_hidden_exit_is_routed_through_like_any_exit() {
    let (_, g) = graph();
    // The Dirt Road (7:1137): north, a Hidden exit with `EXF_PASSABLE`, never listed but
    // walked without a search, to the Black Weed Fields.
    let north = g.edge((7, 1137), 0).unwrap();
    assert_eq!(north.kind, ExitType::Hidden);
    assert!(north.passable());
    let anyone = Traveller::default();
    assert!(anyone.can_take(north));
    let r = route(&g, (7, 1137), (7, 1142), &anyone).unwrap();
    assert_eq!(commands(&r), ["n"]);
    // The Hidden Keep's north is searched for.
    assert!(!g.edge(HIDDEN_KEEP, 0).unwrap().passable());
}

#[test]
fn a_blocked_hidden_exit_is_opened_by_the_phrases_of_its_rooms_remote_actions() {
    let (_, g) = graph();
    // The Slimy Tunnel in the sewers (1:619): west, a Hidden exit with one block, which the
    // room's Remote Action north takes off at `push switch`. A search never finds it.
    let west = g.edge((1, 619), 3).unwrap();
    assert_eq!(west.kind, ExitType::Hidden);
    assert_eq!(west.unblock, ["push switch"]);
    // The Dark Sanctum (17:2235): five blocks (`exitpar2` 5, in order), one Remote Action
    // each, typed from the fifth down.
    let sanctum = g.edge((17, 2235), 3).unwrap();
    assert_eq!(
        sanctum.unblock,
        ["press d", "press e", "press a", "press t", "press h"]
    );
    // A Hidden exit found by searching needs no phrase.
    assert!(g.edge(HIDDEN_KEEP, 0).unwrap().unblock.is_empty());
}

#[test]
fn opened_hidden_exits_whose_name_is_blank_are_marked() {
    let (_, g) = graph();
    // An opened Hidden exit is listed by its name message (`exitpar4`); these five name message
    // 1, which is blank, so the game prints an empty slot: `north, , west` (C-076).
    let marked: BTreeSet<(Key, u8)> = g
        .all_edges()
        .iter()
        .filter(|e| e.lists_blank)
        .map(|e| (e.from, e.dir))
        .collect();
    let want: BTreeSet<(Key, u8)> = [
        ((1, 1972), 2),
        ((1, 2100), 3),
        ((1, 2103), 2),
        ((12, 2383), 3),
        ((1, 2441), 8),
    ]
    .into_iter()
    .collect();
    assert_eq!(marked, want);
    // The Labyrinth Passage with the brass plaque: east opens at `river`, and a Go to (which
    // takes Hidden exits as a last resort) says it before the move.
    let east = g.edge((1, 1972), 2).unwrap();
    assert_eq!(east.unblock, ["river"]);
    let r =
        wayfarer_world::route_or_search(&g, (1, 1972), (1, 1983), &Traveller::default()).unwrap();
    assert_eq!(commands(&r), ["e"]);
    assert_eq!(r[0].unblock, ["river"]);
}

#[test]
fn a_key_exit_is_taken_with_its_key_and_a_hidden_one_as_a_last_resort() {
    use wayfarer_world::{distances, distances_any, route_or_search};
    let (_pack, g) = graph();
    // Thrag's Gigantic Cavern, 1:1810: in by a Key exit east of the Guardroom (1:1794; the large
    // iron key, item 476); the Small Room (1:1797) joins it by Hidden exits no verified path takes.
    let (guardroom, small_room, cavern) = ((1, 1794), (1, 1797), (1, 1810));
    let plain = Traveller::default();
    // The Small Room is only reached from the cavern, so the Key exit is the way in: without the
    // key there is no route, hidden exits or not.
    assert!(route(&g, guardroom, cavern, &plain).is_none());
    // The key makes it one step through the Key exit.
    let with_key = Traveller {
        keys: BTreeSet::from([476]),
        ..Traveller::default()
    };
    let r = route(&g, guardroom, cavern, &with_key).unwrap();
    assert_eq!(
        r.iter().map(|s| s.exit_type).collect::<Vec<_>>(),
        [ExitType::Key]
    );
    // From the Small Room it is one hidden step up: no route by known exits, one as a last
    // resort (a walk the player asks for), each costing a search.
    assert!(route(&g, small_room, cavern, &plain).is_none());
    let r = route_or_search(&g, small_room, cavern, &plain).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].exit_type, ExitType::Hidden);
    // Reach counts it; the plain distances do not.
    assert!(!distances(&g, small_room, u32::MAX, &plain).contains_key(&cavern));
    assert_eq!(
        distances_any(&g, small_room, u32::MAX, &plain).get(&cavern),
        Some(&1)
    );
    // A traveller allowed every hidden exit finds it with the plain route too.
    let mut all = Traveller::default();
    all.allow.insert(Category::Hidden);
    assert!(route(&g, small_room, cavern, &all).is_some());
}

#[test]
fn an_item_exit_is_taken_with_the_item_carried() {
    let (_pack, g) = graph();
    // 1:1541 up 8 to 1:1490 asks for the rope and grapple (item 191).
    let (below, above) = ((1, 1541), (1, 1490));
    assert!(route(&g, below, above, &Traveller::default()).is_none());
    let with = Traveller {
        items: BTreeSet::from([191]),
        ..Traveller::default()
    };
    let r = route(&g, below, above, &with).expect("a way with the rope");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].exit_type, ExitType::Item);
}

#[test]
fn a_whole_room_the_pack_lacks_leaves_the_position_unknown_until_a_known_one() {
    // C-053: Faye in a player's house north of Slum Street, Bend (1:1096): "Shop White House
    // Room" is no room of the pack. Seen whole (its exits too), the position is unknown;
    // a title alone (a line misread as one) changes nothing.
    let (_, g) = graph();
    let bend = (1, 1096);
    let mut b = Belief::at(bend);
    b.observe(&g, Some("Shop White House Room"), None, false);
    assert!(b.certain(), "a title alone");
    b.observe(&g, Some("Shop White House Room"), Some(dirs(&["w"])), false);
    assert!(b.is_unknown());
    // Out again: the Bend it left is near, and it relocates there.
    b.observe(
        &g,
        Some("Slum Street, Bend"),
        Some(dirs(&["n", "s", "e"])),
        false,
    );
    assert_eq!(b.best(), Some(bend));
    // The way into the house is there, and shows, but no one takes it.
    let door = g
        .edges(bend)
        .iter()
        .find(|e| e.to.0 == 15)
        .expect("the house");
    assert_eq!(door.gate, wayfarer_world::Gate::Unnamed);
    assert!(!Traveller::anyone().can_take(door));
}

/// The Monastery (C-077): the Inner Gate's gate north to the Courtyard is locked with `exitpar2`
/// -300, opened by a lever in the room east or west of it; the Courtyard has its own lever for
/// the gate back. Chancellor Annora's office is past the Courtyard.
const INNER_GATE: Key = (1, 1331);
const COURTYARD: Key = (1, 1375);
const LARGE_OFFICE: Key = (1, 1333);
const MONASTERY_ENTRANCE: Key = (1, 1322);

#[test]
fn a_locked_door_a_lever_opens_carries_the_lever() {
    let (_, g) = graph();
    let levers: Vec<&wayfarer_world::Edge> = g
        .all_edges()
        .iter()
        .filter(|e| !e.lever.is_empty())
        .collect();
    let own = levers.iter().filter(|e| e.lever.len() == 1).count();
    for e in levers.iter().filter(|e| e.lever.len() > 1) {
        let way: Vec<(&str, ExitType)> = e
            .lever
            .iter()
            .map(|s| (s.command.as_str(), s.kind))
            .collect();
        eprintln!("lever elsewhere: {:?} {} {:?}", e.from, e.dir, way);
    }
    // 18 levers in the door's own room (one of them a monster's answer: the shadow guards'
    // door to Morukai, 9:1423); four in another, two of them (C-120) reached through a hidden
    // exit: the Grand Stair's gate (7:150) from its Guard Room, and one more.
    assert_eq!((levers.len(), own), (22, 18));
    assert!(
        levers
            .iter()
            .all(|e| matches!(e.kind, ExitType::Door | ExitType::Gate))
    );
    let gate = g.edge(INNER_GATE, 0).unwrap();
    assert_eq!(
        (gate.kind, gate.to, gate.params[1]),
        (ExitType::Gate, COURTYARD, -300)
    );
    let lever: Vec<(&str, ExitType)> = gate
        .lever
        .iter()
        .map(|e| (e.command.as_str(), e.kind))
        .collect();
    assert_eq!(
        lever,
        [
            ("e", ExitType::Normal),
            ("pull lever", ExitType::RemoteAction),
            ("w", ExitType::Normal),
        ]
    );
    let back = g.edge(COURTYARD, 1).unwrap();
    assert_eq!(commands_of(&back.lever), ["pull lever"]);
    // Gates whose lever is reached only by a Hidden exit carry it too (C-120): the Grand
    // Stair's southeast (7/150, the Guard Room 7/152 three steps off) and the Black Fortress
    // gatehouse's east (7/1173, from 7/1175 next door).
    let grand_stair: Vec<(&str, ExitType)> = g
        .edge((7, 150), 6)
        .unwrap()
        .lever
        .iter()
        .map(|e| (e.command.as_str(), e.kind))
        .collect();
    assert_eq!(
        grand_stair,
        [
            ("sw", ExitType::Normal),
            ("se", ExitType::Normal),
            ("s", ExitType::Hidden),
            ("pull lever", ExitType::RemoteAction),
            ("n", ExitType::Hidden),
            ("nw", ExitType::Normal),
            ("ne", ExitType::Normal),
        ]
    );
    let gatehouse = &g.edge((7, 1173), 2).unwrap().lever;
    assert_eq!(commands_of(gatehouse), ["s", "pull lever", "n"]);
    assert_eq!(gatehouse[0].kind, ExitType::Hidden);
}

fn commands_of(edges: &[wayfarer_world::Edge]) -> Vec<&str> {
    edges.iter().map(|e| e.command.as_str()).collect()
}

#[test]
fn a_locked_gate_a_lever_opens_is_routed_through() {
    let (_, g) = graph();
    let weak = Traveller {
        strength: Some(50),
        ..Traveller::default()
    };
    let gate = g.edge(INNER_GATE, 0).unwrap();
    assert_eq!(weak.door_odds(gate), Some(0));
    assert!(weak.can_take(gate));
    // Straight through the gate; the lever rides on the step, for when it refuses.
    let r = route(&g, INNER_GATE, COURTYARD, &weak).unwrap();
    assert_eq!(commands(&r), ["n"]);
    assert_eq!(commands(&r[0].lever), ["e", "pull lever", "w"]);
    // Chancellor Annora, from the Monastery's door (a hidden exit on the way, searched for as
    // Go to does).
    let r = route_or_search(&g, MONASTERY_ENTRANCE, LARGE_OFFICE, &weak).unwrap();
    assert_eq!(&commands(&r)[..2], ["n", "n"]);
    assert_eq!(r.last().unwrap().to, LARGE_OFFICE);
    // A door with no lever and no odds is still left out.
    let mut no_lever = gate.clone();
    no_lever.lever.clear();
    assert!(!weak.can_take(&no_lever));
}

/// C-117: the Ancient Ruin's Demolished Doors (3:557) west into the Inside Arch is an armed
/// trap. A character with the disarm-traps skill disarms it first; one without walks into it,
/// and its route counts the trap as costly, so a trap-free way round of a few more steps wins.
#[test]
fn an_armed_trap_is_disarmed_by_a_character_that_can() {
    let (_pack, g) = graph();
    let (doors, arch) = ((3, 557), (3, 559));
    let trap = g.edge(doors, 3).expect("the Demolished Doors' west exit");
    assert!(trap.armed_trap(), "{trap:?}");
    assert_eq!(trap.to, arch);

    let plain = Traveller::default();
    let r = route(&g, doors, arch, &plain).unwrap();
    assert_eq!(commands(&r), ["w"]);
    assert!(!r[0].disarm);

    let sasha = Traveller {
        disarm: Some(119),
        ..Traveller::default()
    };
    let r = route(&g, doors, arch, &sasha).unwrap();
    assert_eq!(commands(&r), ["w"]);
    assert!(r[0].disarm);
    // No skill, no disarm.
    let none = Traveller {
        disarm: Some(0),
        ..Traveller::default()
    };
    assert!(!route(&g, doors, arch, &none).unwrap()[0].disarm);

    // Every armed trap in the data: kind 0 or 3, a Trap or a Spell Trap.
    let armed = g.all_edges().iter().filter(|e| e.armed_trap()).count();
    assert!(armed > 200, "{armed} armed traps");
}

/// C-118: the Darkened Tunnels' hidden passage north of 15:477 has two blocks, taken off in any
/// order by `pull lever` in 15:730 and 15:604, far off in the maze. A walk through it goes to
/// both first, the one nearest the passage last, then through; a loop's traveller does not.
#[test]
fn a_hidden_exit_opened_by_levers_elsewhere_is_reached_by_pulling_them_first() {
    let (_pack, g) = graph();
    let (passage, beyond) = ((15, 477), (15, 478));
    let e = g.edge(passage, 0).expect("the passage north");
    assert!(e.sealed());
    assert!(e.levers_any_order);
    let mut rooms: Vec<Key> = e.levers.iter().map(|l| l.room).collect();
    rooms.sort_unstable();
    assert_eq!(rooms, [(15, 604), (15, 730)]);
    assert!(e.levers.iter().all(|l| l.phrase == "pull lever"));

    // There is a way round of some 115 steps, cheaper than a lever tour: the route through
    // the passage itself is asked for.
    let r = route_within(&g, passage, beyond, &Traveller::default(), |k| k == beyond).unwrap();
    let pulls: Vec<Key> = r
        .iter()
        .filter(|s| s.exit_type == ExitType::RemoteAction)
        .map(|s| s.from)
        .collect();
    assert_eq!(pulls.len(), 2, "{:?}", commands(&r));
    // The route walks to each lever and back to the passage, then goes north.
    let last = r.last().unwrap();
    assert_eq!(
        (last.from, last.to, last.command.as_str()),
        (passage, beyond, "n")
    );
    assert!(last.passable);
    let at_pull = r
        .iter()
        .position(|s| s.exit_type == ExitType::RemoteAction)
        .unwrap();
    assert_eq!(r[at_pull].command, "pull lever");
    // Back at the passage from the second lever.
    assert_eq!(r[r.len() - 2].to, passage);

    // A loop's legs never take it.
    let looping = Traveller {
        no_levers: true,
        ..Traveller::default()
    };
    assert!(route_within(&g, passage, beyond, &looping, |k| k == beyond).is_none());
}

/// C-119: the Black Fortress's Dark Chamber (7:1283) has a panel, `push panel`, that takes a
/// character to the Darkened Tunnels (15:335) when no monster is in the room.
#[test]
fn the_dark_chambers_panel_is_a_way_into_the_darkened_tunnels() {
    let (_pack, g) = graph();
    let r = route(&g, (7, 1283), (15, 335), &Traveller::default()).unwrap();
    assert_eq!(commands(&r), ["push panel"]);
    // Only with no monster in the room: the walker says so when the game will not.
    assert!(r[0].needs_empty);
}

/// C-120: the dark bishop's Elaborate Chamber (7:1227, the Black Fortress) is some 240 steps
/// from Silvermere's Temple through the Black House and the Grand Stair, whose gate (7:150) a
/// lever in a Guard Room (7:152) behind one hidden exit opens; the way round by the Western
/// Road and the Dragon's Teeth tunnels is some 290. A Go to prices the hidden exit by the
/// searches it is expected to take and goes under the Black House; a loop's traveller, which
/// keeps to known exits, goes round.
#[test]
fn a_go_to_searches_for_a_hidden_exit_when_that_is_the_short_way() {
    let (_pack, g) = graph();
    let (temple, chamber) = ((1, 2189), (7, 1227));
    let (black_house_foyer, western_road) = ((1, 1313), (2, 41));
    let (grand_stair_gate, guard_room) = ((7, 150), (7, 152));
    // Riley: level 16, strength 90, the Black House key (172) and the slums' western door key
    // (569) held, a perception that finds a hidden exit one try in four.
    let riley = Traveller {
        level: Some(16),
        class: Some(3),
        strength: Some(90),
        perception: Some(40),
        keys: BTreeSet::from([172, 569, 593, 746]),
        ..Traveller::default()
    };
    assert_eq!(riley.search_odds(), Some(25));

    // The gate past the Grand Stair: no strength bashes it, its lever is in the Guard Room,
    // three steps away through a hidden exit the lever path searches for.
    let gate = g.edge(grand_stair_gate, 6).expect("the Grand Stair's gate");
    assert_eq!((gate.kind, gate.params[1]), (ExitType::Gate, -250));
    assert_eq!(riley.door_odds(gate), Some(0));
    let lever_cmds: Vec<&str> = gate.lever.iter().map(|e| e.command.as_str()).collect();
    assert_eq!(gate.lever.len(), 7, "{lever_cmds:?}");
    let hidden = gate
        .lever
        .iter()
        .find(|s| s.kind == ExitType::Hidden)
        .expect("a hidden exit on the way to the lever");
    assert_eq!((hidden.from, hidden.to), ((7, 148), guard_room));
    assert!(hidden.needs_search());
    assert_eq!(gate.lever[3].from, guard_room);
    assert_eq!(gate.lever[3].kind, ExitType::RemoteAction);
    assert!(riley.can_take(gate));

    let passes = |r: &[wayfarer_world::Step], k: Key| r.iter().any(|s| s.to == k);
    // A Go to: under the Black House and up the Grand Stair.
    let go = route_or_search(&g, temple, chamber, &riley).unwrap();
    assert!(passes(&go, black_house_foyer), "{:?}", commands(&go));
    assert!(passes(&go, grand_stair_gate));
    assert!(!passes(&go, western_road));
    let through_gate = go.iter().find(|s| s.from == grand_stair_gate).unwrap();
    assert_eq!(commands(&through_gate.lever).len(), 7);
    assert!(go.len() < 260, "{} steps", go.len());
    // A loop's legs keep to known exits: the long way round, west out of the slums.
    let round = route(&g, temple, chamber, &riley).unwrap();
    assert!(passes(&round, western_road), "{:?}", commands(&round));
    assert!(!passes(&round, black_house_foyer));
    assert!(
        round.len() > go.len() + 30,
        "{} vs {}",
        round.len(),
        go.len()
    );
    // Once the hidden exit is found the lever path is plain walking and the route the same.
    let mut found = riley.clone();
    found.found.insert(((7, 148), 1));
    let again = route_or_search(&g, temple, chamber, &found).unwrap();
    assert_eq!(commands(&again), commands(&go));
    // A character at the 3 % floor expects 34 searches (68 steps' worth): more than the way
    // round costs over the Black House way, now that Riley's rusty iron key makes the slums'
    // Western Door certain (C-130: a keyed door with its key is one step, not a bash at 30 %),
    // so it goes the long way round.
    let clumsy = Traveller {
        perception: Some(10),
        ..riley.clone()
    };
    assert_eq!(clumsy.search_odds(), Some(3));
    let r = route_or_search(&g, temple, chamber, &clumsy).unwrap();
    assert!(passes(&r, western_road), "{} steps", r.len());
    // Without the key the door is a bash at 30 %, four steps' worth, and the searches win.
    let mut keyless = clumsy.clone();
    keyless.keys.remove(&569);
    let r = route_or_search(&g, temple, chamber, &keyless).unwrap();
    assert!(passes(&r, black_house_foyer), "{} steps", r.len());
    // With the perception not seen yet, one search is assumed.
    let unseen = Traveller {
        perception: None,
        ..riley.clone()
    };
    assert_eq!(unseen.search_odds(), None);
    assert_eq!(
        commands(&route_or_search(&g, temple, chamber, &unseen).unwrap()),
        commands(&go)
    );
}

#[test]
fn a_blocked_passage_whose_phrase_asks_for_an_item_needs_it_carried() {
    let (pack, g) = graph();
    // The Catacombs' Broken Stairwell (9:1032): east is blocked, and the room's Remote Action
    // opens it at the texts of message 1780, which asks for the titanium fork (item 983,
    // `exitpar4`). The first text is `use fork east`.
    let east = g.edge((9, 1032), 2).unwrap();
    assert_eq!(east.kind, ExitType::Hidden);
    assert_eq!(east.unblock, ["use fork east"]);
    assert_eq!(east.needs, [983]);
    assert!(!east.sealed());
    // A loader told that `use` is a command word prefers the text that is not one.
    let g2 = RoomGraph::load_with(&pack, &|p| !p.starts_with("use ")).unwrap();
    assert_eq!(g2.edge((9, 1032), 2).unwrap().unblock, ["aim fork east"]);
    assert_eq!(g2.edge((9, 1032), 2).unwrap().needs, [983]);
    // Without the fork there is no way through; with it, the passage is one step, its phrase
    // typed first.
    let beyond = (9, 1049);
    assert!(route_or_search(&g, (9, 1032), beyond, &Traveller::default()).is_none());
    let with_fork = Traveller {
        items: BTreeSet::from([983]),
        ..Traveller::default()
    };
    let r = route_or_search(&g, (9, 1032), beyond, &with_fork).unwrap();
    assert_eq!(commands(&r), ["e"]);
    assert_eq!(r[0].unblock, ["use fork east"]);
    // The phrases that ask for nothing still ask for nothing.
    assert!(g.edge((1, 619), 3).unwrap().needs.is_empty());
}

#[test]
fn an_items_script_that_teleports_is_a_way_from_the_rooms_holding_its_fixture() {
    let (_, g) = graph();
    // The potion of levitation (item 992): its spell runs a text block whose script, where the
    // waterfall (item 993, a fixture of Mossy Cave, Waterfall 3:1) is, teleports to Waterfall,
    // Dirt Path (9:1009) above the Catacombs, which no exit of the pack reaches.
    let up: Vec<_> = g
        .edges((3, 1))
        .iter()
        .filter(|e| e.kind == ExitType::ItemCommand)
        .collect();
    assert_eq!(up.len(), 1, "{up:?}");
    assert_eq!(up[0].to, (9, 1009));
    assert_eq!(up[0].command, "use potion of levitation");
    assert_eq!(
        up[0].phrases,
        ["use potion of levitation", "drink potion of levitation"]
    );
    assert_eq!(up[0].gate, Gate::Item { item: 992 });
    assert!(route(&g, (3, 1), (9, 1009), &Traveller::default()).is_none());
    let with_potion = Traveller {
        items: BTreeSet::from([992]),
        ..Traveller::default()
    };
    let r = route(&g, (3, 1), (9, 1009), &with_potion).unwrap();
    assert_eq!(commands(&r), ["use potion of levitation"]);
    // The waterfall is in no other room, and no other item's script is such a way.
    let ways: Vec<_> = g
        .all_edges()
        .iter()
        .filter(|e| e.kind == ExitType::ItemCommand)
        .collect();
    assert_eq!(ways.len(), 1, "{ways:?}");
    assert!(ways[0].kind.is_command() && ways[0].kind.is_phrase());
}

#[test]
fn a_monsters_answer_that_opens_an_exit_is_its_lever() {
    let (_, g) = graph();
    // The Ancient Darkwood Tree (9:1423): west to Morukai's Mystical Chambers is a door no
    // bash or pick opens (`exitpar2` -999). The shadow guards the room spawns answer `morukai`,
    // `orfeo`, ... with a script that runs a Remote Action on that door: the question is the
    // lever, in the door's own room.
    let door = g.edge((9, 1423), 3).unwrap();
    assert_eq!(door.kind, ExitType::Door);
    let pulls: Vec<&str> = door.lever.iter().map(|e| e.command.as_str()).collect();
    assert_eq!(pulls, ["ask shadow guard morukai"]);
    let t = Traveller {
        strength: Some(50),
        ..Traveller::default()
    };
    assert_eq!(t.door_odds(door), Some(0));
    assert!(t.can_take(door));
    let r = route(&g, (9, 1423), (9, 1425), &t).unwrap();
    assert_eq!(commands(&r), ["w"]);
    assert_eq!(commands(&r[0].lever), ["ask shadow guard morukai"]);
    // The Great Pyramid's stone sphinxes open the blocked way up of their room when asked the
    // riddle's answer: the question is the exit's phrase.
    assert_eq!(
        g.edge((12, 1920), 8).unwrap().unblock,
        ["ask stone sphinx fire"]
    );
    assert_eq!(
        g.edge((12, 2001), 8).unwrap().unblock,
        ["ask stone sphinx sun"]
    );
    // Balthazar (Fine Mansion, Study, 2:2577) sends the asker to the Silver River before the
    // waterfall at `journey`, with no condition: a way through from his room.
    let ways: Vec<_> = g
        .all_edges()
        .iter()
        .filter(|e| e.kind == ExitType::AskCommand)
        .collect();
    assert_eq!(ways.len(), 6, "{ways:?}");
    let journey = ways.iter().find(|e| e.from == (2, 2577)).unwrap();
    assert_eq!(
        (journey.to, journey.command.as_str()),
        ((1, 2406), "ask Balthazar journey")
    );
    assert_eq!(
        commands(&route(&g, (2, 2577), (1, 2406), &t).unwrap()),
        ["ask Balthazar journey"]
    );
    // An answer with a condition (Jorah's transport: alignment and a quest flag) is no way.
    assert!(!ways.iter().any(|e| e.from == (1, 2327)));
}
