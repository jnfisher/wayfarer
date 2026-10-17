//! Build the pack from the fixtures and check it against MANIFEST.json, the export and the KB's
//! MegaMud numbers (`kb/megamud-formats.md`).

use std::path::{Path, PathBuf};

use wayfarer_world::{WorldPack, build};

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

// Slow (2-4 s, nearly all of this crate's test time): it builds the whole pack, 26,720 rooms and
// 1,957 paths walked. Everything that needs a built pack is checked here so it is built once
// (nextest runs each test in its own process).
#[test]
fn pack_from_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("world.sqlite");
    let report = build(
        &repo("tests/fixtures/world"),
        Some(&repo("assets/megamud")),
        &out,
    )
    .unwrap();

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

    // MegaMud results within a hair of the KB's.
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
    assert_eq!(pack.meta("schema_version").unwrap().as_deref(), Some("1"));
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
