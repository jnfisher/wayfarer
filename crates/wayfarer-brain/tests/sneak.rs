//! Sneaking and backstab (docs/brain.md "Sneaking", card C-048): with `movement.sneak` on, the
//! bot sneaks before a move it makes itself, tries again when the game says the attempt failed
//! or a move shows no `Sneaking...`, and opens a fight with `bs`. The Arena of `game/arena.rs`. On a paused clock.

mod game;

use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock};

/// The Arena with the brain's settings from `text`, its first re-sync done.
async fn in_arena_with(monsters: &[&str], text: &str) -> (Game, Run) {
    let settings = test_settings(text);
    let clock = Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)));
    let brain = Brain::new(world(), clock, None).with_settings(settings);
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(monsters, 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    (g, run)
}

const SNEAK: &str = "[movement]\nsneak = true\n";

fn attempt(hp: i32) -> String {
    line("Attempting to sneak...", hp)
}

fn noticed(hp: i32) -> String {
    line("Attempting to sneak...You don't think you're sneaking.", hp)
}

#[tokio::test(start_paused = true)]
async fn a_sneaker_sneaks_before_it_walks_to_rest() {
    let (mut g, run) = in_arena_with(&[], SNEAK).await;
    start(&mut g, &[], 20).await;
    g.expect("sneak", attempt(20));
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Sneak: sneaking").await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "sneak", "u", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn a_failure_it_noticed_is_tried_again_before_the_move() {
    let (mut g, run) = in_arena_with(&[], SNEAK).await;
    start(&mut g, &[], 20).await;
    g.expect("sneak", noticed(20));
    g.expect("sneak", noticed(20));
    g.expect("sneak", attempt(20));
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "sneak", "sneak", "sneak", "u", "rest"]
    );
}

#[tokio::test(start_paused = true)]
async fn after_five_failures_it_moves_on_unsneaked() {
    let (mut g, run) = in_arena_with(&[], SNEAK).await;
    start(&mut g, &[], 20).await;
    for _ in 0..5 {
        g.expect("sneak", noticed(20));
    }
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Sneak: 5 attempts failed; moving on unsneaked")
        .await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played.iter().filter(|c| *c == "sneak").count(), 5);
    assert_eq!(played.last().map(String::as_str), Some("rest"));
}

#[tokio::test(start_paused = true)]
async fn back_from_a_rest_it_sneaks_in_and_backstabs() {
    let (mut g, run) = in_arena_with(&[], SNEAK).await;
    start(&mut g, &[], 20).await;
    g.expect("sneak", attempt(20));
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    g.idle(Duration::from_secs(20)).await;
    // The rest ended the sneak: it sneaks again before walking back. The move shows no
    // `Sneaking...`, so the sneak had ended; `bs` still opens (a plain attack, then).
    g.expect("sneak", attempt(31));
    g.expect("d", arena(&["large giant rat"], 31));
    g.expect("bs rat", line(ENGAGED, 31));
    g.output(&resting(31)).await;
    g.narrated("Fight: large giant rat (backstab)").await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "sneak", "u", "rest", "sneak", "d", "bs rat"]
    );
}

#[tokio::test(start_paused = true)]
async fn sneak_off_changes_nothing() {
    let (mut g, run) = in_arena_with(&[], "").await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest"]);
}

/// What a sneaking move shows before the room (LJNCMDB.C:5207-5211).
fn sneaking(room_text: &str) -> String {
    format!("Sneaking...\r\n{room_text}")
}

#[tokio::test(start_paused = true)]
async fn a_move_that_shows_no_sneaking_is_followed_by_another_sneak() {
    let settings = test_settings(SNEAK);
    let clock = Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)));
    let brain = Brain::new(world(), clock, None).with_settings(settings);
    let (mut g, run) = Game::start_brain(SessionState::Realm, room(VILLAGE_ENTRANCE), brain).await;
    // A sneaker's moves are not pipelined: none goes while one is in flight.
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", room(VILLAGE_ENTRANCE));
    // The attempt said nothing more, and the first move shows `Sneaking...`: it goes on. The
    // second shows nothing: the first move's roll failed, so it sneaks again before the third.
    g.expect("sneak", attempt(35));
    g.expect("w", sneaking(&room(NARROW_PATH)));
    g.expect("w", room(NARROW_ROAD));
    g.expect("sneak", attempt(35));
    g.expect("d", sneaking(&arena(&[], 35)));
    g.send(ToClient::RunLoop {
        session: ID.into(),
        id: "1:2140:8:doors:0".into(),
        skip: Some(vec![key((1, 2156))]),
    })
    .await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    let (_, played) = g.close(run).await;
    assert_eq!(
        &played[..7],
        ["st", "look", "sneak", "w", "w", "sneak", "d"]
    );
}
