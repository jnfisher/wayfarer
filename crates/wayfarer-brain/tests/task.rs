//! Running a loop (docs/brain.md "Running a loop"): `run_loop` on the Newhaven loop of the
//! fixture world (the Arena, the Dungeon Entrance behind its door, the Small Cavern north of
//! that), against the fake gateway of `game/mod.rs`, whose game answers each command from the
//! test's script with texts cut from the `room-walk` and `fight-arena` captures. On a paused
//! clock: waits (the five minutes a stop passed by is left alone) take no time.

mod game;

use std::path::Path;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, GIVE_UPS, InstantClock, LEG_TRIES, MAX_BASHES, Store};
use wayfarer_proto::{BotAction, BotMode, BotState};

const NEWHAVEN_LOOP: &str = "1:2140:8:doors:0";
/// North of the Dungeon Entrance: a Lair of cave bears, the stop the tests untick.
const SMALL_CAVERN: Key = (1, 2156);

impl Game {
    async fn run_loop(&self, skip: &[Key]) {
        self.send(ToClient::RunLoop {
            session: ID.into(),
            id: NEWHAVEN_LOOP.into(),
            skip: Some(skip.iter().map(|&k| key(k)).collect()),
        })
        .await;
    }
}

/// A brain attached at `at`, its first re-sync done, ready for `run_loop`: its re-sync's `st`
/// and `look` are answered with full HP and the room.
async fn ready_at(at: Key) -> (Game, Run) {
    let (mut g, run) = Game::start(SessionState::Realm, at).await;
    g.pipelining = true;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", room(at));
    (g, run)
}

/// The same, the history and the re-sync's room being the Arena as `arena` shows it (the
/// caller expects the `look`).
async fn ready_with(history: String) -> (Game, Run) {
    let hp = if history.contains("[HP=30") { 30 } else { 35 };
    let (mut g, run) = Game::start_with(SessionState::Realm, history).await;
    g.pipelining = true;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(hp));
    (g, run)
}

/// The Dungeon Entrance as the game shows it after `n` from the Arena.
fn dungeon_entrance() -> String {
    room(BEYOND_ARENA)
}

/// The InputLines the brain sent, re-sync included.
fn sent(seen: &[ToGateway]) -> Vec<String> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::InputLine { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// From the Village Entrance: to the Arena (three steps, the second and third pipelined), a
/// rat fought there, on to the Dungeon Entrance (its locked door bashed), the Small Cavern
/// passed by (unticked), back to the Arena, then the Dungeon Entrance and the Arena again: no
/// stop waits for a respawn clock.
async fn round_the_loop() -> (Vec<ToGateway>, Vec<String>, Vec<String>) {
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&["large giant rat"], 35));
    g.expect("a rat", line(ENGAGED, 35));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Running loop Newhaven: re-syncing").await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (3 steps)")
        .await;
    g.narrated("Fight: large giant rat").await;
    for hp in [33, 31] {
        g.round("The large giant rat lunges at you!", hp).await;
        g.round("You whap large giant rat for 6 damage!", hp).await;
    }
    // Back to full HP with the kill, so the wait for respawns is no rest.
    g.expect("look", arena(&[], 35));
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", dungeon_entrance());
    g.expect("s", arena(&[], 35));
    g.output(&kill(35)).await;
    g.narrated("Stop 1/3 cleared: 1 large giant rat").await;
    g.narrated("Loop Newhaven: to stop 2/3 Dungeon, Entrance (one step, one door)")
        .await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.narrated("Skipping Small Cavern (unticked)").await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (one step)")
        .await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    // No respawn clock: on to the Dungeon Entrance at once, and the round goes on.
    g.expect("n", dungeon_entrance());
    g.expect("s", arena(&[], 35));
    g.narrated("Loop Newhaven: to stop 2/3 Dungeon, Entrance (one step, one door)")
        .await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (one step)")
        .await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    let said = notices(&seen);
    (seen, played, said)
}

