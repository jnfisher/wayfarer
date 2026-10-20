//! The store's round trips, its schema versions, and what it does with a file it cannot read.

use rusqlite::Connection;
use wayfarer_store::{
    BotRecord, BotState, Error, Event, EventKind, Note, SCHEMA_VERSION, Sample, Store, Totals,
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
