//! The brain against a fake gateway over a `ChannelLink`: fixtures replayed as live output and
//! as history, the re-sync's timing on a paused clock, restarts and refusal.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use wayfarer_brain::{Brain, BrainError, ChannelLink, FixedClock, World, apply_patch};
use wayfarer_parse::{GameTexts, Parser, Registry};
use wayfarer_proto::{CharacterState, Chunk, SessionInfo, SessionState, ToClient, ToGateway};

const ID: &str = "kasimir";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn registry() -> Arc<Registry> {
    static REG: OnceLock<Arc<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        let texts = GameTexts::from_export(fixtures().join("world")).expect("world export loads");
        Arc::new(Registry::new(&texts))
    })
    .clone()
}

/// What the fake gateway sends: output chunks, and the lines the player typed between them.
#[derive(Clone, Debug)]
enum Step {
    Out(String),
    Sent(String),
}

/// A capture's `raw.bin`, cut into chunks of about `size` characters as socket reads would.
fn capture(name: &str, size: usize) -> Vec<Step> {
    let raw = std::fs::read(fixtures().join("captures").join(name).join("raw.bin")).unwrap();
    let text = wayfarer_world::cp437::decode(&raw);
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(size)
        .map(|c| Step::Out(c.iter().collect()))
        .collect()
}

/// A transcript: a chunk per line, the `>>> 'cmd'` markers as typed lines.
fn transcript(name: &str) -> Vec<Step> {
    let text = std::fs::read_to_string(fixtures().join("transcripts").join(format!("{name}.txt")))
        .unwrap();
    let mut steps = Vec::new();
    for line in text.split('\n') {
        if line.starts_with("### ") {
            continue;
        }
        if let Some(rest) = line.strip_prefix(">>> ") {
            if let Some(cmd) = rest.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
                steps.push(Step::Sent(cmd.replace("\\'", "'").replace("\\\\", "\\")));
            }
            continue;
        }
        steps.push(Step::Out(format!("{line}\r\n")));
    }
    steps
}

fn info(state: SessionState, last_seq: u64) -> SessionInfo {
    SessionInfo {
        id: ID.into(),
        name: "Kasimir".into(),
        server: "bbs.example.org:23".into(),
        state,
        last_seq,
    }
}

/// The gateway's end of the link: answers history requests from its log, and reports each
/// line the brain sends as `sent`, as the gateway does.
struct Fake {
    tx: Option<mpsc::Sender<ToClient>>,
    rx: mpsc::Receiver<ToGateway>,
    log: Vec<Chunk>,
    /// Everything the brain sent, in order.
    seen: Vec<ToGateway>,
}

type Run = JoinHandle<(Brain, Result<(), BrainError>)>;

fn start(brain: Option<Brain>) -> (Fake, Run) {
    let (to_gateway, rx) = mpsc::channel(4096);
    let (tx, from_gateway) = mpsc::channel(4096);
    let mut brain = brain.unwrap_or_else(new_brain);
    let run = tokio::spawn(async move {
        let mut link = ChannelLink::new(to_gateway, from_gateway);
        let result = brain.run(&mut link).await;
        (brain, result)
    });
    let fake = Fake {
        tx: Some(tx),
        rx,
        log: Vec::new(),
        seen: Vec::new(),
    };
    (fake, run)
}

fn new_brain() -> Brain {
    let clock = FixedClock(UNIX_EPOCH + Duration::from_secs(1_792_411_200));
    Brain::new(World::from_registry(registry()), Arc::new(clock), None)
}

impl Fake {
    /// Send to the brain, unless the fake has hung up.
    async fn send(&self, msg: ToClient) {
        if let Some(tx) = &self.tx {
            tx.send(msg).await.unwrap();
        }
    }

    /// Append to the session's log and send it as live output.
    async fn output(&mut self, text: &str) {
        let seq = self.log.len() as u64 + 1;
        self.log.push(Chunk {
            seq,
            text: text.into(),
        });
        self.send(ToClient::Output {
            session: ID.into(),
            seq,
            text: text.into(),
        })
        .await;
    }

    /// Append to the log without sending it (output from before the brain attached).
    fn history(&mut self, text: &str) {
        let seq = self.log.len() as u64 + 1;
        self.log.push(Chunk {
            seq,
            text: text.into(),
        });
    }

