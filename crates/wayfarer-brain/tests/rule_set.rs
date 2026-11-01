//! The rule set (C-064, docs/brain.md "Rule sets"): after the re-sync, at a quiet prompt, the
//! brain asks `help 1.20`; a 1.20 server's topic makes the session 1.20 and is not game events,
//! a 1.11p server's answer makes it 1.11p, and with the setting off nothing is asked. The
//! Arena of `game/arena.rs`, on a paused clock.

mod game;

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock};
use wayfarer_proto::{SessionState, ToGateway};

const HELP: &str = include_str!("../../../tests/fixtures/qs120/help-1.20.txt");

/// The topic as a 1.20 server sends it, then the prompt.
fn topic(hp: i32) -> String {
    let room = arena(&[], hp);
    let prompt = &room[room.rfind("\x1b[79D").expect("the prompt")..];
    format!("{}\r\n{prompt}", HELP.trim_end().replace('\n', "\r\n"))
}

fn rules(seen: &[ToGateway]) -> wayfarer_proto::RuleSetView {
    as_ui_sees(seen)
        .rule_set
        .expect("the rule set on the character")
}

fn sent(seen: &[ToGateway]) -> Vec<String> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::InputLine { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_1_20_server_answers_with_its_topic() {
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&[], 35)).await;
    g.expect("help 1.20", topic(35));
    // The re-sync's commands go unanswered here (five seconds each), then the probe.
    g.idle(Duration::from_secs(40)).await;
    let (seen, played) = g.close(run).await;
    assert!(played.contains(&"help 1.20".to_owned()), "{played:?}");
    let r = rules(&seen);
    assert_eq!(r.rules, "1.20");
    assert_eq!(r.reason.as_deref(), Some("the server answered help 1.20"));
    assert_eq!(r.probes, 1);
    assert_eq!(
        r.help.first().map(String::as_str),
        Some("Quicksilver 1.20: how the rules differ from 1.11p")
    );
    // The topic's lines are no game events for the UIs.
    let topic_events: Vec<_> = seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Event { line, .. } if HELP.lines().any(|l| !l.is_empty() && l == line) => {
                Some(line.clone())
            }
            _ => None,
        })
        .collect();
    assert!(topic_events.is_empty(), "{topic_events:?}");
    assert!(
        notices(&seen)
            .iter()
            .any(|n| n == "The server plays Quicksilver 1.20 (the server answered help 1.20)"),
        "{:?}",
        notices(&seen)
    );
    // Asked once only.
    assert_eq!(sent(&seen).iter().filter(|s| *s == "help 1.20").count(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_1_11p_server_has_no_such_topic() {
    // The harness answers `help 1.20` as a stock server does.
    let (mut g, run) = in_arena(&[]).await;
    g.idle(Duration::from_secs(20)).await;
    let (seen, _) = g.close(run).await;
    let r = rules(&seen);
    assert_eq!(r.rules, "1.11p");
    assert_eq!(r.reason.as_deref(), Some("no help 1.20 topic"));
    assert!(
        !notices(&seen).iter().any(|n| n.contains("plays")),
        "1.11p is told nowhere but the snapshot: {:?}",
        notices(&seen)
    );
    assert_eq!(sent(&seen).iter().filter(|s| *s == "help 1.20").count(), 1);
}

#[tokio::test(start_paused = true)]
async fn with_the_setting_off_nothing_is_asked() {
    let clock = FixedClock(UNIX_EPOCH + Duration::from_secs(T0));
    let brain = Brain::new(world(), Arc::new(clock), None)
        .with_settings(test_settings("[bot]\nask_rules = false\n"));
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(&[], 35), brain).await;
    g.idle(Duration::from_secs(60)).await;
    let (seen, _) = g.close(run).await;
    assert!(!sent(&seen).contains(&"help 1.20".to_owned()));
    assert_eq!(rules(&seen).rules, "unknown");
}

#[tokio::test(start_paused = true)]
async fn guard_is_never_sent_on_1_11p() {
    // The bot runs a while on a stock server: whatever it does, `guard` is not among it.
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, _) = g.close(run).await;
    assert!(
        !sent(&seen).iter().any(|s| s.starts_with("guard")),
        "{:?}",
        sent(&seen)
    );
}
