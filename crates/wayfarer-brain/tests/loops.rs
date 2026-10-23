//! Loops (docs/brain.md "Loops"): `loops_request` and `walk_loop` against the fake gateway of
//! `game/mod.rs`, whose game answers each command from the test's script. On a paused clock.

mod game;

use game::arena::{arena, bot};
use game::*;
use wayfarer_proto::{BossStatus, BotState, GeneratedLoop, MegaMudLoop};

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
            query: None,
            limit: None,
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

/// A `loops_request` with a `query` and a `limit`: the three kinds, and how many matched.
async fn loops_matching(
    g: &mut Game,
    query: &str,
    limit: Option<u32>,
) -> (Vec<GeneratedLoop>, Vec<MegaMudLoop>, u32) {
    g.send(ToClient::LoopsRequest {
        session: ID.into(),
        anchor: None,
        radius: 15,
        doors: None,
        query: Some(query.into()),
        limit,
    })
    .await;
    g.until(|m| match m {
        ToGateway::Loops {
            generated,
            megamud,
            matches,
            ..
        } => Some((generated.clone(), megamud.clone(), *matches)),
        ToGateway::Warning { text, .. } => panic!("{text}"),
        _ => None,
    })
    .await
}

#[tokio::test(start_paused = true)]
async fn a_search_finds_the_matching_loops_from_anywhere_nearest_first_and_cuts_the_list() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    // Nothing asked for: the nearest dozen at most, each with its distance, nearest first.
    let (generated, megamud, matches) = loops_matching(&mut g, "", None).await;
    let mut near: Vec<(u32, String)> = generated
        .iter()
        .map(|l| (l.distance.expect("a distance"), l.name.clone()))
        .chain(
            megamud
                .iter()
                .map(|m| (m.distance.expect("a distance"), m.file.clone())),
        )
        .collect();
    assert!(near.len() <= 12 && matches >= u32::try_from(near.len()).unwrap());
    let sorted = {
        let mut v = near.clone();
        v.sort_by_key(|(d, _)| *d);
        v
    };
    near.sort_by_key(|(d, _)| *d);
    assert_eq!(near, sorted);
    // A word of one loop's description finds it, wherever it is; a word nothing has finds none.
    let (_, megamud, matches) = loops_matching(&mut g, "cave bear", None).await;
    assert!(
        megamud.iter().any(|m| m.file == "NARNLOO3.mp"),
        "{:?}",
        megamud.iter().map(|m| &m.file).collect::<Vec<_>>()
    );
    assert!(matches >= 1);
    let (generated, megamud, matches) = loops_matching(&mut g, "no such place zzz", None).await;
    assert_eq!((generated.len(), megamud.len(), matches), (0, 0, 0));
    // The limit cuts the list, and `matches` still says how many there were.
    let (generated, megamud, all) = loops_matching(&mut g, "", Some(2)).await;
    assert_eq!(generated.len() + megamud.len(), 2);
    assert!(all > 2, "{all}");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_loops_of_the_area_around_the_character() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let (generated, megamud) = g.loops_request(None).await;
    // The fixture pack's MegaMud loops through Newhaven: NARNLOO2.mp, NARNLOO3.mp and
    // Narnloop.mp (`megamud.rs`), and others further off, up to the twelve nearest.
    assert!((3..=12).contains(&megamud), "{megamud}");
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
    g.expect("d", arena(&[], 35));
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.expect("n", room(SMALL_CAVERN));
    g.expect("s", room(BEYOND_ARENA));
    g.expect("s", arena(&[], 35));
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
    g.expect("s", arena(&[], 35));
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
    g.walk_narrated("Walking to Newhaven, Arena: 3 steps").await;
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    let said = g.narrated("Stopped walking: asked to stop").await;
    assert!(
        said.iter()
            .any(|s| s == "Stopped loop Newhaven: playing here"),
        "{said:?}"
    );
    // No next stop, however long it waits: the bot plays where it is.
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    assert_eq!(
        as_ui_sees(&seen).position.unwrap().room,
        Some(NARROW_PATH.1)
    );
    let b = bot(&seen);
    assert_eq!((b.state, b.journey), (BotState::Running, None));
}

#[tokio::test(start_paused = true)]
async fn combat_on_the_way_stops_the_leg_and_the_loop_walk_stays() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let path = room(NARROW_PATH);
    let cut = path.rfind(PROMPT).unwrap();
    g.expect(
        "w",
        format!("{}\x1b[1;31m*Combat Engaged*\r\n{PROMPT}", &path[..cut]),
    );
    g.walk_loop(NEWHAVEN_LOOP).await;
    g.walk_narrated("Stopped walking: combat started").await;
    // The bot fights; the loop walk goes on after.
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    let journey = bot(&seen).journey.expect("still on the loop walk");
    assert_eq!(journey.name, "loop Newhaven");
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

#[tokio::test(start_paused = true)]
async fn a_boss_stop_says_whether_the_boss_should_be_up() {
    // The Minotaur Chieftain's Large Wooden Room (24 hours after a kill). Killed by the
    // character, bot or no bot: down for a day from then.
    let chieftain: Key = (1, 1865);
    let (mut g, run) = Game::start(SessionState::Realm, chieftain).await;
    let stop_there = |loops: &[GeneratedLoop]| {
        loops
            .iter()
            .flat_map(|l| &l.stops)
            .find(|s| (s.map, s.room) == chieftain)
            .cloned()
            .expect("a stop at the Chieftain's room")
    };
    let (generated, _) = g.loops_request(None).await;
    assert_eq!(
        stop_there(&generated).bosses,
        [BossStatus {
            name: "minotaur chieftain".into(),
            regen_hours: 24,
            up_in_minutes: None,
        }]
    );
    g.output(&arena::line(
        "The minotaur chieftain curses at you spitefully, and dies!",
        35,
    ))
    .await;
    let (generated, _) = g.loops_request(None).await;
    let boss = stop_there(&generated).bosses[0].clone();
    assert_eq!(boss.up_in_minutes, Some(24 * 60), "{boss:?}");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn shut_in_a_hidden_pocket_the_loops_beyond_it_are_still_listed() {
    // 9:841 in the Bandit Keep: its only way out is a hidden exit no verified path takes, and
    // many Hallways look alike, so the room may be one of several.
    let (mut g, run) = Game::start(SessionState::Realm, (9, 841)).await;
    let (_, megamud, _) = loops_matching(&mut g, "hkeploop", None).await;
    let files: Vec<_> = megamud.iter().map(|m| m.file.as_str()).collect();
    assert!(files.contains(&"HKEPLOOP.mp"), "{files:?}");
    g.close(run).await;
}