    async fn play(&mut self, steps: &[Step]) {
        for s in steps {
            match s {
                Step::Out(text) => self.output(text).await,
                Step::Sent(text) => {
                    self.send(ToClient::Sent {
                        session: ID.into(),
                        seq: self.log.len() as u64,
                        text: text.clone(),
                    })
                    .await
                }
            }
        }
    }

    /// The brain's next message, after answering it the way the gateway would. `None` when
    /// the brain has gone.
    async fn next(&mut self) -> Option<ToGateway> {
        let msg = self.rx.recv().await?;
        match &msg {
            ToGateway::HistoryRequest {
                after_seq,
                max_chunks,
                ..
            } => {
                let max = *max_chunks as usize;
                let chunks: Vec<Chunk> = match after_seq {
                    Some(after) => self
                        .log
                        .iter()
                        .filter(|c| c.seq > *after)
                        .take(max)
                        .cloned()
                        .collect(),
                    None => self.log[self.log.len().saturating_sub(max)..].to_vec(),
                };
                let from_seq = chunks.first().map_or(self.log.len() as u64 + 1, |c| c.seq);
                self.send(ToClient::History {
                    session: ID.into(),
                    from_seq,
                    chunks,
                })
                .await;
            }
            ToGateway::InputLine { text, .. } => {
                self.send(ToClient::Sent {
                    session: ID.into(),
                    seq: self.log.len() as u64,
                    text: text.clone(),
                })
                .await;
            }
            _ => {}
        }
        self.seen.push(msg.clone());
        Some(msg)
    }

    /// Read until `want` matches a message, which it returns.
    async fn until<T>(&mut self, mut want: impl FnMut(&ToGateway) -> Option<T>) -> T {
        loop {
            let msg = self.next().await.expect("the brain went away");
            if let Some(t) = want(&msg) {
                return t;
            }
        }
    }

    /// The brain's next line to the game.
    async fn input(&mut self) -> String {
        self.until(|m| match m {
            ToGateway::InputLine { text, .. } => Some(text.clone()),
            _ => None,
        })
        .await
    }

    /// The next whole snapshot.
    async fn character(&mut self) -> CharacterState {
        self.until(|m| match m {
            ToGateway::Character { state, .. } => Some((**state).clone()),
            _ => None,
        })
        .await
    }

    /// Say hello back.
    async fn welcome(&mut self, state: SessionState) {
        let hello = self.next().await;
        assert!(matches!(hello, Some(ToGateway::Hello { .. })), "{hello:?}");
        self.send(ToClient::Welcome {
            protocol: wayfarer_proto::PROTOCOL,
            server_build: "test".into(),
            sessions: vec![info(state, self.log.len() as u64)],
        })
        .await;
    }

    /// Hang up, and read what the brain had still to say.
    async fn close(mut self, run: Run) -> (Brain, Vec<ToGateway>) {
        self.tx.take();
        while self.next().await.is_some() {}
        let (brain, result) = run.await.unwrap();
        result.unwrap();
        (brain, self.seen)
    }
}

/// The snapshot UIs end up with: the first whole one with every later patch applied.
fn as_ui_sees(seen: &[ToGateway]) -> Value {
    let mut ui = Value::Null;
    for msg in seen {
        match msg {
            ToGateway::Character { state, .. } => ui = serde_json::to_value(state).unwrap(),
            ToGateway::CharacterPatch { patch, .. } => apply_patch(&mut ui, patch),
            _ => {}
        }
    }
    ui
}

