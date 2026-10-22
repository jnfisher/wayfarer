//! The spell plan (docs/brain.md "Buying spells"): `spell_plan_request` answered from the pack's
//! scrolls, the character's class, level and spellbook, and the coins it carries. The Arena of
//! `game/arena.rs`. On a paused clock.

mod game;

use game::arena::*;
use game::*;
use wayfarer_proto::{BotAction, SpellPlanItem};

/// The capture's status screen as a character of `class` with `mana` of 20.
fn caster_status(class: &str, hp: i32, mana: i32) -> String {
    let screen = status(hp).replace("Warrior    ", &format!("{class:<10} "));
    let hits = "Thievery:    \x1b[36m    0\r\n";
    assert!(screen.contains(hits));
    screen.replacen(
        hits,
        &format!(
            "{hits}\x1b[32mMana: \x1b[36m * {mana}/20    \x1b[32mSpellcasting: \x1b[36m  40     \
             \x1b[32mTraps:       \x1b[36m    0\r\n"
        ),
        1,
    )
}

fn spells_screen(rows: &[(i32, i32, &str, &str)], hp: i32) -> String {
    let mut s = String::from("You have the following spells:\r\nLevel Mana Short Spell Name\r\n");
    for (level, mana, short, name) in rows {
        s.push_str(&format!("  {level}   {mana}    {short}  {name}\r\n"));
    }
    s + &prompt(hp)
}

async fn plan(g: &mut Game) -> (u64, Vec<SpellPlanItem>) {
    g.send(ToClient::SpellPlanRequest { session: ID.into() })
        .await;
    g.until(|m| match m {
        ToGateway::SpellPlan { cash, items, .. } => Some((*cash, items.clone())),
        ToGateway::Warning { text, .. } => panic!("{text}"),
        _ => None,
    })
    .await
}

#[tokio::test(start_paused = true)]
async fn a_mage_is_offered_the_scrolls_it_lacks_nearest_the_arena() {
    let (mut g, run) = in_arena(&[]).await;
    g.expect("st", caster_status("Mage", 35, 20));
    g.expect("look", arena(&[], 35));
    g.expect(
        "spells",
        spells_screen(&[(1, 1, "mmis", "magic missile")], 35),
    );
    g.bot(BotAction::Start).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
    g.narrated("re-synced").await;
    let (_, items) = plan(&mut g).await;
    // What it knows is not offered; what the Newhaven Spell Shop has is, with a way there.
    assert!(!items.is_empty());
    assert!(
        !items.iter().any(|i| i.spell == "magic missile"),
        "{items:?}"
    );
    let shop = items
        .iter()
        .find(|i| i.shop == "Newhaven Spell Shop")
        .expect("a Newhaven spell");
    assert!(shop.steps.is_some(), "{shop:?}");
    // Those it can read now come first.
    let first_later = items.iter().position(|i| !i.usable).unwrap_or(items.len());
    assert!(items[..first_later].iter().all(|i| i.usable));
    assert!(items[first_later..].iter().all(|i| !i.usable));
    // No coins carried: only free scrolls are affordable.
    assert!(
        items.iter().filter(|i| i.affordable).all(|i| i.price == 0),
        "{items:?}"
    );
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_plan_waits_for_the_class_and_the_spellbook() {
    let (mut g, run) = in_arena(&[]).await;
    g.send(ToClient::SpellPlanRequest { session: ID.into() })
        .await;
    let said = g
        .until(|m| match m {
            ToGateway::Warning { text, .. } if text.starts_with("spell plan") => Some(text.clone()),
            _ => None,
        })
        .await;
    assert!(said.contains("not known"), "{said}");
    g.close(run).await;
}
