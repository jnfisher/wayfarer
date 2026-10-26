//! Flee (docs/brain.md "Flee"): the bot started in the Newhaven Arena against the fake game of
//! `game/mod.rs`, its HP dropped under the flee line (30%) by a giant rat. On a paused clock.

mod game;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::policy::{self, Candidate};
use wayfarer_brain::{Brain, FixedClock};
use wayfarer_parse::Parser;
use wayfarer_proto::{BotAction, BotMode, BotState};
use wayfarer_world::LockState;

const RAT: &str = "large giant rat";
const SMALL_CAVERN: Key = (1, 2156);

fn lunge(hp: i32) -> String {
    line("The large giant rat lunges at you!", hp)
}

fn follows(hp: i32) -> String {
    line("The large giant rat creeps in after you!", hp)
}

/// The Arena with its north door shown open (the capture has it closed).
fn arena_door_open(monsters: &[&str], hp: i32) -> String {
    arena(monsters, hp).replace("closed door north", "open door north")
}

/// Fighting the rat in the Arena at full HP.
async fn fighting() -> (Game, Run) {
    fighting_with("").await
}

/// The same, the brain's settings being `text` (a TOML character file).
async fn fighting_with(text: &str) -> (Game, Run) {
    let settings = test_settings(text);
    let clock = Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)));
    let brain = Brain::new(world(), clock, None).with_settings(settings);
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(&[RAT], 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    // Twice: the attack's answer may come after the first.
    for _ in 0..2 {
        g.round("The large giant rat lunges at you!", 20).await;
    }
    (g, run)
}

