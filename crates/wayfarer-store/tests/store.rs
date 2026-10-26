//! The store's round trips, its schema versions, and what it does with a file it cannot read.

use rusqlite::Connection;
use wayfarer_store::{
    BotRecord, BotState, CustomLoop, Error, Event, EventKind, Note, PartyRecord, Place, PlaceKind,
    RuleRecord, SCHEMA_VERSION, Sample, Store, Totals,
};

const T0: i64 = 1_792_411_200;

fn kill(at: i64, exp: i64) -> Event {
    Event {
        at,
        kind: EventKind::Kill,
        value: exp,
        detail: Some("giant rat".into()),
        room: Some((1, 2150)),
    }
}

fn event(at: i64, kind: EventKind) -> Event {
    Event {
        at,
        kind,
        value: 0,
        detail: None,
        room: None,
    }
}

#[test]
fn the_bot_state_round_trips_and_is_replaced() {
    let store = Store::in_memory().unwrap();
    assert_eq!(store.bot("aldric").unwrap(), None);
    let paused = BotRecord {
        state: BotState::Paused,
        home: Some((1, 2150)),
        loop_id: Some("1:2140:8:doors:0".into()),
        loop_stops: vec![(1, 2150), (1, 2152)],
        stop_index: 1,
        skip: vec![0],
        paused_reason: Some("you took over".into()),
        monster_rules: vec![
            RuleRecord {
                pattern: "gigantic black ooze".into(),
                rule: "never_fight".into(),
                source: "session".into(),
                at: T0 - 60,
            },
            RuleRecord {
                pattern: "giant rat".into(),
                rule: "flee_from".into(),
                source: "player".into(),
                at: T0 - 30,
            },
        ],
    };
    store.save_bot("aldric", &paused, T0).unwrap();
    assert_eq!(store.bot("aldric").unwrap().as_ref(), Some(&paused));
    assert_eq!(store.bot("other").unwrap(), None);
    let stopped = BotRecord::new(BotState::Stopped);
    store.save_bot("aldric", &stopped, T0 + 5).unwrap();
    assert_eq!(store.bot("aldric").unwrap(), Some(stopped));
}

#[test]
fn runs_count_their_events_and_keep_their_samples() {
    let store = Store::in_memory().unwrap();
    let run = store.start_run("aldric", T0, None).unwrap();
    assert_eq!(store.open_run("aldric").unwrap(), Some(run.clone()));
    let first = store.record(run.id, &kill(T0 + 10, 0)).unwrap();
    store.add_value(first, 9).unwrap();
    store.record(run.id, &kill(T0 + 40, 12)).unwrap();
    store
        .record(
            run.id,
            &Event {
                value: 5,
                ..event(T0 + 50, EventKind::Exp)
            },
        )
        .unwrap();
    store
        .record(run.id, &event(T0 + 60, EventKind::Rest))
        .unwrap();
    store
        .record(run.id, &event(T0 + 70, EventKind::Flee))
        .unwrap();
    store
        .record(run.id, &event(T0 + 80, EventKind::Death))
        .unwrap();
    for (i, exp) in [100, 121].into_iter().enumerate() {
        let s = Sample {
            at: T0 + 60 * (i as i64 + 1),
            exp: Some(exp),
            hp: Some(30),
        };
        store.sample(run.id, &s).unwrap();
    }
    store.end_run(run.id, T0 + 90).unwrap();

    assert_eq!(
        store.totals(run.id).unwrap(),
        Totals {
            kills: 2,
            deaths: 1,
            flees: 1,
            rests: 1,
            level_ups: 0,
            exp_gained: 26,
        }
    );
    let events = store.events(run.id).unwrap();
    assert_eq!(events.len(), 6);
    assert_eq!(events[0], kill(T0 + 10, 9));
    let samples = store.samples(run.id).unwrap();
    assert_eq!(
        samples.iter().map(|s| s.exp).collect::<Vec<_>>(),
        [Some(100), Some(121)]
    );
    assert_eq!(store.open_run("aldric").unwrap(), None);

    // The totals the brain keeps as it goes are the same sums.
    let mut t = Totals::default();
    for e in &events {
        t.add(e.kind, e.value);
    }
    assert_eq!(t, store.totals(run.id).unwrap());
}

