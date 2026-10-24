//! Fight here (docs/brain.md "Fight here"): the bot started in the Newhaven Arena against the
//! fake gateway of `game/mod.rs`, whose game answers each command from the test's script, with
//! texts cut from the `fight-arena` and `room-walk` captures. On a paused clock: a test's
//! minutes of game time take no time.

mod game;

use std::collections::BTreeSet;

use game::arena::*;
use game::*;
use wayfarer_brain::ATTACK_TRIES;
use wayfarer_brain::executor::{ATTACK_REFUSALS, Action, Executor, Expect, Outcome, Refused};
use wayfarer_brain::policy::{self, RestAt};
use wayfarer_parse::Parser;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{BotAction, BotMode, BotState};

#[tokio::test(start_paused = true)]
async fn start_attacks_once_waits_while_engaged_and_looks_after_the_kill() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    // The fight: nothing is sent while engaged (the script has nothing; anything would fail).
    for hp in [33, 33, 30, 30] {
        g.round("The large giant rat lunges at you!", hp).await;
        g.round("You whap large giant rat for 6 damage!", hp).await;
    }
    g.expect("look", arena(&[], 30));
    g.output(&kill(30)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.mode), (BotState::Running, BotMode::Idle));
    assert_eq!(b.narration, "Waiting here");
}

/// Two monsters, then a third that arrives: each attacked once, the next found by looking.
async fn two_and_an_arrival() -> (Vec<String>, Vec<String>) {
    let (mut g, run) = in_arena(&[]).await;
    let both = ["large giant rat", "fat giant rat"];
    start(&mut g, &both, 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 32).await;
    g.expect("look", arena(&["fat giant rat"], 32));
    g.expect("a rat", line(ENGAGED, 32));
    g.output(&kill(32)).await;
    g.narrated("Fight: fat giant rat").await;
    g.round("The fat giant rat lunges at you!", 30).await;
    g.expect("look", arena(&[], 30));
    g.output(&kill(30)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(30)).await;
    // An arrival: attacked at once.
    g.expect("a rat", line(ENGAGED, 30));
    g.output(&line(
        "A thin giant rat creeps into the room from nowhere.",
        30,
    ))
    .await;
    g.narrated("Fight: thin giant rat").await;
    g.round("The thin giant rat lunges at you!", 30).await;
    let (seen, played) = g.close(run).await;
    (played, notices(&seen))
}

#[tokio::test(start_paused = true)]
async fn the_next_monster_is_found_by_looking_and_an_arrival_is_attacked() {
    let (played, _) = two_and_an_arrival().await;
    assert_eq!(
        played,
        ["st", "look", "a rat", "look", "a rat", "look", "a rat"]
    );
}

#[tokio::test(start_paused = true)]
async fn the_same_game_gives_the_same_commands() {
    let first = two_and_an_arrival().await;
    let second = two_and_an_arrival().await;
    assert_eq!(first, second);
}

#[tokio::test(start_paused = true)]
async fn by_default_low_hp_rests_where_it_stands_even_in_a_room_that_spawns() {
    // C-053: `health.rest_quiet_room` off (the default): the Arena spawns, and it rests there.
    let clock = Arc::new(wayfarer_brain::FixedClock(
        std::time::UNIX_EPOCH + Duration::from_secs(T0),
    ));
    let brain = wayfarer_brain::Brain::new(world(), clock, None);
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(&[], 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[], 20).await;
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Arena")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn low_hp_rests_in_a_quiet_room_then_walks_back() {
    let (mut g, run) = in_arena(&[]).await;
    // 20 of 35 is 57%: below 80. The Arena spawns; the Narrow Road, up the stairs, doesn't.
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: walking to Newhaven, Narrow Road to rest (Newhaven, Arena spawns)")
        .await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    // The resting prompt, redrawn as HP comes back; 31 of 35 is 89%, rested at 85%.
    for hp in [24, 28] {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    g.expect("d", arena(&[], 31));
    g.output(&resting(31)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Waiting here").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d"]);
    let said = notices(&seen);
    // The walks go through the walker, each step confirmed.
    assert!(
        said.contains(&"Arrived: Newhaven, Narrow Road".to_owned()),
        "{said:?}"
    );
    assert!(
        said.contains(&"Arrived: Newhaven, Arena".to_owned()),
        "{said:?}"
    );
    assert_eq!(as_ui_sees(&seen).position.unwrap().room, Some(ARENA.1));
}

