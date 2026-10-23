//! The CI replay layer (`docs/replay.md`, `docs/plan/07-development-and-testing.md` test
//! layer 3): a live-check recording played through a replay gateway into a brain on the
//! recorded clock, which must want to send exactly what the brain sent live (`sent.log`'s
//! lines with origin `brain`), in order. On a paused clock, so a minute of recording takes
//! none. Each test is about half a second: most of it loading the world pack (`game::world`).
//!
//! The fixtures (`tests/fixtures/replay/`) are re-recorded by a card that changes a decision
//! on purpose; `docs/replay.md` says how.

mod game;

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use game::common::repo;
use tokio::sync::watch;
use wayfarer_brain::{Brain, ChannelLink, ReplayClock, Store};
use wayfarer_gateway::replay::{self, Pace, Recording, Speed};
use wayfarer_gateway::{Hub, Origin};

/// Replay `tests/fixtures/replay/<name>/probe` and return what the replay's brain sent, and
/// what the live one did.
async fn replay(name: &str) -> (Vec<String>, Vec<String>) {
    let world = game::world();
    let recording =
        Arc::new(Recording::load(&repo(&format!("tests/fixtures/replay/{name}/probe"))).unwrap());
    let logs = tempfile::tempdir().unwrap();
    let pace = Arc::new(Pace::new(recording.origin_ms(), Speed::Times(1.0)));
    let (gateway, connector) =
        replay::gateway(recording.clone(), pace.clone(), logs.path(), None).unwrap();
    let hub = Hub::new(gateway.clone(), None);
    let origin = UNIX_EPOCH + Duration::from_millis(recording.origin_ms());
    let clock = Arc::new(ReplayClock::paced(origin, pace.start(), 1.0));

    // The brain, attached while the recorded one was, as `wayfarer replay --brain` runs it.
    let (attach, mut attached) = watch::channel(false);
    let brain = tokio::spawn({
        let hub = hub.clone();
        async move {
            let mut brain = Brain::new(world, clock, None)
                .with_settings(recorded_policy())
                .with_store(Store::in_memory().unwrap());
            while attached.wait_for(|on| *on).await.is_ok() {
                let (tx, rx) = hub.connect();
                let mut link = ChannelLink::new(tx, rx);
                tokio::select! {
                    _ = brain.run(&mut link) => return,
                    _ = attached.wait_for(|on| !*on) => {}
                }
            }
        }
    });
    replay::drive_brain(&hub, &recording, &pace, |on| {
        attach.send_replace(on);
        let hub = hub.clone();
        async move { hub.wait_for_brain(on).await }
    })
    .await;
    connector.finished().wait_for(|f| *f).await.unwrap();
    gateway.shutdown_now().await;
    hub.stop();
    drop(attach);
    brain.await.unwrap();

    let mine = Recording::load(&logs.path().join(&recording.id)).unwrap();
    let wanted = mine.sent_by(Origin::Brain).into_iter().map(String::from);
    let live = recording
        .sent_by(Origin::Brain)
        .into_iter()
        .map(String::from);
    (wanted.collect(), live.collect())
}

#[tokio::test(start_paused = true)]
async fn the_walk_recording_replays_to_the_same_commands() {
    // Recorded after a Go to became the bot's journey: each walk turns the bot on (a re-sync
    // first) and the live check turns it off again on arrival.
    let (wanted, live) = replay("walk-2").await;
    assert_eq!(
        live,
        [
            "st", "exp", "i", "look", "st", "exp", "i", "look", "e", "e", "n", "st", "exp", "i",
            "look", "s", "w", "w"
        ]
    );
    assert_eq!(wanted, live);
}

#[tokio::test(start_paused = true)]
async fn the_fight_recording_replays_to_the_same_commands() {
    let (wanted, live) = replay("fight-2").await;
    assert!(live.contains(&"a rat".to_owned()), "{live:?}");
    assert_eq!(wanted, live);
}

/// The HP lines the recordings were made with (flee below 30, rest below 80 to 95), before the
/// defaults became bolder: a replay must decide as the recorded brain did.
fn recorded_policy() -> wayfarer_settings::Settings {
    let over = wayfarer_settings::Overrides::parse(
        "[health]\nflee_below_hp = 30\nrest_below_hp = 80\nrest_until_hp = 95\n",
    )
    .unwrap();
    wayfarer_settings::Settings::resolve(&[(wayfarer_settings::Layer::Mine, &over)]).unwrap()
}