#[test]
fn runs_list_newest_first_per_session() {
    let store = Store::in_memory().unwrap();
    for i in 0..12 {
        let r = store
            .start_run("aldric", T0 + i * 100, Some("loop"))
            .unwrap();
        store.record(r.id, &kill(T0 + i * 100 + 1, i)).unwrap();
        store.end_run(r.id, T0 + i * 100 + 50).unwrap();
    }
    store.start_run("other", T0, None).unwrap();
    let runs = store.runs("aldric", 10).unwrap();
    assert_eq!(runs.len(), 10);
    assert_eq!(runs[0].run.started_at, T0 + 1100);
    assert_eq!(runs[0].totals.exp_gained, 11);
    assert_eq!(runs[0].run.loop_id.as_deref(), Some("loop"));
    assert_eq!(runs[9].run.started_at, T0 + 200);
}

#[test]
fn other_open_runs_are_ended() {
    let store = Store::in_memory().unwrap();
    let a = store.start_run("aldric", T0, None).unwrap();
    let b = store.start_run("aldric", T0 + 10, None).unwrap();
    store.end_other_runs("aldric", Some(b.id), T0 + 20).unwrap();
    assert_eq!(store.open_run("aldric").unwrap().map(|r| r.id), Some(b.id));
    let runs = store.runs("aldric", 10).unwrap();
    assert_eq!(runs[1].run.id, a.id);
    assert_eq!(runs[1].run.ended_at, Some(T0 + 20));
    store.end_other_runs("aldric", None, T0 + 30).unwrap();
    assert_eq!(store.open_run("aldric").unwrap(), None);
}

#[test]
fn places_are_kept_per_session_newest_first() {
    let store = Store::in_memory().unwrap();
    let place = |at, kind, room| Place {
        at,
        kind,
        room,
        detail: None,
    };
    store
        .add_place("aldric", &place(T0, PlaceKind::Flee, (1, 5)))
        .unwrap();
    store
        .add_place("aldric", &place(T0 + 9, PlaceKind::Death, (1, 1217)))
        .unwrap();
    store
        .add_place("bryn", &place(T0 + 5, PlaceKind::Stopped, (2, 3)))
        .unwrap();
    let places = store.places("aldric", 10).unwrap();
    assert_eq!(
        places.iter().map(|p| (p.kind, p.room)).collect::<Vec<_>>(),
        [(PlaceKind::Death, (1, 1217)), (PlaceKind::Flee, (1, 5))]
    );
    assert_eq!(store.places("aldric", 1).unwrap().len(), 1);
}

#[test]
fn a_missing_store_is_created_and_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, note) = Store::open(&path).unwrap();
    assert_eq!(note, Some(Note::Created(path.clone())));
    assert_eq!(store.version().unwrap(), SCHEMA_VERSION);
    store
        .save_bot("aldric", &BotRecord::new(BotState::Running), T0)
        .unwrap();
    drop(store);
    let (store, note) = Store::open(&path).unwrap();
    assert_eq!(note, None);
    assert_eq!(
        store.bot("aldric").unwrap().unwrap().state,
        BotState::Running
    );
}

#[test]
fn a_corrupt_store_is_moved_aside_and_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    std::fs::write(&path, b"this is not a database, it is a shopping list").unwrap();
    let (store, note) = Store::open(&path).unwrap();
    let Some(Note::Recreated { moved_to, .. }) = &note else {
        panic!("{note:?}");
    };
    assert_eq!(moved_to, &dir.path().join("store.sqlite.corrupt"));
    assert!(moved_to.is_file());
    assert!(note.unwrap().to_string().contains("created afresh"));
    assert_eq!(store.bot("aldric").unwrap(), None);
    store
        .save_bot("aldric", &BotRecord::new(BotState::Paused), T0)
        .unwrap();
}

#[test]
fn a_store_missing_its_tables_is_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(&format!(
        "PRAGMA user_version = {SCHEMA_VERSION}; CREATE TABLE unrelated (x);"
    ))
    .unwrap();
    drop(conn);
    let (_, note) = Store::open(&path).unwrap();
    assert!(matches!(note, Some(Note::Recreated { .. })), "{note:?}");
}