#[tokio::test(start_paused = true)]
async fn started_where_the_position_is_unknown_home_is_where_the_resync_finds_it() {
    // Seen on the rig: a brain just attached, its history showing no room. Start re-syncs
    // and its `look` shows the Arena; home is the Arena, so after the rest up the stairs the
    // bot walks back down and fights there again.
    let (mut g, run) = Game::start_lost(SessionState::Realm, prompt(35)).await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(20));
    g.expect("look", arena(&[], 20));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in an unknown room: re-syncing").await;
    g.narrated("Playing from Newhaven, Arena").await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    for hp in [24, 28] {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    g.expect("d", arena(&["large giant rat"], 31));
    g.expect("a rat", line(ENGAGED, 31));
    g.output(&resting(31)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Fight: large giant rat").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d", "a rat"]);
    let said = notices(&seen);
    assert!(!said.contains(&"Waiting here".to_owned()), "{said:?}");
    for arrived in ["Arrived: Newhaven, Narrow Road", "Arrived: Newhaven, Arena"] {
        assert!(said.contains(&arrived.to_owned()), "{said:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn rest_is_sent_again_when_the_prompt_lacks_resting() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", prompt(20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    g.idle(Duration::from_secs(10)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn a_monster_that_attacks_while_resting_wakes_fight() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    g.output(&resting(22)).await;
    g.idle(Duration::from_secs(5)).await;
    // It arrives: Recover keeps resting; it attacks: Fight takes the character.
    g.output(&format!(
        "\x1b[79D\x1b[KA thin giant rat creeps into the room from nowhere.\r\n{}",
        resting(22)
    ))
    .await;
    g.idle(Duration::from_secs(5)).await;
    g.expect("a rat", line(ENGAGED, 21));
    g.output(&line("The thin giant rat lunges at you!", 21))
        .await;
    g.narrated("Fight: thin giant rat").await;
    g.round("The thin giant rat lunges at you!", 21).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn pause_with_nothing_to_go_on_with_turns_the_bot_off_and_start_resyncs() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    // No loop or journey: pausing is turning the bot off.
    g.bot(BotAction::Pause).await;
    g.narrated("Stopped: asked to pause").await;
    // Off: the fight goes on by itself, and nothing is sent after the kill.
    g.round("The large giant rat lunges at you!", 33).await;
    g.output(&kill(33)).await;
    g.idle(Duration::from_secs(60)).await;
    // ►: on again where it stands, re-syncing (`st` ... `look`) first.
    g.expect("st", status(33));
    g.expect("look", arena(&[], 33));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
    g.narrated("Waiting here").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "st", "look"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn stop_with_nothing_to_drop_turns_the_bot_off() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    g.output(&kill(35)).await;
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    let b = bot(&seen);
    assert_eq!(
        (b.state, b.reason.as_deref()),
        (BotState::Stopped, Some("asked to stop"))
    );
}

#[tokio::test(start_paused = true)]
async fn an_unknown_combat_state_counts_as_engaged_until_the_game_is_quiet() {
    // The brain attaches mid-fight: the character's blows, but no `*Combat Engaged*` it saw.
    // The guard takes the unknown state as engaged; only once its blows stop does the bot
    // take the fight to have lapsed and attack.
    let blow = "You whap large giant rat for 6 damage!";
    let history = format!("{}{}", arena(&["large giant rat"], 35), line(blow, 35));
    let (mut g, run) = Game::start_with(SessionState::Realm, history).await;
    g.expect("st", status(35));
    g.expect("look", arena(&["large giant rat"], 34));
    g.bot(BotAction::Start).await;
    // The blows go on through the re-sync (its `exp` and `i` wait out their answers) and
    // after it.
    for _ in 0..8 {
        g.round(blow, 34).await;
    }
    // Quiet: the fight lapsed (pitfall 42); now it attacks.
    g.expect("a rat", line(ENGAGED, 34));
    g.narrated("Fight: large giant rat").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    let said = notices(&seen);
    let waited = said.iter().position(|n| n == "Fight: in combat");
    let attacked = said.iter().position(|n| n == "Fight: large giant rat");
    assert!(waited.is_some() && waited < attacked, "{said:?}");
}

#[tokio::test(start_paused = true)]
async fn a_monster_attacking_a_character_that_is_not_fighting_is_fought() {
    // Its blows are not the character's: it is not engaged, so the bot engages at once
    // (seen live: a rat lunging at an idle character in the Arena).
    let (mut g, run) = in_arena(&["fat giant rat"]).await;
    g.output(&line("The fat giant rat lunges at you!", 35))
        .await;
    g.expect("st", status(35));
    g.expect("look", arena(&["fat giant rat"], 35));
    g.expect("a rat", line(ENGAGED, 35));
    g.bot(BotAction::Start).await;
    for _ in 0..3 {
        g.output(&line("The fat giant rat lunges at you!", 35))
            .await;
        g.idle(Duration::from_secs(2)).await;
    }
    let said = g.narrated("Fight: fat giant rat").await;
    assert!(!said.contains(&"Fight: in combat".to_owned()), "{said:?}");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn a_refused_target_is_skipped_for_the_session() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line("You do not see rat here!", 35));
    g.expect("look", arena(&["large giant rat"], 35));
    g.narrated("Fight: large giant rat").await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look"]);
}

#[tokio::test(start_paused = true)]
async fn a_refusal_the_game_words_otherwise_skips_the_target_too() {
    // `unbalanced`, one of the executor's other attack refusals: no `*Combat Engaged*` comes,
    // and the attack does not go again.
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect(
        "a rat",
        line(
            "Such an attack would result in a very unbalanced combat round.",
            35,
        ),
    );
    g.expect("look", arena(&["large giant rat"], 35));
    g.narrated("Fight: large giant rat").await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look"]);
}

#[tokio::test(start_paused = true)]
async fn an_attack_never_answered_is_tried_again_then_left_alone() {
    // A bare prompt: neither engaged nor refused. Each try waits out the action's timeout.
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &["large giant rat"], 35).await;
    for _ in 0..ATTACK_TRIES {
        g.expect("a rat", prompt(35));
        g.expect("look", arena(&["large giant rat"], 35));
    }
    g.narrated("Fight: large giant rat").await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look", "a rat", "look"]);
}

#[test]
fn every_attack_refusal_line_refuses_the_attack() {
    // Each refusal's own example line (patterns.json), through the parser and the executor.
    let registry = world().registry;
    for name in ATTACK_REFUSALS {
        let pattern = registry
            .patterns()
            .iter()
            .find(|p| p.name == name && p.template.is_none())
            .unwrap_or_else(|| panic!("no pattern {name}"));
        let mut parser = Parser::new(registry.clone());
        let mut events = parser.feed(&format!("{}\n", pattern.example));
        events.extend(parser.finish());
        let mut ex = Executor::default();
        let now = tokio::time::Instant::now();
        ex.submit(Action::new("a rat", Expect::Engaged)).unwrap();
        ex.take_ready(now);
        for ev in &events {
            ex.event(&GameEvent::from_event(ev));
        }
        assert_eq!(
            ex.finished(true, now).map(|(_, o)| o),
            Some(Outcome::Refused(Refused::Target)),
            "{name}: {events:?}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_go_to_keeps_the_bot_on_and_it_plays_where_it_arrives() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.expect("u", room(NARROW_ROAD));
    g.walk_to(NARROW_ROAD).await;
    g.narrated("Arrived: Newhaven, Narrow Road").await;
    // On, and at home where it arrived: it does not walk back to the Arena.
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.journey), (BotState::Running, None));
}

#[tokio::test(start_paused = true)]
async fn walk_to_while_the_bots_look_is_in_flight_walks_once_it_is_answered() {
    // Seen live: the player asked for a walk while the bot's `look` after a kill was out. The
    // walk's first step waits for that answer (one action at a time), then goes.
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.after_cmd = Some((
        "look".into(),
        ToClient::WalkTo {
            session: ID.into(),
            map: NARROW_ROAD.0,
            room: NARROW_ROAD.1,
            stop_short: None,
        },
    ));
    g.expect("look", arena(&[], 35));
    g.expect("u", room(NARROW_ROAD));
    g.output(&kill(35)).await;
    g.narrated("Arrived: Newhaven, Narrow Road").await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look", "u"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn the_bot_needs_the_realm() {
    let (mut g, run) = Game::start_with(SessionState::Menu, arena(&[], 35)).await;
    g.bot(BotAction::Start).await;
    assert_eq!(
        g.narration().await,
        "bot: the character is not in the realm"
    );
    // Leaving the realm stops a running bot.
    g.set_state(SessionState::Realm).await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.set_state(SessionState::Menu).await;
    g.narrated("Stopped: the session left the realm").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
    assert_eq!(bot(&seen).state, BotState::Stopped);
}

// Log-off under attack (C-027) and the wave 3 follow-ups.

/// A move refused for combat (`exit_combat`): the walk ends where it is.
fn refused_in_combat(hp: i32) -> String {
    line("You may not enter that room while in combat.", hp)
}

#[tokio::test(start_paused = true)]
async fn logging_off_stops_the_bot_answers_ready_to_leave_and_sends_nothing_more() {
    // Nothing attacking: no flee (`flee.rs` has the log-off under attack).
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.send(ToClient::LoggingOff { session: ID.into() }).await;
    g.narrated("Stopped: the gateway is logging off").await;
    let ready = g
        .until(|m| match m {
            ToGateway::ReadyToLeave { session } => Some(session.clone()),
            _ => None,
        })
        .await;
    assert_eq!(ready, ID);
    // The gateway's `x`, interrupted; a re-sync asked for; the fight going on: nothing goes.
    g.typed("x").await;
    g.output(&line(
        "Your meditation has been interrupted - you may not exit now!",
        33,
    ))
    .await;
    g.send(ToClient::Resync { session: ID.into() }).await;
    // A monster comes in and attacks: still nothing goes.
    for _ in 0..5 {
        g.round("The large giant rat lunges at you!", 31).await;
    }
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
    assert_eq!(bot(&seen).state, BotState::Stopped);
    let ready = seen
        .iter()
        .position(|m| matches!(m, ToGateway::ReadyToLeave { .. }))
        .unwrap();
    let after: Vec<_> = seen[ready..]
        .iter()
        .filter(|m| matches!(m, ToGateway::InputLine { .. }))
        .collect();
    assert!(after.is_empty(), "{after:?}");
}

#[tokio::test(start_paused = true)]
async fn logging_off_is_answered_with_no_bot_and_for_an_unknown_session() {
    let (mut g, run) = in_arena(&[]).await;
    for session in [ID, "nobody"] {
        g.send(ToClient::LoggingOff {
            session: session.into(),
        })
        .await;
        let ready = g
            .until(|m| match m {
                ToGateway::ReadyToLeave { session } => Some(session.clone()),
                _ => None,
            })
            .await;
        assert_eq!(ready, session);
    }
    let (_, played) = g.close(run).await;
    assert!(played.is_empty());
}

#[tokio::test(start_paused = true)]
async fn after_two_failed_walks_home_it_says_it_could_not_walk_home() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("d", refused_in_combat(34));
    g.expect("d", refused_in_combat(34));
    g.output(&resting(34)).await;
    let said = g
        .narrated("Rested; could not walk home, waiting here")
        .await;
    assert_eq!(
        said.iter()
            .filter(|n| *n == "Walking to Newhaven, Arena: one step")
            .count(),
        2,
        "{said:?}"
    );
    // Bounded: no third walk.
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d", "d"]);
    let b = bot(&seen);
    assert_eq!(b.narration, "Rested; could not walk home, waiting here");
    assert!(
        seen.iter()
            .any(|m| matches!(m, ToGateway::Notice { text, .. }
            if text == "Rested; could not walk home, waiting here")),
        "a notice"
    );
}

