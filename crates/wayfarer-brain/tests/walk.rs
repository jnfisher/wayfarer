//! Walking (docs/brain.md "Walking") against the fake gateway of `game/mod.rs`, whose game
//! answers each command from the test's script. On a paused clock.

mod game;

use game::arena::{arena, bot};
use game::*;
use wayfarer_brain::MAX_BASHES;
use wayfarer_proto::{BotState, ExitKind};

#[tokio::test(start_paused = true)]
async fn walks_a_route_confirming_each_step() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&[], 35));
    g.walk_to(ARENA).await;
    assert_eq!(
        g.walk_narration().await,
        "Walking to Newhaven, Arena: 3 steps"
    );
    assert_eq!(g.walk_narration().await, "Arrived: Newhaven, Arena");
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w", "w", "d"]);

    // The walk showed on the character with the commands still to type, and is gone at the end
    // (the first view of it comes with the re-sync's whole character).
    assert!(seen.iter().any(|m| match m {
        ToGateway::CharacterPatch { patch, .. } => {
            patch["walking"]["to"] == serde_json::json!({"map": 1, "room": 2150})
                && patch["walking"]["commands"] == serde_json::json!(["w", "w", "d"])
        }
        ToGateway::Character { state, .. } =>
            state.walking.as_ref().is_some_and(|w| {
                (w.to.map, w.to.room) == ARENA && w.commands == ["w", "w", "d"]
            }),
        _ => false,
    }));
    assert!(seen.iter().any(|m| matches!(
        m,
        ToGateway::CharacterPatch { patch, .. }
            if patch["walking"]["commands"] == serde_json::json!(["d"])
    )));
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.walking, None);
    let pos = ui.position.unwrap();
    assert_eq!(
        (pos.map, pos.room, pos.candidates),
        (Some(1), Some(2150), 1)
    );
    assert_eq!(pos.title.as_deref(), Some("Newhaven, Arena"));

    // The catch-up's view, one when the route starts, one after each room, centred on it; the
    // route in it while walking. Arriving ends the route in the last room's view.
    let v = views(&seen);
    let centers: Vec<RoomKey> = v.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        centers,
        [
            VILLAGE_ENTRANCE,
            VILLAGE_ENTRANCE,
            NARROW_PATH,
            NARROW_ROAD,
            ARENA
        ]
        .map(key)
    );
    assert_eq!(v[0].1, None, "the catch-up's view");
    assert_eq!(
        v[1].1,
        Some(
            [VILLAGE_ENTRANCE, NARROW_PATH, NARROW_ROAD, ARENA]
                .map(key)
                .to_vec()
        )
    );
    assert_eq!(v[3].1, Some([NARROW_ROAD, ARENA].map(key).to_vec()));
    assert_eq!(v[4].1, None);
}

#[tokio::test(start_paused = true)]
async fn the_map_view_puts_the_believed_room_at_the_center() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.send(ToClient::MapRequest {
        session: ID.into(),
        radius: Some(4),
    })
    .await;
    let (rooms, exits) = g
        .until(|m| match m {
            ToGateway::MapView { rooms, exits, .. } => Some((rooms.clone(), exits.clone())),
            _ => None,
        })
        .await;
    let at = |k: Key| rooms.iter().find(|r| (r.map, r.room) == k).unwrap();
    let c = &rooms[0];
    assert_eq!((c.map, c.room, c.x, c.y, c.level), (1, 2140, 0, 0, 0));
    assert!(c.candidate && c.visited);
    assert_eq!(c.name, "Newhaven, Village Entrance");
    let shop = at(WEAPONS_SHOP);
    assert_eq!((shop.x, shop.y), (0, -1));
    assert_eq!(shop.kind, Some(wayfarer_proto::RoomKind::Shop));
    assert!(!shop.visited && !shop.candidate);
    assert_eq!((at(NARROW_PATH).x, at(NARROW_PATH).y), (-1, 0));
    let arena = at(ARENA);
    assert_eq!((arena.x, arena.level), (-2, -1));
    assert_eq!(arena.kind, Some(wayfarer_proto::RoomKind::Arena));
    // Within four steps only.
    assert!(rooms.iter().all(|r| r.x.abs() <= 4 && r.y.abs() <= 4));
    assert!(
        exits.iter().any(|e| {
            e.from == key(VILLAGE_ENTRANCE) && e.to == key(WEAPONS_SHOP) && e.dir == "n"
        })
    );
    assert!(
        exits.iter().any(|e| e.kind == ExitKind::Door),
        "the Arena's door"
    );
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_position_follows_a_character_played_by_hand() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.typed("n").await;
    g.output(&format!("n\r\n{}", room(WEAPONS_SHOP))).await;
    let center = g
        .until(|m| match m {
            ToGateway::MapView { center, .. } => Some(*center),
            _ => None,
        })
        .await;
    assert_eq!(center, key(WEAPONS_SHOP));
    let (_, played) = g.close(run).await;
    assert_eq!(played, Vec::<String>::new());
}

