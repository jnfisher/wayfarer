//! Custom loops (docs/brain.md "Custom loops"): a loop the player makes from an existing one
//! by adding a stop, kept under a name and run like any other. Against the fake gateway of
//! `game/mod.rs`, on a paused clock.

mod game;

use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, InstantClock, Store};
use wayfarer_proto::{GeneratedLoop, MegaMudLoop};
use wayfarer_world::{Traveller, route};

const SMALL_CAVERN: Key = (1, 2156);
const HIDDEN_KEEP: Key = (9, 724);
/// Inside the Bandit Keep, on `HKEPLOOP.mp`.
const KEEP_WALL_EDGE: Key = (9, 750);
/// The Living Quarters, near the end of `HKEPLOOP.mp`'s path.
const LIVING_QUARTERS: Key = (9, 809);
/// Leo the Quick's Elaborate Bedroom: 15 steps from the Living Quarters, through seven doors
/// and a hidden exit (`Hkepleoq.mp`).
const LEO: Key = (9, 870);

impl Game {
    async fn save_loop(&self, name: &str, base: Option<&str>, stops: &[Key]) {
        self.send(ToClient::SaveLoop {
            session: ID.into(),
            name: name.into(),
            base: base.map(str::to_owned),
            stops: stops.iter().map(|&k| key(k)).collect(),
        })
        .await;
    }

    async fn delete_loop(&self, id: &str) {
        self.send(ToClient::DeleteLoop {
            session: ID.into(),
            id: id.into(),
        })
        .await;
    }

    /// `loops_request`: the generated, MegaMud and custom loops listed around the character.
    async fn all_loops(&mut self) -> (Vec<GeneratedLoop>, Vec<MegaMudLoop>, Vec<GeneratedLoop>) {
        self.send(ToClient::LoopsRequest {
            session: ID.into(),
            anchor: None,
            radius: 8,
            doors: None,
            query: None,
            limit: None,
        })
        .await;
        self.until(|m| match m {
            ToGateway::Loops {
                generated,
                megamud,
                custom,
                ..
            } => Some((generated.clone(), megamud.clone(), custom.clone())),
            ToGateway::Warning { text, .. } => panic!("{text}"),
            _ => None,
        })
        .await
    }
}

#[tokio::test(start_paused = true)]
async fn a_loop_made_from_another_is_saved_listed_and_run() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    // Newhaven's loop without its Small Cavern, and its Arena twice over: the player's own.
    g.save_loop(
        "Arena and Entrance",
        Some("1:2140:8:doors:0"),
        &[ARENA, BEYOND_ARENA],
    )
    .await;
    assert_eq!(
        g.narration().await,
        "Saved loop Arena and Entrance (2 stops): custom:arena-and-entrance"
    );
    let (generated, _, custom) = g.all_loops().await;
    // The generated Newhaven loop is as it was; the custom one is listed beside it.
    assert_eq!(generated.len(), 1);
    assert_eq!(custom.len(), 1);
    let l = &custom[0];
    assert_eq!(
        (l.id.as_str(), l.name.as_str()),
        ("custom:arena-and-entrance", "Arena and Entrance")
    );
    let stops: Vec<Key> = l.stops.iter().map(|s| (s.map, s.room)).collect();
    assert_eq!(stops, [ARENA, BEYOND_ARENA]);
    assert!(l.stops[0].arena && l.stops[1].lair);
    // The path closes: Arena, the Entrance, back to the Arena.
    assert_eq!(
        l.path.first().map(|k| (k.map, k.room)),
        l.path.last().map(|k| (k.map, k.room))
    );
    assert!(l.steps >= 2 && l.doors >= 1, "{l:?}");

    // Saved again under the same name in another case: it is replaced, not added.
    g.save_loop(
        "ARENA AND ENTRANCE",
        None,
        &[ARENA, BEYOND_ARENA, SMALL_CAVERN],
    )
    .await;
    assert_eq!(
        g.narration().await,
        "Changed loop ARENA AND ENTRANCE (3 stops): custom:arena-and-entrance"
    );
    let (_, _, custom) = g.all_loops().await;
    assert_eq!(custom.len(), 1);
    assert_eq!(custom[0].stops.len(), 3);
    assert_eq!(custom[0].name, "ARENA AND ENTRANCE");

    // Run it: Task mode starts on its nearest stop, under its own name.
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&[], 35));
    g.pipelining = true;
    g.send(ToClient::RunLoop {
        session: ID.into(),
        id: "custom:arena-and-entrance".into(),
        skip: Some(vec![key(SMALL_CAVERN)]),
    })
    .await;
    g.narrated("Loop ARENA AND ENTRANCE: to stop 1/3 Newhaven, Arena (3 steps)")
        .await;
    g.narrated("Stop 1/3 cleared: nothing there").await;
    g.freeze();
    let (seen, _) = g.close(run).await;
    let task = bot(&seen).task.expect("the loop on the snapshot");
    assert_eq!(
        (task.loop_id.as_str(), task.loop_name.as_str()),
        ("custom:arena-and-entrance", "ARENA AND ENTRANCE")
    );
    assert_eq!(task.stops, [ARENA, BEYOND_ARENA, SMALL_CAVERN].map(key));
}

