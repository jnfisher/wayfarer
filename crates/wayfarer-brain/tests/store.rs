//! The store (docs/brain.md "Restarts" and "Statistics"): a brain with a store saves the bot's
//! state on every change and counts each run; a second brain on the same store resumes the bot
//! as the first left it. Against the fake gateway of `game/mod.rs`, in the Newhaven Arena, on
//! a paused clock that the brain's wall clock follows ([`InstantClock`]).

mod game;

use std::path::Path;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, InstantClock, SAMPLE_EVERY, Store};
use wayfarer_proto::{BotAction, BotState, CombatStats, RunSummary, StatsPart, TimeSpent};
use wayfarer_store::{BotRecord, EventKind};

fn brain_on(path: &Path) -> Brain {
    let (store, _) = Store::open(path).unwrap();
    let clock = InstantClock::new(UNIX_EPOCH + Duration::from_secs(T0));
    Brain::new(world(), Arc::new(clock), None).with_store(store)
}

/// `brain` attached in the Arena with `monsters`, the attach's re-sync done.
async fn arena_brain(brain: Brain, monsters: &[&str]) -> (Game, Run) {
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(monsters, 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    (g, run)
}

/// The store as the brain left it (opened again once the brain has gone).
fn store_at(path: &Path) -> Store {
    Store::open(path).unwrap().0
}

/// The narration so far: a resume is said before the catch-up's snapshot, which
/// `Game::start_brain` has already read.
fn said(g: &Game) -> Vec<String> {
    g.seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Notice { text, .. } | ToGateway::Warning { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn assert_said(g: &Game, text: &str) {
    let said = said(g);
    assert!(said.iter().any(|s| s == text), "{said:?}");
}

impl Game {
    async fn combat(&mut self) -> Option<CombatStats> {
        self.send(ToClient::StatsRequest { session: ID.into() })
            .await;
        self.until(|m| match m {
            ToGateway::Stats { combat, .. } => Some(combat.clone()),
            _ => None,
        })
        .await
    }

    async fn time(&mut self) -> Option<TimeSpent> {
        self.send(ToClient::StatsRequest { session: ID.into() })
            .await;
        self.until(|m| match m {
            ToGateway::Stats { time, .. } => Some(time.as_deref().copied()),
            _ => None,
        })
        .await
    }

    async fn stats(&mut self) -> (Vec<RunSummary>, usize) {
        self.send(ToClient::StatsRequest { session: ID.into() })
            .await;
        self.until(|m| match m {
            ToGateway::Stats { runs, samples, .. } => Some((runs.clone(), samples.len())),
            _ => None,
        })
        .await
    }
}

#[tokio::test(start_paused = true)]
async fn a_ui_asking_for_the_state_during_the_catch_up_does_not_stop_the_resume() {
    // Seen live (2026-10-26): with the web UI open, a restarted brain got its `state_request`
    // while catching up. The session was published early, the catch-up never counted as the
    // first, the bot was never resumed, and from then on nothing was saved: Start ran the bot
    // with no run, and the old run stayed open with its numbers frozen.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_brain(brain_on(&path), &[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.close(run).await;
    let (mut g, run) = Game::start_brain_with(
        SessionState::Realm,
        arena(&[], 35),
        brain_on(&path),
        Some(ToClient::StateRequest { session: None }),
    )
    .await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    assert_said(&g, "Resumed: fighting in Newhaven, Arena (brain restarted)");
    let (seen, _) = g.close(run).await;
    let ui = as_ui_sees(&seen);
    assert_eq!(bot(&seen).state, BotState::Running);
    assert_eq!(
        ui.run.expect("the run goes on").run_started_at,
        "2026-10-19T12:00:20Z"
    );
}

#[tokio::test(start_paused = true)]
async fn a_restarted_brain_resumes_the_bot_paused_with_its_home_and_stop_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");

    // The first brain: Start, a kill, a sample; gone while running. Its record is then made
    // paused, as a bot paused on a Go to leaves it (a journey is not stored).
    let (mut g, run) = arena_brain(brain_on(&path), &["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 33).await;
    g.expect("look", arena(&[], 33));
    g.output(&kill(33)).await;
    g.narrated("Waiting here").await;
    g.idle(SAMPLE_EVERY + Duration::from_secs(5)).await;
    let (seen, _) = g.close(run).await;
    let ui = as_ui_sees(&seen);
    let r = ui.run.expect("the run on the snapshot");
    assert_eq!((r.kills, r.exp_gained), (1, 9));
    assert_eq!(r.run_started_at, "2026-10-19T12:00:20Z");

    let store = store_at(&path);
    let mut saved = store.bot(ID).unwrap().unwrap();
    assert_eq!(saved.state, wayfarer_store::BotState::Running);
    assert_eq!(saved.home, Some(ARENA));
    saved.state = wayfarer_store::BotState::Paused;
    saved.paused_reason = Some("asked to pause".into());
    store.save_bot(ID, &saved, 0).unwrap();
    let open = store
        .open_run(ID)
        .unwrap()
        .expect("the run goes on while paused");
    let events = store.events(open.id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        (events[0].kind, events[0].value, events[0].room),
        (EventKind::Kill, 9, Some(ARENA))
    );
    let samples = store.samples(open.id).unwrap();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].hp, Some(33));
    drop(store);

    // The second brain, on the same store: paused again, home and all, and it says so.
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 33), brain_on(&path)).await;
    assert_said(
        &g,
        "Resumed paused (asked to pause): fighting in Newhaven, Arena (brain restarted)",
    );
    g.idle(Duration::from_secs(20)).await;
    let (runs, samples) = g.stats().await;
    assert_eq!(runs.len(), 1);
    assert_eq!(
        (
            runs[0].kills,
            runs[0].exp_gained,
            runs[0].ended_at.as_deref()
        ),
        (1, 9, None)
    );
    assert_eq!(samples, 1);
    // Resume: the re-sync, then on in the Arena.
    g.bot(BotAction::Resume).await;
    g.narrated("Resuming: re-syncing").await;
    g.narrated("Waiting here").await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (seen, _) = g.close(run).await;
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.bot.unwrap().state, BotState::Stopped);
    assert_eq!(ui.run, None);

    let store = store_at(&path);
    assert_eq!(
        store.bot(ID).unwrap().unwrap().state,
        wayfarer_store::BotState::Stopped
    );
    assert_eq!(store.open_run(ID).unwrap(), None);
    let runs = store.runs(ID, 10).unwrap();
    assert_eq!(runs.len(), 1, "the resumed run is the same run");
    assert_eq!((runs[0].totals.kills, runs[0].totals.exp_gained), (1, 9));
    assert!(runs[0].run.ended_at.is_some());
}

