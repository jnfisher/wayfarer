//! Exits that need a bought item (docs/brain.md "Walking"): the Homely Hearth's stairs in
//! Silvermere (1:313 up) take a room ticket, which its common room sells. A walk upstairs buys one
//! on the way, and the ticket is used up by the stairs. On a paused clock.

mod game;

use game::*;
use wayfarer_world::{Route, Traveller, route_buying};

const STAIRWELL: Key = (1, 313);
const BARDS_ROOM: Key = (1, 851);
const INSERT: &str = "You insert the ticket into the box, and walk up the stairs.";

/// The route the brain plans from `from` to `to`, and the step the ticket is bought before.
fn plan(from: Key, to: Key) -> (Route, Option<usize>) {
    let a = atlas();
    let (route, buy) = route_buying(
        &a.graph,
        from,
        to,
        &Traveller::default(),
        &a.exit_items,
        |_| true,
    )
    .expect("a way");
    (route, buy.map(|b| b.at))
}

/// Script the walk: each move answered with its room, the ticket bought before step `at`, the
/// stairs taking it.
fn script(g: &mut Game, route: &Route, at: Option<usize>) {
    for (i, step) in route.iter().enumerate() {
        if Some(i) == at {
            g.expect(
                "buy room ticket",
                says("You just bought room ticket for 2 gold crowns."),
            );
        }
        let answer = if step.from == STAIRWELL && step.command == "u" {
            format!("{INSERT}\r\n{}", room(step.to))
        } else {
            room(step.to)
        };
        g.expect(&step.command, answer);
    }
}

#[tokio::test(start_paused = true)]
async fn a_walk_upstairs_buys_a_room_ticket_on_the_way_and_the_next_buys_another() {
    let (mut g, run) = Game::start(SessionState::Realm, STAIRWELL).await;
    let (up, at) = plan(STAIRWELL, BARDS_ROOM);
    assert!(at.is_some(), "a ticket to buy: {up:?}");
    script(&mut g, &up, at);
    g.walk_to(BARDS_ROOM).await;
    let said = g.narration().await;
    assert!(
        said.starts_with("Walking to Bard's Room: ")
            && said.ends_with(", buying a room ticket at The Homely Hearth, Common Room"),
        "{said}"
    );
    assert_eq!(g.narration().await, "Arrived: Bard's Room");
    // Down needs no ticket; up again needs a new one: the last went into the box.
    let (down, none) = plan(BARDS_ROOM, STAIRWELL);
    assert_eq!(none, None);
    script(&mut g, &down, None);
    // The stairwell is the sheriff's lair: walk in, not one room short.
    g.walk_to_short(STAIRWELL, Some(false)).await;
    g.narrated("Arrived: The Homely Hearth, Stairwell").await;
    script(&mut g, &up, at);
    g.walk_to(BARDS_ROOM).await;
    g.narrated("Arrived: Bard's Room").await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played.iter().filter(|c| *c == "buy room ticket").count(),
        2,
        "{played:?}"
    );
    // Bought before the stairs, each time.
    let first_buy = played.iter().position(|c| c == "buy room ticket").unwrap();
    let first_up = played.iter().position(|c| c == "u").unwrap();
    assert!(first_buy < first_up, "{played:?}");
}

#[tokio::test(start_paused = true)]
async fn a_ticket_the_shop_will_not_sell_stops_the_walk() {
    let (mut g, run) = Game::start(SessionState::Realm, STAIRWELL).await;
    let (up, at) = plan(STAIRWELL, BARDS_ROOM);
    let at = at.expect("a ticket to buy");
    for step in &up[..at] {
        g.expect(&step.command, room(step.to));
    }
    g.expect("buy room ticket", says("You cannot afford room ticket."));
    g.walk_to(BARDS_ROOM).await;
    let said = g
        .until(|m| match m {
            ToGateway::Warning { text, .. } if text.starts_with("Stopped walking") => {
                Some(text.clone())
            }
            _ => None,
        })
        .await;
    assert_eq!(
        said,
        "Stopped walking: could not buy a room ticket: cannot afford it"
    );
    let (_, played) = g.close(run).await;
    assert!(!played.contains(&"u".to_owned()), "{played:?}");
}

#[tokio::test(start_paused = true)]
async fn a_ticket_carried_is_used_and_none_is_bought() {
    let (mut g, run) = Game::start(SessionState::Realm, STAIRWELL).await;
    g.output(&says(
        "You are carrying room ticket\r\nYou have no keys.\r\nWealth: 0 copper farthings",
    ))
    .await;
    let mut t = Traveller::default();
    t.items.insert(924);
    let a = atlas();
    let (up, buy) =
        route_buying(&a.graph, STAIRWELL, BARDS_ROOM, &t, &a.exit_items, |_| true).unwrap();
    assert_eq!(buy, None);
    script(&mut g, &up, None);
    g.walk_to(BARDS_ROOM).await;
    g.narrated("Arrived: Bard's Room").await;
    let (_, played) = g.close(run).await;
    assert!(
        !played.contains(&"buy room ticket".to_owned()),
        "{played:?}"
    );
}
