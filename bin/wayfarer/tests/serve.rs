//! `wayfarer serve`'s wiring: a hub, a brain attached to it in-process, a UI over channels and
//! an in-test game server. No socket but the game's own, on 127.0.0.1:0.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use wayfarer::spawn_brain;
use wayfarer_brain::World;
use wayfarer_gateway::{Config, Credentials, Gateway, Hub, Options};
use wayfarer_parse::{GameTexts, Registry};
use wayfarer_proto::{ClientKind, PROTOCOL, ToClient, ToGateway};

/// Far longer than any test should take; only a broken wiring waits this long.
const PATIENCE: Duration = Duration::from_secs(5);

async fn recv(rx: &mut mpsc::Receiver<ToClient>) -> ToClient {
    tokio::time::timeout(PATIENCE, rx.recv())
        .await
        .expect("a message arrives")
        .expect("the connection is open")
}

#[tokio::test]
async fn the_in_process_brain_attaches_and_the_ui_gets_its_character() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let texts = GameTexts::from_export(fixtures.join("world")).expect("world export loads");
    let world = World::from_registry(Arc::new(Registry::new(&texts)));

    let game = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = Config::parse(&format!(
        "[[server]]\nid = \"local\"\nhost = \"127.0.0.1\"\nport = {}\n\n\
         [[character]]\nid = \"kasimir\"\nname = \"Kasimir\"\nserver = \"local\"\n",
        game.local_addr().unwrap().port()
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gateway = Gateway::new(
        &config,
        &Credentials::parse("").unwrap(),
        &dir.path().join("logs"),
        Options::default(),
    )
    .unwrap();
    let hub = Hub::new(gateway.clone(), None);
    let brain = spawn_brain(&hub, world, None, None);

    let (to_hub, mut from_hub) = hub.connect();
    to_hub
        .send(ToGateway::Hello {
            protocol: PROTOCOL,
            client: ClientKind::Ui,
            token: None,
        })
        .await
        .unwrap();
    assert!(matches!(
        recv(&mut from_hub).await,
        ToClient::Welcome { .. }
    ));

    // The brain may attach before or after the UI's welcome.
    let mut attached = false;
    while !attached {
        attached = matches!(
            recv(&mut from_hub).await,
            ToClient::BrainState { attached: true }
        );
    }

    gateway.open("kasimir").unwrap();
    let (mut game, _) = tokio::time::timeout(PATIENCE, game.accept())
        .await
        .expect("the gateway connects")
        .unwrap();
    game.write_all(
        b"\r\nNewhaven, Village Entrance\r\nObvious exits: north, south\r\n\r\n[HP=30]:",
    )
    .await
    .unwrap();

    loop {
        match recv(&mut from_hub).await {
            ToClient::Character { session, state } if session == "kasimir" => {
                if state.hp == Some(30) {
                    break;
                }
            }
            ToClient::CharacterPatch { patch, .. }
                if patch.get("hp").and_then(|v| v.as_i64()) == Some(30) =>
            {
                break;
            }
            _ => {}
        }
    }

    // Stopping the hub ends the brain's connection, and the brain with it.
    hub.stop();
    tokio::time::timeout(PATIENCE, brain)
        .await
        .expect("the brain stops with the hub")
        .unwrap()
        .unwrap();
}