#[tokio::test(start_paused = true)]
async fn a_loop_that_is_not_valid_is_refused_and_one_can_be_deleted() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let refused = |text: String| text.starts_with("save_loop:");
    g.save_loop("   ", None, &[ARENA]).await;
    assert!(refused(g.narration().await));
    g.save_loop("???", None, &[ARENA]).await;
    assert!(refused(g.narration().await));
    g.save_loop("Empty", None, &[]).await;
    assert!(refused(g.narration().await));
    g.save_loop("Nowhere", None, &[ARENA, (9, 99_999)]).await;
    assert_eq!(g.narration().await, "save_loop: no room 9/99999");
    g.save_loop("Fine", None, &[ARENA]).await;
    assert_eq!(g.narration().await, "Saved loop Fine (1 stop): custom:fine");
    g.delete_loop("1:2140:8:doors:0").await;
    assert!(
        g.narration()
            .await
            .starts_with("delete_loop: no custom loop")
    );
    g.delete_loop("custom:fine").await;
    assert_eq!(g.narration().await, "Deleted loop custom:fine");
    g.delete_loop("custom:fine").await;
    assert!(
        g.narration()
            .await
            .starts_with("delete_loop: no custom loop")
    );
    let (_, _, custom) = g.all_loops().await;
    assert!(custom.is_empty());
    // A loop not saved is not run.
    g.send(ToClient::RunLoop {
        session: ID.into(),
        id: "custom:fine".into(),
        skip: None,
    })
    .await;
    assert!(g.narration().await.starts_with("run_loop: no loop"));
    g.close(run).await;
}

fn brain_on(path: &Path) -> Brain {
    let (store, _) = Store::open(path).unwrap();
    let clock = InstantClock::new(UNIX_EPOCH + Duration::from_secs(T0));
    Brain::new(world(), Arc::new(clock), None).with_store(store)
}

#[tokio::test(start_paused = true)]
async fn a_saved_loop_survives_a_restart_of_the_brain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, room(VILLAGE_ENTRANCE), brain_on(&path)).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.save_loop("Keeper", Some("1:2140:8:doors:0"), &[BEYOND_ARENA, ARENA])
        .await;
    g.narrated("Saved loop Keeper (2 stops): custom:keeper")
        .await;
    g.close(run).await;

    // A second brain on the same store: the loop is listed, in the order it was saved.
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, room(VILLAGE_ENTRANCE), brain_on(&path)).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    let (_, _, custom) = g.all_loops().await;
    assert_eq!(custom.len(), 1);
    let stops: Vec<Key> = custom[0].stops.iter().map(|s| (s.map, s.room)).collect();
    assert_eq!(stops, [BEYOND_ARENA, ARENA]);
    g.close(run).await;
}

#[test]
fn leo_the_quicks_room_is_a_hidden_exit_and_seven_doors_from_the_living_quarters() {
    // The route the "Hidden Keep with Leo the Quick" loop takes, found by the router once the
    // hidden exits the verified paths search (`Hkepleoq.mp`) are known.
    let atlas = atlas();
    let strong = Traveller {
        strength: Some(97),
        ..Traveller::default()
    };
    // Without them the hidden exit west of 9:819 is not taken: no way.
    assert!(route(&atlas.graph, LIVING_QUARTERS, LEO, &strong).is_none());
    let knowing = Traveller {
        searchable: atlas.knowledge.all_hidden_exits(),
        ..strong
    };
    let leg = route(&atlas.graph, LIVING_QUARTERS, LEO, &knowing).expect("a way to Leo");
    assert_eq!(leg.last().map(|s| s.to), Some(LEO));
    let hidden = leg
        .iter()
        .filter(|s| knowing.searchable.contains(&(s.from, s.dir)))
        .count();
    assert_eq!(hidden, 1, "{leg:?}");
    assert_eq!(leg.iter().filter(|s| s.door.is_some()).count(), 7);
    // And the way back to the keep's courtyard through the same exits.
    assert!(route(&atlas.graph, LEO, KEEP_WALL_EDGE, &knowing).is_some());
}

