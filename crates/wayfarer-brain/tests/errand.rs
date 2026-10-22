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
    g.until(|m| match m {
        ToGateway::Notice { text, .. } | ToGateway::Warning { text, .. }
            if text.starts_with(starts) =>
        {
            Some(text.clone())
        }
        _ => None,
    })
    .await
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