/// Replay `steps` as live output to a brain in `state`; the brain's final snapshot and
/// everything it sent.
async fn replay(steps: &[Step]) -> (CharacterState, Vec<ToGateway>) {
    let (mut fake, run) = start(None);
    fake.welcome(SessionState::Menu).await;
    fake.character().await;
    fake.play(steps).await;
    let (brain, seen) = fake.close(run).await;
    let snapshot = brain.snapshot(ID).unwrap();

    // The patches applied to the first snapshot give the last one.
    assert_eq!(as_ui_sees(&seen), serde_json::to_value(&snapshot).unwrap());
    // The events sent are the parser's.
    let mut parser = Parser::new(registry());
    let mut want = Vec::new();
    let mut seq = 0;
    for s in steps {
        match s {
            Step::Out(text) => {
                seq += 1;
                want.extend(parser.feed_at(seq, text));
            }
            Step::Sent(text) => parser.sent(text),
        }
    }
    let got: Vec<(u64, u64, String, String)> = seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Event {
                seq,
                line_no,
                name,
                line,
                ..
            } => Some((*seq, *line_no, name.clone(), line.clone())),
            _ => None,
        })
        .collect();
    let want: Vec<(u64, u64, String, String)> = want
        .into_iter()
        .map(|e| (e.seq, e.line_no, e.name, e.line))
        .collect();
    assert_eq!(got, want);
    // Nothing went to the game outside the realm.
    assert!(
        !seen
            .iter()
            .any(|m| matches!(m, ToGateway::InputLine { .. })),
        "sent to the game in the menu"
    );
    (snapshot, seen)
}

fn stats(s: &CharacterState) -> [Option<u32>; 6] {
    let st = s.stats.clone().unwrap_or_default();
    [
        st.strength,
        st.intellect,
        st.willpower,
        st.agility,
        st.health,
        st.charm,
    ]
}

fn exits(list: &[&str]) -> Option<Vec<String>> {
    Some(list.iter().map(|s| s.to_string()).collect())
}

#[tokio::test]
async fn status_screens() {
    let (s, _) = replay(&capture("status-screens", 400)).await;
    assert_eq!(s.name.as_deref(), Some("Aldric Stonebrook"));
    assert_eq!(s.race.as_deref(), Some("Human"));
    assert_eq!(s.class.as_deref(), Some("Warrior"));
    assert_eq!(s.level, Some(1));
    assert_eq!((s.hp, s.hp_max), (Some(35), Some(35)));
    assert_eq!((s.mana, s.mana_max), (None, None));
    assert_eq!(stats(&s), [50, 40, 40, 40, 50, 40].map(Some));
    assert_eq!((s.lives, s.cp), (Some(9), Some(80)));
    assert_eq!(
        (s.exp, s.exp_next, s.exp_percent),
        (Some(0), Some(1000), Some(0))
    );
    assert_eq!(s.room_title.as_deref(), Some("Newhaven, Village Entrance"));
    assert_eq!(
        s.room_exits,
        exits(&["north", "south", "west", "southeast"])
    );
    assert_eq!(s.prompt_form.as_deref(), Some("hp"));
    assert_eq!(s.statuses, Some(vec![]));
    let parse = s.parse.unwrap();
    assert!(
        parse.lines > 50 && parse.unclassified < parse.lines,
        "{parse:?}"
    );
}

#[tokio::test]
async fn fight_arena() {
    let (s, seen) = replay(&capture("fight-arena", 400)).await;
    assert_eq!(s.name.as_deref(), Some("Aldric Stonebrook"));
    assert_eq!((s.hp, s.hp_max), (Some(35), Some(35)));
    // `You gain 9 experience.`, then the `exp` screen says the same.
    assert_eq!((s.exp, s.exp_next), (Some(9), Some(991)));
    assert_eq!(s.in_combat, Some(false));
    assert_eq!(s.resting, Some(false));
    assert_eq!(s.room_title.as_deref(), Some("Newhaven, Narrow Road"));
    assert_eq!(s.room_exits, exits(&["north", "east", "west", "down"]));
    // The fight showed on the way.
    let patched = |field: &str, want: Value| {
        seen.iter().any(|m| {
            matches!(m, ToGateway::CharacterPatch { patch, .. } if patch.get(field) == Some(&want))
        })
    };
    assert!(patched("in_combat", Value::Bool(true)));
    assert!(patched("resting", Value::Bool(true)));
}

#[tokio::test]
async fn room_walk() {
    let (s, seen) = replay(&capture("room-walk", 400)).await;
    assert_eq!(s.hp, Some(35));
    assert_eq!(s.room_title.as_deref(), Some("Newhaven, Village Entrance"));
    assert_eq!(
        s.room_exits,
        exits(&["north", "south", "west", "southeast"])
    );
    assert_eq!(s.in_combat, None);
    // The dark sewer has no title or exits to show.
    assert!(seen.iter().any(|m| matches!(
        m,
        ToGateway::CharacterPatch { patch, .. }
            if patch.get("room_title") == Some(&Value::Null)
    )));
}

