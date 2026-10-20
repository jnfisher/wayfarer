//! Replay (`src/replay.rs`): a recording played into a gateway on tokio's paused clock, so its
//! pace is exact and takes no real time.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::time::Instant;
use wayfarer_gateway::replay::{self, Pace, Recording, Speed};
use wayfarer_gateway::{Hub, Origin, Output, SessionState};
use wayfarer_proto::{BotAction, ClientKind, PROTOCOL, ToClient, ToGateway};

const ID: &str = "waychecka";
const T0: u64 = 1_792_411_200_000;

/// A recording: the realm prompt, a line typed at +2 s and one the brain sent at +2.2 s, the
/// echo at +3 s and another prompt at +3.5 s; the brain attached at +1 s and was told to start
/// the bot at +1.5 s.
fn record(dir: &Path) {
    let dir = dir.join(ID);
    std::fs::create_dir_all(&dir).unwrap();
    let chunk = |seq: u64, ms: u64, text: &str| {
        serde_json::json!({"seq": seq, "ts": T0 + ms, "text": text}).to_string()
    };
    let output = [
        chunk(41, 0, "\r\n[HP=31/MA=6]:"),
        chunk(42, 3_000, "st\r\nName: Waychecka\r\n"),
        chunk(43, 3_500, "[HP=31/MA=6]:"),
    ];
    std::fs::write(dir.join("output.log"), output.join("\n") + "\n").unwrap();
    let sent = [
        serde_json::json!({"seq": 41, "ts": T0 + 2_000, "origin": "ui", "text": "st"}),
        serde_json::json!({"seq": 41, "ts": T0 + 2_200, "origin": "brain", "text": "exp"}),
    ];
    let sent: Vec<String> = sent.iter().map(|v| v.to_string()).collect();
    std::fs::write(dir.join("sent.log"), sent.join("\n") + "\n").unwrap();
    let start = ToClient::BotControl {
        session: ID.to_owned(),
        action: BotAction::Start,
    };
    let brain = [
        serde_json::json!({"seq": 41, "ts": T0 + 1_000, "attached": true}),
        serde_json::json!({"seq": 41, "ts": T0 + 1_500, "msg": start}),
    ];
    let brain: Vec<String> = brain.iter().map(|v| v.to_string()).collect();
    std::fs::write(dir.join("brain.log"), brain.join("\n") + "\n").unwrap();
}

async fn next(rx: &mut broadcast::Receiver<Output>) -> Output {
    tokio::time::timeout(Duration::from_secs(3600), rx.recv())
        .await
        .expect("the replay goes on")
        .expect("the session publishes")
}

#[tokio::test(start_paused = true)]
async fn a_recording_plays_on_its_pace_with_its_sent_lines_between_its_chunks() {
    let src = tempfile::tempdir().unwrap();
    record(src.path());
    let recording = Arc::new(Recording::load(&src.path().join(ID)).unwrap());
    assert_eq!(recording.sent_by(Origin::Ui), ["st"]);
    assert_eq!(recording.sent_by(Origin::Brain), ["exp"]);
    let out = tempfile::tempdir().unwrap();
    let pace = Arc::new(Pace::new(recording.origin_ms(), Speed::Times(2.0)));
    let start = pace.start();
    let (gateway, connector) =
        replay::gateway(recording.clone(), pace.clone(), out.path(), None).unwrap();
    let mut rx = gateway.subscribe(ID).unwrap();
    let mut finished = connector.finished();

    // Twice as fast: each recorded gap takes half as long.
    let at = |rx: Output| (rx, start.elapsed());
    let (first, t) = at(next(&mut rx).await);
    assert!(matches!(first, Output::Chunk(ref c) if c.seq == 1 && c.text.ends_with("]:")));
    assert_eq!(t, Duration::ZERO);
    // The realm prompt: the tracker follows the text, as live.
    tokio::task::yield_now().await;
    assert_eq!(gateway.state(ID).unwrap(), SessionState::Realm);
    let (sent, t) = at(next(&mut rx).await);
    assert!(matches!(sent, Output::Sent(ref s) if s.text == "st" && s.seq == 1));
    assert_eq!(t, Duration::from_secs(1));
    // The brain's recorded line is not published: a replay's brain hears its own.
    let (echo, t) = at(next(&mut rx).await);
    assert!(matches!(echo, Output::Chunk(ref c) if c.text.starts_with("st\r\n")));
    assert_eq!(t, Duration::from_millis(1_500));

    // A line typed into the replay is recorded as sent but goes nowhere, with no `sent`; the
    // brain's lines are its `sent`.
    gateway.send_line(ID, "i").await.unwrap();
    gateway
        .send_line_as(ID, "look", Origin::Brain)
        .await
        .unwrap();
    let (own, _) = at(next(&mut rx).await);
    assert!(matches!(own, Output::Sent(ref s) if s.text == "look"));
    let (last, t) = at(next(&mut rx).await);
    assert!(matches!(last, Output::Chunk(ref c) if c.seq == 3));
    assert_eq!(t, Duration::from_millis(1_750));
    finished.wait_for(|f| *f).await.unwrap();
    let mine = Recording::load(&out.path().join(ID)).unwrap();
    assert_eq!(mine.sent_by(Origin::Brain), ["look"]);
    assert_eq!(mine.sent_by(Origin::Ui), ["i"]);
    assert_eq!(
        mine.chunks.len(),
        3,
        "the replay's own log has the same chunks"
    );

    // The recording stays open; stopping it logs nothing off.
    assert_eq!(gateway.state(ID).unwrap(), SessionState::Realm);
    let stopped = Instant::now();
    gateway.shutdown().await;
    assert_eq!(stopped.elapsed(), Duration::ZERO);
    assert_eq!(gateway.state(ID).unwrap(), SessionState::Closed);
    let mine = Recording::load(&out.path().join(ID)).unwrap();
    assert_eq!(mine.sent.len(), 2, "no log-off steps");
}