#[tokio::test(start_paused = true)]
async fn a_running_bot_resumes_running_and_fights() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let store = store_at(&path);
    let rec = BotRecord {
        home: Some(ARENA),
        ..BotRecord::new(wayfarer_store::BotState::Running)
    };
    store.save_bot(ID, &rec, T0 as i64 - 600).unwrap();
    let run = store.start_run(ID, T0 as i64 - 600, None).unwrap();
    drop(store);

    let (mut g, task) = Game::start_brain(
        SessionState::Realm,
        arena(&["large giant rat"], 35),
        brain_on(&path),
    )
    .await;
    assert_said(&g, "Resumed: fighting in Newhaven, Arena (brain restarted)");
    // The attach's re-sync first, then the fight.
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.expect("look", arena(&[], 35));
    g.output(&kill(35)).await;
    g.narrated("Waiting here").await;
    let (seen, played) = g.close(task).await;
    assert_eq!(played, ["a rat", "look"]);
    let r = as_ui_sees(&seen).run.expect("the run");
    // The run that was open goes on: started ten minutes before this brain.
    assert_eq!(r.run_started_at, "2026-10-19T11:50:00Z");
    assert_eq!(r.kills, 1);
    let store = store_at(&path);
    assert_eq!(store.open_run(ID).unwrap().map(|r| r.id), Some(run.id));
}