#[tokio::test(start_paused = true)]
async fn run_walks_to_the_nearest_stop_fights_moves_on_skips_and_waits() {
    let (seen, played, said) = round_the_loop().await;
    // After the re-sync's `st` and `look`.
    assert_eq!(
        played,
        [
            "st", "look", "w", "w", "d", "a rat", "look", "n", "bash n", "n", "s", "n", "s"
        ]
    );
    // One attack, and nothing at all sent while engaged.
    let lines = sent(&seen);
    assert_eq!(lines.iter().filter(|l| l.starts_with("a ")).count(), 1);
    let engaged = lines.iter().position(|l| l == "a rat").unwrap();
    assert_eq!(lines[engaged + 1], "look", "{lines:?}");
    // Never to the unticked Small Cavern.
    assert!(!said.iter().any(|s| s.contains("to stop 3/3")), "{said:?}");
    let b = bot(&seen);
    assert_eq!((b.state, b.mode), (BotState::Running, BotMode::Task));
    let task = b.task.expect("the loop on the snapshot");
    assert_eq!(
        (task.loop_id.as_str(), task.loop_name.as_str(), task.stop),
        (NEWHAVEN_LOOP, "Newhaven", 1)
    );
    assert_eq!(task.stops, [ARENA, BEYOND_ARENA, SMALL_CAVERN].map(key));
    assert_eq!(task.skip, [key(SMALL_CAVERN)]);
}

#[tokio::test(start_paused = true)]
async fn the_same_game_gives_the_same_commands() {
    let (_, played_a, said_a) = round_the_loop().await;
    let (_, played_b, said_b) = round_the_loop().await;
    assert_eq!((played_a, said_a), (played_b, said_b));
}

#[tokio::test(start_paused = true)]
async fn moves_along_a_known_route_are_pipelined_two_at_a_time() {
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&[], 35));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    // The second `w` went while the first was in flight, and `d` while the second was.
    assert_eq!(g.ahead, ["w", "d"]);
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", dungeon_entrance());
    g.narrated("Stop 2/3 cleared: nothing there").await;
    // A door on the way: nothing goes ahead of it.
    assert_eq!(g.ahead, ["w", "d"]);
    g.expect("s", arena(&[], 35));
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn two_rooms_in_one_chunk_confirm_both_pipelined_moves() {
    // Seen live: the game answered both `w`s before the brain read either, and both rooms came
    // in one chunk. Each move is confirmed by its own room, not by where the belief ended up.
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    g.expect(
        "w",
        format!("{}w\r\n{}", room(NARROW_PATH), room(NARROW_ROAD)),
    );
    g.expect("w", NOTHING);
    g.expect("d", arena(&[], 35));
    g.run_loop(&[SMALL_CAVERN]).await;
    let said = g.narrated("Stop 1/3 cleared: nothing there").await;
    assert!(!said.iter().any(|s| s.starts_with("Replanned")), "{said:?}");
    assert_eq!(g.ahead, ["w", "d"]);
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", dungeon_entrance());
    g.expect("s", arena(&[], 35));
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn a_room_that_disagrees_voids_the_move_ahead_and_the_walk_replans() {
    // The brain believes it is at the Village Entrance; the first `w` shows the Narrow Road
    // (it was on the Narrow Path). The second `w`, already gone, is refused there.
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_ROAD));
    g.expect("w", says("There is no exit in that direction!"));
    g.expect("d", arena(&[], 35));
    g.run_loop(&[SMALL_CAVERN]).await;
    let said = g.narrated("Stop 1/3 cleared: nothing there").await;
    assert!(
        said.iter()
            .any(|s| s == "Replanned (a move went ahead of a wrong step): one step"),
        "{said:?}"
    );
    // Nothing went ahead after the wrong room: the `d` waited for the replan.
    assert_eq!(g.ahead, ["w"]);
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", dungeon_entrance());
    g.expect("s", arena(&[], 35));
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    let (_, played) = g.close(run).await;
    assert_eq!(&played[..5], ["st", "look", "w", "w", "d"]);
}