#[tokio::test(start_paused = true)]
async fn after_two_interrupted_walks_out_it_warns_that_it_rests_in_a_spawning_room() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", refused_in_combat(20));
    g.expect("u", refused_in_combat(20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    let warning = "Recover: resting in Newhaven, Arena though it spawns: 2 walks to a quiet room \
                   were stopped";
    g.narrated(warning).await;
    g.narrated("Recover: resting to 85% in Newhaven, Arena")
        .await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "u", "rest"]);
    let warned: Vec<_> = seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Warning { text, .. } if text.starts_with("Recover") => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(warned, [warning], "once, as a warning");
}

#[tokio::test(start_paused = true)]
async fn knocked_down_is_not_quiet_time_and_the_quiet_clock_starts_again_when_it_rises() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("You whap large giant rat for 6 damage!", 35).await;
    // Down: no blows of its own for 16 s, only the rat's. Not a lapse: no second attack.
    g.output(&line("You are smashed to the ground!", 33)).await;
    for _ in 0..4 {
        g.round("The large giant rat lunges at you!", 33).await;
    }
    g.output(&line("Slightly dazed you get back onto your feet.", 33))
        .await;
    let up = tokio::time::Instant::now();
    // Quiet from here: 12 s later the fight has lapsed, and it attacks again.
    g.expect("a rat", line(ENGAGED, 33));
    g.idle(Duration::from_secs(11)).await;
    assert_eq!(g.played, ["st", "look", "a rat"], "not yet");
    g.idle(Duration::from_secs(2)).await;
    assert_eq!(g.played, ["st", "look", "a rat", "a rat"]);
    let attacked = g
        .seen
        .iter()
        .filter(|m| matches!(m, ToGateway::InputLine { text, .. } if text == "a rat"))
        .count();
    assert_eq!(attacked, 2);
    assert!(up.elapsed() >= Duration::from_secs(12));
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn a_player_is_no_attacker_without_a_pvp_line_and_does_not_keep_recover_from_resting() {
    // Bob stands in the Arena. Lines of his read like a monster's blows (`mon_dodge_you`, and
    // `mon_miss_builtin`, which takes the verb into the name: `Bob swings`), and HP is low
    // now: Recover rests, out of the Arena as usual.
    let (mut g, run) = in_arena(&["Bob"]).await;
    start(&mut g, &["Bob"], 35).await;
    g.narrated("Waiting here").await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    let blows = [
        "Bob lunges at you, but you dodge out of the way!",
        "Bob swings at you with club.",
    ];
    g.output(&blows.map(|b| line(b, 20)).concat()).await;
    g.narrated("Recover: walking to Newhaven, Narrow Road to rest (Newhaven, Arena spawns)")
        .await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest"]);

    // A PvP line makes Bob an attacker: Recover waits for that fight to be over.
    let (mut g, run) = in_arena(&["Bob"]).await;
    start(&mut g, &["Bob"], 35).await;
    g.narrated("Waiting here").await;
    let blows = [
        "Bob moves to attack you!",
        "Bob lunges at you, but you dodge out of the way!",
    ];
    g.output(&blows.map(|b| line(b, 20)).concat()).await;
    g.idle(Duration::from_secs(5)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
}

#[tokio::test(start_paused = true)]
async fn in_combat_goes_false_in_the_snapshot_when_the_fight_lapses() {
    let blow = "You whap large giant rat for 6 damage!";
    let history = format!(
        "{}{}{}",
        arena(&["large giant rat"], 35),
        line(ENGAGED, 35),
        line(blow, 35)
    );
    let (mut g, run) = Game::start_with(SessionState::Realm, history).await;
    // The re-sync, then the fight goes on for a while.
    g.idle(Duration::from_secs(10)).await;
    for _ in 0..3 {
        g.round(blow, 34).await;
    }
    assert_eq!(as_ui_sees(&g.seen).in_combat, Some(true));
    // Then silence, no `*Combat Off*`: the last blow was 4 s ago.
    g.idle(Duration::from_secs(7)).await;
    assert_eq!(as_ui_sees(&g.seen).in_combat, Some(true), "not yet");
    g.idle(Duration::from_secs(2)).await;
    assert_eq!(as_ui_sees(&g.seen).in_combat, Some(false));
    // A new fight sets it again.
    g.output(&line(ENGAGED, 34)).await;
    g.idle(Duration::from_secs(1)).await;
    assert_eq!(as_ui_sees(&g.seen).in_combat, Some(true));
    g.close(run).await;
}

// The policies, on the world pack.

#[test]
fn the_arena_spawns_and_the_narrow_road_is_where_to_rest() {
    let a = atlas();
    let anyone = wayfarer_world::Traveller::anyone();
    assert!(policy::spawns_here(&a, Some(ARENA), true));
    assert!(!policy::spawns_here(&a, Some(NARROW_ROAD), true));
    // Not sure where it is: as if it spawned.
    assert!(policy::spawns_here(&a, Some(NARROW_ROAD), false));
    // With `health.rest_quiet_room` on.
    assert_eq!(
        policy::rest_at(&a, Some(ARENA), true, &anyone, true),
        RestAt::Walk(NARROW_ROAD)
    );
    assert_eq!(
        policy::rest_at(&a, Some(NARROW_ROAD), true, &anyone, true),
        RestAt::Here
    );
    assert_eq!(
        policy::rest_at(&a, None, false, &anyone, true),
        RestAt::HereAnyway
    );
    // Off (the default, C-053): where it stands, a lair or an arena too.
    assert_eq!(
        policy::rest_at(&a, Some(ARENA), true, &anyone, false),
        RestAt::Here
    );
}

#[test]
fn no_rest_is_walked_into_a_players_house() {
    // Slum Street, Bend (1:1096) spawns; north of it is a house (15:874) the pack has no name
    // for, which the graph leaves out (C-053): the quiet room is elsewhere, or none.
    let a = atlas();
    let anyone = wayfarer_world::Traveller::anyone();
    let bend = (1, 1096);
    let house = a
        .graph
        .edges(bend)
        .iter()
        .find(|e| e.to == (15, 874))
        .unwrap();
    assert!(!anyone.can_take(house));
    match policy::rest_at(&a, Some(bend), true, &anyone, true) {
        RestAt::Walk(k) => assert!(!a.graph.room(k).unwrap().name.is_empty(), "{k:?}"),
        RestAt::Here | RestAt::HereAnyway => {}
    }
}

#[test]
fn targets_are_hostile_attackers_first_and_never_skipped() {
    let ms = &atlas().monsters;
    assert!(policy::hostile(ms, "large giant rat"));
    assert!(!policy::hostile(ms, "healer"), "an NPC");
    assert!(!policy::hostile(ms, "Thornwyn"), "no such monster");
    let here: Vec<String> = ["healer", "large giant rat", "fat giant rat"]
        .map(String::from)
        .to_vec();
    let none = BTreeSet::new();
    let rules = wayfarer_world::MonsterRules::defaults();
    assert_eq!(
        policy::target(ms, &rules, &here, &[], &none, false),
        Some("large giant rat")
    );
    let attackers = vec!["fat giant rat".to_owned()];
    assert_eq!(
        policy::target(ms, &rules, &here, &attackers, &none, false),
        Some("fat giant rat")
    );
    let skip: BTreeSet<String> = ["fat giant rat".to_owned()].into();
    assert_eq!(
        policy::target(ms, &rules, &here, &attackers, &skip, false),
        Some("large giant rat")
    );
    assert!(policy::is_one_of(
        ms,
        "fat giant rat",
        &["giant rat".into()]
    ));
}

/// The bot says in a few words what it is doing, for the Parties panel (C-046).
#[tokio::test(start_paused = true)]
async fn the_bot_says_what_it_is_doing() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    let (seen, _) = g.close(run).await;
    assert_eq!(
        bot(&seen).doing.as_deref(),
        Some("attacking large giant rat")
    );

    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.narrated("Recover: walking to Newhaven, Narrow Road to rest (Newhaven, Arena spawns)")
        .await;
    let (seen, _) = g.close(run).await;
    assert_eq!(bot(&seen).doing.as_deref(), Some("walking to rest"));

    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    g.bot(BotAction::Pause).await;
    g.narrated("Stopped: asked to pause").await;
    let (seen, _) = g.close(run).await;
    // Off: nothing.
    assert_eq!(bot(&seen).doing, None);
}