#[tokio::test(start_paused = true)]
async fn no_exit_looks_and_replans_from_where_the_game_says() {
    // Pitfall 34: the brain thinks it is at the Village Entrance; it is on the Narrow Path.
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", says("There is no exit in that direction!"));
    g.expect("look", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&[], 35));
    g.walk_to(ARENA).await;
    let said = g.walk_narrated("Arrived: Newhaven, Arena").await;
    assert_eq!(
        said,
        [
            "Walking to Newhaven, Arena: 3 steps",
            "No exit `w`: looking to relocate",
            "Replanned (relocated): 2 steps",
            "Arrived: Newhaven, Arena",
        ]
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "look", "w", "d"]);
}

#[tokio::test(start_paused = true)]
async fn a_second_no_exit_stops_the_walk() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", says("There is no exit in that direction!"));
    g.expect("look", room(VILLAGE_ENTRANCE));
    g.expect("w", says("There is no exit in that direction!"));
    g.walk_to(ARENA).await;
    let said = g
        .walk_narrated("Stopped walking: no exit `w` twice: lost")
        .await;
    assert_eq!(said.len(), 4, "{said:?}");
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w", "look", "w"]);
    assert_eq!(as_ui_sees(&seen).walking, None);
}

#[tokio::test(start_paused = true)]
async fn a_shut_door_is_opened_then_walked_through() {
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&[], 35)).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is now open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    let said = g.walk_narrated(&format!("Arrived: {name}")).await;
    assert_eq!(said[0], format!("Walking to {name}: one step, one door"));
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "open n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn a_locked_door_is_bashed_at_most_three_times() {
    // Bashed open on the second try.
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&[], 35)).await;
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("Your attempts to bash through fail!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    g.walk_narrated(&format!("Arrived: {name}")).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "bash n", "bash n", "n"]);

    // Three failures: the door is marked, and there is no other way.
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&[], 35)).await;
    g.expect("n", says("The door is closed!"));
    for _ in 0..MAX_BASHES {
        g.expect("bash n", says("Your attempts to bash through fail!"));
    }
    g.walk_to(BEYOND_ARENA).await;
    let said = g.walk_narration().await;
    assert!(said.starts_with("Walking to"), "{said}");
    let said = g.walk_narration().await;
    assert!(
        said.starts_with("Stopped walking: the door would not bash open; no way to"),
        "{said}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "bash n", "bash n", "bash n"]);
}

