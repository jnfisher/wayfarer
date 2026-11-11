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

/// C-093: a character hung up nearly dead, with a monster in its room, is connected to recover:
/// the brain asks the gateway to connect, and on entering the realm leaves the bot stopped
/// (a stopped bot never hangs up) and sends `sysop goto silvermere` at the first prompt,
/// before the re-sync and before anything attacks it.
#[tokio::test(start_paused = true)]
async fn connecting_to_recover_goes_to_the_town_with_the_bot_stopped() {
    use game::arena::*;
    use wayfarer_proto::{BotState, SessionAction};
    let clock = Arc::new(wayfarer_brain::FixedClock(
        std::time::UNIX_EPOCH + Duration::from_secs(T0),
    ));
    let brain = wayfarer_brain::Brain::new(world(), clock, None);
    let (mut g, run) =
        Game::start_brain(SessionState::Closed, arena(&["large giant rat"], 2), brain).await;
    g.send(ToClient::RecoverTo {
        session: ID.into(),
        town: "silvermere".into(),
    })
    .await;
    g.until(|m| match m {
        ToGateway::SessionControl {
            action: SessionAction::Connect,
            ..
        } => Some(()),
        _ => None,
    })
    .await;
    g.expect("sysop goto silvermere", prompt(2));
    g.set_state(SessionState::Realm).await;
    g.output(&prompt(2)).await;
    g.narrated("Entered the realm to recover: sysop goto Silvermere, the bot stays stopped")
        .await;
    g.until(|m| match m {
        ToGateway::Notice { text, .. } if text.starts_with("Sysop goto: in") => Some(()),
        _ => None,
    })
    .await;
    g.idle(Duration::from_secs(20)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(
        played.first().map(String::as_str),
        Some("sysop goto silvermere")
    );
    assert!(
        !seen.iter().any(|m| matches!(
            m,
            ToGateway::SessionControl {
                action: SessionAction::HangUp,
                ..
            }
        )),
        "hung up"
    );
    assert_ne!(bot(&seen).state, BotState::Running);
}