#[tokio::test(start_paused = true)]
async fn combat_on_the_way_stops_the_leg_after_the_moves_in_flight_and_fight_takes_over() {
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    let path = room(NARROW_PATH);
    let cut = path.rfind(PROMPT).unwrap();
    g.expect(
        "w",
        format!("{}\x1b[1;31m*Combat Engaged*\r\n{PROMPT}", &path[..cut]),
    );
    // Already gone when the fight began: answered, and nothing goes after it.
    g.expect("w", room(NARROW_ROAD));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Stopped walking: combat started").await;
    g.idle(Duration::from_secs(8)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "w", "w"]);
    assert_eq!(bot(&seen).mode, BotMode::Fight);
}

#[tokio::test(start_paused = true)]
async fn a_hostile_monster_on_the_way_stops_the_leg_and_the_loop_goes_on_after_the_fight() {
    // From the Dungeon Entrance the nearest stop is itself; then the Arena, whose rat Fight
    // takes, then back to the loop.
    let (mut g, run) = ready_at(NARROW_ROAD).await;
    g.expect("d", arena(&["large giant rat"], 35));
    g.expect("a rat", line(ENGAGED, 35));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 33).await;
    g.expect("look", arena(&[], 35));
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", dungeon_entrance());
    g.output(&kill(35)).await;
    g.narrated("Stop 1/3 cleared: 1 large giant rat").await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.expect("s", arena(&[], 35));
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (one step)")
        .await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn pause_on_the_way_and_resume_goes_on_to_the_same_stop() {
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (3 steps)")
        .await;
    g.bot(BotAction::Pause).await;
    g.narrated("Paused: asked to pause").await;
    g.idle(Duration::from_secs(30)).await;
    assert_eq!(g.played, ["st", "look", "w", "w"]);
    // Resume re-syncs, then walks on to the same stop.
    g.expect("st", status(35));
    g.expect("look", room(NARROW_ROAD));
    g.expect("d", arena(&[], 35));
    g.bot(BotAction::Resume).await;
    g.narrated("Resuming: re-syncing").await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (one step)")
        .await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", dungeon_entrance());
    g.expect("s", arena(&[], 35));
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    let (seen, _) = g.close(run).await;
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn unticking_the_stop_it_walks_to_turns_it_round() {
    // On the way from the Arena to the Dungeon Entrance, the Dungeon Entrance is unticked: the
    // leg ends after the move in flight and the bot waits in the Arena, the only stop left.
    let (mut g, run) = ready_with(arena(&[], 35)).await;
    g.expect("look", arena(&[], 35));
    g.expect("n", says("The door is closed!"));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Loop Newhaven: to stop 2/3 Dungeon, Entrance (one step, one door)")
        .await;
    // Unticked while the `n` is in flight: the door it meets is left shut.
    g.run_loop(&[SMALL_CAVERN, BEYOND_ARENA]).await;
    g.narrated("Loop Newhaven: passing by Dungeon, Entrance, Small Cavern")
        .await;
    g.narrated("Waiting for respawns in Newhaven, Arena").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "n"]);
    let task = bot(&seen).task.unwrap();
    assert_eq!(task.skip, [key(BEYOND_ARENA), key(SMALL_CAVERN)]);
}

#[tokio::test(start_paused = true)]
async fn stop_ends_the_loop() {
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (3 steps)")
        .await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "w", "w"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.task), (BotState::Stopped, None));
}

fn brain_on(path: &Path) -> Brain {
    let (store, _) = Store::open(path).unwrap();
    let clock = InstantClock::new(std::time::UNIX_EPOCH + Duration::from_secs(T0));
    Brain::new(world(), Arc::new(clock), None).with_store(store)
}

