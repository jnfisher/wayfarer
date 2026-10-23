//! Sneaking and backstab (docs/brain.md "Sneaking", card C-048): with `movement.sneak` on, the
//! bot sneaks before a move it makes itself, tries again when the game says the attempt failed,
//! and opens a fight with `bs` while sneaking. The Arena of `game/arena.rs`. On a paused clock.

mod game;

use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock};
use wayfarer_settings::{Layer, Overrides, Settings};

/// The Arena with the brain's settings from `text`, its first re-sync done.
async fn in_arena_with(monsters: &[&str], text: &str) -> (Game, Run) {
    let over = Overrides::parse(text).unwrap();
    let settings = Settings::resolve(&[(Layer::Character, &over)]).unwrap();
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
    // The rest ended the sneak: it sneaks again before walking back, and opens with `bs`.
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
