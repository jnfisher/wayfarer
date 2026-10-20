//! Running one of MegaMud's loops (docs/brain.md "Running a loop"): `run_loop` on a `.mp` file
//! of the fixture pack's MegaMud paths (`tests/fixtures/megamud`), against the fake gateway of
//! `game/mod.rs`. On a paused clock.

mod game;

use std::path::Path;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, InstantClock, Store};
use wayfarer_proto::{BotMode, BotState, MegaMudLoop};
use wayfarer_world::{Traveller, route};

/// The Chieftain's Large Wooden Room: off the Wooden Passages loop, in the middle of it.
const CHIEFTAIN: Key = (1, 1865);
const DUNGEON_ENTRANCE: Key = BEYOND_ARENA;

impl Game {
    async fn run_mm(&self, file: &str, skip: &[Key]) {
        self.send(ToClient::RunLoop {
            session: ID.into(),
            id: file.into(),
            skip: Some(skip.iter().map(|&k| key(k)).collect()),
        })
        .await;
    }

    /// The MegaMud loops `loops_request` lists around the character.
    async fn megamud_loops(&mut self, radius: u32) -> Vec<MegaMudLoop> {
        self.send(ToClient::LoopsRequest {
            session: ID.into(),
            anchor: None,
            radius,
            doors: None,
        })
        .await;
        self.until(|m| match m {
            ToGateway::Loops { megamud, .. } => Some(megamud.clone()),
            ToGateway::Warning { text, .. } => panic!("{text}"),
            _ => None,
        })
        .await
    }
}

/// A brain attached at `at`, its first re-sync done, ready for `run_loop`.
async fn ready_at(at: Key) -> (Game, Run) {
    let (mut g, run) = Game::start(SessionState::Realm, at).await;
    g.pipelining = true;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", room(at));
    (g, run)
}

/// The stops of the fixture's MegaMud loop `file`.
fn stops_of(file: &str) -> Vec<Key> {
    atlas()
        .mm_loops
        .iter()
        .find(|l| l.path.file == file)
        .unwrap()
        .stops()
}

/// Expect each step of the route from `from` to `to`, each answered with its room.
fn expect_leg(g: &mut Game, from: Key, to: Key) -> usize {
    let r = route(&atlas().graph, from, to, &Traveller::default()).unwrap();
    for s in &r {
        g.expect(&s.command, room(s.to));
    }
    r.len()
}

#[tokio::test(start_paused = true)]
async fn the_wooden_passages_loop_goes_to_its_nearest_stop_and_round_once() {
    let stops = stops_of("Mwodloop.mp");
    let n = stops.len();
    // The nearest stop to the Chieftain's room, the first of equals in loop order.
    let near = |k: Key| {
        route(&atlas().graph, CHIEFTAIN, k, &Traveller::default())
            .unwrap()
            .len()
    };
    let first = (0..n).min_by_key(|&i| (near(stops[i]), i)).unwrap();
    let (mut g, run) = ready_at(CHIEFTAIN).await;
    expect_leg(&mut g, CHIEFTAIN, stops[first]);
    for k in 1..n {
        let (a, b) = (stops[(first + k - 1) % n], stops[(first + k) % n]);
        expect_leg(&mut g, a, b);
    }
    // The passages are Normal rooms, always ready: round two starts at once, from the nearest
    // stop again, its leg left unanswered.
    let last = stops[(first + n - 1) % n];
    for s in route(&atlas().graph, last, stops[first], &Traveller::default())
        .unwrap()
        .iter()
        .take(1)
    {
        g.expect(&s.command, PROMPT);
    }
    g.run_mm("Mwodloop.mp", &[]).await;
    g.narrated("Running loop Wooden Passages: re-syncing").await;
    let end = (first + n - 1) % n + 1;
    g.narrated(&format!("Stop {end}/{n} cleared: nothing there"))
        .await;
    while !g.narration().await.starts_with("Walking to") {}
    let (seen, played) = g.close(run).await;
    let said = notices(&seen);
    // Every stop cleared once, in loop order from the nearest.
    let cleared: Vec<&String> = said.iter().filter(|s| s.contains(" cleared")).collect();
    assert_eq!(cleared.len(), n, "{said:?}");
    assert!(cleared[0].starts_with(&format!("Stop {}/{n} ", first + 1)));
    assert!(!played.iter().any(|c| c.starts_with("a ")));
    let b = bot(&seen);
    assert_eq!((b.state, b.mode), (BotState::Running, BotMode::Task));
    let task = b.task.unwrap();
    assert_eq!(
        (task.loop_id.as_str(), task.loop_name.as_str()),
        ("Mwodloop.mp", "Wooden Passages")
    );
    assert_eq!(
        task.stops,
        stops.iter().map(|&k| key(k)).collect::<Vec<_>>()
    );
    assert!(!task.stops.contains(&key(CHIEFTAIN)));
}

