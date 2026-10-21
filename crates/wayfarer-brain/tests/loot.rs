//! Coins and the attack command (docs/brain.md "Coins"): after a fight the bot picks up the
//! coins the settings want, as many as the load allows, and starts fights with the command the
//! settings name. The Arena of `game/arena.rs`, on a paused clock.

mod game;

use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock};
use wayfarer_proto::BotAction;
use wayfarer_settings::{Layer, Overrides, Settings};

/// The Arena with `monsters` and, in the room's description, the floor's `notice`.
fn floor_room(monsters: &[&str], hp: i32, notice: &str) -> String {
    let room = arena(monsters, hp);
    let cut = room
        .rfind("\x1b[79D\x1b[K\x1b[0;37m[HP=")
        .expect("the prompt");
    format!(
        "{}\x1b[0;36mYou notice {notice} here.\r\n{}",
        &room[..cut],
        &room[cut..]
    )
}

/// The inventory screen with a load of `current` of `max`.
fn inventory(current: u32, max: u32, hp: i32) -> String {
    let pct = current * 100 / max;
    format!(
        "You are carrying Nothing!\r\nWealth: 0 copper farthings\r\n\
         Encumbrance: {current}/{max} - Medium [{pct}%]\r\n{}",
        prompt(hp)
    )
}

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

/// Start the bot with the load `load` (current, max) known, and a rat to kill whose death
/// leaves `floor` in the room.
async fn kill_a_rat_leaving(g: &mut Game, load: Option<(u32, u32)>, floor: &str) {
    g.expect("st", status(35));
    if let Some((current, max)) = load {
        g.expect("i", inventory(current, max, 35));
    }
    g.expect("look", arena(&["large giant rat"], 35));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.expect("look", floor_room(&[], 35, floor));
    g.output(&kill(35)).await;
}

#[tokio::test(start_paused = true)]
async fn platinum_is_picked_up_after_a_kill_by_default() {
    let (mut g, run) = in_arena_with(&["large giant rat"], "").await;
    kill_a_rat_leaving(&mut g, None, "5 platinum pieces").await;
    g.expect(
        "get 5 platinum",
        format!("You picked up 5 platinum pieces\r\n{}", prompt(35)),
    );
    g.narrated("Loot: picking up 5 platinum").await;
    g.narrated("Waiting here").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look", "get 5 platinum"]);
}

#[tokio::test(start_paused = true)]
async fn gold_is_left_unless_the_settings_want_it() {
    let (mut g, run) = in_arena_with(&["large giant rat"], "").await;
    kill_a_rat_leaving(&mut g, None, "12 gold crowns").await;
    g.narrated("Waiting here").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look"]);
}

#[tokio::test(start_paused = true)]
async fn only_as_many_coins_as_the_load_allows_are_taken() {
    // A load of 650 of 1000: Medium may reach 66% (669), so 19 more units: 59 gold.
    let (mut g, run) = in_arena_with(&["large giant rat"], "[cash]\ngold = true\n").await;
    kill_a_rat_leaving(&mut g, Some((650, 1000)), "100 gold crowns").await;
    g.expect(
        "get 59 gold",
        format!("You picked up 59 gold crowns\r\n{}", prompt(35)),
    );
    g.narrated("Loot: picking up 59 gold").await;
    g.narrated("Waiting here").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "i", "look", "a rat", "look", "get 59 gold"]);
}

#[tokio::test(start_paused = true)]
async fn a_bigger_capacity_takes_them_all() {
    // A dwarf's capacity is 20% larger: the same load of 650 is of 1200, and 100 gold fit.
    let (mut g, run) = in_arena_with(&["large giant rat"], "[cash]\ngold = true\n").await;
    kill_a_rat_leaving(&mut g, Some((650, 1200)), "100 gold crowns").await;
    g.expect(
        "get 100 gold",
        format!("You picked up 100 gold crowns\r\n{}", prompt(35)),
    );
    g.narrated("Loot: picking up 100 gold").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "i", "look", "a rat", "look", "get 100 gold"]);
}

#[tokio::test(start_paused = true)]
async fn nothing_is_taken_that_makes_the_character_heavy() {
    // At the Medium ceiling already: only the coins that weigh nothing (two) fit.
    let (mut g, run) = in_arena_with(&["large giant rat"], "[cash]\ngold = true\n").await;
    kill_a_rat_leaving(&mut g, Some((669, 1000)), "100 gold crowns").await;
    g.expect(
        "get 2 gold",
        format!("You picked up 2 gold crowns\r\n{}", prompt(35)),
    );
    g.narrated("Loot: picking up 2 gold").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "i", "look", "a rat", "look", "get 2 gold"]);
}

#[tokio::test(start_paused = true)]
async fn the_attack_command_is_the_one_in_the_settings() {
    let (mut g, run) = in_arena_with(
        &["large giant rat"],
        "[combat]\nattack_command = \"bash\"\n",
    )
    .await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("bash rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "bash rat"]);
}