#[tokio::test(start_paused = true)]
async fn a_stored_loop_survives_and_its_stored_stops_win() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let store = store_at(&path);
    // Not the stops this id makes now (as after a level-up).
    let rec = BotRecord {
        home: Some(ARENA),
        loop_id: Some("1:2140:8:doors:0".into()),
        loop_stops: vec![ARENA, BEYOND_ARENA],
        stop_index: 1,
        skip: vec![0],
        ..BotRecord::new(wayfarer_store::BotState::Running)
    };
    store.save_bot(ID, &rec, T0 as i64).unwrap();
    drop(store);

    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_on(&path)).await;
    assert_said(
        &g,
        "Resumed: running loop 1:2140:8:doors:0 at stop 2/2 (brain restarted)",
    );
    let all = said(&g);
    assert!(
        all.iter().any(|s| s.contains("keeping the stored stops")),
        "{all:?}"
    );
    // The loop goes on at its stored stop, the Dungeon Entrance; the Arena, its other stored
    // stop, unticked.
    g.expect("n", room(BEYOND_ARENA));
    g.narrated("Loop 1:2140:8:doors:0: to stop 2/2 Dungeon, Entrance (one step, one door)")
        .await;
    g.narrated("Loop 1:2140:8:doors:0: stepping out of Dungeon, Entrance and back in")
        .await;
    g.freeze();
    g.bot(BotAction::Pause).await;
    g.narrated("Paused: asked to pause").await;
    g.close(run).await;
    let saved = store_at(&path).bot(ID).unwrap().unwrap();
    assert_eq!(saved.state, wayfarer_store::BotState::Paused);
    assert_eq!(
        (
            saved.loop_id,
            saved.loop_stops,
            saved.stop_index,
            saved.skip
        ),
        (rec.loop_id, rec.loop_stops, 1, vec![0])
    );
}

#[tokio::test(start_paused = true)]
async fn outside_the_realm_the_bot_is_not_resumed_and_its_run_ends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let store = store_at(&path);
    let rec = BotRecord::new(wayfarer_store::BotState::Running);
    store.save_bot(ID, &rec, T0 as i64).unwrap();
    store.start_run(ID, T0 as i64, None).unwrap();
    drop(store);

    let (g, run) =
        Game::start_brain(SessionState::Menu, "Main menu: ".into(), brain_on(&path)).await;
    assert_said(
        &g,
        "Not resuming the bot: the character is not in the realm",
    );
    g.close(run).await;
    let store = store_at(&path);
    assert_eq!(
        store.bot(ID).unwrap().unwrap().state,
        wayfarer_store::BotState::Stopped
    );
    assert_eq!(store.open_run(ID).unwrap(), None);
}

#[tokio::test(start_paused = true)]
async fn a_stopped_bot_stays_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    store_at(&path)
        .save_bot(ID, &BotRecord::new(wayfarer_store::BotState::Stopped), 0)
        .unwrap();
    let (g, run) = arena_brain(brain_on(&path), &["large giant rat"]).await;
    let (seen, played) = g.close(run).await;
    assert!(played.is_empty(), "{played:?}");
    let said: Vec<_> = seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Notice { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!said.iter().any(|s| s.starts_with("Resumed")), "{said:?}");
    assert_eq!(as_ui_sees(&seen).bot.unwrap().state, BotState::Stopped);
}