#[tokio::test(start_paused = true)]
async fn a_locked_door_open_refused_then_bashed() {
    // `open` on a locked door (the reply the room-walk capture has), then bash.
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(&[], 35)).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is locked."));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    g.walk_narrated(&format!("Arrived: {name}")).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "open n", "bash n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn a_refusal_marks_the_exit_and_the_replan_avoids_it() {
    // The Healer's level exit (Narrow Road west, levels 1-3), refused: the brain doesn't know
    // the level yet, so it plans through; then it can't.
    let healer = (1, 2190);
    let (mut g, run) = Game::start(SessionState::Realm, NARROW_ROAD).await;
    g.expect(
        "w",
        says("You have progressed too far to go through this exit!"),
    );
    g.walk_to(healer).await;
    let said = g.walk_narration().await;
    assert_eq!(said, "Walking to Newhaven, Healer: one step");
    let said = g.walk_narration().await;
    assert!(
        said.starts_with("Stopped walking: `w` refused: level too high; no way to"),
        "{said}"
    );
    // The exit stays marked for the session: a second walk doesn't try it.
    g.walk_to(healer).await;
    let said = g.walk_narration().await;
    assert!(
        said.starts_with("Not walking to Newhaven, Healer: no way there"),
        "{said}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
}

#[tokio::test(start_paused = true)]
async fn a_refused_exit_is_walked_round() {
    // Silvermere Docks 1:27 west to 1:33 refused: round by River Street (s, w, n).
    let (from, to) = ((1, 27), (1, 33));
    let a = atlas();
    let around = wayfarer_world::route(
        &a.graph,
        from,
        to,
        &wayfarer_world::Traveller {
            blocked: [(from, 3)].into(),
            ..Default::default()
        },
    )
    .unwrap();
    let (mut g, run) = Game::start(SessionState::Realm, from).await;
    // Two neighbouring rooms are "Docks" with the same exits: the lower key is believed.
    let pos = g.position().await;
    assert_eq!((pos.room, pos.candidates), (Some(27), 2), "{pos:?}");
    g.expect("w", says("You may not go through this exit!"));
    for s in &around {
        g.expect(&s.command, room(s.to));
    }
    g.walk_to(to).await;
    let name = a.graph.room(to).unwrap().name.clone();
    let said = g.walk_narrated(&format!("Arrived: {name}")).await;
    assert!(
        said[1].starts_with("Replanned (`w` refused: forbidden)"),
        "{said:?}"
    );
    let (_, played) = g.close(run).await;
    let mut want = vec!["w".to_owned()];
    want.extend(around.iter().map(|s| s.command.clone()));
    assert_eq!(played, want);
}

#[tokio::test(start_paused = true)]
async fn combat_mid_walk_stops_after_the_move_in_flight() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let path = room(NARROW_PATH);
    let cut = path.rfind(PROMPT).unwrap();
    g.expect(
        "w",
        format!("{}\x1b[1;31m*Combat Engaged*\r\n{PROMPT}", &path[..cut]),
    );
    g.walk_to(ARENA).await;
    g.walk_narrated("Stopped walking: combat started").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.walking, None);
    assert_eq!(ui.position.unwrap().room, Some(NARROW_PATH.1));
}

#[tokio::test(start_paused = true)]
async fn a_typed_move_stops_the_walk() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.after_cmd = Some((
        "w".into(),
        ToClient::Sent {
            session: ID.into(),
            seq: 1,
            text: "n".into(),
        },
    ));
    g.walk_to(ARENA).await;
    g.walk_narrated("Stopped walking: you moved").await;
    // The bot plays where the player took the character: no more of the walk.
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.journey), (BotState::Running, None));
}

#[tokio::test(start_paused = true)]
async fn a_typed_line_that_is_no_move_leaves_the_walk_alone() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&[], 35));
    g.after_cmd = Some((
        "w".into(),
        ToClient::Sent {
            session: ID.into(),
            seq: 1,
            text: "say hello".into(),
        },
    ));
    g.walk_to(ARENA).await;
    g.walk_narrated("Arrived: Newhaven, Arena").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "w", "d"]);
}

#[tokio::test(start_paused = true)]
async fn walk_stop_stops_after_the_move_in_flight() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.walk_to(ARENA).await;
    g.walk_narrated("Walking to Newhaven, Arena: 3 steps").await;
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    let said = g.narrated("Stopped walking: asked to stop").await;
    // The bot stays on, playing where the walk stopped.
    assert!(
        said.contains(&"Stopped Newhaven, Arena: playing here".to_owned()),
        "{said:?}"
    );
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.journey), (BotState::Running, None));
    assert_eq!(
        as_ui_sees(&seen).position.unwrap().room,
        Some(NARROW_PATH.1)
    );
}

