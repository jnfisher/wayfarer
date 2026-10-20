//! Loops (docs/brain.md "Loops"): `loops_request` and `walk_loop` against the fake gateway of
//! `game/mod.rs`, whose game answers each command from the test's script. On a paused clock.

mod game;

use game::*;
use wayfarer_proto::GeneratedLoop;

/// North of the Dungeon Entrance: a Lair of cave bears.
const SMALL_CAVERN: Key = (1, 2156);
const NEWHAVEN_LOOP: &str = "1:2140:8:doors:0";

impl Game {
    async fn loops_request(&mut self, doors: Option<bool>) -> (Vec<GeneratedLoop>, usize) {
        self.send(ToClient::LoopsRequest {
            session: ID.into(),
            anchor: None,
            radius: 8,
            doors,
        })
        .await;
        self.until(|m| match m {
            ToGateway::Loops {
                generated, megamud, ..
            } => Some((generated.clone(), megamud.len())),
            ToGateway::Warning { text, .. } => panic!("{text}"),
            _ => None,
        })
        .await
    }

    async fn walk_loop(&self, id: &str) {
        self.send(ToClient::WalkLoop {
            session: ID.into(),
            id: id.into(),
        })
        .await;
    }
}

#[tokio::test(start_paused = true)]
async fn the_loops_of_the_area_around_the_character() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let (generated, megamud) = g.loops_request(None).await;
    // The fixture pack's MegaMud loops through Newhaven: NARNLOO2.mp, NARNLOO3.mp and
    // Narnloop.mp (`megamud.rs`).
    assert_eq!(megamud, 3);
    assert_eq!(generated.len(), 1);
    let l = &generated[0];
    assert_eq!(
        (l.id.as_str(), l.name.as_str()),
        (NEWHAVEN_LOOP, "Newhaven")
    );
    let stops: Vec<Key> = l.stops.iter().map(|s| (s.map, s.room)).collect();
    assert_eq!(stops, [ARENA, BEYOND_ARENA, SMALL_CAVERN]);
    assert!(l.stops[0].arena && !l.stops[0].lair);
    assert!(l.stops[1].lair && l.stops[2].lair);
    assert!(l.stops[0].monsters.iter().any(|m| m == "giant rat"));
    assert_eq!((l.steps, l.doors), (4, 1));
    assert_eq!(
        l.path,
        [ARENA, BEYOND_ARENA, SMALL_CAVERN, BEYOND_ARENA, ARENA].map(key)
    );

    // Without doors the Lairs behind the Arena's door are out of the area.
    let (generated, _) = g.loops_request(Some(false)).await;
    assert_eq!(generated[0].id, "1:2140:8:nodoors:0");
    assert_eq!(generated[0].stops.len(), 1);
    assert_eq!(generated[0].steps, 0);
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn walk_loop_walks_each_stop_and_back_to_the_first() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", room(ARENA));
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.expect("n", room(SMALL_CAVERN));
    g.expect("s", room(BEYOND_ARENA));
    g.expect("s", room(ARENA));
    // Asked for without a `loops_request` first: the id says which loop.
    g.walk_loop(NEWHAVEN_LOOP).await;
    let said = g
        .narrated("Loop Newhaven done: back at Newhaven, Arena")
        .await;
    let stops: Vec<&str> = said
        .iter()
        .map(String::as_str)
        .filter(|s| s.starts_with("Loop") || s.starts_with("Walking loop"))
        .collect();
    assert_eq!(
        stops,
        [
            "Walking loop Newhaven: 3 stops, 4 steps, from Newhaven, Arena",
            "Loop stop 1/3: Newhaven, Arena",
            "Loop stop 2/3: Dungeon, Entrance",
            "Loop stop 3/3: Small Cavern",
            "Loop Newhaven done: back at Newhaven, Arena",
        ]
    );
    assert!(
        said.iter()
            .any(|s| s == "Walking to Newhaven, Arena: 3 steps")
    );
    // Nothing more once the loop is done.
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w", "w", "d", "n", "bash n", "n", "n", "s", "s"]);
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.walking, None);
    assert_eq!(ui.position.unwrap().room, Some(ARENA.1));
}

#[tokio::test(start_paused = true)]
async fn walk_loop_starts_at_the_nearest_stop() {
    // From the Dungeon Entrance: that stop first (already there), round and back to it.
    let (mut g, run) = Game::start(SessionState::Realm, BEYOND_ARENA).await;
    g.expect("n", room(SMALL_CAVERN));
    g.expect("s", room(BEYOND_ARENA));
    g.expect("s", room(ARENA));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_loop(NEWHAVEN_LOOP).await;
    let said = g
        .narrated("Loop Newhaven done: back at Dungeon, Entrance")
        .await;
    assert_eq!(
        said[0],
        "Walking loop Newhaven: 3 stops, 4 steps, from Dungeon, Entrance"
    );
    assert!(said.iter().any(|s| s == "Loop stop 1/3: Dungeon, Entrance"));
    assert!(said.iter().any(|s| s == "Loop stop 3/3: Newhaven, Arena"));
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "s", "s", "n"]);
}

#[tokio::test(start_paused = true)]
async fn walk_stop_ends_the_loop() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.walk_loop(NEWHAVEN_LOOP).await;
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    let said = g.narrated("Stopped walking: asked to stop").await;
    assert!(
        said.iter()
            .any(|s| s == "Loop Newhaven stopped: asked to stop")
    );
    // No next stop, however long it waits.
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    assert_eq!(
        as_ui_sees(&seen).position.unwrap().room,
        Some(NARROW_PATH.1)
    );
}

#[tokio::test(start_paused = true)]
async fn combat_ends_the_loop() {
    // On the way to a stop.
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let path = room(NARROW_PATH);
    let cut = path.rfind(PROMPT).unwrap();
    g.expect(
        "w",
        format!("{}\x1b[1;31m*Combat Engaged*\r\n{PROMPT}", &path[..cut]),
    );
    g.walk_loop(NEWHAVEN_LOOP).await;
    let said = g.narrated("Stopped walking: combat started").await;
    assert!(
        said.iter()
            .any(|s| s == "Loop Newhaven stopped: combat started")
    );
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w"]);

    // Arriving at a stop: the walk arrives, and the loop goes no further.
    let (mut g, run) = Game::start(SessionState::Realm, NARROW_ROAD).await;
    let arena = room(ARENA);
    let cut = arena.rfind(PROMPT).unwrap();
    g.expect(
        "d",
        format!("{}\x1b[1;31m*Combat Engaged*\r\n{PROMPT}", &arena[..cut]),
    );
    g.walk_loop(NEWHAVEN_LOOP).await;
    let said = g.narrated("Arrived: Newhaven, Arena").await;
    assert!(
        said.iter()
            .any(|s| s == "Loop Newhaven stopped: combat started")
    );
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["d"]);
    assert!(!seen.iter().any(|m| matches!(
        m,
        ToGateway::Notice { text, .. } if text.starts_with("Loop stop")
    )));
}

#[tokio::test(start_paused = true)]
async fn an_unknown_loop_is_a_warning() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    for id in ["1:2140:8:doors:7", "nonsense", "1:2140:99:doors:0"] {
        g.walk_loop(id).await;
        assert_eq!(g.narration().await, format!("walk_loop: no loop {id:?}"));
    }
    let (_, played) = g.close(run).await;
    assert!(played.is_empty());
}
