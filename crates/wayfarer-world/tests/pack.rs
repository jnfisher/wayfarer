//! Build the pack from the fixtures and check it against MANIFEST.json and the export. MegaMud's
//! path set is not in this repository (John, 2026-10-17); `megamud_paths_verify` checks it
//! against a copy outside the repository and only runs when asked.

#[path = "../../wayfarer-parse/tests/common/mod.rs"]
mod parse_common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use wayfarer_parse::{GameTexts, Registry};
use wayfarer_world::{WorldPack, build};

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

// Slow (3-5 s, nearly all of this crate's test time): it builds the whole pack, 26,720 rooms and
// 1,957 paths walked, then the parser's registry from it and the golden fixtures on that. Everything
// that needs a built pack is checked here so it is built once (nextest runs each test in its own
// process).
#[test]
fn pack_from_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("world.sqlite");
    let report = build(&repo("tests/fixtures/world"), None, &out).unwrap();

    // Row counts equal MANIFEST.json for every content table.
    let want = [
        ("rooms", 26_720),
        ("items", 1_950),
        ("monsters", 1_101),
        ("spells", 1_379),
        ("shops", 178),
        ("messages", 3_867),
        ("texts", 3_467),
        ("classes", 15),
        ("races", 13),
        ("actions", 64),
    ];
    for (table, rows) in want {
        let (_, _, got, manifest) = report
            .tables
            .iter()
            .find(|t| t.1 == table)
            .unwrap_or_else(|| panic!("{table} not built"));
        assert_eq!((*got, *manifest), (rows, rows), "{table}");
    }
    assert!(
        report
            .runtime
            .iter()
            .any(|(t, n)| t == "WCCACMSR" && *n == 795)
    );

    assert!(report.megamud.is_none());

    let pack = WorldPack::open(&out).unwrap();
    assert_eq!(pack.meta("schema_version").unwrap().as_deref(), Some("1"));
    let classes = pack.classes().unwrap();
    assert_eq!(classes.len(), 15);
    assert!(classes.iter().any(|(_, n)| n == "Warrior"));
    assert!(pack.races().unwrap().iter().any(|(_, n)| n == "Human"));
    let shops = pack.shop_types().unwrap();
    assert_eq!(shops.len(), 178);
    assert!(shops.iter().any(|&(_, t)| t == 7), "a bank");
    assert!(
        pack.meta("source.WCCMP001.file_sha256")
            .unwrap()
            .unwrap()
            .starts_with("7ebe0ff9")
    );

    // Room 1/1 with the exits and types the export holds.
    let gates = pack.room(1, 1).unwrap().unwrap();
    assert_eq!(gates.name, "Town Gates");
    assert_eq!(gates.mm_id, 0x4EB0_0065);
    let exits: Vec<_> = pack
        .exits(1, 1)
        .unwrap()
        .into_iter()
        .map(|e| (e.dir, e.to_map, e.to_room, e.exit_type, e.params))
        .collect();
    assert_eq!(
        exits,
        [
            (0, 1, 3, "Block Guard".to_string(), [1, 0, 0, 0]),
            (1, 1, 100, "Block Guard".to_string(), [1, 0, 0, 0]),
            (2, 1, 1381, "Gate".to_string(), [2, 0, 0, 0]),
            (3, 1, 101, "Normal".to_string(), [8, 2, 1, 0]),
        ]
    );
    assert!(
        pack.room_raw(1, 1)
            .unwrap()
            .unwrap()
            .contains("\"mapfile\"")
    );
    assert!(pack.room(1, 999_999).unwrap().is_none());
    assert!(
        pack.rooms_by_name("Town Gates")
            .unwrap()
            .iter()
            .any(|r| (r.map, r.room) == (1, 1))
    );
    let shops = pack.rooms_by_mm_id(0x4BA0_0010).unwrap();
    assert_eq!(
        shops.iter().map(|r| (r.map, r.room)).collect::<Vec<_>>(),
        [(1, 2307)]
    );

    // A known monster, item, shop and spell by number.
    let orc = pack.monster(6).unwrap().unwrap();
    assert_eq!(
        (orc.name.as_str(), orc.exp, orc.max_hp),
        ("orc rogue", 22, 30)
    );
    let robes = pack.item(9).unwrap().unwrap();
    assert_eq!((robes.name.as_str(), robes.cost), ("grey robes", 5));
    let swords = pack.shop(5).unwrap().unwrap();
    assert_eq!(swords.name, "Sword Shop");
    assert_eq!(swords.stock.len(), 19);
    assert_eq!(swords.stock[0], (64, 32));
    assert_eq!(pack.item(64).unwrap().unwrap().name, "longsword");
    let mm = pack.spell(1).unwrap().unwrap();
    assert_eq!(
        (mm.name.as_str(), mm.short_name.as_str()),
        ("magic missile", "mmis")
    );

    // A known text block decodes to readable text; a 62-part block joins in order.
    let old_man = pack.text_block(2).unwrap().unwrap();
    assert!(
        old_man.starts_with("The old man looks you over, and with a grin says"),
        "{old_man:?}"
    );
    assert!(pack.text_block(4684).unwrap().unwrap().len() > 60_000);
    assert!(pack.text_block(-5).unwrap().is_none());
    assert!(pack.message(1).unwrap().is_some());

    // No MegaMud data was given, so its tables are empty.
    assert!(pack.waypoints().unwrap().is_empty());
    assert!(pack.loops().unwrap().is_empty());
    assert!(pack.paths().unwrap().is_empty());
    game_texts_equal_the_export(&pack);
}

