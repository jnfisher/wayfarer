//! Typing beside the bot (docs/brain.md "Typing beside the bot"): the lines a UI types while the
//! bot plays come to the brain as `user_input`; every line goes between the bot's actions, and
//! none pauses the bot: a move (typed, or keys) drops its loop or journey and it plays where
//! the character goes. Against the fake gateway of `game/mod.rs`, which fails a test that sends a line while
//! another is in flight. On a paused clock.

mod game;

use game::arena::*;
use game::*;
use wayfarer_proto::{BotAction, BotState};

impl Game {
    /// A line the player typed, as the gateway forwards it to a brain that arbitrates.
    async fn user_input(&self, text: &str) {
        self.send(user_input(text)).await;
    }
}

fn user_input(text: &str) -> ToClient {
    ToClient::UserInput {
        session: ID.into(),
        text: text.into(),
        from: "ui-1".into(),
    }
}

/// The `arbitrate` messages, in order.
fn arbitrated(seen: &[ToGateway]) -> Vec<bool> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::Arbitrate { session, on } if session == ID => Some(*on),
            _ => None,
        })
        .collect()
}

/// Every line sent to the game, in order.
fn lines(seen: &[ToGateway]) -> Vec<String> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::InputLine { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn talk_goes_at_the_next_prompt_and_the_bot_keeps_fighting() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    // Engaged, nothing in flight, at a prompt: it goes at once.
    g.expect("gossip hi", line("You gossip: hi", 35));
    g.user_input("gossip hi").await;
    g.round("The large giant rat lunges at you!", 33).await;
    // A look, mid-fight, the same way.
    g.expect("l", arena(&["large giant rat"], 33));
    g.user_input("l").await;
    g.round("You whap large giant rat for 6 damage!", 33).await;
    // The bot fights on: the kill, then its own look.
    g.expect("look", arena(&[], 33));
    g.output(&kill(33)).await;
    g.narrated("Waiting here").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "gossip hi", "l", "look"]);
    let said = notices(&seen);
    assert!(
        !said
            .iter()
            .any(|n| n.contains("took over") || n.starts_with("Paused")),
        "{said:?}"
    );
    assert_eq!(bot(&seen).state, BotState::Running);
    // Start turned arbitration on.
    assert_eq!(arbitrated(&seen), [true]);
}

#[tokio::test(start_paused = true)]
async fn a_line_that_is_no_command_and_no_exit_phrase_is_talk_and_pauses_nothing() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.expect("heya", at_hp(&arena(&[], 35), 35));
    g.user_input("heya").await;
    g.idle(Duration::from_secs(5)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "heya"]);
    let said = notices(&seen);
    assert!(
        !said
            .iter()
            .any(|n| n.contains("took over") || n.starts_with("Paused")),
        "{said:?}"
    );
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn talk_typed_while_the_bots_action_is_in_flight_goes_after_its_answer() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.expect("who", line("Current Adventurers", 35));
    // `who` comes while `a rat` is in flight: it waits for the answer and its prompt (the fake
    // game fails the test if two lines are ever in flight at once).
    g.after_cmd = Some(("a rat".into(), user_input("who")));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 33).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "who"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn talk_goes_between_the_steps_of_the_bots_walk() {
    let (mut g, run) = in_arena(&[]).await;
    // 20 of 35: Recover walks up to the Narrow Road and rests there.
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.after_cmd = Some(("u".into(), user_input("gossip hi")));
    g.expect("gossip hi", line("You gossip: hi", 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    g.idle(Duration::from_secs(5)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "gossip hi", "rest"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn a_typed_move_keeps_the_bot_on_where_the_character_goes() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.after_cmd = Some(("a rat".into(), user_input("u")));
    g.expect("u", at_hp(&room(NARROW_ROAD), 35));
    g.narrated("Fight: large giant rat").await;
    g.idle(Duration::from_secs(5)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
    // Still on: a move typed by hand pauses nothing.
    assert_eq!(bot(&seen).state, BotState::Running);
    assert_eq!(arbitrated(&seen), [true]);
    assert_eq!(
        as_ui_sees(&seen).position.unwrap().room,
        Some(NARROW_ROAD.1)
    );
}

#[tokio::test(start_paused = true)]
async fn an_attack_typed_while_engaged_goes_and_the_bot_stays_on() {
    let (mut g, run) = in_arena(&["large giant rat", "fat giant rat"]).await;
    start(&mut g, &["large giant rat", "fat giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 34).await;
    // The player's own attack is theirs to type: it goes, and the bot plays on.
    g.expect("a fat", line(ENGAGED, 34));
    g.user_input("a fat").await;
    g.idle(Duration::from_secs(1)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "a fat"]);
    assert_eq!(bot(&seen).state, BotState::Running);
    assert_eq!(lines(&seen).last().map(String::as_str), Some("a fat"));
}

#[tokio::test(start_paused = true)]
async fn an_attack_out_of_combat_goes_and_the_bot_stays_on() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    // Out of combat after the quiet spell the re-sync's prompt started.
    g.idle(Duration::from_secs(30)).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.user_input("a rat").await;
    g.idle(Duration::from_secs(1)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn keys_count_as_a_move_and_leave_the_bot_on() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.send(ToClient::KeysSent { session: ID.into() }).await;
    g.idle(Duration::from_secs(1)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn stopped_the_brain_arbitrates_nothing_and_a_late_line_goes_straight() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    g.until(|m| matches!(m, ToGateway::Arbitrate { on: false, .. }).then_some(()))
        .await;
    // A line the gateway forwarded before it heard: it goes at once, as the player's.
    g.expect("n", line("You cannot go that way!", 35));
    g.user_input("n").await;
    g.idle(Duration::from_secs(5)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "n"]);
    assert_eq!(arbitrated(&seen), [true, false]);
    assert_eq!(lines(&seen).last().map(String::as_str), Some("n"));
}

#[tokio::test(start_paused = true)]
async fn an_attack_typed_while_the_bots_attack_is_in_flight_goes_after_its_answer() {
    let (mut g, run) = in_arena(&["large giant rat", "fat giant rat"]).await;
    start(&mut g, &["large giant rat", "fat giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.expect("a fat", line(ENGAGED, 35));
    g.after_cmd = Some(("a rat".into(), user_input("a fat")));
    g.narrated("Fight: large giant rat").await;
    g.idle(Duration::from_secs(1)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "a fat"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn a_line_waiting_when_logging_off_starts_is_dropped_and_nothing_typed_goes_out() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 20).await;
    // The rat's line with no prompt after it: talk waits for one.
    g.output("The large giant rat lunges at you!\r\n").await;
    g.user_input("gossip hi").await;
    // Under attack, but a hangup (`bot.log_off`) does not flee first.
    g.send(ToClient::LoggingOff { session: ID.into() }).await;
    // A line typed while logging off is refused as well.
    g.user_input("gossip bye").await;
    g.output(&prompt(20)).await;
    g.until(|m| matches!(m, ToGateway::ReadyToLeave { .. }).then_some(()))
        .await;
    // The gateway logs off: out of the realm, into the menus.
    g.set_state(SessionState::Menu).await;
    g.idle(Duration::from_secs(10)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    // Nothing typed went, and nothing at all after the attack.
    let sent = lines(&seen);
    assert!(!sent.iter().any(|l| l.starts_with("gossip")), "{sent:?}");
    assert_eq!(sent.last().map(String::as_str), Some("a rat"));
    let warned: Vec<_> = seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Warning { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        warned,
        [
            "Not sent: gossip hi (the gateway is logging off)",
            "Not sent: gossip bye (the gateway is logging off)",
        ]
    );
}