#[test]
fn an_unversioned_file_is_migrated_and_a_newer_one_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    // An empty SQLite file at version 0: every migration runs.
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE keep (x); INSERT INTO keep VALUES (1);")
        .unwrap();
    let (store, note) = Store::open(&path).unwrap();
    assert_eq!(note, None);
    assert_eq!(store.version().unwrap(), SCHEMA_VERSION);
    drop(store);
    let kept: i64 = Connection::open(&path)
        .unwrap()
        .query_row("SELECT x FROM keep", [], |r| r.get(0))
        .unwrap();
    assert_eq!(kept, 1);

    let newer = dir.path().join("newer.sqlite");
    Connection::open(&newer)
        .unwrap()
        .execute_batch(&format!("PRAGMA user_version = {};", SCHEMA_VERSION + 1))
        .unwrap();
    let err = Store::open(&newer).unwrap_err();
    assert!(matches!(err, Error::Newer { .. }), "{err}");
    assert!(newer.is_file());
}

#[test]
fn a_version_1_store_keeps_its_bot_and_gains_the_monster_rules() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    // Version 1 as C-029 wrote it, with a bot saved.
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE bot_state (
                 session TEXT PRIMARY KEY, state TEXT NOT NULL, home_map INTEGER,
                 home_room INTEGER, loop_id TEXT, loop_stops TEXT NOT NULL DEFAULT '[]',
                 stop_index INTEGER NOT NULL DEFAULT 0, skip TEXT NOT NULL DEFAULT '[]',
                 paused_reason TEXT, updated_at INTEGER NOT NULL);
             CREATE TABLE runs (id INTEGER PRIMARY KEY, session TEXT NOT NULL,
                 started_at INTEGER NOT NULL, ended_at INTEGER, loop_id TEXT);
             CREATE TABLE events (id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL, at INTEGER
                 NOT NULL, kind TEXT NOT NULL, value INTEGER NOT NULL DEFAULT 0, detail TEXT,
                 room_map INTEGER, room_room INTEGER);
             CREATE TABLE samples (run_id INTEGER NOT NULL, at INTEGER NOT NULL, exp INTEGER,
                 hp INTEGER);
             INSERT INTO bot_state (session, state, home_map, home_room, updated_at)
                 VALUES ('aldric', 'running', 1, 2150, 0);
             PRAGMA user_version = 1;",
        )
        .unwrap();
    let (store, note) = Store::open(&path).unwrap();
    assert_eq!(note, None);
    assert_eq!(store.version().unwrap(), SCHEMA_VERSION);
    let bot = store.bot("aldric").unwrap().unwrap();
    assert_eq!(bot.home, Some((1, 2150)));
    assert!(bot.monster_rules.is_empty());
}

#[test]
fn boss_kills_are_kept_per_session_and_the_latest_wins() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, _) = Store::open(&path).unwrap();
    assert!(store.boss_kills("aldric").unwrap().is_empty());
    store.boss_killed("aldric", "Leo the Quick", T0).unwrap();
    store
        .boss_killed("aldric", "minotaur chieftain", T0 + 5)
        .unwrap();
    store
        .boss_killed("kasimir", "Leo the Quick", T0 + 9)
        .unwrap();
    // Killed again two hours later: the later kill replaces the earlier.
    store
        .boss_killed("aldric", "leo the quick", T0 + 7200)
        .unwrap();
    drop(store);
    // Kept across a reopen; names are lower case.
    let (store, _) = Store::open(&path).unwrap();
    let mut kills = store.boss_kills("aldric").unwrap();
    kills.sort();
    assert_eq!(
        kills,
        [
            ("leo the quick".to_owned(), T0 + 7200),
            ("minotaur chieftain".to_owned(), T0 + 5)
        ]
    );
    assert_eq!(
        store.boss_kills("kasimir").unwrap(),
        [("leo the quick".to_owned(), T0 + 9)]
    );
}