#[tokio::test(start_paused = true)]
async fn nothing_goes_outside_the_realm() {
    let (mut g, run) = Game::start(SessionState::Menu, VILLAGE_ENTRANCE).await;
    g.walk_to(ARENA).await;
    assert_eq!(
        g.walk_narration().await,
        "walk_to: the character is not in the realm"
    );
    // Leaving the realm mid-walk stops it; nothing more is sent.
    g.set_state(SessionState::Realm).await;
    g.expect("w", room(NARROW_PATH));
    g.walk_to(ARENA).await;
    assert_eq!(
        g.walk_narration().await,
        "Walking to Newhaven, Arena: 3 steps"
    );
    // The session leaves the realm while the first move is in flight.
    g.after_cmd = Some((
        "w".into(),
        ToClient::SessionState {
            session: ID.into(),
            state: SessionState::Menu,
        },
    ));
    g.walk_narrated("Stopped walking: the session left the realm")
        .await;
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
}

/// The Flaming Chamber: its east door to the Burning Valley is open in the pack, with
/// `exitpar2` -999 (no strength opens it locked).
const FLAMING_CHAMBER: Key = (17, 2978);
const BURNING_VALLEY: Key = (17, 2976);
/// The Hidden Keep in the Darkwood Forest; a Hidden exit north to the Keep Wall.
const HIDDEN_KEEP: Key = (9, 724);
const KEEP_WALL: Key = (9, 725);

/// The status screen's strength line, as `st` shows it.
fn strength(n: u32) -> String {
    format!("\r\nStrength:  {n}     Agility: 40          Tracking:        0\r\n{PROMPT}")
}

#[tokio::test(start_paused = true)]
async fn a_locked_door_beyond_the_characters_strength_is_given_up_without_a_bash() {
    let (mut g, run) = Game::start(SessionState::Realm, FLAMING_CHAMBER).await;
    g.output(&strength(50)).await;
    g.expect("e", says("The door is closed!"));
    g.walk_to(BURNING_VALLEY).await;
    let said = g
        .walk_narrated("The door east is beyond my strength (needs more than 999)")
        .await;
    assert_eq!(said[0], "Walking to Burning Valley: one step, one door");
    let stop = g.walk_narration().await;
    assert!(
        stop.starts_with("Stopped walking: the door east cannot be opened; no way to"),
        "{stop}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["e"]);
}

#[tokio::test(start_paused = true)]
async fn a_door_the_character_may_open_is_still_bashed() {
    // Strength unknown: the odds are not checked, and the door is bashed as before.
    let (mut g, run) = Game::start(SessionState::Realm, FLAMING_CHAMBER).await;
    g.expect("e", says("The door is closed!"));
    g.expect("bash e", says("You bashed the door open."));
    g.expect("e", room(BURNING_VALLEY));
    g.walk_to(BURNING_VALLEY).await;
    g.walk_narrated("Arrived: Burning Valley").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["e", "bash e", "e"]);
}

/// Search for the hidden exit north of the Hidden Keep as the player, and find it.
async fn found_north(g: &mut Game) {
    g.typed("search n").await;
    g.output(&format!(
        "search n\r\n{}",
        says("You found an exit to the north!")
    ))
    .await;
}

#[tokio::test(start_paused = true)]
async fn a_hidden_exit_found_lately_is_walked_without_a_search() {
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    found_north(&mut g).await;
    g.expect("n", room(KEEP_WALL));
    g.walk_to(KEEP_WALL).await;
    g.walk_narrated("Arrived: Darkwood Forest, Keep Wall").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n"]);
}