/// Under the line mid-fight: up to the Narrow Road, the rat follows; east to the Narrow Path,
/// where nothing follows; rest there; walk home; fight again.
async fn flee_rest_and_return() -> (Vec<String>, Vec<String>) {
    let (mut g, run) = fighting().await;
    // 8 of 35 is 22%.
    g.expect(
        "u",
        format!("{}{}", at_hp(&room(NARROW_ROAD), 8), follows(8)),
    );
    g.expect("e", at_hp(&room(NARROW_PATH), 8));
    g.output(&lunge(8)).await;
    g.narrated("Fleeing: HP 22%: up to Newhaven, Narrow Road")
        .await;
    g.narrated("Followed by large giant rat: east to Newhaven, Narrow Path")
        .await;
    g.narrated("Safe in Newhaven, Narrow Path").await;
    // Its own auto-combat lapses (no `*Combat Off*` here); then Recover rests where it is.
    g.expect("rest", format!("You are now resting.\r\n{}", resting(8)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Path")
        .await;
    // 31 of 35 is 89%: rested at 85%.
    for hp in [16, 24] {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    g.expect("w", at_hp(&room(NARROW_ROAD), 31));
    g.expect("d", arena(&[RAT], 31));
    g.expect("a rat", line(ENGAGED, 31));
    g.output(&resting(31)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Fight: large giant rat").await;
    let (seen, played) = g.close(run).await;
    (played, notices(&seen))
}

#[tokio::test(start_paused = true)]
async fn on_the_walk_home_after_a_rest_a_monster_on_the_way_is_fought() {
    // Faye on the live rig (2026-10-26): Recover's walk home went past bandits. A leg of the
    // loop stops where Fight would take the character; the walk home does too.
    let (mut g, run) = fighting().await;
    g.expect(
        "u",
        format!("{}{}", at_hp(&room(NARROW_ROAD), 8), follows(8)),
    );
    g.expect("e", at_hp(&room(NARROW_PATH), 8));
    g.output(&lunge(8)).await;
    g.narrated("Safe in Newhaven, Narrow Path").await;
    g.expect("rest", format!("You are now resting.\r\n{}", resting(8)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Path")
        .await;
    for hp in [16, 24] {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    // A rat on the Narrow Road, on the way home: fought there, not walked past.
    let road = at_hp(&room(NARROW_ROAD), 31);
    let at = road.find("\x1b[0;32mObvious exits").unwrap();
    let road_with_rat = format!(
        "{}\x1b[0;35mAlso here: \x1b[1;35m{RAT}\x1b[0m\x1b[0;35m.\r\n{}",
        &road[..at],
        &road[at..]
    );
    g.expect("w", road_with_rat);
    g.expect("a rat", line(ENGAGED, 31));
    g.output(&resting(31)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Stopped walking: a monster to fight").await;
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 31).await;
    let (_, played) = g.close(run).await;
    assert!(!played.contains(&"d".to_owned()), "{played:?}");
    assert_eq!(played.last().map(String::as_str), Some("a rat"));
}

#[tokio::test(start_paused = true)]
async fn under_the_line_it_flees_keeps_running_when_followed_rests_and_returns() {
    let (played, said) = flee_rest_and_return().await;
    assert_eq!(
        played,
        ["st", "look", "a rat", "u", "e", "rest", "w", "d", "a rat"]
    );
    assert!(
        said.contains(&"Arrived: Newhaven, Arena".to_owned()),
        "{said:?}"
    );
    assert!(!said.iter().any(|n| n.starts_with("Stopped")), "{said:?}");
}

#[tokio::test(start_paused = true)]
async fn a_monster_that_comes_in_while_resting_under_the_line_starts_a_new_flee() {
    let (mut g, run) = fighting().await;
    g.expect(
        "u",
        format!("{}{}", at_hp(&room(NARROW_ROAD), 8), follows(8)),
    );
    g.expect("e", at_hp(&room(NARROW_PATH), 8));
    g.output(&lunge(8)).await;
    g.narrated("Safe in Newhaven, Narrow Path").await;
    g.expect("rest", format!("You are now resting.\r\n{}", resting(8)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Path")
        .await;
    // Another rat comes in and strikes while HP is still under the line: running again, from
    // the start (not back where it came from), with every room to run through.
    g.expect("e", at_hp(&room(VILLAGE_ENTRANCE), 8));
    g.idle(Duration::from_secs(1)).await;
    g.output(&lunge(8)).await;
    g.narrated("Fleeing: HP 22%: east to Newhaven, Village Entrance")
        .await;
    g.expect("rest", format!("You are now resting.\r\n{}", resting(8)));
    g.narrated("Safe in Newhaven, Village Entrance").await;
    g.narrated("Recover: resting to 85% in Newhaven, Village Entrance")
        .await;
    let (seen, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "a rat", "u", "e", "rest", "e", "rest"]
    );
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn with_the_line_above_recovers_it_still_rests_and_walks_back() {
    // A flee line of 80%: the first blow (24 of 35, 68%) sets the flee off, with HP above the
    // 60% Recover would rest below. It rests to 85% anyway, then goes back.
    let clock = FixedClock(UNIX_EPOCH + Duration::from_secs(T0));
    let brain = Brain::new(world(), Arc::new(clock), None).with_flee_below(80);
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(&[RAT], 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 35).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 24));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(24)));
    g.output(&lunge(24)).await;
    g.narrated("Fleeing: HP 68%: up to Newhaven, Narrow Road")
        .await;
    g.narrated("Safe in Newhaven, Narrow Road").await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    g.expect("d", arena(&[RAT], 31));
    g.expect("a rat", line(ENGAGED, 31));
    g.idle(Duration::from_secs(20)).await;
    g.output(&resting(31)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u", "rest", "d", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn the_same_flee_gives_the_same_commands() {
    assert_eq!(flee_rest_and_return().await, flee_rest_and_return().await);
}

#[tokio::test(start_paused = true)]
async fn the_mode_is_flee_while_it_runs_and_the_follower_is_watched_for() {
    let (mut g, run) = fighting().await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 8));
    g.output(&lunge(8)).await;
    g.narrated("Flee: in Newhaven, Narrow Road, watching for followers")
        .await;
    assert_eq!(bot(&g.seen).mode, BotMode::Flee);
    // The watch is 1.5 s: nothing yet at 1 s.
    let before = g.seen.len();
    g.idle(Duration::from_secs(1)).await;
    assert!(
        notices(&g.seen[before..]).is_empty(),
        "{:?}",
        notices(&g.seen[before..])
    );
    g.narrated("Safe in Newhaven, Narrow Road").await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
}

#[tokio::test(start_paused = true)]
async fn a_shut_door_is_passed_over_for_the_next_exit_at_once() {
    // The north door shows open; `up` answers that a (locked) door is shut. The next candidate
    // goes straight away: the door north.
    let (mut g, run) = Game::start_with(SessionState::Realm, arena_door_open(&[RAT], 35)).await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", arena_door_open(&[RAT], 35));
    g.expect("a rat", line(ENGAGED, 35));
    g.bot(BotAction::Start).await;
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 20).await;
    g.expect("u", line("The door is closed!", 8));
    g.expect("n", at_hp(&room(BEYOND_ARENA), 8));
    g.output(&lunge(8)).await;
    g.narrated("Fleeing: HP 22%: up to Newhaven, Narrow Road")
        .await;
    g.narrated("Fleeing: HP 22%: north to Dungeon, Entrance")
        .await;
    g.narrated("Flee: in Dungeon, Entrance, watching for followers")
        .await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u", "n"]);
}

#[tokio::test(start_paused = true)]
async fn a_shut_door_that_is_not_locked_is_opened_then_taken() {
    // Only the door is left (up refused while in combat); the game says it is shut, not
    // locked (`Closed!`): `open`, then through it.
    let (mut g, run) = Game::start_with(SessionState::Realm, arena_door_open(&[RAT], 35)).await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", arena_door_open(&[RAT], 35));
    g.expect("a rat", line(ENGAGED, 35));
    g.bot(BotAction::Start).await;
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 20).await;
    g.expect("u", line("You may not enter that room while in combat.", 8));
    g.expect("n", line("The door is Closed!", 8));
    g.expect("open north", line("The door is now open.", 8));
    g.expect("n", at_hp(&room(BEYOND_ARENA), 8));
    g.output(&lunge(8)).await;
    g.narrated("Fleeing: HP 22%: opening the door north").await;
    g.narrated("Flee: in Dungeon, Entrance, watching for followers")
        .await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u", "n", "open north", "n"]);
}

#[tokio::test(start_paused = true)]
async fn followed_through_every_room_it_turns_and_fights_and_the_bot_goes_on() {
    let (mut g, run) = fighting_with("[health]\nflee_rooms = 3\n").await;
    g.expect(
        "u",
        format!("{}{}", at_hp(&room(NARROW_ROAD), 8), follows(8)),
    );
    g.expect(
        "e",
        format!("{}{}", at_hp(&room(NARROW_PATH), 8), follows(8)),
    );
    g.expect(
        "e",
        format!("{}{}", at_hp(&room(VILLAGE_ENTRANCE), 8), follows(8)),
    );
    g.output(&lunge(8)).await;
    let why = "Flee: followed by large giant rat through 3 rooms; fighting back";
    let said = g.narrated(why).await;
    assert!(
        said.contains(
            &"Followed by large giant rat: east to Newhaven, Village Entrance".to_owned()
        ),
        "{said:?}"
    );
    g.freeze();
    let (seen, played) = g.close(run).await;
    // No `break`, and the bot is not stopped: the rat is fought where it stands.
    assert_eq!(played, ["st", "look", "a rat", "u", "e", "e"]);
    assert_eq!(bot(&seen).state, BotState::Running);
    assert!(
        seen.iter()
            .any(|m| matches!(m, ToGateway::Warning { text, .. } if text == why)),
        "a warning"
    );
}

#[tokio::test(start_paused = true)]
async fn cornered_it_fights_back_with_a_warning_and_the_bot_goes_on() {
    // Up is refused while in combat; the north door is locked (the pack, a cold start).
    let (mut g, run) = fighting().await;
    g.expect("u", line("You may not enter that room while in combat.", 8));
    g.output(&lunge(8)).await;
    let why = "Flee: cornered in Newhaven, Arena: no exit to flee by; fighting back";
    g.narrated(why).await;
    g.output(&lunge(6)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
    assert_eq!(bot(&seen).state, BotState::Running);
    assert!(
        seen.iter()
            .any(|m| matches!(m, ToGateway::Warning { text, .. } if text == why)),
        "a warning"
    );
}

#[tokio::test(start_paused = true)]
async fn the_line_is_checked_while_a_walk_is_in_flight() {
    // Recover walks out of the room beyond the Arena (it spawns) to the Narrow Road: south,
    // then up. The rat hits the character on the way: at 22% the walk stops after its move,
    // and Flee takes the character up from the Arena.
    let beyond = |hp| at_hp(&room(BEYOND_ARENA), hp);
    let (mut g, run) = Game::start_with(SessionState::Realm, beyond(35)).await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(20));
    g.expect("look", beyond(20));
    g.expect(
        "s",
        format!(
            "{}{}",
            line("The thin giant rat lunges at you!", 8),
            arena(&["thin giant rat"], 8)
        ),
    );
    g.expect("u", at_hp(&room(NARROW_ROAD), 8));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(8)));
    g.bot(BotAction::Start).await;
    let said = g.narrated("Safe in Newhaven, Narrow Road").await;
    let walked = said
        .iter()
        .position(|n| n.starts_with("Recover: walking to"));
    let stopped = said.iter().position(|n| n == "Stopped walking: fleeing");
    let fled = said
        .iter()
        .position(|n| n == "Fleeing: HP 22%: up to Newhaven, Narrow Road");
    assert!(
        walked.is_some() && walked < stopped && stopped < fled,
        "{said:?}"
    );
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "s", "u", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn nothing_is_sent_while_paused_or_stopped() {
    for action in [BotAction::Pause, BotAction::Stop] {
        let (mut g, run) = fighting().await;
        g.bot(action).await;
        g.idle(Duration::from_secs(1)).await;
        g.output(&lunge(8)).await;
        g.output(&lunge(5)).await;
        g.idle(Duration::from_secs(60)).await;
        let (seen, played) = g.close(run).await;
        assert_eq!(played, ["st", "look", "a rat"], "{action:?}");
        assert!(
            !notices(&seen).iter().any(|n| n.starts_with("Fleeing")),
            "{action:?}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn logging_off_under_attack_flees_first_then_answers_ready_to_leave() {
    let (mut g, run) = fighting().await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 18));
    g.send(ToClient::LoggingOff { session: ID.into() }).await;
    let said = g.narrated("Stopped: the gateway is logging off").await;
    assert_eq!(
        said,
        [
            "Logging off under attack: fleeing first",
            "Logging off under attack: fleeing first: up to Newhaven, Narrow Road",
            "Flee: in Newhaven, Narrow Road, watching for followers",
            "Safe in Newhaven, Narrow Road",
            "Stopped: the gateway is logging off",
        ]
    );
    let ready = g
        .until(|m| match m {
            ToGateway::ReadyToLeave { session } => Some(session.clone()),
            _ => None,
        })
        .await;
    assert_eq!(ready, ID);
    // The gateway's `x`, a re-sync asked for: nothing goes.
    g.typed("x").await;
    g.send(ToClient::Resync { session: ID.into() }).await;
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
    let ready = seen
        .iter()
        .position(|m| matches!(m, ToGateway::ReadyToLeave { .. }))
        .unwrap();
    let flee = seen
        .iter()
        .position(|m| matches!(m, ToGateway::InputLine { text, .. } if text == "u"))
        .unwrap();
    assert!(flee < ready);
    let after: Vec<_> = seen[ready..]
        .iter()
        .filter(|m| matches!(m, ToGateway::InputLine { .. }))
        .collect();
    assert!(after.is_empty(), "{after:?}");
    assert_eq!(bot(&seen).state, BotState::Stopped);
}

#[tokio::test(start_paused = true)]
async fn logging_off_cornered_breaks_off_and_still_answers() {
    let (mut g, run) = fighting().await;
    g.expect(
        "u",
        line("You may not enter that room while in combat.", 18),
    );
    g.expect("break", prompt(18));
    g.send(ToClient::LoggingOff { session: ID.into() }).await;
    let ready = g
        .until(|m| match m {
            ToGateway::ReadyToLeave { session } => Some(session.clone()),
            _ => None,
        })
        .await;
    assert_eq!(ready, ID);
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u", "break"]);
    assert_eq!(bot(&seen).state, BotState::Stopped);
}

#[test]
fn a_follow_text_is_read_as_one() {
    // The rat's own follow text (from the pack), or the generic pattern.
    let mut parser = Parser::new(world().registry.clone());
    let mut events = parser.feed("The large giant rat creeps in after you!\r\n");
    events.extend(parser.finish());
    assert!(
        events.iter().any(|e| e.name.starts_with("monster_follows")),
        "{events:?}"
    );
}

// `flee_exits`, on the world pack.

fn exits(room: Key, doors: &BTreeMap<(Key, u8), LockState>) -> Vec<Candidate> {
    policy::flee_exits(&atlas(), Some(room), true, &BTreeSet::new(), doors)
}

fn dirs(c: &[Candidate]) -> Vec<(u8, Key)> {
    c.iter().map(|c| (c.dir, c.to)).collect()
}

const N: u8 = 0;
const S: u8 = 1;
const E: u8 = 2;
const W: u8 = 3;
const U: u8 = 8;

#[test]
fn from_the_arena_up_is_first_and_the_door_only_when_known_open() {
    // The Arena's north door to the Dungeon is locked in the pack (a cold start): up alone.
    let none = BTreeMap::new();
    let up = exits(ARENA, &none);
    assert_eq!(dirs(&up), [(U, NARROW_ROAD)]);
    assert_eq!((up[0].door, up[0].quiet), (None, true));
    // Seen open: after up, as a door to a room that spawns.
    let open = BTreeMap::from([((ARENA, N), LockState::Open)]);
    let both = exits(ARENA, &open);
    assert_eq!(dirs(&both), [(U, NARROW_ROAD), (N, BEYOND_ARENA)]);
    assert_eq!(
        (both[1].door, both[1].quiet),
        (Some(LockState::Open), false)
    );
    // Up refused this session: only the door, and only if open.
    let blocked = BTreeSet::from([(ARENA, U)]);
    let a = atlas();
    assert!(policy::flee_exits(&a, Some(ARENA), true, &blocked, &none).is_empty());
    assert_eq!(
        dirs(&policy::flee_exits(&a, Some(ARENA), true, &blocked, &open)),
        [(N, BEYOND_ARENA)]
    );
    let locked = BTreeMap::from([((ARENA, N), LockState::Locked)]);
    assert!(policy::flee_exits(&a, Some(ARENA), true, &blocked, &locked).is_empty());
}

#[test]
fn from_the_narrow_path_a_plain_exit_to_a_quiet_room_not_protected_first() {
    // Every exit is plain and quiet; the Spell Shop and the General Store are protected rooms
    // (refused while in combat), so the Village Entrance and the Narrow Road come first.
    let c = exits(NARROW_PATH, &BTreeMap::new());
    assert_eq!(
        dirs(&c),
        [
            (E, VILLAGE_ENTRANCE),
            (W, NARROW_ROAD),
            (N, (1, 2144)),
            (S, (1, 2145)),
        ]
    );
    assert!(c.iter().all(|c| c.quiet && c.door.is_none()));
}

#[test]
fn from_the_narrow_road_never_the_gated_healer() {
    // West to the Healer is a level-gated exit: never a candidate. Down to the Arena spawns.
    let c = exits(NARROW_ROAD, &BTreeMap::new());
    assert_eq!(dirs(&c), [(E, NARROW_PATH), (N, (1, 2147)), (9, ARENA)]);
    assert!(!c[2].quiet);
}

#[test]
fn from_a_dead_end_its_one_exit_even_into_a_room_that_spawns() {
    let c = exits(SMALL_CAVERN, &BTreeMap::new());
    assert_eq!(dirs(&c), [(S, BEYOND_ARENA)]);
    assert!(!c[0].quiet);
    assert_eq!(
        dirs(&exits(WEAPONS_SHOP, &BTreeMap::new())),
        [(S, VILLAGE_ENTRANCE)]
    );
}

#[test]
fn not_sure_of_the_room_no_candidate() {
    let a = atlas();
    let none = BTreeMap::new();
    assert!(policy::flee_exits(&a, Some(ARENA), false, &BTreeSet::new(), &none).is_empty());
    assert!(policy::flee_exits(&a, None, true, &BTreeSet::new(), &none).is_empty());
}

#[tokio::test(start_paused = true)]
async fn mortally_wounded_it_sends_nothing_and_says_so_once() {
    let (mut g, run) = fighting_with("[health]\nhangup = false\n").await;
    g.round("The large giant rat lunges at you!", -4).await;
    g.round("You may not do that while you are mortally wounded!", -6)
        .await;
    let sent = |g: &Game| {
        g.seen
            .iter()
            .filter(|m| matches!(m, ToGateway::InputLine { .. }))
            .count()
    };
    let before = sent(&g);
    for hp in [-8, -9, -11] {
        g.round("The large giant rat bites you!", hp).await;
    }
    g.idle(Duration::from_secs(120)).await;
    assert_eq!(sent(&g), before);
    let (seen, _) = g.close(run).await;
    let said = notices(&seen);
    let hurt = said
        .iter()
        .filter(|n| n.starts_with("Mortally wounded"))
        .count();
    assert_eq!(hurt, 1, "{said:?}");
}

fn hangups(seen: &[ToGateway]) -> usize {
    seen.iter()
        .filter(|m| {
            matches!(
                m,
                ToGateway::SessionControl {
                    action: wayfarer_proto::SessionAction::HangUp,
                    ..
                }
            )
        })
        .count()
}

#[tokio::test(start_paused = true)]
async fn under_the_hangup_line_with_a_monster_hitting_it_hangs_up_once_and_stops_the_bot() {
    let (mut g, run) = fighting().await;
    // 3 of 35 is 8%, under the 10% line.
    g.round("The large giant rat lunges at you!", 3).await;
    g.round("The large giant rat lunges at you!", 2).await;
    let (seen, _) = g.close(run).await;
    assert_eq!(hangups(&seen), 1);
    let said = notices(&seen);
    assert!(
        said.iter()
            .any(|n| n.starts_with("Stopped: hung up at 8% HP")),
        "{said:?}"
    );
    assert_eq!(bot(&seen).state, BotState::Stopped);
}

#[tokio::test(start_paused = true)]
async fn back_in_the_realm_after_a_hangup_it_waits_for_a_new_blow_before_hanging_up_again() {
    // Manuel, 2026-10-24: hung up at -18%, then every entry hung up again at once, the rat
    // that had been hitting him being remembered across the drop though it had not struck.
    // Now the rat is only here: under the flee line, it flees.
    let (mut g, run) = fighting().await;
    g.round("The large giant rat lunges at you!", 3).await;
    hang_up_and_come_back(&mut g).await;
    g.expect("st", status(3));
    g.expect("look", arena(&[RAT], 3));
    g.expect("u", at_hp(&room(NARROW_ROAD), 3));
    g.bot(BotAction::Start).await;
    // On the paused clock: a hang-up instead fails here at once rather than waiting forever.
    tokio::time::timeout(
        Duration::from_secs(60),
        g.narrated("Safe in Newhaven, Narrow Road"),
    )
    .await
    .expect("it fled to safety");
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(hangups(&seen), 1, "{:?}", notices(&seen));
    assert_eq!(played.last().map(String::as_str), Some("u"));
}

#[tokio::test(start_paused = true)]
async fn back_in_the_realm_after_a_hangup_a_new_blow_hangs_up_again() {
    let (mut g, run) = fighting().await;
    g.round("The large giant rat lunges at you!", 3).await;
    hang_up_and_come_back(&mut g).await;
    // The rat strikes before the bot has looked for a way out.
    g.freeze();
    g.bot(BotAction::Start).await;
    g.idle(Duration::from_secs(1)).await;
    g.round("The large giant rat lunges at you!", 2).await;
    let (seen, _) = g.close(run).await;
    assert_eq!(hangups(&seen), 2, "{:?}", notices(&seen));
}

/// The line dropped: the logon menus, then into the realm again with the rat still there,
/// not attacking.
async fn hang_up_and_come_back(g: &mut Game) {
    g.set_state(SessionState::Menu).await;
    g.output("[MAJORMUD]: ").await;
    g.idle(Duration::from_secs(5)).await;
    g.set_state(SessionState::Realm).await;
    g.output(&arena(&[RAT], 3)).await;
}

#[tokio::test(start_paused = true)]
async fn the_hangup_can_be_turned_off_and_has_its_own_line() {
    // Off: HP at 8% hangs nothing up.
    let (mut g, run) = fighting_with("[health]\nhangup = false\n").await;
    g.freeze();
    g.round("The large giant rat lunges at you!", 3).await;
    let (seen, _) = g.close(run).await;
    assert_eq!(hangups(&seen), 0);
    // A line of 3% leaves 8% alone.
    let (mut g, run) = fighting_with("[health]\nhangup_below_hp = 3\n").await;
    g.freeze();
    g.round("The large giant rat lunges at you!", 3).await;
    let (seen, _) = g.close(run).await;
    assert_eq!(hangups(&seen), 0);
}