/// The parser's game texts from the pack are the export's, record for record, so the registry
/// has the same patterns in the same order; the golden fixtures pass on it.
fn game_texts_equal_the_export(pack: &WorldPack) {
    let t = Instant::now();
    let from_pack = pack.game_texts().unwrap();
    eprintln!("WorldPack::game_texts: {:?}", t.elapsed());
    let from_export = GameTexts::from_export(repo("tests/fixtures/world")).unwrap();
    assert_eq!(from_pack.messages.len(), 3_867);
    assert_eq!(from_pack.monsters.len(), 1_101);
    assert_eq!(from_pack.room_names.len(), 26_720);
    assert!(from_pack == from_export, "pack and export texts differ");

    let (a, b) = (Registry::new(&from_pack), Registry::new(&from_export));
    let key = |r: &Registry| -> Vec<(String, String)> {
        r.patterns()
            .iter()
            .map(|p| (p.name.clone(), p.source.clone()))
            .collect()
    };
    assert!(a.patterns().len() > 6_000);
    assert_eq!(a.patterns().len(), b.patterns().len());
    assert!(key(&a) == key(&b), "pack and export registries differ");
    assert_eq!(a.room_name_count(), b.room_name_count());

    parse_common::golden_parity(&Arc::new(a));
}

#[test]
fn opening_something_else_fails() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.sqlite");
    // An empty file is a valid, empty SQLite database: no pack_meta.
    std::fs::write(&path, b"").unwrap();
    assert!(WorldPack::open(&path).is_err());
    assert!(WorldPack::open(&dir.path().join("missing.sqlite")).is_err());
}

#[test]
fn building_from_a_missing_export_fails_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("world.sqlite");
    assert!(build(&dir.path().join("nope"), None, &out).is_err());
    assert!(!out.exists());
}

