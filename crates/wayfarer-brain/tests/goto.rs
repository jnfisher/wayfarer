//! Go to (docs/brain.md "Go to"): `find`, and a walk to a boss lair stopping one room short,
//! against the fake gateway of `game/mod.rs`. On a paused clock.

mod game;

use game::*;
use wayfarer_proto::{Found, FoundKind};

/// The Minotaur Chieftain's Large Wooden Room, and the Wooden Passage rooms north of it.
const CHIEFTAIN: Key = (1, 1865);
const BY_CHIEFTAIN: Key = (1, 1864);
const PASSAGE: Key = (1, 1863);

async fn find(g: &mut Game, query: &str) -> Vec<Found> {
    g.send(ToClient::Find {
        session: ID.into(),
        query: query.into(),
    })
    .await;
    g.until(|m| match m {
        ToGateway::Found { results, .. } => Some(results.clone()),
        _ => None,
    })
    .await
}

/// A brain whose character is certainly in the Wooden Passage two rooms north of the
/// Chieftain's: it starts in his room (the only one of its name) and walks out.
async fn in_the_passage() -> (Game, Run) {
    let (mut g, run) = Game::start(SessionState::Realm, CHIEFTAIN).await;
    g.expect("n", room(BY_CHIEFTAIN));
    g.expect("n", room(PASSAGE));
    g.walk_to(PASSAGE).await;
    g.narrated("Arrived: Wooden Passage").await;
    (g, run)
}

#[tokio::test(start_paused = true)]
async fn find_answers_with_rooms_places_and_monsters_nearest_first() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let found = find(&mut g, "arena").await;
    let arena = found.iter().find(|f| f.name == "Newhaven, Arena").unwrap();
    assert_eq!(arena.kind, FoundKind::Rooms);
    assert_eq!(arena.count, 2);
    assert_eq!(
        (arena.rooms[0].map, arena.rooms[0].room),
        ARENA,
        "nearest first"
    );
    assert_eq!(arena.rooms[0].steps, Some(3));
    assert!(arena.rooms[1].no_route);

    let found = find(&mut g, "chief").await;
    let chief = found
        .iter()
        .find(|f| f.kind == FoundKind::Monster && f.name == "minotaur chieftain")
        .unwrap();
    assert_eq!(
        chief.monsters.as_deref(),
        Some(&["minotaur chieftain".into()][..])
    );
    let lair = &chief.rooms[0];
    assert_eq!(((lair.map, lair.room), lair.boss), (CHIEFTAIN, true));

    // One letter is not a query.
    assert!(find(&mut g, "a").await.is_empty());
    let (_, played) = g.close(run).await;
    assert!(played.is_empty(), "find sends nothing to the game");
}

#[tokio::test(start_paused = true)]
async fn a_walk_to_a_boss_lair_stops_one_room_short() {
    let (mut g, run) = in_the_passage().await;
    g.expect("s", room(BY_CHIEFTAIN));
    g.walk_to(CHIEFTAIN).await;
    assert_eq!(
        g.narration().await,
        "Walking to Large Wooden Room: one step, stopping one room short"
    );
    assert_eq!(
        g.narration().await,
        "Stopped one room short of Large Wooden Room (boss: minotaur chieftain)"
    );
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["n", "n", "s"]);
    // `walking` said which lair it stops short of.
    assert!(seen.iter().any(|m| matches!(
        m,
        ToGateway::CharacterPatch { patch, .. }
            if patch["walking"]["short_of"]
                == serde_json::json!({"map": 1, "room": 1865, "name": "Large Wooden Room",
                    "bosses": ["minotaur chieftain"]})
                && patch["walking"]["to"] == serde_json::json!({"map": 1, "room": 1864})
    )));
    let pos = as_ui_sees(&seen).position.unwrap();
    assert_eq!((pos.map, pos.room), (Some(1), Some(1864)));
}

#[tokio::test(start_paused = true)]
async fn with_stop_short_off_it_walks_in() {
    let (mut g, run) = in_the_passage().await;
    g.expect("s", room(BY_CHIEFTAIN));
    g.expect("s", room(CHIEFTAIN));
    g.walk_to_short(CHIEFTAIN, Some(false)).await;
    assert_eq!(g.narration().await, "Walking to Large Wooden Room: 2 steps");
    assert_eq!(g.narration().await, "Arrived: Large Wooden Room");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "n", "s", "s"]);
}

#[tokio::test(start_paused = true)]
async fn next_to_the_lair_it_stops_at_once() {
    // The champion's cavern, from the room north of it.
    let start = (1, 2122);
    let (mut g, run) = Game::start(SessionState::Realm, start).await;
    g.walk_to((1, 2124)).await;
    assert_eq!(
        g.narration().await,
        "Stopped one room short of Crystal-Studded Cavern, Southern End (boss: minotaur champion)"
    );
    let (_, played) = g.close(run).await;
    assert!(played.is_empty());
}

#[tokio::test(start_paused = true)]
async fn an_ordinary_lair_is_walked_into() {
    // The Dungeon Entrance is a Lair room of plain monsters, not a boss's.
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is now open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    g.narrated("Arrived: Dungeon, Entrance").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "open n", "n"]);
}
