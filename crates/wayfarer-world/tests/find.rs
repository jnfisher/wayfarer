//! Go to (`find`) and boss lairs, against the pack built from `tests/fixtures/world`
//! (`common::pack_path`). The fixture pack has no MegaMud waypoints: places come from the
//! shops, and boss lairs from the monsters alone.

mod common;

use std::sync::OnceLock;

use wayfarer_world::{
    Finder, Found, FoundKind, Key, Monsters, PlaceKind, Reach, RoomGraph, Traveller, WorldPack,
};

const VILLAGE_ENTRANCE: Key = (1, 2140);
/// The Minotaur Chieftain's Large Wooden Room.
const CHIEFTAIN: Key = (1, 1865);
/// The minotaur champion's cavern.
const CHAMPION: Key = (1, 2124);
/// A Wooden Passage room spawning plain minotaurs.
const PASSAGE: Key = (1, 1839);
/// The Wooden Passage room south of the Chieftain's.
const PASSAGE_BY_CHIEFTAIN: Key = (1, 1864);
const NEWHAVEN_ARENA: [Key; 2] = [(1, 2150), (1, 2155)];

struct World {
    graph: RoomGraph,
    monsters: Monsters,
    finder: Finder,
}

fn world() -> &'static World {
    static W: OnceLock<World> = OnceLock::new();
    W.get_or_init(|| {
        let pack = WorldPack::open(common::pack_path()).unwrap();
        let graph = RoomGraph::load(&pack).unwrap();
        let monsters = Monsters::load(&pack).unwrap();
        let finder = Finder::load(&pack, &graph, &monsters).unwrap();
        World {
            graph,
            monsters,
            finder,
        }
    })
}

fn find(query: &str, from: Key) -> Vec<Found> {
    let w = world();
    w.finder.find(
        &w.graph,
        &w.monsters,
        query,
        Some(from),
        &Traveller::default(),
    )
}

#[test]
fn boss_lairs_are_the_rooms_of_monsters_that_spawn_one_at_a_time() {
    let w = world();
    let names = |k: Key| -> Option<Vec<String>> {
        let room = w.graph.room(k).unwrap();
        let by_pool = w
            .monsters
            .boss_lair(room)
            .map(|v| v.iter().map(|m| m.name.clone()).collect::<Vec<_>>());
        let by_finder = w
            .finder
            .boss_lair(k, &w.monsters)
            .map(|v| v.iter().map(|m| m.name.clone()).collect::<Vec<_>>());
        assert_eq!(by_pool, by_finder, "{k:?}");
        by_pool
    };
    assert_eq!(names(CHIEFTAIN).unwrap(), ["minotaur chieftain"]);
    assert_eq!(names(CHAMPION).unwrap(), ["minotaur champion"]);
    assert!(names(PASSAGE).is_none());
    for k in NEWHAVEN_ARENA {
        assert!(names(k).is_none(), "{k:?}");
    }
    // The Dungeon Entrance is a Lair room, but not a boss's.
    assert!(names((1, 2152)).is_none());
}

#[test]
fn woo_lists_the_wooden_passage_with_its_count_and_nearest_room() {
    let found = find("woo", PASSAGE);
    let passage = found
        .iter()
        .find(|f| f.kind == FoundKind::Rooms && f.name == "Wooden Passage")
        .unwrap();
    // 41 on map 1 and 22 on map 9.
    assert_eq!(passage.count, 63);
    assert_eq!(passage.rooms.len(), wayfarer_world::find::MAX_ROOMS);
    assert_eq!(passage.rooms[0].key, PASSAGE);
    assert_eq!(passage.rooms[0].reach, Reach::Steps(0));
    assert!(found.len() <= wayfarer_world::find::MAX_RESULTS);
    // Nearest first within a result.
    assert!(passage.rooms.windows(2).all(|p| p[0].reach <= p[1].reach));
}

#[test]
fn chief_finds_the_chieftain_with_his_lair_flagged_boss() {
    let found = find("chief", PASSAGE);
    let chief = found
        .iter()
        .find(|f| f.kind == FoundKind::Monster && f.name == "minotaur chieftain")
        .unwrap();
    assert_eq!(chief.bosses, ["minotaur chieftain"]);
    let lair = chief.rooms.iter().find(|r| r.key == CHIEFTAIN).unwrap();
    assert!(lair.boss);
    assert_eq!(lair.name, "Large Wooden Room");
    assert!(matches!(lair.reach, Reach::Steps(_)));
    // From the Wooden Passage, the minotaur chieftain comes before chieftains far away.
    let first_monster = found.iter().find(|f| f.kind == FoundKind::Monster).unwrap();
    assert_eq!(first_monster.name, "minotaur chieftain");
}

#[test]
fn skali_finds_the_three_shop_rooms_as_places() {
    let found = find("skali", VILLAGE_ENTRANCE);
    let places: Vec<&str> = found
        .iter()
        .filter(|f| f.kind == FoundKind::Place(PlaceKind::Shop))
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(places.len(), 3, "{places:?}");
    assert!(places.iter().all(|n| n.starts_with("Skali's Fine Armour")));
    // Not listed again as room names.
    assert!(!found.iter().any(|f| f.kind == FoundKind::Rooms));
}

#[test]
fn arena_finds_newhavens_arena_with_its_steps() {
    let found = find("arena", VILLAGE_ENTRANCE);
    let arena = found.iter().find(|f| f.name == "Newhaven, Arena").unwrap();
    assert_eq!(arena.count, 2);
    assert_eq!(arena.rooms[0].key, NEWHAVEN_ARENA[0]);
    assert_eq!(arena.rooms[0].reach, Reach::Steps(3));
    // No exit leads into the other.
    assert_eq!(arena.rooms[1].reach, Reach::NoRoute);
    // Results the character has no way to sort last.
    let lost = |f: &Found| f.rooms.first().is_some_and(|r| r.reach == Reach::NoRoute);
    let first_lost = found.iter().position(lost).unwrap_or(found.len());
    assert!(found[first_lost..].iter().all(lost));
}

#[test]
fn two_letters_are_enough_and_one_is_not() {
    assert!(!find("wo", VILLAGE_ENTRANCE).is_empty());
    assert!(find("w", VILLAGE_ENTRANCE).is_empty());
    assert!(find(" ", VILLAGE_ENTRANCE).is_empty());
}

#[test]
fn a_seen_name_with_an_adjective_finds_the_monster() {
    let found = find("thin giant rat", VILLAGE_ENTRANCE);
    assert!(
        found
            .iter()
            .any(|f| f.kind == FoundKind::Monster && f.name == "giant rat")
    );
}

#[test]
fn the_neighbour_of_the_chieftains_room_is_one_step_short() {
    let w = world();
    let r = wayfarer_world::route(&w.graph, PASSAGE, CHIEFTAIN, &Traveller::default()).unwrap();
    assert_eq!(r.last().unwrap().from, PASSAGE_BY_CHIEFTAIN);
}