#[tokio::test(start_paused = true)]
async fn skip_passes_a_megamud_loops_stop_by() {
    // NARNLOO2.mp: the Arena and the Dungeon Entrance behind its door; the Entrance unticked.
    let (mut g, run) = ready_at(VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&[], 35));
    g.run_mm("NARNLOO2.mp", &[DUNGEON_ENTRANCE]).await;
    g.narrated("Loop Newhaven Arena: to stop 1/2 Newhaven, Arena (3 steps)")
        .await;
    g.narrated("Stop 1/2 cleared: nothing there").await;
    g.narrated("Skipping Dungeon, Entrance (unticked)").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "w", "w", "d"]);
    let task = bot(&seen).task.unwrap();
    assert_eq!(task.loop_id, "NARNLOO2.mp");
    assert_eq!(task.skip, [key(DUNGEON_ENTRANCE)]);
}

#[tokio::test(start_paused = true)]
async fn an_unknown_file_or_a_loop_with_nothing_to_hunt_is_not_run() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.run_mm("NOSUCH.mp", &[]).await;
    assert_eq!(g.narration().await, "run_loop: no loop \"NOSUCH.mp\"");
    g.run_mm("Bosploop.mp", &[]).await;
    assert_eq!(
        g.narration().await,
        "Not running loop Oasis Loop: nothing to hunt on it"
    );
    let (seen, played) = g.close(run).await;
    assert!(played.is_empty(), "{played:?}");
    assert_eq!(bot(&seen).state, BotState::Stopped);
}

#[tokio::test(start_paused = true)]
async fn the_loops_through_newhaven_list_their_stops_and_whether_they_run() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let mm = g.megamud_loops(8).await;
    let files: Vec<&str> = mm.iter().map(|l| l.file.as_str()).collect();
    assert_eq!(files, ["NARNLOO2.mp", "NARNLOO3.mp", "Narnloop.mp"]);
    let narn3 = &mm[1];
    assert_eq!(narn3.description, "Newhaven Arena (cave bear)");
    let stops: Vec<Key> = narn3.stops.iter().map(|s| (s.map, s.room)).collect();
    assert_eq!(stops, [ARENA, DUNGEON_ENTRANCE, (1, 2156)]);
    assert!(narn3.stops[0].arena && narn3.stops[2].lair);
    assert!(
        mm.iter().all(|l| l.runnable && l.reason.is_none()),
        "{mm:?}"
    );
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn a_loop_with_a_leg_no_route_takes_is_listed_with_the_reason() {
    // CRSTLOOP.mp, the Mummy Loop: north of the Stone Hallway is a level 10-only exit, and the
    // character is level 1 (its status screen in the history).
    let tomb = stops_of("CRSTLOOP.mp")[0];
    let (mut g, run) =
        Game::start_with(SessionState::Realm, format!("{}{}", status(35), room(tomb))).await;
    let mm = g.megamud_loops(2).await;
    let crst = mm.iter().find(|l| l.file == "CRSTLOOP.mp").unwrap();
    assert!(!crst.runnable);
    let why = crst.reason.as_deref().unwrap();
    assert!(why.starts_with("no way from "), "{why}");
    assert!(
        why.ends_with(": the exit north of Crypt, Stone Hallway is for level 10 only"),
        "{why}"
    );
    g.run_mm("CRSTLOOP.mp", &[]).await;
    assert_eq!(
        g.narration().await,
        format!("Not running loop Mummy Loop: {why}")
    );
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_loops_traveller_knows_the_hidden_exits_its_path_searches_for() {
    // VLEFLOOP.mp, the Giant Efreetis: `se[search se]` and `nw[search nw]` on its path. Its
    // legs go through them: the walker searches first.
    let stops = stops_of("VLEFLOOP.mp");
    let g0 = &atlas().graph;
    let t = Traveller {
        searchable: atlas().knowledge.hidden_exits("VLEFLOOP.mp"),
        ..Traveller::default()
    };
    assert_eq!(t.searchable.len(), 2);
    // Without the path's knowledge the hidden exits are not taken.
    let (a, b) = (0..stops.len())
        .map(|i| (stops[i], stops[(i + 1) % stops.len()]))
        .find(|&(a, b)| {
            route(g0, a, b, &Traveller::default())
                .is_none_or(|r| route(g0, a, b, &t).is_some_and(|h| h.len() < r.len()))
        })
        .expect("a leg through a hidden exit");
    let leg = route(g0, a, b, &t).unwrap();
    let hidden = leg
        .iter()
        .position(|s| t.searchable.contains(&(s.from, s.dir)))
        .unwrap();
    let (mut g, run) = ready_at(a).await;
    for (i, s) in leg.iter().enumerate() {
        if i == hidden {
            g.expect(
                &format!("search {}", s.command),
                says(&format!("You found an exit to the {}!", long(&s.command))),
            );
        }
        g.expect(&s.command, room(s.to));
    }
    // The leg after it, left unanswered.
    let after = stops[(stops.iter().position(|&k| k == b).unwrap() + 1) % stops.len()];
    let next = route(g0, b, after, &t).unwrap();
    for s in &next {
        g.expect(&s.command, PROMPT);
    }
    g.run_mm("VLEFLOOP.mp", &[]).await;
    let to = g0.room(b).unwrap().name.clone();
    g.narrated(&format!("Arrived: {to}")).await;
    while !g.narration().await.starts_with("Walking to") {}
    let (_, played) = g.close(run).await;
    let search = played
        .iter()
        .position(|c| c.starts_with("search "))
        .unwrap();
    assert_eq!(played[search + 1], leg[hidden].command);
}

/// `se` as the game names it in a search's answer: `southeast`.
fn long(dir: &str) -> &'static str {
    match dir {
        "n" => "north",
        "s" => "south",
        "e" => "east",
        "w" => "west",
        "ne" => "northeast",
        "nw" => "northwest",
        "se" => "southeast",
        "sw" => "southwest",
        "u" => "up",
        _ => "down",
    }
}