#[tokio::test(start_paused = true)]
async fn the_walker_searches_for_a_hidden_exit_that_has_hidden_again() {
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    found_north(&mut g).await;
    // Five minutes on, it has hidden again: the walker knows it is there, and searches.
    g.idle(wayfarer_brain::HIDDEN_FOR + Duration::from_secs(1))
        .await;
    g.expect(
        "search n",
        says("You notice nothing different to the north."),
    );
    // Is it showing after all (found lately by someone else)? No.
    g.expect("look", room(HIDDEN_KEEP));
    g.expect("search n", says("You found an exit to the north!"));
    g.expect("n", room(KEEP_WALL));
    g.walk_to(KEEP_WALL).await;
    let said = g.walk_narrated("Arrived: Darkwood Forest, Keep Wall").await;
    assert_eq!(
        said,
        [
            "Walking to Darkwood Forest, Keep Wall: one step",
            "Searching for the hidden exit north",
            "Arrived: Darkwood Forest, Keep Wall",
        ]
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n", "look", "search n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn the_walker_searches_until_the_hidden_exit_is_found() {
    use wayfarer_brain::{NARRATE_EVERY, RECHECK_EVERY};
    // A clumsy character: the exit comes on the 23rd search. `look` after the first empty
    // search and every fifth after it; a word every tenth.
    const FOUND_ON: u32 = 23;
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    found_north(&mut g).await;
    g.idle(wayfarer_brain::HIDDEN_FOR + Duration::from_secs(1))
        .await;
    let mut expected = Vec::new();
    for n in 1..FOUND_ON {
        g.expect(
            "search n",
            says("You notice nothing different to the north."),
        );
        expected.push("search n");
        if n % RECHECK_EVERY == 1 {
            g.expect("look", room(HIDDEN_KEEP));
            expected.push("look");
        }
    }
    g.expect("search n", says("You found an exit to the north!"));
    g.expect("n", room(KEEP_WALL));
    expected.extend(["search n", "n"]);
    g.walk_to(KEEP_WALL).await;
    let said = g.walk_narrated("Arrived: Darkwood Forest, Keep Wall").await;
    let still: Vec<String> = (1..FOUND_ON)
        .filter(|n| n % NARRATE_EVERY == 0)
        .map(|n| format!("Still searching for the hidden exit north ({n} tries)"))
        .collect();
    let mut want = vec![
        "Walking to Darkwood Forest, Keep Wall: one step".to_owned(),
        "Searching for the hidden exit north".to_owned(),
    ];
    want.extend(still);
    want.push("Arrived: Darkwood Forest, Keep Wall".to_owned());
    assert_eq!(said, want);
    let (_, played) = g.close(run).await;
    assert_eq!(played, expected);
}

#[tokio::test(start_paused = true)]
async fn a_hidden_exit_never_found_blocks_after_max_searches() {
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    // Str 50: the doors of the long way round are beyond it too.
    g.output(&strength(50)).await;
    found_north(&mut g).await;
    g.idle(wayfarer_brain::HIDDEN_FOR + Duration::from_secs(1))
        .await;
    let mut expected = Vec::new();
    for n in 1..=wayfarer_brain::MAX_SEARCHES {
        g.expect(
            "search n",
            says("You notice nothing different to the north."),
        );
        expected.push("search n");
        if n % wayfarer_brain::RECHECK_EVERY == 1 && n < wayfarer_brain::MAX_SEARCHES {
            g.expect("look", room(HIDDEN_KEEP));
            expected.push("look");
        }
    }
    g.walk_to(KEEP_WALL).await;
    g.walk_narrated("Searching for the hidden exit north").await;
    // Blocked, the walk plans again: only the long way round is left, by a rope and grapple
    // bought on the way (docs/brain.md "Walking").
    let replan = loop {
        let n = g.walk_narration().await;
        if n.starts_with("Replanned") || n.starts_with("Stopped walking") {
            break n;
        }
    };
    g.freeze();
    assert!(
        replan.starts_with("Replanned (no hidden exit north found)")
            && replan.contains("buying a rope and grapple"),
        "{replan}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, expected);
}

#[tokio::test(start_paused = true)]
async fn a_search_in_the_dark_stops_the_walk() {
    // `search` needs light: in a room too dark to see the game answers with the darkness line
    // (as silvermere did in Newhaven's Secret Passage, C-037's live check).
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    found_north(&mut g).await;
    g.idle(wayfarer_brain::HIDDEN_FOR + Duration::from_secs(1))
        .await;
    g.expect(
        "search n",
        says("The room is very dark - you can't see anything"),
    );
    g.walk_to(KEEP_WALL).await;
    g.walk_narrated("Searching for the hidden exit north").await;
    assert_eq!(
        g.walk_narration().await,
        "Stopped walking: too dark to search for the hidden exit north"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n"]);
}

/// The Hidden Keep as the game shows it with the hidden exit north found (by anyone, lately).
fn keep_with_north_showing() -> String {
    room(HIDDEN_KEEP).replace("Obvious exits: ", "Obvious exits: secret passage north, ")
}

#[tokio::test(start_paused = true)]
async fn an_empty_search_for_an_exit_already_showing_moves_on_after_a_look() {
    // A search for an exit someone found lately finds nothing (it needs no search,
    // `kb/navigation.md` "Hidden"); the look after it shows the exit, and the walker goes.
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    found_north(&mut g).await;
    g.idle(wayfarer_brain::HIDDEN_FOR + Duration::from_secs(1))
        .await;
    g.expect(
        "search n",
        says("You notice nothing different to the north."),
    );
    g.expect("look", keep_with_north_showing());
    g.expect("n", room(KEEP_WALL));
    g.walk_to(KEEP_WALL).await;
    g.walk_narrated("Arrived: Darkwood Forest, Keep Wall").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n", "look", "n"]);
}

#[tokio::test(start_paused = true)]
async fn a_passable_hidden_exit_is_walked_without_a_search() {
    // The Dirt Road (7:1137) north to the Black Weed Fields: never listed, but `EXF_PASSABLE`.
    let (road, fields) = ((7, 1137), (7, 1142));
    let (mut g, run) = Game::start(SessionState::Realm, road).await;
    g.expect("n", room(fields));
    g.walk_to(fields).await;
    let said = g.walk_narrated("Arrived: Black Weed Fields").await;
    assert!(!said.iter().any(|s| s.contains("Searching")), "{said:?}");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n"]);
}

/// The Slimy Tunnel in the sewers: west, a Hidden exit `push switch` opens (a search never
/// finds it), to the Narrow Stone Tunnel.
const SLIMY_TUNNEL: Key = (1, 619);
const NARROW_TUNNEL: Key = (1, 2320);

/// The Slimy Tunnel with its stone door open.
fn tunnel_with_west_open() -> String {
    room(SLIMY_TUNNEL).replace("Obvious exits: ", "Obvious exits: secret passage west, ")
}

#[tokio::test(start_paused = true)]
async fn a_hidden_exit_a_phrase_opens_is_opened_with_it_not_searched_for() {
    let (mut g, run) = Game::start(SessionState::Realm, SLIMY_TUNNEL).await;
    g.expect(
        "push switch",
        says("You push the switch, and you hear a loud grinding noise!\r\nA small stone door pops open!"),
    );
    g.expect("look", tunnel_with_west_open());
    g.expect("w", room(NARROW_TUNNEL));
    g.walk_to(NARROW_TUNNEL).await;
    let said = g.walk_narration().await;
    assert!(said.starts_with("Walking to "), "{said}");
    assert_eq!(
        g.walk_narration().await,
        "Opening the hidden exit west: push switch"
    );
    let arrived = g.walk_narration().await;
    assert!(arrived.starts_with("Arrived: "), "{arrived}");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["push switch", "look", "w"]);
}

#[tokio::test(start_paused = true)]
async fn a_hidden_exit_opened_shows_by_its_own_name_and_is_walked() {
    // Like the Crypt's knight statue: `push eye` at the Mossy Cave's Rocky Outcropping (3:36)
    // opens east, and the exits line names it by `exitpar4`'s "wide stone passage %s". Beyond is
    // the Witch's Cave, a boss lair: the walk goes all the way in.
    let (outcrop, beyond) = ((3, 36), (3, 37));
    let open = room(outcrop).replace(
        "Obvious exits: ",
        "Obvious exits: wide stone passage east, ",
    );
    let (mut g, run) = Game::start(SessionState::Realm, outcrop).await;
    g.expect("push eye", says("A section of the rock wall slides aside!"));
    g.expect("look", open);
    g.expect("e", room(beyond));
    g.walk_to_short(beyond, Some(false)).await;
    // Until it arrives or gives up: a walk that took the open exit for shut turns away.
    let mut said = Vec::new();
    let end = loop {
        let n = g.walk_narration().await;
        said.push(n.clone());
        if n.starts_with("Arrived") || n.starts_with("Stopped") || n.starts_with("Not ") {
            break n;
        }
    };
    assert_eq!(end, "Arrived: Witch's Cave", "{said:?}");
    assert!(
        said.iter()
            .any(|s| s == "Opening the hidden exit east: push eye"),
        "{said:?}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["push eye", "look", "e"]);
}

#[tokio::test(start_paused = true)]
async fn a_hidden_exit_its_phrase_did_not_open_is_walked_round() {
    // Someone else's puzzle half done, or the pack wrong: the look shows no exit west, and
    // the walk goes the long way round (south first) without typing the phrase again.
    let (mut g, run) = Game::start(SessionState::Realm, SLIMY_TUNNEL).await;
    g.expect(
        "push switch",
        says("You push the switch, and you hear a loud grinding noise!"),
    );
    g.expect("look", room(SLIMY_TUNNEL));
    g.expect("s", room((1, 618)));
    g.walk_to(NARROW_TUNNEL).await;
    let replan = loop {
        let n = g.walk_narration().await;
        if n.starts_with("Replanned") {
            break n;
        }
    };
    assert!(
        replan.starts_with("Replanned (the hidden exit west did not open): "),
        "{replan}"
    );
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    g.narrated("Stopped walking: asked to stop").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["push switch", "look", "s"]);
}

#[tokio::test(start_paused = true)]
async fn a_hidden_exit_the_exits_line_shows_is_routed_through_without_a_search() {
    // Found by someone else: the brain never searched, but the room lists it.
    let (mut g, run) = Game::start_with(SessionState::Realm, keep_with_north_showing()).await;
    g.expect("n", room(KEEP_WALL));
    g.walk_to(KEEP_WALL).await;
    g.walk_narrated("Arrived: Darkwood Forest, Keep Wall").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n"]);
}

/// Crypt Level 1: the Stone Hallway's north (1:1056) to the Small Chamber (1:1063) is sealed:
/// two blocks, taken off by the levers at 1:1044 and 1:1038; no search finds it. `Cry1cry2.mp`
/// pulls both and goes through, a path window. The walks start at the Lower Level (1:1028), the
/// one room of its name: the Crypt has 109 Stone Hallways.
const CRYPT_ENTRY: Key = (1, 1028);
const CRYPT_HALLWAY: Key = (1, 1056);
const CRYPT_CHAMBER: Key = (1, 1063);

/// The way from the Lower Level to the first lever, each move with the room it comes to.
const TO_LEVER: [(&str, i64); 10] = [
    ("n", 1029),
    ("n", 1030),
    ("n", 1031),
    ("n", 1032),
    ("w", 1039),
    ("w", 1040),
    ("s", 1041),
    ("s", 1042),
    ("s", 1043),
    ("s", 1044),
];

/// The window's moves after the first lever, each with the room it comes to; `None` is the
/// second lever.
const CRYPT_WINDOW: [(&str, Option<i64>); 28] = [
    ("n", Some(1043)),
    ("n", Some(1042)),
    ("n", Some(1041)),
    ("n", Some(1040)),
    ("e", Some(1039)),
    ("e", Some(1032)),
    ("e", Some(1033)),
    ("e", Some(1034)),
    ("s", Some(1035)),
    ("s", Some(1036)),
    ("s", Some(1037)),
    ("s", Some(1038)),
    ("pull lever", None),
    ("n", Some(1037)),
    ("n", Some(1036)),
    ("n", Some(1035)),
    ("n", Some(1034)),
    ("w", Some(1033)),
    ("w", Some(1032)),
    ("n", Some(1045)),
    ("n", Some(1046)),
    ("n", Some(1047)),
    ("n", Some(1052)),
    ("n", Some(1053)),
    ("n", Some(1054)),
    ("n", Some(1055)),
    ("n", Some(1056)),
    ("n", Some(1063)),
];

const LEVER: &str = "You pull the lever, and hear a grinding sound in the distance.";

#[tokio::test(start_paused = true)]
async fn a_sealed_hidden_exit_is_opened_by_the_paths_levers_and_walked_without_a_search() {
    let (mut g, run) = Game::start(SessionState::Realm, CRYPT_ENTRY).await;
    for (cmd, to) in TO_LEVER {
        g.expect(cmd, room((1, to)));
    }
    g.expect("pull lever", says(LEVER));
    for (cmd, to) in CRYPT_WINDOW {
        match to {
            Some(r) => g.expect(cmd, room((1, r))),
            None => g.expect(cmd, says(LEVER)),
        }
    }
    g.walk_to(CRYPT_CHAMBER).await;
    let said = g.walk_narrated("Arrived: Crypt, Small Chamber").await;
    assert_eq!(said[0], "Walking to Crypt, Small Chamber: 39 steps");
    let levers = said
        .iter()
        .filter(|s| *s == "Typing `pull lever` to open the way ahead")
        .count();
    assert_eq!(levers, 2, "{said:?}");
    assert!(!said.iter().any(|s| s.contains("Search")), "{said:?}");
    let (_, played) = g.close(run).await;
    let mut expected: Vec<&str> = TO_LEVER.map(|(c, _)| c).to_vec();
    expected.push("pull lever");
    expected.extend(CRYPT_WINDOW.map(|(c, _)| c));
    assert_eq!(played, expected);
}

/// Walk from the Lower Level to the Stone Hallway (1:1056), whose exits line is `hallway`.
async fn to_the_hallway(g: &mut Game, hallway: String) {
    let way = [
        1029, 1030, 1031, 1032, 1045, 1046, 1047, 1052, 1053, 1054, 1055,
    ];
    for r in way {
        g.expect("n", room((1, r)));
    }
    g.expect("n", hallway);
    g.walk_to(CRYPT_HALLWAY).await;
    g.walk_narrated("Arrived: Crypt, Stone Hallway").await;
}

#[tokio::test(start_paused = true)]
async fn a_sealed_hidden_exit_the_exits_line_shows_is_walked_straight_through() {
    // Someone pulled the levers: the hallway lists north, so no lever and no search.
    let (mut g, run) = Game::start(SessionState::Realm, CRYPT_ENTRY).await;
    let open = room(CRYPT_HALLWAY).replace("Obvious exits: ", "Obvious exits: north, ");
    to_the_hallway(&mut g, open).await;
    g.expect("n", room(CRYPT_CHAMBER));
    g.walk_to(CRYPT_CHAMBER).await;
    g.walk_narrated("Arrived: Crypt, Small Chamber").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played.last().map(String::as_str), Some("n"));
    assert_eq!(played.len(), 13);
}

#[tokio::test(start_paused = true)]
async fn a_shut_sealed_hidden_exit_is_never_searched_for() {
    // faye at the hallway, north shut: the walk goes back for the levers, not `search n`.
    let (mut g, run) = Game::start(SessionState::Realm, CRYPT_ENTRY).await;
    to_the_hallway(&mut g, room(CRYPT_HALLWAY)).await;
    g.expect("s", room((1, 1055)));
    g.walk_to(CRYPT_CHAMBER).await;
    assert_eq!(
        g.walk_narration().await,
        "Walking to Crypt, Small Chamber: 43 steps"
    );
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    g.narrated("Stopped walking: asked to stop").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played.last().map(String::as_str), Some("s"));
    assert!(
        !played.iter().any(|c| c.starts_with("search")),
        "{played:?}"
    );
}
