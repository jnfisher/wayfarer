//! Path windows: a stretch of a MegaMud path that opens a sealed Hidden exit from elsewhere and
//! goes through it, on the pack built from the fixtures (`tests/fixtures/megamud-windows` adds
//! `Cry1cry2.mp`, which pulls the Crypt's two levers).

mod common;

use wayfarer_world::route::{Traveller, route};
use wayfarer_world::{Category, ExitType, Key, PathKnowledge, RoomGraph, WorldPack};

const CRYPT_ENTRY: Key = (1, 1028);
const LEVER_ONE: Key = (1, 1044);
const LEVER_TWO: Key = (1, 1038);
/// The Stone Hallway: north, sealed, to the Small Chamber.
const HALLWAY: Key = (1, 1056);
const CHAMBER: Key = (1, 1063);
const NORTH: u8 = 0;

/// The graph as the brain has it before the windows: the paths' crossings learned.
fn graph() -> (RoomGraph, PathKnowledge) {
    let pack = WorldPack::open(common::pack_path()).unwrap();
    let mut g = RoomGraph::load(&pack).unwrap();
    let k = PathKnowledge::load(&pack, &g).unwrap();
    g.learn_crossings(&k);
    let k = PathKnowledge::load(&pack, &g).unwrap();
    (g, k)
}

#[test]
fn the_crypt_exit_north_is_sealed_and_never_searched_for() {
    let (g, k) = graph();
    let e = g.edge(HALLWAY, NORTH).unwrap();
    // Two blocks, no `EXF_NEEDSEARCH`, and the levers are in other rooms.
    assert_eq!(
        (e.kind, e.params[0], e.params[1]),
        (ExitType::Hidden, 48, -2)
    );
    assert!(e.unblock.is_empty());
    assert!(e.sealed());
    // `Cry1cry2.mp` walks through it, but it is no exit to search for.
    assert!(
        k.exits("Cry1cry2.mp")
            .iter()
            .any(|x| (x.from, x.dir) == (HALLWAY, NORTH) && x.sealed)
    );
    assert!(!k.all_hidden_exits().contains(&(HALLWAY, NORTH)));
    assert!(!k.hidden_exits("Cry1cry2.mp").contains(&(HALLWAY, NORTH)));
    // Not even for a traveller who may search every hidden exit and knows it is there.
    let mut t = Traveller::default();
    t.allow.insert(Category::Hidden);
    t.searchable.insert((HALLWAY, NORTH));
    assert!(!t.can_take(e));
    // Showing (someone pulled the levers): taken, straight through.
    t.found.insert((HALLWAY, NORTH));
    assert!(t.can_take(e));
    let r = route(&g, HALLWAY, CHAMBER, &t).unwrap();
    assert_eq!(
        r.iter().map(|s| s.command.as_str()).collect::<Vec<_>>(),
        ["n"]
    );
}

#[test]
fn the_window_pulls_both_levers_then_goes_through() {
    let (mut g, k) = graph();
    let windows: Vec<_> = k.windows().collect();
    assert_eq!(windows.len(), 1, "{windows:?}");
    let w = windows[0];
    assert_eq!(w.file, "Cry1cry2.mp");
    // From the first lever (step 10 of the path) to the exit (step 38).
    assert_eq!((w.steps[0].from, w.steps.len()), (LEVER_ONE, 29));
    let typed: Vec<(Key, &str)> = w
        .steps
        .iter()
        .filter(|s| s.kind == ExitType::RemoteAction)
        .map(|s| (s.from, s.command.as_str()))
        .collect();
    assert_eq!(
        typed,
        [(LEVER_ONE, "pull lever"), (LEVER_TWO, "pull lever")]
    );
    assert!(
        w.steps
            .iter()
            .all(|s| s.kind != ExitType::RemoteAction || s.to == s.from)
    );
    let last = w.steps.last().unwrap();
    assert_eq!((last.from, last.dir, last.to), (HALLWAY, NORTH, CHAMBER));

    // Without it there is no way into the chamber; with it, the way is the path's.
    let t = Traveller::default();
    assert!(route(&g, CRYPT_ENTRY, CHAMBER, &t).is_none());
    assert_eq!(g.learn_windows(&k), 1);
    let edge = g
        .edges(LEVER_ONE)
        .iter()
        .find(|e| e.kind == ExitType::PathWindow)
        .unwrap();
    assert_eq!((edge.to, edge.command.as_str()), (CHAMBER, "Cry1cry2.mp"));
    assert!(edge.dir >= 10);
    assert!(!edge.taken_by("pull lever"));
    let r = route(&g, CRYPT_ENTRY, CHAMBER, &t).unwrap();
    let mut expected = vec![
        "n",
        "n",
        "n",
        "n",
        "w",
        "w",
        "s",
        "s",
        "s",
        "s",
        "pull lever",
    ];
    expected.extend(["n", "n", "n", "n", "e", "e", "e", "e", "s", "s", "s", "s"]);
    expected.extend(["pull lever", "n", "n", "n", "n", "w", "w"]);
    expected.extend(["n"; 9]);
    assert_eq!(
        r.iter().map(|s| s.command.as_str()).collect::<Vec<_>>(),
        expected
    );
    // The sealed exit is walked without a search; a lever leaves the character where it is.
    let through = r.last().unwrap();
    assert_eq!(
        (through.exit_type, through.passable),
        (ExitType::Hidden, true)
    );
    let lever = r
        .iter()
        .find(|s| s.exit_type == ExitType::RemoteAction)
        .unwrap();
    assert_eq!((lever.from, lever.to), (LEVER_ONE, LEVER_ONE));
    // A window costs its steps: from the hallway, back to the levers and round.
    let back = route(&g, HALLWAY, CHAMBER, &t).unwrap();
    assert_eq!((back[0].command.as_str(), back.len()), ("s", 43));
}