/// MegaMud's path set, checked against the real rooms: `data/megamud` (stock v2.2), or the
/// directory or zip in `WAYFARER_MEGAMUD`. Run it with
/// `cargo nextest run -p wayfarer-world --run-ignored only`.
// Slow (builds the whole pack and walks 1,957 paths); never run in CI.
#[test]
#[ignore = "slow: builds the whole pack and walks every MegaMud path"]
fn megamud_paths_verify() {
    let megamud = std::env::var_os("WAYFARER_MEGAMUD")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo("data/megamud"));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("world.sqlite");
    let report = build(&repo("tests/fixtures/world"), Some(&megamud), &out).unwrap();

    // MegaMud results within a hair of the KB's (`kb/megamud-formats.md`).
    let m = report.megamud.as_ref().unwrap();
    assert_eq!(m.waypoints, 783);
    assert!(m.waypoints_resolved >= 720, "{m:?}");
    assert_eq!(m.paths, 1_957);
    assert_eq!(m.loops, 195);
    assert!(
        m.steps_exact * 100 >= m.steps_title_matched() * 99,
        "exact {} of {}",
        m.steps_exact,
        m.steps_title_matched()
    );
    assert!(m.steps_title_matched() >= 43_000, "{m:?}");
    assert!(m.loops_closed >= 170, "{m:?}");

    let pack = WorldPack::open(&out).unwrap();
    // Waypoints and loops with their verification.
    let wps = pack.waypoints().unwrap();
    assert_eq!(wps.len(), 783);
    let aaly = wps.iter().find(|w| w.code == "AALY").unwrap();
    assert_eq!(aaly.rooms, [(3, 606)]);
    assert_eq!(wps.iter().filter(|w| w.is_bank).count(), 5);
    let loops = pack.loops().unwrap();
    assert_eq!(loops.len(), 195);
    let slmb = loops.iter().find(|l| l.file == "SLMBLOOP.mp").unwrap();
    assert_eq!(slmb.description, "Black Star Key Gather Loop");
    let v = &slmb.verification;
    assert!(v.walked && v.closes, "{v:?}");
    assert_eq!(
        (v.start, v.steps_checked, v.steps_exact),
        (Some((1, 1224)), 40, 40)
    );
    let steps = pack.path_steps("SLMBLOOP.mp").unwrap();
    assert_eq!(steps.len(), 40);
    assert_eq!(steps[0].walk, Some(((1, 1224), "exact".to_string())));
    assert_eq!(pack.paths().unwrap().len(), 1_957);

    // The loops wayfarer may list or run: the 183 that walk to the end, all closing; those
    // through Newhaven (radius 8) among them.
    let mut graph = wayfarer_world::RoomGraph::load(&pack).unwrap();
    let monsters = wayfarer_world::Monsters::load(&pack).unwrap();
    let mm = wayfarer_world::loops::megamud_loops(&pack, &graph, &monsters).unwrap();
    assert_eq!(mm.len(), 183);
    assert!(mm.iter().all(|l| wayfarer_world::loops::verified(&l.path)));
    let slmb = mm.iter().find(|l| l.path.file == "SLMBLOOP.mp").unwrap();
    assert_eq!(slmb.rooms.first(), Some(&(1, 1224)));
    assert_eq!(slmb.rooms.last(), Some(&(1, 1224)));
    let area = wayfarer_world::Area {
        anchor: (1, 2140),
        radius: 8,
    }
    .rooms(&graph, &wayfarer_world::Traveller::default());
    let through = wayfarer_world::loops::megamud_through(&mm, &area);
    eprintln!(
        "MegaMud loops through Newhaven: {:?}",
        through.iter().map(|l| &l.path.file).collect::<Vec<_>>()
    );
    assert!(through.iter().all(|l| l.path.verification.closes));

    // As tasks: seven loops have nothing on them to hunt (the Oasis, the Lost City, the
    // Storm Giant Fortress, the Small Pyramid, the White Forest); the rest have stops.
    let idle: Vec<&str> = mm
        .iter()
        .filter(|l| l.not_runnable().is_some())
        .map(|l| l.path.file.as_str())
        .collect();
    assert_eq!(idle.len(), 7, "{idle:?}");
    let wooden = mm.iter().find(|l| l.path.file == "Mwodloop.mp").unwrap();
    assert!(!wooden.stops().contains(&(1, 1865)));
    // What the paths know: the hidden exit into the keep, and of the 183 loops' legs between
    // stops, those no route takes for a strong, high-level character (an Item, Cast or Level
    // exit, a door no bash opens): printed, for the record.
    let knowledge = wayfarer_world::PathKnowledge::load(&pack, &graph).unwrap();
    assert!(
        knowledge
            .hidden_exits("Ftrehkep.mp")
            .contains(&((9, 724), 0))
    );
    // The path windows, printed for the record: the Crypt's two levers before 1/1056 north
    // among them, and the sealed exit never searched for.
    for w in knowledge.windows() {
        let last = w.steps.last().unwrap();
        let typed: Vec<&str> = w
            .steps
            .iter()
            .filter(|s| s.kind == wayfarer_world::ExitType::RemoteAction)
            .map(|s| s.command.as_str())
            .collect();
        eprintln!(
            "window {}: {}/{} to {}/{} dir {}, {} steps, typing {typed:?}",
            w.file,
            w.steps[0].from.0,
            w.steps[0].from.1,
            last.from.0,
            last.from.1,
            last.dir,
            w.steps.len()
        );
    }
    let crypt = knowledge
        .windows()
        .find(|w| {
            w.steps
                .last()
                .is_some_and(|s| (s.from, s.dir) == ((1, 1056), 0))
        })
        .expect("the Crypt's window");
    assert_eq!(crypt.steps[0].from, (1, 1044));
    assert!(!knowledge.all_hidden_exits().contains(&((1, 1056), 0)));
    let windows = graph.learn_windows(&knowledge);
    eprintln!("path windows: {windows}");
    assert!(windows >= 20, "{windows}");
    let strong = wayfarer_world::Traveller {
        level: Some(60),
        strength: Some(100),
        picklocks: Some(100),
        ..wayfarer_world::Traveller::default()
    };
    let unroutable: Vec<&str> = mm
        .iter()
        .filter(|l| {
            let mut t = strong.clone();
            t.searchable = knowledge.hidden_exits(&l.path.file);
            let s = l.stops();
            (0..s.len())
                .any(|i| wayfarer_world::route(&graph, s[i], s[(i + 1) % s.len()], &t).is_none())
        })
        .map(|l| l.path.file.as_str())
        .collect();
    eprintln!("MegaMud loops with a leg no route takes: {unroutable:?}");
    assert!(unroutable.len() < 20, "{unroutable:?}");
}