#[tokio::test(start_paused = true)]
async fn instant_feeds_as_fast_as_the_session_reads() {
    let src = tempfile::tempdir().unwrap();
    record(src.path());
    let recording = Arc::new(Recording::load(&src.path().join(ID)).unwrap());
    let out = tempfile::tempdir().unwrap();
    let pace = Arc::new(Pace::new(recording.origin_ms(), Speed::Instant));
    let elapsed = pace.elapsed();
    let (gateway, connector) = replay::gateway(recording, pace, out.path(), None).unwrap();
    let t = Instant::now();
    connector.finished().wait_for(|f| *f).await.unwrap();
    assert_eq!(t.elapsed(), Duration::ZERO);
    assert_eq!(
        *elapsed.borrow(),
        3_500,
        "the recorded time reached the last chunk"
    );
    gateway.shutdown_now().await;
}

#[tokio::test(start_paused = true)]
async fn the_brain_attaches_and_hears_the_recorded_messages_when_it_did() {
    let src = tempfile::tempdir().unwrap();
    record(src.path());
    let recording = Arc::new(Recording::load(&src.path().join(ID)).unwrap());
    let out = tempfile::tempdir().unwrap();
    let pace = Arc::new(Pace::new(recording.origin_ms(), Speed::Times(1.0)));
    let (gateway, _connector) =
        replay::gateway(recording.clone(), pace.clone(), out.path(), None).unwrap();
    let hub = Hub::new(gateway.clone(), None);
    let start = pace.start();

    let (brain_tx, mut brain_rx) = tokio::sync::mpsc::channel(256);
    let link = std::sync::Mutex::new(None);
    replay::drive_brain(&hub, &recording, &pace, |on| {
        assert!(on, "the recording's brain never went");
        assert_eq!(start.elapsed(), Duration::from_secs(1));
        let (tx, mut rx) = hub.connect();
        let brain_tx = brain_tx.clone();
        *link.lock().unwrap() = Some(tx.clone());
        let hub = hub.clone();
        async move {
            tx.send(ToGateway::Hello {
                protocol: PROTOCOL,
                client: ClientKind::Brain,
                token: None,
            })
            .await
            .unwrap();
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    let _ = brain_tx.send((msg, Instant::now())).await;
                }
            });
            hub.wait_for_brain(true).await;
        }
    })
    .await;
    // Let the forwarder catch up.
    tokio::time::sleep(Duration::from_millis(1)).await;
    let mut control = None;
    while let Ok((msg, at)) = brain_rx.try_recv() {
        if let ToClient::BotControl { action, .. } = msg {
            control = Some((action, at - start));
        }
    }
    assert_eq!(
        control,
        Some((BotAction::Start, Duration::from_millis(1_500)))
    );
    // The replay keeps its own brain.log.
    let mine = Recording::load(&out.path().join(ID)).unwrap();
    assert_eq!(mine.brain.len(), 2);
    assert_eq!(mine.brain[0].attached, Some(true));
    assert!(matches!(
        mine.brain[1].msg,
        Some(ToClient::BotControl { .. })
    ));
    drop(link);
    gateway.shutdown_now().await;
}