#[tokio::test(start_paused = true)]
async fn a_restarted_brain_goes_on_with_the_loop_at_the_same_stop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");

    // The first brain: in the Arena, cleared, and gone as the walk to the Dungeon Entrance
    // starts (its `n` never answered).
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_on(&path)).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", arena(&[], 35));
    g.expect("n", PROMPT);
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Loop Newhaven: to stop 2/3 Dungeon, Entrance (one step, one door)")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "n"]);
    let saved = Store::open(&path).unwrap().0.bot(ID).unwrap().unwrap();
    assert_eq!(
        (saved.loop_id.as_deref(), saved.stop_index, saved.skip),
        (Some(NEWHAVEN_LOOP), 1, vec![2])
    );

    // The second brain, the character still in the Arena: the loop goes on to stop 2.
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_on(&path)).await;
    g.pipelining = true;
    g.expect("n", dungeon_entrance());
    g.expect("s", arena(&[], 35));
    g.narrated("Loop Newhaven: to stop 2/3 Dungeon, Entrance (one step, one door)")
        .await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert!(
        notices(&seen)
            .iter()
            .any(|s| s == "Resumed: running loop Newhaven at stop 2/3 (brain restarted)"),
        "{:?}",
        notices(&seen)
    );
    assert_eq!(played, ["n", "s"]);
}

#[tokio::test(start_paused = true)]
async fn with_hp_short_it_rests_before_waiting_for_respawns() {
    // 30 of 35 is 85%: not low enough for Recover by itself, short of the 95% a rest reaches.
    let (mut g, run) = ready_with(arena(&[], 30)).await;
    g.expect("look", arena(&[], 30));
    g.expect("u", at_hp(&room(NARROW_ROAD), 30));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(30)));
    g.run_loop(&[BEYOND_ARENA, SMALL_CAVERN]).await;
    g.narrated("Waiting for respawns: resting first").await;
    g.narrated("Recover: resting to 95% in Newhaven, Narrow Road")
        .await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("d", arena(&[], 34));
    g.output(&resting(34)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Waiting for respawns in Newhaven, Arena").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d"]);
}

