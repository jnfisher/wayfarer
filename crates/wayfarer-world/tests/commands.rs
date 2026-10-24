//! Room commands and MegaMud's crossings as ways through (C-052), on the pack built from the
//! fixtures (`tests/fixtures/megamud-crossings` adds the three paths these need).

mod common;

use wayfarer_world::route::{Traveller, route};
use wayfarer_world::{ExitType, Gate, PathKnowledge, RoomGraph, WorldPack};

const NEWHAVEN: (i64, i64) = (1, 2140);
const SLUM_STREET: (i64, i64) = (1, 1182);
const ENTRANCE: (i64, i64) = (1, 65);
const SPECTRAL_THRONE: (i64, i64) = (1, 382);

fn graph() -> (WorldPack, RoomGraph) {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let graph = RoomGraph::load(&pack).unwrap();
    (pack, graph)
}

/// What the brain's command table would claim, for these phrases.
fn game_command(p: &str) -> bool {
    ["use", "look", "l", "read"].contains(&p.split(' ').next().unwrap_or(""))
}

#[test]
fn ring_chime_takes_the_route_into_the_strange_mansion() {
    let (_, mut g) = graph();
    let chime = g
        .edges(SLUM_STREET)
        .iter()
        .find(|e| e.kind == ExitType::RoomCommand)
        .expect("the chime");
    assert_eq!((chime.to, chime.dir), (ENTRANCE, 10));
    assert_eq!(chime.phrases, ["use chime", "ring chime"]);
    // Not listed among the room's exits.
    assert!(!g.listed_exits(SLUM_STREET).0.contains(10));

    g.choose_phrases(game_command);
    let t = Traveller {
        strength: Some(60),
        ..Traveller::default()
    };
    let r = route(&g, NEWHAVEN, SPECTRAL_THRONE, &t).expect("a route in");
    let cmds: Vec<&str> = r.iter().map(|s| s.command.as_str()).collect();
    let at = cmds
        .iter()
        .position(|&c| c == "ring chime")
        .expect("ring chime");
    assert_eq!(r[at].from, SLUM_STREET);
    assert_eq!(
        &cmds[at..],
        ["ring chime", "n", "n", "w", "move bookcase", "e", "n", "e"]
    );
}

#[test]
fn a_room_command_with_no_phrase_the_game_reads_is_not_taken() {
    let (_, mut g) = graph();
    // Every phrase claimed by a game command: none left.
    g.choose_phrases(|_| true);
    let t = Traveller {
        strength: Some(60),
        ..Traveller::default()
    };
    assert!(route(&g, NEWHAVEN, ENTRANCE, &t).is_none());
}

#[test]
fn a_level_gate_holds_a_low_character_back() {
    let (_, g) = graph();
    let vortex = g
        .edges((3, 25))
        .iter()
        .find(|e| e.kind == ExitType::RoomCommand)
        .expect("the vortex");
    assert_eq!(vortex.gate, Gate::Level { min: 20, max: 0 });
    let low = Traveller {
        level: Some(10),
        ..Traveller::default()
    };
    let high = Traveller {
        level: Some(20),
        ..Traveller::default()
    };
    assert!(!low.can_take(vortex));
    assert!(high.can_take(vortex));
    // The pool at 12:73 casts on the way: no way through.
    assert!(
        g.edges((12, 73))
            .iter()
            .all(|e| e.kind != ExitType::RoomCommand)
    );
}

#[test]
fn megamud_crossings_are_added_once_and_not_where_a_room_command_is() {
    let (pack, mut g) = graph();
    let k = PathKnowledge::load(&pack, &g).unwrap();
    let crossings: Vec<_> = k.crossings().collect();
    assert!(
        crossings.contains(&((7, 141), (7, 142), "go arch".to_owned())),
        "{crossings:?}"
    );
    // The chime is a room command already: no crossing.
    assert!(crossings.iter().all(|c| c.0 != SLUM_STREET));
    let added = g.learn_crossings(&k);
    assert_eq!(added, crossings.len());
    let arch = g
        .edges((7, 141))
        .iter()
        .find(|e| e.kind == ExitType::PathCommand)
        .expect("go arch");
    assert_eq!((arch.to, arch.command.as_str()), ((7, 142), "go arch"));
    assert!(arch.dir >= 10);
    assert!(arch.taken_by("Go Arch"));
    // The graph is whole after the rebuild: every room's exits still there.
    assert!(
        g.edges(SLUM_STREET)
            .iter()
            .any(|e| e.kind == ExitType::RoomCommand)
    );
    assert_eq!(g.learn_crossings(&k), 0, "known ones are not added twice");
}
