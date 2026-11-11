//! Go to (docs/brain.md "Go to"): `find`, and a walk to a boss lair stopping one room short,
//! against the fake gateway of `game/mod.rs`. On a paused clock.

mod game;

use game::arena::prompt;
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
    g.walk_narrated("Arrived: Wooden Passage").await;
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
        g.walk_narration().await,
        "Walking to Large Wooden Room: one step, stopping one room short"
    );
    assert_eq!(
        g.walk_narration().await,
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
    assert_eq!(
        g.walk_narration().await,
        "Walking to Large Wooden Room: 2 steps"
    );
    assert_eq!(g.walk_narration().await, "Arrived: Large Wooden Room");
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
        g.walk_narration().await,
        "Stopped one room short of Crystal-Studded Cavern, Southern End (boss: minotaur champion)"
    );
    let (_, played) = g.close(run).await;
    assert!(played.is_empty());
}

#[tokio::test(start_paused = true)]
async fn an_ordinary_lair_is_walked_into() {
    // The Dungeon Entrance is a Lair room of plain monsters, not a boss's.
    let (mut g, run) = Game::start_with(SessionState::Realm, game::arena::arena(&[], 35)).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is now open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    g.walk_narrated("Arrived: Dungeon, Entrance").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "open n", "n"]);
}

/// Slum Street, Intersection, the only room of its name; Slum Street west of it, where the
/// chime is; the Strange Mansion's Entrance the chime puts the character in (C-052).
const SLUM_INTERSECTION: Key = (1, 1183);
const SLUM_STREET: Key = (1, 1182);
const MANSION_ENTRANCE: Key = (1, 65);
const SPECTRAL_THRONE: Key = (1, 382);

#[tokio::test(start_paused = true)]
async fn the_spectral_mage_is_found_and_walked_to_by_ringing_the_chime() {
    let (mut g, run) = Game::start(SessionState::Realm, SLUM_INTERSECTION).await;
    let found = find(&mut g, "spectral mage").await;
    let mage = found
        .iter()
        .find(|f| f.kind == FoundKind::Monster && f.name == "spectral mage")
        .unwrap();
    let throne = &mage.rooms[0];
    assert_eq!((throne.map, throne.room), SPECTRAL_THRONE);
    assert!(!throne.no_route, "the chime is a way in");

    // `use chime` is the USE command, which never reaches the room's script.
    g.expect("w", room(SLUM_STREET));
    // The teleport shows no room (as on John's rig, 2026-10-24): the walker looks (C-055).
    g.expect(
        "ring chime",
        format!(
            "You ring the chime...\r\nYou find yourself...elsewhere.\r\n{}",
            prompt(35)
        ),
    );
    g.expect("look", room(MANSION_ENTRANCE));
    g.walk_to(MANSION_ENTRANCE).await;
    g.walk_narrated("Arrived: Strange Mansion, Entrance").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "ring chime", "look"]);
}

#[tokio::test(start_paused = true)]
async fn a_walk_stopped_while_the_chime_rings_still_looks_where_it_went() {
    // A party leader's hold (or any stop) that comes while the phrase is in flight: the walk
    // stops, but only after the look, or the character is lost (Faye on the live rig,
    // 2026-10-25: her followers had no room to walk back to for five minutes).
    let (mut g, run) = Game::start(SessionState::Realm, SLUM_INTERSECTION).await;
    g.expect("w", room(SLUM_STREET));
    g.expect("ring chime", NOTHING);
    g.expect("look", room(MANSION_ENTRANCE));
    g.walk_to(SPECTRAL_THRONE).await;
    g.until(|m| match m {
        ToGateway::InputLine { text, .. } if text == "ring chime" => Some(()),
        _ => None,
    })
    .await;
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    g.output(&format!(
        "ring chime\r\nYou ring the chime...\r\nYou find yourself...elsewhere.\r\n{}",
        prompt(35)
    ))
    .await;
    g.walk_narrated("Stopped walking: asked to stop").await;
    let pos = g.position().await;
    assert_eq!(pos.room, Some(MANSION_ENTRANCE.1), "{pos:?}");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "ring chime", "look"]);
}

#[tokio::test(start_paused = true)]
async fn a_phrase_that_is_only_speech_is_not_taken_for_the_move() {
    // C-055: believed on Slum Street but in the Entrance already, the chime is speech there;
    // the look shows where the character is, and it is where the walk was going.
    let (mut g, run) = Game::start(SessionState::Realm, SLUM_INTERSECTION).await;
    g.expect("w", room(SLUM_STREET));
    g.expect(
        "ring chime",
        format!("You say \"ring chime\"\r\n{}", prompt(35)),
    );
    g.expect("look", room(SLUM_STREET));
    g.expect(
        "ring chime",
        format!("You ring the chime...\r\n{}", prompt(35)),
    );
    g.expect("look", room(MANSION_ENTRANCE));
    g.walk_to(MANSION_ENTRANCE).await;
    g.walk_narrated("Arrived: Strange Mansion, Entrance").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "ring chime", "look", "ring chime", "look"]);
}

/// `room(k)` with `monster` in its `Also here:`.
fn with_monster(k: Key, monster: &str) -> String {
    let text = room(k);
    let atlas = atlas();
    let name = &atlas.graph.room(k).unwrap().name;
    let title = format!("\x1b[1;36m{name}\r\n");
    let at = text.find(&title).unwrap() + title.len();
    format!(
        "{}\x1b[0;35mAlso here: \x1b[1;35m{monster}\x1b[0m\x1b[0;35m.\r\n{}",
        &text[..at],
        &text[at..]
    )
}

/// C-091: a Go to whose legs keep stopping for a monster gone before the fight (no `*Combat
/// Engaged*`, so no fight reset the count) still gets there, because each leg ended closer.
/// Before, the third such stop ended it: "3 walks ... ended elsewhere".
#[tokio::test(start_paused = true)]
async fn a_go_to_whose_legs_each_get_closer_is_not_given_up() {
    let from = game::WEAPONS_SHOP;
    let to = ARENA;
    let atlas = atlas();
    let steps = wayfarer_world::route(
        &atlas.graph,
        from,
        to,
        &wayfarer_world::Traveller::default(),
    )
    .expect("a way from the shop to the Arena");
    assert!(steps.len() >= 4, "{}", steps.len());
    let (mut g, run) = Game::start(SessionState::Realm, from).await;
    let rats = ["fat giant rat", "thin giant rat", "large giant rat"];
    let mut want = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        let answer = match rats.get(i) {
            Some(rat) => with_monster(s.to, rat),
            // The captured Arena lists a rat: a skip lasts one room now (C-094), so it would
            // be fought there.
            None if s.to == ARENA => game::arena::arena(&[], 35),
            None => room(s.to),
        };
        g.expect(&s.command, answer);
        want.push(s.command.clone());
        if i < rats.len() {
            g.expect("a rat", says("You do not see rat here!"));
            g.expect("look", room(s.to));
            want.extend(["a rat".to_owned(), "look".to_owned()]);
        }
    }
    g.walk_to(to).await;
    let name = atlas.graph.room(to).unwrap().name.clone();
    g.walk_narrated(&format!("Arrived: {name}")).await;
    let (_, played) = g.close(run).await;
    let tail: Vec<String> = played
        .iter()
        .skip_while(|c| *c != &steps[0].command)
        .cloned()
        .collect();
    assert_eq!(tail, want);
}
