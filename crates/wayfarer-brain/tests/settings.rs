//! Settings (docs/plan/04-settings.md): the brain reads the thresholds it used to have as
//! constants from the settings in force. Against the fake gateway of `game/mod.rs`, on a
//! paused clock.

mod game;

use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock, HIDDEN_FOR, MAX_SEARCHES, World};
use wayfarer_settings::{Layer, Overrides, Settings};

/// A world whose atlas carries `settings` (a fresh pack: the shared one is not changed).
fn world_with(settings: Settings) -> World {
    let pack = wayfarer_world::WorldPack::open(common::pack_path()).unwrap();
    World::from_pack(pack).unwrap().with_settings(settings)
}

fn settings(text: &str) -> Settings {
    let over = Overrides::parse(text).unwrap();
    Settings::resolve(&[(Layer::Character, &over)]).unwrap()
}

fn brain(text: &str) -> Brain {
    let clock = Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)));
    Brain::new(world_with(settings(text)), clock, None)
}

#[tokio::test(start_paused = true)]
async fn the_rest_line_is_a_setting() {
    // As `low_hp_rests_in_a_quiet_room_then_walks_back`, with rest until 90: 32 of 35 is 91%.
    let (mut g, run) = Game::start_brain(
        SessionState::Realm,
        arena(&[], 35),
        brain("[health]\nrest_until_hp = 90\n"),
    )
    .await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 90% in Newhaven, Narrow Road")
        .await;
    for hp in [24, 28] {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    g.expect("d", arena(&[], 32));
    g.output(&resting(32)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d"]);
}

#[tokio::test(start_paused = true)]
async fn the_search_limit_is_a_setting() {
    // Three searches and no more: the walker blocks the hidden exit (the default is 300).
    assert_eq!(
        settings("").int("movement.search_limit"),
        MAX_SEARCHES as i64
    );
    let hidden_keep: Key = (9, 724);
    let keep_wall: Key = (9, 725);
    let (mut g, run) = Game::start_brain(
        SessionState::Realm,
        room(hidden_keep),
        brain("[movement]\nsearch_limit = 3\n"),
    )
    .await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    // Str 50: the doors of the long way round are beyond it too.
    g.output(&format!(
        "\r\nStrength:  50     Agility: 40          Tracking:        0\r\n{PROMPT}"
    ))
    .await;
    // Found by the player, then hidden again five minutes on: the walker searches.
    g.typed("search n").await;
    g.output(&format!(
        "search n\r\n{}",
        says("You found an exit to the north!")
    ))
    .await;
    g.idle(HIDDEN_FOR + Duration::from_secs(1)).await;
    // A `look` after the first empty search, then two more: three in all.
    let nothing = says("You notice nothing different to the north.");
    g.expect("search n", nothing.clone());
    g.expect("look", room(hidden_keep));
    g.expect("search n", nothing.clone());
    g.expect("search n", nothing);
    g.walk_to(keep_wall).await;
    let stop = loop {
        let n = g.narration().await;
        if n.starts_with("Stopped walking") {
            break n;
        }
    };
    assert!(
        stop.starts_with("Stopped walking: no hidden exit north found"),
        "{stop}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n", "look", "search n", "search n"]);
}