/// From the Arena, a Go to the Village Entrance (`u`, `e`, `e`) paused once `u` has gone.
async fn paused_on_a_go_to(g: &mut Game) {
    start(g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.expect("u", room(NARROW_ROAD));
    g.after_cmd = Some((
        "u".into(),
        ToClient::BotControl {
            session: ID.into(),
            action: BotAction::Pause,
        },
    ));
    g.walk_to(VILLAGE_ENTRANCE).await;
    g.narrated("Paused: asked to pause").await;
    // Paused: the walk stopped after its move in flight, and nothing more goes.
    g.idle(Duration::from_secs(30)).await;
}

#[tokio::test(start_paused = true)]
async fn pause_on_a_go_to_keeps_it_and_resume_walks_on() {
    let (mut g, run) = in_arena(&[]).await;
    paused_on_a_go_to(&mut g).await;
    // ►: re-sync, then on from where the walk stopped.
    g.expect("st", status(35));
    g.expect("look", room(NARROW_ROAD));
    g.expect("e", room(NARROW_PATH));
    g.expect("e", room(VILLAGE_ENTRANCE));
    g.bot(BotAction::Resume).await;
    g.narrated("Resuming: re-syncing").await;
    g.walk_narrated("Arrived: Newhaven, Village Entrance").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "st", "look", "e", "e"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.journey), (BotState::Running, None));
}

#[tokio::test(start_paused = true)]
async fn stop_while_paused_on_a_go_to_drops_it_and_the_bot_stays_off() {
    let (mut g, run) = in_arena(&[]).await;
    paused_on_a_go_to(&mut g).await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.journey), (BotState::Stopped, None));
}

#[tokio::test(start_paused = true)]
async fn a_go_to_with_the_bot_off_turns_it_on() {
    let (mut g, run) = in_arena(&[]).await;
    g.expect("st", status(35));
    g.expect("look", arena(&[], 35));
    g.expect("u", room(NARROW_ROAD));
    g.walk_to(NARROW_ROAD).await;
    g.narrated("Going to Newhaven, Narrow Road: re-syncing")
        .await;
    g.narrated("Arrived: Newhaven, Narrow Road").await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.journey), (BotState::Running, None));
}