fn brain_on(path: &Path) -> Brain {
    let (store, _) = Store::open(path).unwrap();
    let clock = InstantClock::new(std::time::UNIX_EPOCH + Duration::from_secs(T0));
    Brain::new(world(), Arc::new(clock), None).with_store(store)
}

#[tokio::test(start_paused = true)]
async fn a_restarted_brain_resumes_a_megamud_loop_from_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");

    // The first brain: in the Arena, cleared, gone as the walk to the Dungeon Entrance starts.
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_on(&path)).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", arena(&[], 35));
    g.expect("n", PROMPT);
    g.run_mm("NARNLOO2.mp", &[]).await;
    g.narrated("Loop Newhaven Arena: to stop 2/2 Dungeon, Entrance (one step, one door)")
        .await;
    g.close(run).await;
    let saved = Store::open(&path).unwrap().0.bot(ID).unwrap().unwrap();
    assert_eq!(
        (saved.loop_id.as_deref(), saved.stop_index),
        (Some("NARNLOO2.mp"), 1)
    );

    // The second brain goes on to stop 2 under the loop's own name.
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_on(&path)).await;
    g.pipelining = true;
    g.expect("n", room(DUNGEON_ENTRANCE));
    g.expect("s", arena(&[], 35));
    g.narrated("Stop 2/2 cleared: nothing there").await;
    // On round to the Arena, and no clock keeps it from the Entrance again.
    g.narrated("Stop 1/2 cleared: nothing there").await;
    g.freeze();
    let (seen, _) = g.close(run).await;
    let said = notices(&seen);
    assert!(
        said.iter()
            .any(|s| s == "Resumed: running loop Newhaven Arena at stop 2/2 (brain restarted)"),
        "{said:?}"
    );
    assert!(!said.iter().any(|s| s.contains("no longer")), "{said:?}");
}

#[test]
fn a_loop_inside_the_keep_is_reachable_by_another_paths_hidden_exit() {
    // The Bandit Keep loop runs inside the keep; from the Hidden Keep outside it the door east
    // is locked past any bash (exitpar2 -250). The way in is the hidden exit north, which
    // another path (`Ftrehkep.mp`) searches for: the loop's traveller knows every verified
    // path's hidden exits, so the walker searches for it first.
    let hidden_keep: Key = (9, 724);
    let stops = stops_of("HKEPLOOP.mp");
    let g0 = &atlas().graph;
    let own = Traveller {
        searchable: atlas().knowledge.hidden_exits("HKEPLOOP.mp"),
        strength: Some(60),
        ..Traveller::default()
    };
    assert!(
        stops
            .iter()
            .all(|&s| route(g0, hidden_keep, s, &own).is_none())
    );
    let all = Traveller {
        searchable: atlas().knowledge.all_hidden_exits(),
        ..own
    };
    let leg = route(g0, hidden_keep, stops[0], &all).expect("a way in");
    assert_eq!(leg[0].command, "n");
    assert!(all.searchable.contains(&(leg[0].from, leg[0].dir)));
}