#[tokio::test(start_paused = true)]
async fn a_stop_it_cannot_get_to_is_passed_by_after_three_legs() {
    // Each `n` toward the Dungeon Entrance is refused: three legs, then the stop is passed by
    // this time round and the bot waits in the Arena.
    let (mut g, run) = ready_with(arena(&[], 35)).await;
    g.expect("look", arena(&[], 35));
    for _ in 0..3 {
        g.expect("n", says("You may not enter that room while in combat."));
    }
    g.run_loop(&[SMALL_CAVERN]).await;
    let said = g.narrated("Waiting for respawns: next in 1 m").await;
    assert!(
        said.iter().any(|s| s
            == "Stop 2/3 Dungeon, Entrance: could not get there; passing it by this time round"),
        "{said:?}"
    );
    // The leg's narration is said once (it does not change); the walker's, each time.
    let legs = said
        .iter()
        .filter(|s| s.starts_with("Walking to Dungeon, Entrance"))
        .count();
    assert_eq!(legs, 3, "{said:?}");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "n", "n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn an_arena_it_cannot_get_to_waits_its_delay_and_the_bot_stops_after_three_give_ups() {
    // The Arena the only ticked stop, every `d` toward it refused: three legs, then it is
    // passed by and waits the stock delay (an arena has none after a kill, but a stop passed
    // by does); after three give-ups in a row the bot stops itself with a warning. A `d`
    // beyond the script's fails the test.
    let (mut g, run) = ready_at(NARROW_ROAD).await;
    let tries = (LEG_TRIES * GIVE_UPS) as usize;
    for _ in 0..tries {
        g.expect("d", says("You may not enter that room while in combat."));
    }
    g.run_loop(&[BEYOND_ARENA, SMALL_CAVERN]).await;
    let started = tokio::time::Instant::now();
    let said = g.narrated("Waiting for respawns: next in 5 m").await;
    assert!(
        said.iter()
            .any(|s| s
                == "Stop 1/3 Newhaven, Arena: could not get there; passing it by this time round"),
        "{said:?}"
    );
    let why = "Stopped: loop Newhaven: could not get to a stop 3 times in a row";
    g.narrated(why).await;
    let took = started.elapsed();
    assert!(took >= Duration::from_secs(2 * 5 * 60), "{took:?}");
    g.idle(Duration::from_secs(30 * 60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played[..2], ["st", "look"]);
    assert_eq!(played[2..], vec!["d"; tries]);
    assert!(
        seen.iter()
            .any(|m| matches!(m, ToGateway::Warning { text, .. } if text == why)),
        "{:?}",
        notices(&seen)
    );
    let b = bot(&seen);
    assert_eq!((b.state, b.task), (BotState::Stopped, None));
}

#[tokio::test(start_paused = true)]
async fn stops_it_cannot_reach_are_passed_by_each_round_without_giving_up() {
    // The Arena's door north will not bash open: the Dungeon Entrance and the Small Cavern
    // behind it are passed by, said once, and the Arena goes round on its own. No give-up.
    let (mut g, run) = ready_with(arena(&[], 35)).await;
    g.expect("look", arena(&[], 35));
    g.expect("n", says("The door is closed!"));
    for _ in 0..MAX_BASHES {
        g.expect("bash n", says("Your attempts to bash through fail!"));
    }
    g.run_loop(&[]).await;
    let skipping = "Skipping 2 stops it cannot reach this round: the door north of Newhaven, \
                    Arena would not open";
    let said = g.narrated("Waiting for respawns in Newhaven, Arena").await;
    assert!(said.iter().any(|s| s == skipping), "{said:?}");
    // Round two: a rat comes in, is fought and killed; the stops behind the door are passed by
    // again without a word.
    g.expect("a rat", line(ENGAGED, 35));
    g.expect("look", arena(&[], 35));
    g.output(&arena(&["large giant rat"], 35)).await;
    g.narrated("Fight: large giant rat").await;
    g.output(&kill(35)).await;
    let said = g.narrated("Waiting for respawns in Newhaven, Arena").await;
    assert!(
        said.iter()
            .any(|s| s == "Stop 1/3 cleared: 1 large giant rat"),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|s| s.starts_with("Skipping 2")),
        "{said:?}"
    );
    g.idle(Duration::from_secs(30 * 60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played[2..6], ["n", "bash n", "bash n", "bash n"]);
    assert_eq!(bot(&seen).state, BotState::Running);
    assert!(!notices(&seen).iter().any(|s| s.starts_with("Stopped:")));
}

#[tokio::test(start_paused = true)]
async fn the_bot_stops_at_once_when_no_stop_can_be_reached() {
    // The Arena unticked; the door to the other two will not bash open: one walked leg, and
    // the bot stops itself (not after three give-ups).
    let (mut g, run) = ready_with(arena(&[], 35)).await;
    g.expect("look", arena(&[], 35));
    g.expect("n", says("The door is closed!"));
    for _ in 0..MAX_BASHES {
        g.expect("bash n", says("Your attempts to bash through fail!"));
    }
    g.run_loop(&[ARENA]).await;
    let why = "Stopped: loop Newhaven: none of its stops can be reached from here";
    g.narrated(why).await;
    g.idle(Duration::from_secs(30 * 60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played[2..], ["n", "bash n", "bash n", "bash n"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.task), (BotState::Stopped, None));
}

#[tokio::test(start_paused = true)]
async fn the_only_stop_ready_is_stepped_out_of_and_back_into() {
    // The Arena and the Small Cavern unticked: the Dungeon Entrance, a Lair, is the loop's one
    // stop. Cleared, it steps out (to the Arena, which is no Lair) and back in, so a lair that
    // spawns when a player enters brings its monster, instead of waiting in the room.
    let (mut g, run) = ready_at(BEYOND_ARENA).await;
    g.expect("s", arena(&[], 35));
    g.expect("n", dungeon_entrance());
    g.run_loop(&[ARENA, SMALL_CAVERN]).await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.narrated("Loop Newhaven: stepping out of Dungeon, Entrance and back in")
        .await;
    g.narrated("Loop Newhaven: to stop 2/3 Dungeon, Entrance (one step, one door)")
        .await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played[2..], ["s", "n"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn an_arena_is_waited_in_not_stepped_out_of() {
    // The Arena is the one stop ticked: monsters come back there at once.
    let (mut g, run) = ready_with(arena(&[], 35)).await;
    g.expect("look", arena(&[], 35));
    g.run_loop(&[BEYOND_ARENA, SMALL_CAVERN]).await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.narrated("Waiting for respawns in Newhaven, Arena").await;
    g.idle(Duration::from_secs(60)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
}