#[tokio::test(start_paused = true)]
async fn hidden_keep_with_leo_the_quick_is_listed_with_its_boss_and_a_path_through_the_hidden_exit()
{
    // The player's modification: Bandit Keep's stops with Leo's room added after the Living
    // Quarters stops. The character is inside the keep.
    let (mut g, run) = Game::start(SessionState::Realm, KEEP_WALL_EDGE).await;
    let (_, megamud, _) = g.all_loops().await;
    let bandit = megamud
        .iter()
        .find(|l| l.file == "HKEPLOOP.mp")
        .expect("the Bandit Keep loop around the keep");
    let mut stops: Vec<Key> = bandit.stops.iter().map(|s| (s.map, s.room)).collect();
    // After the last Living Quarters stop on the path.
    let after = stops
        .iter()
        .rposition(|&k| k == LIVING_QUARTERS)
        .expect("the Living Quarters on the loop");
    stops.insert(after + 1, LEO);
    g.save_loop(
        "Hidden Keep with Leo the Quick",
        Some("HKEPLOOP.mp"),
        &stops,
    )
    .await;
    assert_eq!(
        g.narration().await,
        format!(
            "Saved loop Hidden Keep with Leo the Quick ({} stops): custom:hidden-keep-with-leo-the-quick",
            stops.len()
        )
    );
    let (_, _, custom) = g.all_loops().await;
    let l = custom
        .iter()
        .find(|l| l.id == "custom:hidden-keep-with-leo-the-quick")
        .expect("the custom loop listed");
    assert_eq!(l.stops.len(), stops.len());
    let leo = l
        .stops
        .iter()
        .position(|s| (s.map, s.room) == LEO)
        .expect("Leo's room a stop");
    assert_eq!(leo, after + 1);
    // Leo is its boss: never killed by this character, so he should be up.
    let boss = &l.stops[leo].bosses;
    assert_eq!(boss.len(), 1, "{boss:?}");
    assert_eq!(boss[0].name, "Leo the Quick");
    assert_eq!((boss[0].regen_hours, boss[0].up_in_minutes), (2, None));
    // The path goes to his room and comes back out: through it and the courtyard stops.
    let on_path = |k: Key| l.path.iter().any(|p| (p.map, p.room) == k);
    assert!(
        on_path(LEO) && on_path(LIVING_QUARTERS),
        "{:?}",
        l.path.len()
    );
    assert!(l.path.len() > bandit.path.len());
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_custom_loop_runs_from_the_hidden_keep_and_searches_for_the_way() {
    // Started outside the keep, at the Hidden Keep: the loop's first leg needs the hidden
    // exit north, which only the verified paths know (`Ftrehkep.mp`).
    let (mut g, run) = Game::start(SessionState::Realm, HIDDEN_KEEP).await;
    g.pipelining = true;
    // Str 50: the Hidden Keep's east door (exitpar2 -250) is beyond it, as Thorgrim's is.
    g.output(&format!(
        "\r\nStrength:  50     Agility: 40          Tracking:        0\r\n{PROMPT}"
    ))
    .await;
    g.save_loop("Leo Run", None, &[KEEP_WALL_EDGE, LIVING_QUARTERS, LEO])
        .await;
    g.narrated("Saved loop Leo Run (3 stops): custom:leo-run")
        .await;
    g.expect("search n", says("You found an exit to the north!"));
    g.send(ToClient::RunLoop {
        session: ID.into(),
        id: "custom:leo-run".into(),
        skip: None,
    })
    .await;
    let said = g.narrated("Searching for the hidden exit north").await;
    assert!(
        said.iter().any(|s| s.starts_with("Loop Leo Run: to stop")),
        "{said:?}"
    );
    // The search goes out and is answered; what follows (the step north, the rest of the 14
    // steps to the first stop) is not scripted.
    g.until(|m| matches!(m, ToGateway::InputLine { text, .. } if text == "search n").then_some(()))
        .await;
    g.freeze();
    let (_, played) = g.close(run).await;
    assert_eq!(
        played.last().map(String::as_str),
        Some("search n"),
        "{played:?}"
    );
}
