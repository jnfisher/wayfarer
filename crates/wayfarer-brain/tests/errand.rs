//! Errands (docs/brain.md "Errands"): pick up all and equip all against the fake game of
//! `game/mod.rs`, with the bot stopped. On a paused clock.

mod game;

use game::arena::*;
use game::*;
use wayfarer_proto::BotAction;

/// The Arena as `look` shows it with things on the floor.
fn arena_with_floor(floor: &str) -> String {
    arena(&[], 35).replace(
        "\x1b[0;32mObvious exits",
        &format!("\x1b[0;36mYou notice {floor} here.\r\n\x1b[0;32mObvious exits"),
    )
}

async fn done(g: &mut Game, starts: &str) -> String {
    let said = |m: &ToGateway| match m {
        ToGateway::Notice { text, .. } | ToGateway::Warning { text, .. }
            if text.starts_with(starts) =>
        {
            Some(text.clone())
        }
        _ => None,
    };
    match tokio::time::timeout(Duration::from_secs(600), g.until(said)).await {
        Ok(text) => text,
        Err(_) => {
            let notes = notices(&g.seen);
            panic!(
                "no {starts:?} in 600 s; said {notes:?}; played {:?}; script {:?}",
                g.played,
                g.script.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>()
            );
        }
    }
}

#[tokio::test(start_paused = true)]
async fn pick_up_all_takes_the_coins_and_the_items_and_says_so() {
    let (mut g, run) = in_arena(&[]).await;
    g.expect(
        "look",
        arena_with_floor("5 gold crowns, 9 copper farthings, torch"),
    );
    g.expect("get 5 gold", line("You picked up 5 gold crowns", 35));
    g.expect("get 9 copper", line("You picked up 9 copper farthings", 35));
    g.expect("get torch", line("You took torch.", 35));
    g.bot(BotAction::PickUpAll).await;
    let text = done(&mut g, "Pick up done").await;
    assert_eq!(text, "Pick up done: 5 gold, 9 copper, torch");
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["look", "get 5 gold", "get 9 copper", "get torch"]);
    assert_eq!(as_ui_sees(&seen).errand, None);
}

#[tokio::test(start_paused = true)]
async fn equip_all_wears_what_wears_and_leaves_the_rest() {
    let (mut g, run) = in_arena(&[]).await;
    g.expect(
        "i",
        line(
            "You are carrying padded vest, iron helm, torch, quarterstaff (Two handed)\r\nYou have no keys.\r\nWealth: 0 copper farthings\r\nEncumbrance: 100/1000 - None [10%]",
            35,
        ),
    );
    g.expect(
        "wear padded vest",
        line("You are now wearing padded vest.", 35),
    );
    g.expect(
        "wear iron helm",
        line("You have no more room to wear that item!", 35),
    );
    g.bot(BotAction::EquipAll).await;
    let text = done(&mut g, "Equip done").await;
    assert_eq!(text, "Equip done: wearing padded vest; left: iron helm");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["i", "wear padded vest", "wear iron helm"]);
}

#[tokio::test(start_paused = true)]
async fn an_errand_is_refused_while_the_bot_runs() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.bot(BotAction::PickUpAll).await;
    let said = done(&mut g, "errand: stop or pause the bot first").await;
    assert!(said.contains("bot"), "{said}");
    g.close(run).await;
}

/// The status screen's character: resynced, with the bot stopped again.
async fn resynced(g: &mut Game) {
    g.expect("st", status(35));
    g.expect("look", arena(&[], 35));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
    g.narrated("re-synced").await;
    g.bot(BotAction::Stop).await;
}

#[tokio::test(start_paused = true)]
async fn train_walks_to_the_trainer_trains_and_stays() {
    let (mut g, run) = in_arena(&[]).await;
    resynced(&mut g).await;
    let ui = as_ui_sees(&g.seen);
    let level = i64::from(ui.level.expect("a level"));
    let a = atlas();
    let class = a
        .classes
        .get(ui.class.as_deref().expect("a class"))
        .copied();
    let dist = wayfarer_world::distances(
        &a.graph,
        ARENA,
        10_000,
        &wayfarer_world::Traveller::default(),
    );
    let (trainer, room, _) = wayfarer_world::trainers::choose(&a.trainers, class, level, |rooms| {
        rooms
            .iter()
            .filter_map(|k| dist.get(k).map(|d| (*d, *k)))
            .min()
    })
    .expect("a trainer for the level");
    let route = wayfarer_world::route(&a.graph, ARENA, room, &wayfarer_world::Traveller::default())
        .expect("a way to the trainer");
    for st in route {
        g.expect(&st.command, game::room(st.to));
    }
    g.expect(
        "train",
        line(
            "You hand over 5 silver nobles and you receive training to attain level 2.",
            35,
        ),
    );
    g.expect(
        "train",
        line("You do not have the required experience to train yet!", 35),
    );
    g.bot(BotAction::Train).await;
    let text = done(&mut g, "Train done").await;
    assert!(text.contains("1 level, paid 5 silver"), "{text}");
    assert!(
        text.contains("not enough experience")
            && text.contains(&format!("At {}: train stats here", trainer.name)),
        "{text}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(&played[played.len() - 2..], ["train", "train"]);
}

#[tokio::test(start_paused = true)]
async fn recover_needs_a_death_on_record() {
    let (mut g, run) = in_arena(&[]).await;
    g.bot(BotAction::RecoverPile).await;
    let said = done(&mut g, "Recover: no death").await;
    assert!(said.contains("on record"), "{said}");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn recover_goes_to_where_it_died_and_picks_up_and_equips() {
    let (mut g, run) = in_arena(&[]).await;
    // It died here (the fake game leaves it standing in the Arena, reborn).
    g.output(&line("You have been killed!", 35)).await;
    g.expect("look", arena_with_floor("12 gold crowns, padded vest"));
    g.expect("get 12 gold", line("You picked up 12 gold crowns", 35));
    g.expect("get padded vest", line("You took padded vest.", 35));
    g.expect(
        "i",
        line(
            "You are carrying padded vest\r\nYou have no keys.\r\nWealth: 0 copper farthings\r\nEncumbrance: 100/1000 - None [10%]",
            35,
        ),
    );
    g.expect(
        "wear padded vest",
        line("You are now wearing padded vest.", 35),
    );
    g.bot(BotAction::RecoverPile).await;
    let text = done(&mut g, "Pick up done").await;
    assert_eq!(
        text,
        "Pick up done: 12 gold, padded vest; Equip done: wearing padded vest"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(
        played,
        [
            "look",
            "get 12 gold",
            "get padded vest",
            "i",
            "wear padded vest"
        ]
    );
}
