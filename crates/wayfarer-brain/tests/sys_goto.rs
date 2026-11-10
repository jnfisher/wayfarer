//! `sys_goto` (C-080): a character teleports to a town with `sysop goto`, the brain takes the
//! town's room as where it is (the game shows none), and re-syncs. On a paused clock.

mod game;

use game::*;

const NEWHAVEN_ARENA: Key = (1, 2150);

#[tokio::test(start_paused = true)]
async fn a_sysop_goto_lands_the_character_in_the_towns_room() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    // The game answers with the prompt alone.
    g.expect("sysop goto newhaven", PROMPT.to_owned());
    g.send(ToClient::SysGoto {
        session: ID.into(),
        town: "newhaven".into(),
    })
    .await;
    let landed = g
        .until(|m| match m {
            ToGateway::Notice { text, .. } if text.starts_with("Sysop goto: in") => {
                Some(text.clone())
            }
            _ => None,
        })
        .await;
    assert_eq!(landed, "Sysop goto: in Newhaven, Arena");
    g.idle(Duration::from_secs(5)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["sysop goto newhaven"]);
    let pos = as_ui_sees(&seen).position.unwrap();
    assert_eq!(
        (pos.map, pos.room),
        (Some(NEWHAVEN_ARENA.0), Some(NEWHAVEN_ARENA.1))
    );
}

#[tokio::test(start_paused = true)]
async fn an_unknown_town_is_refused() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.send(ToClient::SysGoto {
        session: ID.into(),
        town: "atlantis".into(),
    })
    .await;
    let said = g
        .until(|m| match m {
            ToGateway::Warning { text, .. } => Some(text.clone()),
            _ => None,
        })
        .await;
    assert_eq!(said, "sysop goto: no town \"atlantis\"");
    let (_, played) = g.close(run).await;
    assert!(played.is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_sysop_goto_waits_for_the_fight_and_the_bot_keeps_running() {
    use game::arena::*;
    use wayfarer_proto::BotState;
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.send(ToClient::SysGoto {
        session: ID.into(),
        town: "silvermere".into(),
    })
    .await;
    g.until(|m| match m {
        ToGateway::Notice { text, .. } if text == "Sysop goto Silvermere: after the fight" => {
            Some(())
        }
        _ => None,
    })
    .await;
    // Engaged: nothing goes (the script has nothing for it).
    g.round("The large giant rat lunges at you!", 33).await;
    g.expect("sysop goto silvermere", prompt(33));
    g.output(&kill(33)).await;
    g.until(|m| match m {
        ToGateway::Notice { text, .. } if text.starts_with("Sysop goto: in") => Some(()),
        _ => None,
    })
    .await;
    g.idle(Duration::from_secs(5)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "sysop goto silvermere"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}