#[tokio::test]
async fn death_rebirth() {
    let (s, seen) = replay(&capture("death-rebirth", 400)).await;
    assert_eq!(s.name.as_deref(), Some("Corvin Ashdown"));
    assert_eq!(
        (s.race.as_deref(), s.class.as_deref()),
        (Some("Elf"), Some("Mage"))
    );
    assert_eq!(s.level, Some(1));
    assert_eq!(
        (s.hp, s.hp_max, s.mana, s.mana_max),
        (Some(20), Some(20), Some(22), Some(22))
    );
    assert_eq!(stats(&s), [35, 60, 50, 50, 30, 50].map(Some));
    assert_eq!(s.lives, Some(8));
    assert_eq!(s.in_combat, Some(false));
    assert_eq!(s.room_title.as_deref(), Some("Newhaven, Healer"));
    assert_eq!(s.room_exits, exits(&["east"]));
    assert_eq!(s.prompt_form.as_deref(), Some("hp_mana"));
    // Dying went below zero.
    assert!(seen.iter().any(|m| matches!(
        m,
        ToGateway::CharacterPatch { patch, .. }
            if patch.get("hp").and_then(Value::as_i64).is_some_and(|hp| hp < 0)
    )));
}

#[tokio::test]
async fn thornwyn_death_transcript() {
    let (s, _) = replay(&transcript("thornwyn-death")).await;
    assert_eq!((s.hp, s.mana), (Some(32), Some(22)));
    assert_eq!(s.in_combat, Some(false));
    // The transcript stops before the healer's exits.
    assert_eq!(s.room_title.as_deref(), Some("Newhaven, Healer"));
    assert_eq!(s.room_exits, None);
    assert_eq!(s.parse.unwrap().unclassified, 0);
}

const ROOM: &str =
    "\r\nNewhaven, Village Entrance\r\nObvious exits: north, south, west, southeast\r\n";
const ST: &str = "st\r\n\r\nName: Aldric Stonebrook                Lives/CP:      9/80   \r\n\
Race: Human       Exp: 0               Perception:     40\r\n\
Class: Warrior    Level: 1             Stealth:         0\r\n\
Hits:    35/35    Armour Class:  11/0  Thievery:        0\r\n\
Strength:  50     Agility: 40          Tracking:        0\r\n\
Intellect: 40     Health:  50          Martial Arts:   13\r\n\
Willpower: 40     Charm:   40          MagicRes:       40\r\n\r\n[HP=35]:";
const EXP: &str =
    "exp\r\nExp: 0 Level: 1 Exp needed for next level: 1000 (1000) [0%]\r\n\r\n[HP=35]:";
const INV: &str = "i\r\n\r\nYou are carrying padded vest (Torso)\r\n\r\nYou have no keys.\r\n\
Wealth: 0 copper farthings\r\nEncumbrance: 500/2400 - Light [20%]\r\n\r\n[HP=35]:";

fn look() -> String {
    format!("look{ROOM}\r\n[HP=35]:")
}

/// A brain attached to a session in `state` whose history ends at a prompt; returns the fake
/// once the brain has published its first snapshot, with that snapshot.
async fn attached(state: SessionState) -> (Fake, Run, CharacterState) {
    let (mut fake, run) = start(None);
    fake.history(&format!("{ROOM}\r\n[HP=30]:"));
    fake.welcome(state).await;
    let first = fake.character().await;
    (fake, run, first)
}

