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
    g.expect("e", says("The door is closed!"));
    g.expect("use large iron key", says("You unlock the door."));
    g.expect("e", room(CAVERN));
    // Thrag's lair: walk in, not one room short of it.
    g.walk_to_short(CAVERN, Some(false)).await;
    assert_eq!(
        g.walk_narration().await,
        "Walking to Gigantic Cavern: one step, one door"
    );
    assert_eq!(g.walk_narration().await, "Arrived: Gigantic Cavern");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["e", "use large iron key", "e"]);
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