#[test]
fn custom_loops_round_trip_keep_their_order_and_are_replaced_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, _) = Store::open(&path).unwrap();
    assert!(store.custom_loops().unwrap().is_empty());
    let keep = CustomLoop {
        id: "custom:hidden-keep-with-leo".into(),
        name: "Hidden Keep with Leo".into(),
        stops: vec![(9, 749), (9, 870), (9, 750)],
        based_on: Some("HKEPLOOP.mp".into()),
    };
    let other = CustomLoop {
        id: "custom:a".into(),
        name: "A".into(),
        stops: vec![(1, 2150)],
        based_on: None,
    };
    store.save_custom_loop(&keep, T0).unwrap();
    store.save_custom_loop(&other, T0).unwrap();
    drop(store);
    // Kept across a reopen, by name; the order of the stops is the order saved.
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(store.custom_loops().unwrap(), [other.clone(), keep.clone()]);
    // The same id again replaces it.
    let longer = CustomLoop {
        stops: vec![(9, 749), (9, 870), (9, 750), (9, 751)],
        ..keep.clone()
    };
    store.save_custom_loop(&longer, T0 + 5).unwrap();
    assert_eq!(store.custom_loops().unwrap(), [other, longer]);
    assert!(
        store
            .delete_custom_loop("custom:hidden-keep-with-leo")
            .unwrap()
    );
    assert!(
        !store
            .delete_custom_loop("custom:hidden-keep-with-leo")
            .unwrap()
    );
    assert_eq!(store.custom_loops().unwrap().len(), 1);
}

#[test]
fn parties_round_trip_and_are_replaced_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, _) = Store::open(&path).unwrap();
    assert!(store.parties().unwrap().is_empty());
    let penny = PartyRecord {
        id: "party-1".into(),
        leader: "penny".into(),
        followers: vec!["lauren".into(), "faye".into()],
    };
    store.save_party(&penny, T0).unwrap();
    drop(store);
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(store.parties().unwrap(), std::slice::from_ref(&penny));
    let fewer = PartyRecord {
        followers: vec!["faye".into()],
        ..penny
    };
    store.save_party(&fewer, T0 + 5).unwrap();
    assert_eq!(store.parties().unwrap(), [fewer]);
    assert!(store.delete_party("party-1").unwrap());
    assert!(!store.delete_party("party-1").unwrap());
    assert!(store.parties().unwrap().is_empty());
}

#[test]
fn blessings_round_trip_per_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, _) = Store::open(&path).unwrap();
    assert!(store.blessings().unwrap().is_empty());
    let both = vec!["bles".to_owned(), "aura".to_owned()];
    store.save_blessings("lauren", &both, T0).unwrap();
    store
        .save_blessings("faye", &["skin".to_owned()], T0)
        .unwrap();
    drop(store);
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(
        store.blessings().unwrap(),
        [
            ("faye".to_owned(), vec!["skin".to_owned()]),
            ("lauren".to_owned(), both)
        ]
    );
    store.save_blessings("faye", &[], T0 + 1).unwrap();
    assert_eq!(store.blessings().unwrap().len(), 1);
}

#[test]
fn run_time_round_trips_per_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, _) = Store::open(&path).unwrap();
    let run = store.start_run("penny", T0, None).unwrap();
    assert_eq!(store.run_time(run.id).unwrap(), None);
    store.save_run_time(run.id, r#"{"fight":1}"#, T0).unwrap();
    store
        .save_run_time(run.id, r#"{"fight":2}"#, T0 + 1)
        .unwrap();
    drop(store);
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(
        store.run_time(run.id).unwrap().as_deref(),
        Some(r#"{"fight":2}"#)
    );
    assert_eq!(store.run_time(run.id + 1).unwrap(), None);
}

#[test]
fn combat_stats_round_trip_per_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(store.combat_stats("penny").unwrap(), None);
    store
        .save_combat_stats("penny", r#"{"since":1}"#, T0)
        .unwrap();
    store
        .save_combat_stats("penny", r#"{"since":2}"#, T0 + 1)
        .unwrap();
    drop(store);
    let (store, _) = Store::open(&path).unwrap();
    assert_eq!(
        store.combat_stats("penny").unwrap().as_deref(),
        Some(r#"{"since":2}"#)
    );
    assert_eq!(store.combat_stats("faye").unwrap(), None);
}
