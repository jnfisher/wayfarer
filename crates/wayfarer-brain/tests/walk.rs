//! Walking (docs/brain.md "Walking") against the fake gateway of `game/mod.rs`, whose game
//! answers each command from the test's script. On a paused clock.

mod game;

use game::*;
use wayfarer_brain::MAX_BASHES;
use wayfarer_proto::ExitKind;

#[tokio::test(start_paused = true)]
async fn walks_a_route_confirming_each_step() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", room(ARENA));
    g.walk_to(ARENA).await;
    assert_eq!(g.narration().await, "Walking to Newhaven, Arena: 3 steps");
    assert_eq!(g.narration().await, "Arrived: Newhaven, Arena");
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w", "w", "d"]);

    // The walk showed on the character with the commands still to type, and is gone at the end.
    assert!(seen.iter().any(|m| matches!(
        m,
        ToGateway::CharacterPatch { patch, .. }
            if patch["walking"]["to"] == serde_json::json!({"map": 1, "room": 2150})
                && patch["walking"]["commands"] == serde_json::json!(["w", "w", "d"])
    )));
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
    g.expect("d", room(ARENA));
    g.walk_to(ARENA).await;
    let said = g.narrated("Arrived: Newhaven, Arena").await;
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
    let said = g.narrated("Stopped walking: no exit `w` twice: lost").await;
    assert_eq!(said.len(), 4, "{said:?}");
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w", "look", "w"]);
    assert_eq!(as_ui_sees(&seen).walking, None);
}

#[tokio::test(start_paused = true)]
async fn a_shut_door_is_opened_then_walked_through() {
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is now open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    let said = g.narrated(&format!("Arrived: {name}")).await;
    assert_eq!(said[0], format!("Walking to {name}: one step, one door"));
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "open n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn a_locked_door_is_bashed_at_most_three_times() {
    // Bashed open on the second try.
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("Your attempts to bash through fail!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    g.narrated(&format!("Arrived: {name}")).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "bash n", "bash n", "n"]);

    // Three failures: the door is marked, and there is no other way.
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is closed!"));
    for _ in 0..MAX_BASHES {
        g.expect("bash n", says("Your attempts to bash through fail!"));
    }
    g.walk_to(BEYOND_ARENA).await;
    let said = g.narration().await;
    assert!(said.starts_with("Walking to"), "{said}");
    let said = g.narration().await;
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
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is locked."));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    g.narrated(&format!("Arrived: {name}")).await;
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
    let said = g.narration().await;
    assert_eq!(said, "Walking to Newhaven, Healer: one step");
    let said = g.narration().await;
    assert!(
        said.starts_with("Stopped walking: `w` refused: level too high; no way to"),
        "{said}"
    );
    // The exit stays marked for the session: a second walk doesn't try it.
    g.walk_to(healer).await;
    let said = g.narration().await;
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
    let said = g.narrated(&format!("Arrived: {name}")).await;
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
    g.narrated("Stopped walking: combat started").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.walking, None);
    assert_eq!(ui.position.unwrap().room, Some(NARROW_PATH.1));
}

#[tokio::test(start_paused = true)]
async fn a_typed_line_stops_the_walk() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.after_next = Some(ToClient::Sent {
        session: ID.into(),
        seq: 1,
        text: "say hello".into(),
    });
    g.walk_to(ARENA).await;
    g.narrated("Stopped walking: you typed a line").await;
    // Nothing more from the walker, however long it waits.
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
}

#[tokio::test(start_paused = true)]
async fn walk_stop_stops_after_the_move_in_flight() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.walk_to(ARENA).await;
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    g.narrated("Stopped walking: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
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
        g.narration().await,
        "walk_to: the character is not in the realm"
    );
    // Leaving the realm mid-walk stops it; nothing more is sent.
    g.set_state(SessionState::Realm).await;
    g.expect("w", room(NARROW_PATH));
    g.walk_to(ARENA).await;
    assert_eq!(g.narration().await, "Walking to Newhaven, Arena: 3 steps");
    // The session leaves the realm while the first move is in flight.
    g.after_next = Some(ToClient::SessionState {
        session: ID.into(),
        state: SessionState::Menu,
    });
    g.narrated("Stopped walking: the session left the realm")
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
        .narrated("The door east is beyond my strength (needs more than 999)")
        .await;
    assert_eq!(said[0], "Walking to Burning Valley: one step, one door");
    let stop = g.narration().await;
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
    g.narrated("Arrived: Burning Valley").await;
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
    g.narrated("Arrived: Darkwood Forest, Keep Wall").await;
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
    g.expect("search n", says("You found an exit to the north!"));
    g.expect("n", room(KEEP_WALL));
    g.walk_to(KEEP_WALL).await;
    let said = g.narrated("Arrived: Darkwood Forest, Keep Wall").await;
    assert_eq!(
        said,
        [
            "Walking to Darkwood Forest, Keep Wall: one step",
            "Searching for the hidden exit north",
            "Arrived: Darkwood Forest, Keep Wall",
        ]
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n", "search n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn two_empty_searches_block_the_hidden_exit() {
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    // Str 50: the doors of the long way round are beyond it too.
    g.output(&strength(50)).await;
    found_north(&mut g).await;
    g.idle(wayfarer_brain::HIDDEN_FOR + Duration::from_secs(1))
        .await;
    for _ in 0..wayfarer_brain::MAX_SEARCHES {
        g.expect(
            "search n",
            says("You notice nothing different to the north."),
        );
    }
    g.walk_to(KEEP_WALL).await;
    g.narrated("Searching for the hidden exit north").await;
    let stop = g.narration().await;
    assert!(
        stop.starts_with("Stopped walking: no hidden exit north found; no way to"),
        "{stop}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n", "search n"]);
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
    g.narrated("Searching for the hidden exit north").await;
    assert_eq!(
        g.narration().await,
        "Stopped walking: too dark to search for the hidden exit north"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n"]);
}
