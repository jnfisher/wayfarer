//! Key exits (docs/brain.md "Walking"): a route takes one only with the key held, and the walker
//! turns it with `use <key>`. Thrag's cavern (1:1810) is in by the large iron key (item 476) east
//! of the Guardroom (1:1794). On a paused clock.

mod game;

use game::*;

const GUARDROOM: Key = (1, 1794);
const CAVERN: Key = (1, 1810);

#[tokio::test(start_paused = true)]
async fn a_key_exit_is_turned_with_use_when_the_key_is_carried() {
    let (mut g, run) = Game::start(SessionState::Realm, GUARDROOM).await;
    // The keys line of `i`: the brain learns it holds the key.
    g.output(&says("You have the following keys:  large iron key."))
        .await;
    // The game's texts (`kb/navigation.md` "Key"; seen at Rhudaur's doors on 2026-11-10): a
    // shut Key exit refuses the move, the key takes a direction, and the door is still shut.
    g.expect("e", says("There is a closed door in that direction!"));
    g.expect(
        "use large iron key e",
        says("You successfully unlocked the door."),
    );
    g.expect("open e", says("The door is now open."));
    g.expect("e", room(CAVERN));
    // Thrag's lair: walk in, not one room short of it.
    g.walk_to_short(CAVERN, Some(false)).await;
    assert_eq!(
        g.walk_narration().await,
        "Walking to Gigantic Cavern: one step, one door"
    );
    assert_eq!(g.walk_narration().await, "Arrived: Gigantic Cavern");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["e", "use large iron key e", "open e", "e"]);
}

#[tokio::test(start_paused = true)]
async fn without_the_key_there_is_no_way_in_and_it_says_so() {
    let (mut g, run) = Game::start(SessionState::Realm, GUARDROOM).await;
    g.walk_to(CAVERN).await;
    let said = g
        .until(|m| match m {
            ToGateway::Warning { text, .. } if text.starts_with("Not walking") => {
                Some(text.clone())
            }
            _ => None,
        })
        .await;
    assert!(said.contains("no way there"), "{said}");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn a_key_exit_that_will_not_unlock_is_given_up() {
    let (mut g, run) = Game::start(SessionState::Realm, GUARDROOM).await;
    g.output(&says("You have the following keys:  large iron key."))
        .await;
    g.expect("e", says("There is a closed door in that direction!"));
    for _ in 0..2 {
        g.expect(
            "use large iron key e",
            says("The key doesn't fit the lock."),
        );
        g.expect("open e", says("The door is locked."));
    }
    g.walk_to_short(CAVERN, Some(false)).await;
    let stop = g
        .until(|m| match m {
            ToGateway::Warning { text, .. } if text.starts_with("Stopped walking") => {
                Some(text.clone())
            }
            _ => None,
        })
        .await;
    assert!(stop.contains("the door east would not unlock"), "{stop}");
    let (_, played) = g.close(run).await;
    assert_eq!(
        played,
        [
            "e",
            "use large iron key e",
            "open e",
            "use large iron key e",
            "open e"
        ]
    );
}

/// The slums' Western Door (C-130): a Door with the rusty iron key (item 569) in `exitpar4`,
/// locked, a bash at Str − 60, west of Slum Street 1/1098 to the Western Road.
const SLUM_STREET: Key = (1, 1098);
const WESTERN_ROAD: Key = (1, 2339);

/// The status screen's strength line, as `st` shows it.
fn strength(n: u32) -> String {
    format!("\r\nStrength:  {n}     Agility: 40          Tracking:        0\r\n{PROMPT}")
}

#[tokio::test(start_paused = true)]
async fn a_keyed_door_is_unlocked_with_use_then_opened() {
    let (mut g, run) = Game::start(SessionState::Realm, SLUM_STREET).await;
    // Too weak to bash it: the key is the way.
    g.output(&strength(50)).await;
    g.output(&says("You have the following keys:  rusty iron key."))
        .await;
    g.expect("w", says("The door is closed!"));
    g.expect(
        "use rusty iron key w",
        says("You successfully unlocked the door."),
    );
    g.expect("open w", says("The door is now open."));
    g.expect("w", room(WESTERN_ROAD));
    g.walk_to(WESTERN_ROAD).await;
    assert_eq!(
        g.walk_narration().await,
        "Walking to Western Road, Silvermere Entrance: one step, one door"
    );
    assert_eq!(
        g.walk_narration().await,
        "Arrived: Western Road, Silvermere Entrance"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "use rusty iron key w", "open w", "w"]);
}

#[tokio::test(start_paused = true)]
async fn a_keyed_door_a_strong_character_bashes_without_the_key() {
    let (mut g, run) = Game::start(SessionState::Realm, SLUM_STREET).await;
    g.output(&strength(130)).await;
    g.expect("w", says("The door is closed!"));
    g.expect("bash w", says("You bashed the door open."));
    g.expect("w", room(WESTERN_ROAD));
    g.walk_to(WESTERN_ROAD).await;
    g.walk_narrated("Arrived: Western Road, Silvermere Entrance")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "bash w", "w"]);
}

/// The Rancid Sewer's gate east of 9/873 (`(2, -200, 0, 976)`): the skeleton key is the only
/// way through to the Shadow Fist Temple and no strength bashes it; the Rhudaur Thieves' Guild
/// sells the key, so a character without one goes by way of the shop (brain.md "Items a shop
/// sells"), as Lexi could not on 2026-11-22 ("no route").
const THIEVES_GUILD: Key = (2, 2561);
const MEDITATION_CHAMBER: Key = (9, 930);

#[tokio::test(start_paused = true)]
async fn a_keyed_gate_without_the_key_is_routed_by_way_of_the_shop_that_sells_it() {
    let (mut g, run) = Game::start(SessionState::Realm, THIEVES_GUILD).await;
    g.output(&strength(130)).await;
    g.walk_to(MEDITATION_CHAMBER).await;
    let said = g.walk_narration().await;
    assert!(
        said.starts_with("Walking to Shadow Fist Temple, Meditation Chamber:")
            && said.contains("buying a skeleton key at Thieve's Guild"),
        "{said}"
    );
    // The walk itself is the shop's and the sewer's business; seen what it came for.
    g.freeze();
    g.close(run).await;
}
