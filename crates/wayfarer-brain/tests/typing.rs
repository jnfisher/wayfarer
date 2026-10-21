//! Typing beside the bot (docs/brain.md "Typing beside the bot"): the lines a UI types while the
//! bot plays come to the brain as `user_input`; talk and look goes between the bot's actions,
//! an act pauses the bot then goes, an attack while engaged is held until forced, and keys
//! pause. Against the fake gateway of `game/mod.rs`, which fails a test that sends a line while
//! another is in flight. On a paused clock.

mod game;

use game::arena::*;
use game::*;
use wayfarer_proto::{BotAction, BotState};

impl Game {
    /// A line the player typed, as the gateway forwards it to a brain that arbitrates.
    async fn user_input(&self, text: &str, force: bool) {
        self.send(user_input(text, force)).await;
    }
}

fn user_input(text: &str, force: bool) -> ToClient {
    ToClient::UserInput {
        session: ID.into(),
        text: text.into(),
        from: "ui-1".into(),
        force,
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
    g.user_input("gossip hi", false).await;
    g.round("The large giant rat lunges at you!", 33).await;
    // A look, mid-fight, the same way.
    g.expect("l", arena(&["large giant rat"], 33));
    g.user_input("l", false).await;
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
    g.user_input("heya", false).await;
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
    g.after_cmd = Some(("a rat".into(), user_input("who", false)));
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
    g.after_cmd = Some(("u".into(), user_input("gossip hi", false)));
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
async fn an_act_pauses_the_bot_after_the_action_in_flight_then_goes() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.after_cmd = Some(("a rat".into(), user_input("u", false)));
    g.expect("u", at_hp(&room(NARROW_ROAD), 35));
    let said = g.narrated("Paused: you took over").await;
    assert!(
        said.contains(&"You took over Aldric".to_owned()),
        "{said:?}"
    );
    // Paused, up the stairs: nothing more goes.
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
    let b = bot(&seen);
    assert_eq!(
        (b.state, b.reason.as_deref()),
        (BotState::Paused, Some("you took over"))
    );
    // Paused is still played by the brain: arbitration stays on.
    assert_eq!(arbitrated(&seen), [true]);
    assert_eq!(
        as_ui_sees(&seen).position.unwrap().room,
        Some(NARROW_ROAD.1)
    );
}

#[tokio::test(start_paused = true)]
async fn an_attack_while_engaged_is_held_and_force_sends_it() {
    let (mut g, run) = in_arena(&["large giant rat", "fat giant rat"]).await;
    start(&mut g, &["large giant rat", "fat giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 34).await;
    g.user_input("a fat", false).await;
    let held = g
        .until(|m| match m {
            ToGateway::InputHeld {
                session,
                text,
                reason,
            } => Some((session.clone(), text.clone(), reason.clone())),
            _ => None,
        })
        .await;
    assert_eq!(
        held,
        (
            ID.to_owned(),
            "a fat".to_owned(),
            "no attack command while combat is engaged".to_owned()
        )
    );
    g.round("The large giant rat lunges at you!", 33).await;
    // `send anyway`: the bot pauses, and the attack goes.
    g.expect("a fat", line(ENGAGED, 33));
    g.user_input("a fat", true).await;
    g.narrated("Paused: you took over").await;
    g.idle(Duration::from_secs(10)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "a fat"]);
    assert_eq!(bot(&seen).state, BotState::Paused);
}

#[tokio::test(start_paused = true)]
async fn an_attack_out_of_combat_is_an_act() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    // Out of combat after the quiet spell the re-sync's prompt started.
    g.idle(Duration::from_secs(30)).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.user_input("a rat", false).await;
    g.narrated("Paused: you took over").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert!(
        !seen
            .iter()
            .any(|m| matches!(m, ToGateway::InputHeld { .. })),
        "held"
    );
}

#[tokio::test(start_paused = true)]
async fn keys_pause_the_bot() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.send(ToClient::KeysSent { session: ID.into() }).await;
    let said = g.narrated("Paused: you took over").await;
    assert!(
        said.contains(&"You took over Aldric".to_owned()),
        "{said:?}"
    );
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert_eq!(bot(&seen).state, BotState::Paused);
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
    g.user_input("n", false).await;
    g.idle(Duration::from_secs(5)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "n"]);
    assert_eq!(arbitrated(&seen), [true, false]);
    assert_eq!(lines(&seen).last().map(String::as_str), Some("n"));
}

#[tokio::test(start_paused = true)]
async fn an_attack_typed_while_the_bots_attack_is_in_flight_is_held_once_that_engages() {
    let (mut g, run) = in_arena(&["large giant rat", "fat giant rat"]).await;
    start(&mut g, &["large giant rat", "fat giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    // Not engaged yet when it comes: an act, so the bot pauses; engaged when it would go.
    g.after_cmd = Some(("a rat".into(), user_input("a fat", false)));
    g.narrated("Paused: you took over").await;
    let held = g
        .until(|m| match m {
            ToGateway::InputHeld { text, .. } => Some(text.clone()),
            _ => None,
        })
        .await;
    assert_eq!(held, "a fat");
    g.idle(Duration::from_secs(10)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert_eq!(bot(&seen).state, BotState::Paused);
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
    g.user_input("gossip hi", false).await;
    // Under attack: the log-off flees first, up to the Narrow Road.
    g.expect("u", at_hp(&room(NARROW_ROAD), 18));
    g.send(ToClient::LoggingOff { session: ID.into() }).await;
    // A line typed mid-flee is refused as well.
    g.user_input("gossip bye", false).await;
    g.output(&prompt(20)).await;
    g.until(|m| matches!(m, ToGateway::ReadyToLeave { .. }).then_some(()))
        .await;
    // The gateway logs off: out of the realm, into the menus.
    g.set_state(SessionState::Menu).await;
    g.idle(Duration::from_secs(10)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
    // Nothing typed went, and nothing at all after the flee's move.
    let sent = lines(&seen);
    assert!(!sent.iter().any(|l| l.starts_with("gossip")), "{sent:?}");
    assert_eq!(sent.last().map(String::as_str), Some("u"));
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