#[tokio::test(start_paused = true)]
async fn a_run_counts_kills_and_ends_with_a_death() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_brain(brain_on(&path), &["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.expect("look", arena(&[], 30));
    g.output(&kill(30)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(5 * 60)).await;
    g.output(&line("You have been killed!", -3)).await;
    g.narrated("Stopped: the character was killed").await;
    let (runs, samples) = g.stats().await;
    let (seen, _) = g.close(run).await;
    assert_eq!(as_ui_sees(&seen).run, None);
    assert_eq!(runs.len(), 1);
    let r = &runs[0];
    assert_eq!((r.kills, r.deaths, r.exp_gained), (1, 1, 9));
    assert!(r.ended_at.is_some());
    assert_eq!(r.minutes, 5);
    // 9 exp over five minutes and some seconds (the re-sync, the fight): 100 to 108 an hour.
    assert!((100..=108).contains(&r.exp_per_hour), "{r:?}");
    assert_eq!(samples, 5);
    let store = store_at(&path);
    let kinds: Vec<_> = store
        .events(store.runs(ID, 1).unwrap()[0].run.id)
        .unwrap()
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(kinds, [EventKind::Kill, EventKind::Death]);
    // Where it died is kept, with the stop after it, though the run is over.
    let places = as_ui_sees(&seen).places.unwrap();
    let kinds: Vec<_> = places.iter().map(|p| p.kind.as_str()).collect();
    assert_eq!(kinds, ["stopped", "death"], "{places:?}");
    assert_eq!(places[1].detail, None);
    assert!(!places[1].name.is_empty(), "{places:?}");
    assert_eq!(store.places(ID, 10).unwrap().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_reset_starts_a_new_run_at_zero_and_keeps_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_brain(brain_on(&path), &["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.expect("look", arena(&[], 35));
    g.output(&kill(35)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(120)).await;
    g.send(ToClient::StatsReset {
        session: ID.into(),
        only: None,
    })
    .await;
    g.narrated("Reset: the combat numbers are at zero, and a new run starts now (the old one is kept in the history)")
        .await;
    let (runs, _) = g.stats().await;
    let (seen, _) = g.close(run).await;
    // The snapshot's run is the new one, at zero; the history has both, newest first.
    let now = as_ui_sees(&seen).run.expect("a run");
    assert_eq!((now.kills, now.exp_gained), (0, 0));
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!((runs[0].kills, runs[0].ended_at.is_none()), (0, true));
    assert_eq!((runs[1].kills, runs[1].ended_at.is_some()), (1, true));
}

#[tokio::test(start_paused = true)]
async fn a_reset_with_no_run_zeroes_the_combat_numbers() {
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&[], 35)).await;
    g.idle(Duration::from_secs(20)).await;
    // The player's own fight, the bot stopped: it counts.
    g.output(&line(
        "You critically smack large giant rat for 34 damage!",
        35,
    ))
    .await;
    g.output(&line("You swing at large giant rat!", 35)).await;
    g.idle(Duration::from_secs(5)).await;
    let c = g.combat().await.expect("combat numbers");
    assert_eq!((c.swings, c.misses, c.accuracy_per_mille), (2, 1, 500));
    assert_eq!(c.rows[0].kind, "crit");
    g.send(ToClient::StatsReset {
        session: ID.into(),
        only: None,
    })
    .await;
    g.narrated("Reset: the combat numbers are at zero").await;
    assert_eq!(g.combat().await, None);
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_run_and_the_combat_numbers_reset_apart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_brain(brain_on(&path), &["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.output(&line("You smack large giant rat for 9 damage!", 35))
        .await;
    g.expect("look", arena(&[], 35));
    g.output(&kill(35)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(10)).await;
    // The run alone: a new one at zero, the combat numbers kept.
    g.send(ToClient::StatsReset {
        session: ID.into(),
        only: Some(StatsPart::Run),
    })
    .await;
    g.narrated("Reset: a new run starts now (the old one is kept in the history)")
        .await;
    let (runs, _) = g.stats().await;
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!(runs[0].kills, 0);
    let c = g.combat().await.expect("combat numbers kept");
    assert_eq!(c.kills, 1);
    // The combat numbers alone: the run goes on.
    g.send(ToClient::StatsReset {
        session: ID.into(),
        only: Some(StatsPart::Combat),
    })
    .await;
    g.narrated("Reset: the combat numbers are at zero").await;
    assert_eq!(g.combat().await, None);
    let (runs, _) = g.stats().await;
    assert_eq!(runs.len(), 2, "no new run: {runs:?}");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn a_run_reset_with_no_run_says_so() {
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&[], 35)).await;
    g.output(&line("You smack large giant rat for 9 damage!", 35))
        .await;
    g.send(ToClient::StatsReset {
        session: ID.into(),
        only: Some(StatsPart::Run),
    })
    .await;
    g.narrated("Reset: no run to reset").await;
    assert!(g.combat().await.is_some(), "the combat numbers are kept");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_runs_time_is_counted_kept_over_a_restart_and_reset_with_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_brain(brain_on(&path), &["large giant rat"]).await;
    assert_eq!(g.time().await, None, "no run, no time");
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    for hp in [33, 30, 30, 28] {
        g.round("The large giant rat lunges at you!", hp).await;
        g.round("You whap large giant rat for 6 damage!", hp).await;
    }
    g.expect("look", arena(&[], 28));
    g.output(&kill(28)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let t = g.time().await.expect("a run's time");
    assert!(t.fight_secs >= 15, "{t:?}");
    // The minute after the kill, and the re-sync before the first attack.
    assert!((60..=80).contains(&t.wait_secs), "{t:?}");
    assert_eq!((t.walk_secs, t.flee_secs, t.away_secs), (0, 0, 0), "{t:?}");
    g.close(run).await;
    // Restarted: the run goes on with the time it had, and the bot waits on in the Arena.
    let (mut g, run) = arena_brain(brain_on(&path), &[]).await;
    let again = g.time().await.expect("the run goes on");
    assert_eq!(again.fight_secs, t.fight_secs, "{again:?}");
    assert!(again.wait_secs >= t.wait_secs + 15, "{again:?}");
    // A new run counts from zero.
    g.send(ToClient::StatsReset {
        session: ID.into(),
        only: Some(StatsPart::Run),
    })
    .await;
    g.narrated("Reset: a new run starts now (the old one is kept in the history)")
        .await;
    let fresh = g.time().await.expect("the new run");
    assert_eq!((fresh.fight_secs, fresh.wait_secs), (0, 0), "{fresh:?}");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_combat_numbers_survive_a_restart_of_the_brain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_brain(brain_on(&path), &[]).await;
    g.output(&line("You smack large giant rat for 9 damage!", 35))
        .await;
    g.output(&line("You smack large giant rat for 4 damage!", 35))
        .await;
    // Saved within COMBAT_SAVE_EVERY (5 s).
    g.idle(Duration::from_secs(10)).await;
    g.close(run).await;
    let (mut g, run) = arena_brain(brain_on(&path), &[]).await;
    let c = g.combat().await.expect("combat numbers");
    let hit = &c.rows[0];
    assert_eq!(
        (hit.kind.as_str(), hit.count, hit.min, hit.max),
        ("hit", 2, 4, 9)
    );
    // The two blows came together: one round of 13.
    let round = c.round.expect("a round");
    assert_eq!((round.count, round.max), (1, 13));
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn without_a_store_the_run_is_still_counted() {
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&["large giant rat"], 35)).await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.expect("look", arena(&[], 35));
    g.output(&kill(35)).await;
    g.narrated("Waiting here").await;
    let (runs, samples) = g.stats().await;
    let (seen, _) = g.close(run).await;
    assert_eq!(as_ui_sees(&seen).run.map(|r| r.kills), Some(1));
    assert_eq!(runs.len(), 1);
    assert_eq!((runs[0].kills, runs[0].exp_gained), (1, 9));
    assert_eq!(samples, 0);
}

/// Fighting the rat in the Arena, at 20 of 35 HP (57%).
async fn fighting_on(path: &Path) -> (Game, Run) {
    fighting_in(brain_on(path)).await
}

/// [`fighting_on`] with `brain`.
async fn fighting_in(brain: Brain) -> (Game, Run) {
    let (mut g, run) = arena_brain(brain, &["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    // Twice: the attack's answer may come after the first.
    for _ in 0..2 {
        g.round("The large giant rat lunges at you!", 20).await;
    }
    (g, run)
}

/// The flee events of the session's newest run: (HP per cent, attacker, room).
fn flees(path: &Path) -> Vec<(i64, Option<String>, Option<Key>)> {
    let store = store_at(path);
    store
        .events(store.runs(ID, 1).unwrap()[0].run.id)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EventKind::Flee)
        .map(|e| (e.value, e.detail, e.room))
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_flee_is_counted_in_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = fighting_on(&path).await;
    // 8 of 35 is 22%: up to the Narrow Road.
    g.expect("u", at_hp(&room(NARROW_ROAD), 8));
    g.output(&line("The large giant rat lunges at you!", 8))
        .await;
    g.narrated("Safe in Newhaven, Narrow Road").await;
    let (runs, _) = g.stats().await;
    assert_eq!(runs[0].flees, 1);
    assert_eq!(as_ui_sees(&g.seen).run.map(|r| r.flees), Some(1));
    g.close(run).await;
    let rat = Some("large giant rat".to_owned());
    assert_eq!(flees(&path), [(22, rat, Some(ARENA))]);
}

#[tokio::test(start_paused = true)]
async fn a_flee_before_a_log_off_is_counted_before_the_run_ends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    // Only a clean exit flees first; a hangup leaves at once.
    use wayfarer_settings::{Layer, Overrides, Settings};
    let clean = Overrides::parse("[bot]\nlog_off = \"clean\"\n").unwrap();
    let settings = Settings::resolve(&[(Layer::Character, &clean)]).unwrap();
    let (mut g, run) = fighting_in(brain_on(&path).with_settings(settings)).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.send(ToClient::LoggingOff { session: ID.into() }).await;
    g.until(|m| matches!(m, ToGateway::ReadyToLeave { .. }).then_some(()))
        .await;
    g.close(run).await;
    assert!(
        store_at(&path).runs(ID, 1).unwrap()[0]
            .run
            .ended_at
            .is_some()
    );
    let rat = Some("large giant rat".to_owned());
    assert_eq!(flees(&path), [(57, rat, Some(ARENA))]);
}