fn inputs(seen: &[ToGateway]) -> Vec<String> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::InputLine { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn attach_catches_up_before_sending_anything() {
    let (mut fake, run) = start(None);
    fake.history("\r\nYou feel better.\r\n");
    fake.history("[HP=31/MA=5]:");
    fake.welcome(SessionState::Realm).await;
    let first = fake.character().await;
    assert_eq!((first.hp, first.mana), (Some(31), Some(5)));
    assert_eq!(first.updated_seq, Some(2));
    // History is fed silently, and nothing went to the game before the snapshot.
    assert!(matches!(
        fake.seen[..2],
        [ToGateway::Hello { .. }, ToGateway::BrainInfo { .. }]
    ));
    assert!(
        !fake
            .seen
            .iter()
            .any(|m| matches!(m, ToGateway::Event { .. } | ToGateway::InputLine { .. })),
        "{:?}",
        fake.seen
    );
    fake.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn resync_sends_its_commands_in_order_at_an_idle_prompt() {
    let t0 = Instant::now();
    let (mut fake, run, first) = attached(SessionState::Realm).await;
    assert_eq!(first.hp, Some(30));
    assert_eq!(first.synced_at, None);

    assert_eq!(fake.input().await, "st");
    assert_eq!(t0.elapsed(), wayfarer_brain::IDLE);
    for (answer, next) in [(ST, "exp"), (EXP, "i"), (INV, "look")] {
        fake.output(answer).await;
        assert_eq!(fake.input().await, next);
        // At the next prompt, not after another idle wait.
        assert!(t0.elapsed() < wayfarer_brain::IDLE * 2);
    }
    fake.output(&look()).await;
    let synced = fake.character().await;
    assert_eq!(synced.synced_at.as_deref(), Some("2026-10-19T12:00:00Z"));
    assert_eq!(synced.name.as_deref(), Some("Aldric Stonebrook"));
    assert_eq!(synced.exp_next, Some(1000));
    assert_eq!(synced.hp, Some(35));
    let notice = fake
        .until(|m| match m {
            ToGateway::Notice { session, text } => Some((session.clone(), text.clone())),
            _ => None,
        })
        .await;
    assert_eq!(notice, (Some(ID.into()), "re-synced".into()));

    // A UI asks again.
    let t1 = Instant::now();
    fake.send(ToClient::Resync { session: ID.into() }).await;
    assert_eq!(fake.input().await, "st");
    assert!(t1.elapsed() <= wayfarer_brain::IDLE);
    let (_, seen) = fake.close(run).await;
    assert_eq!(inputs(&seen), ["st", "exp", "i", "look", "st"]);
}

#[tokio::test(start_paused = true)]
async fn the_player_typing_defers_the_resync() {
    let t0 = Instant::now();
    let (mut fake, run, _) = attached(SessionState::Realm).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    fake.play(&[Step::Sent("look".into())]).await;
    fake.output(&look()).await;
    assert_eq!(fake.input().await, "st");
    assert_eq!(t0.elapsed(), Duration::from_millis(3500));

    // Typing during the re-sync: the rest waits for the next idle prompt.
    tokio::time::sleep(Duration::from_millis(500)).await;
    fake.play(&[Step::Sent("gos hello".into())]).await;
    let typed = Instant::now();
    fake.output(ST).await;
    assert_eq!(fake.input().await, "exp");
    assert_eq!(typed.elapsed(), wayfarer_brain::IDLE);
    fake.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn an_unanswered_command_gives_way_at_the_next_prompt() {
    let (mut fake, run, _) = attached(SessionState::Realm).await;
    assert_eq!(fake.input().await, "st");
    let sent = Instant::now();
    let wait = wayfarer_brain::ANSWER_TIMEOUT + Duration::from_secs(1);
    tokio::time::sleep(wait).await;
    fake.output("\r\n[HP=30]:").await;
    assert_eq!(fake.input().await, "exp");
    assert_eq!(sent.elapsed(), wait);
    fake.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn nothing_is_sent_outside_the_realm() {
    let (mut fake, run, _) = attached(SessionState::Menu).await;
    fake.send(ToClient::Resync { session: ID.into() }).await;
    tokio::time::sleep(Duration::from_secs(30)).await;
    fake.send(ToClient::SessionState {
        session: ID.into(),
        state: SessionState::Logon,
    })
    .await;
    tokio::time::sleep(Duration::from_secs(30)).await;
    // Entering the realm starts the re-sync.
    fake.send(ToClient::SessionState {
        session: ID.into(),
        state: SessionState::Realm,
    })
    .await;
    // Nothing has been sent for a minute: the prompt is idle already.
    let entered = Instant::now();
    assert_eq!(fake.input().await, "st");
    assert_eq!(entered.elapsed(), Duration::ZERO);
    let (_, seen) = fake.close(run).await;
    assert_eq!(inputs(&seen), ["st"]);
}

#[tokio::test(start_paused = true)]
async fn a_restart_gives_the_same_snapshot() {
    let steps = capture("status-screens", 400);
    let mut snapshots = Vec::new();
    for _ in 0..2 {
        let (mut fake, run) = start(None);
        for s in &steps {
            if let Step::Out(text) = s {
                fake.history(text);
            }
        }
        fake.welcome(SessionState::Menu).await;
        snapshots.push(fake.character().await);
        fake.close(run).await;
    }
    assert_eq!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[0].name.as_deref(), Some("Aldric Stonebrook"));
}

#[tokio::test(start_paused = true)]
async fn a_reattach_catches_up_from_the_last_seq_without_a_resync() {
    let (mut fake, run, _) = attached(SessionState::Realm).await;
    for (cmd, answer) in [("st", ST), ("exp", EXP), ("i", INV)] {
        assert_eq!(fake.input().await, cmd);
        fake.output(answer).await;
    }
    assert_eq!(fake.input().await, "look");
    fake.output(&look()).await;
    let synced = fake.character().await;
    let log = fake.log.clone();
    let (brain, _) = fake.close(run).await;

    // The gateway comes back with output the brain has not seen.
    let (mut fake, run) = start(Some(brain));
    fake.log = log;
    fake.history("\r\nYou feel better.\r\n");
    fake.history("[HP=33]:");
    fake.welcome(SessionState::Realm).await;
    let request = fake
        .until(|m| match m {
            ToGateway::HistoryRequest { after_seq, .. } => Some(*after_seq),
            _ => None,
        })
        .await;
    assert_eq!(request, Some(5));
    let again = fake.character().await;
    assert_eq!(again.hp, Some(33));
    assert_eq!(again.synced_at, synced.synced_at);
    tokio::time::sleep(Duration::from_secs(10)).await;
    let (_, seen) = fake.close(run).await;
    assert!(inputs(&seen).is_empty(), "{:?}", inputs(&seen));
    // The output since is new to the UIs: its events go out.
    assert!(
        seen.iter()
            .any(|m| matches!(m, ToGateway::Event { seq: 7, .. }))
    );
}

#[tokio::test(start_paused = true)]
async fn lagging_fetches_the_gap() {
    let (mut fake, run, _) = attached(SessionState::Menu).await;
    fake.history("\r\n[HP=29]:");
    fake.send(ToClient::Lagged {
        session: ID.into(),
        missed: 1,
    })
    .await;
    let hp = |m: &ToGateway| match m {
        ToGateway::CharacterPatch { patch, .. } => patch.get("hp").cloned(),
        _ => None,
    };
    assert_eq!(fake.until(hp).await, 29);
    // Live output after a gap is held until the gap is fetched.
    fake.history("\r\n[HP=28]:");
    fake.output("\r\n[HP=27]:").await;
    assert_eq!(fake.until(hp).await, 28);
    assert_eq!(fake.until(hp).await, 27);
    let (brain, seen) = fake.close(run).await;
    assert_eq!(brain.snapshot(ID).unwrap().hp, Some(27));
    let hps: Vec<Value> = seen.iter().filter_map(hp).collect();
    assert_eq!(hps, [29, 28, 27]);
}

#[tokio::test(start_paused = true)]
async fn state_request_sends_the_snapshot_and_brain_info() {
    let (mut fake, run, first) = attached(SessionState::Menu).await;
    fake.send(ToClient::StateRequest { session: None }).await;
    assert_eq!(fake.character().await, first);
    let info = fake
        .until(|m| match m {
            ToGateway::BrainInfo { build, .. } => Some(build.clone()),
            _ => None,
        })
        .await;
    assert_eq!(info, wayfarer_brain::BUILD);
    fake.close(run).await;
}

#[tokio::test]
async fn a_refused_brain_stops_with_the_reason() {
    let (mut fake, run) = start(None);
    assert!(matches!(fake.next().await, Some(ToGateway::Hello { .. })));
    fake.send(ToClient::Refused {
        reason: "a brain is already attached".into(),
    })
    .await;
    let (_, result) = run.await.unwrap();
    match result {
        Err(BrainError::Refused(reason)) => assert_eq!(reason, "a brain is already attached"),
        other => panic!("{other:?}"),
    }
}
