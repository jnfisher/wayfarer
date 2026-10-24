//! The fake gateway, and behind it the game, that the walking and fighting tests run against
//! (`walk.rs`, `fight.rs`): its session answers each command the brain sends like the game,
//! from a script: rooms cut from the `room-walk` capture where it has them, rooms made from the
//! world pack where it doesn't, and the game's texts. On a paused clock.
//!
//! The world is the pack built from `tests/fixtures/world` (shared with wayfarer-world's tests,
//! `common::pack_path`); each test process loads it once (about half a second: the game
//! texts' patterns and the room graph).

#![allow(dead_code)]

pub mod arena;
#[path = "../../../wayfarer-world/tests/common/mod.rs"]
pub mod common;

use std::collections::VecDeque;
pub use std::sync::Arc;
use std::sync::OnceLock;
pub use std::time::Duration;
use std::time::UNIX_EPOCH;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use wayfarer_brain::{Brain, BrainError, ChannelLink, FixedClock, RESYNC_COMMANDS, World};
pub use wayfarer_proto::{
    CharacterState, Chunk, RoomKey, SessionInfo, SessionState, ToClient, ToGateway,
};
pub use wayfarer_world::{Key, WorldPack};

/// The brain tests' settings: `text` (a character file) over "my defaults" of
/// `health.rest_quiet_room` on and `bot.start_on_logon` off. The scripts were written for
/// walking to a quiet room to rest, the default before C-053, and for a bot that starts only
/// when asked, before C-054; a test of either default turns it back.
pub fn test_settings(text: &str) -> wayfarer_settings::Settings {
    use wayfarer_settings::{Layer, Overrides, Settings};
    let quiet =
        Overrides::parse("[health]\nrest_quiet_room = true\n[bot]\nstart_on_logon = false\n")
            .unwrap();
    let over = Overrides::parse(text).unwrap();
    Settings::resolve(&[(Layer::Mine, &quiet), (Layer::Character, &over)]).unwrap()
}

pub const ID: &str = "aldric";
/// The wall clock's start in the tests: 2026-10-19T12:00:00Z.
pub const T0: u64 = 1_792_411_200;
/// A script answer that sends no output: the command's answer came with an earlier one.
pub const NOTHING: &str = "\0";
pub const PROMPT: &str = "\x1b[79D\x1b[K\x1b[0;37m[HP=35\x1b[0;37m]:";

pub const VILLAGE_ENTRANCE: Key = (1, 2140);
pub const WEAPONS_SHOP: Key = (1, 2141);
pub const NARROW_PATH: Key = (1, 2143);
pub const NARROW_ROAD: Key = (1, 2146);
pub const ARENA: Key = (1, 2150);
/// North of the Arena, through its door.
pub const BEYOND_ARENA: Key = (1, 2152);

pub fn world() -> World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD
        .get_or_init(|| {
            let pack = WorldPack::open(common::pack_path()).unwrap();
            World::from_pack(pack).unwrap()
        })
        .clone()
}

pub fn atlas() -> Arc<wayfarer_brain::Atlas> {
    world().atlas.unwrap()
}

pub fn capture() -> &'static str {
    static RAW: OnceLock<String> = OnceLock::new();
    RAW.get_or_init(|| {
        let raw = std::fs::read(common::repo("tests/fixtures/captures/room-walk/raw.bin")).unwrap();
        wayfarer_world::cp437::decode(&raw)
    })
}

/// The rooms taken from the capture (each shown there under a name no other room there has).
pub const CAPTURED: [Key; 5] = [
    VILLAGE_ENTRANCE,
    WEAPONS_SHOP,
    NARROW_PATH,
    NARROW_ROAD,
    ARENA,
];

/// A room as the game shows it after a move: from the capture for the Newhaven rooms it
/// walks through, else made from the pack (title, a line of description, the exits it lists).
pub fn room(k: Key) -> String {
    let a = atlas();
    let name = &a.graph.room(k).unwrap().name;
    let raw = capture();
    let title = format!("\x1b[1;36m{name}\r\n");
    if CAPTURED.contains(&k) {
        let at = raw.find(&title).unwrap();
        let rest = &raw[at..];
        let end = rest.find("]:").unwrap() + 2;
        return format!("\x1b[0;37;40m\x1b[79D\x1b[K{}", &rest[..end]);
    }
    let (always, _) = a.graph.listed_exits(k);
    const LONG: [&str; 10] = [
        "north",
        "south",
        "east",
        "west",
        "northeast",
        "northwest",
        "southeast",
        "southwest",
        "up",
        "down",
    ];
    let exits: Vec<&str> = always.iter().map(|d| LONG[usize::from(d)]).collect();
    format!(
        "\x1b[0;37;40m\x1b[79D\x1b[K{title}\x1b[79D\x1b[K\x1b[0;37;40m    Somewhere in the realm.\r\n\
         \x1b[0;32mObvious exits: {}\r\n{PROMPT}",
        exits.join(", ")
    )
}

/// A line of the game's, then the prompt.
pub fn says(line: &str) -> String {
    format!("\x1b[1;31;40m{line}\r\n{PROMPT}")
}

pub fn info(state: SessionState, last_seq: u64) -> SessionInfo {
    SessionInfo {
        id: ID.into(),
        name: "Aldric".into(),
        server: "bbs.example.org:23".into(),
        state,
        last_seq,
    }
}

pub type Run = JoinHandle<(Brain, Result<(), BrainError>)>;

/// The gateway, and behind it the game: each command the brain sends is checked against the
/// script and answered with the script's text.
pub struct Game {
    pub tx: Option<mpsc::Sender<ToClient>>,
    pub rx: mpsc::Receiver<ToGateway>,
    pub log: Vec<Chunk>,
    /// Everything the brain sent, in order.
    pub seen: Vec<ToGateway>,
    /// (command, answer) in the order the brain must send them.
    pub script: VecDeque<(String, String)>,
    /// What the gateway says right after the brain's next command goes, before its answer: a
    /// line the player typed (`sent`), a session state.
    pub after_next: Option<ToClient>,
    /// The same, after the brain's command `.0` (the first one to go).
    pub after_cmd: Option<(String, ToClient)>,
    /// The brain's messages read while a command was in flight, still to be acted on.
    pub pending: VecDeque<ToGateway>,
    pub state: SessionState,
    /// The commands the script answered, in order: the walker's (the re-sync's are answered
    /// with a bare prompt and left out).
    pub played: Vec<String>,
    /// A move may go while another is in flight (Task's pipelined legs): one at most, and only
    /// a direction.
    pub pipelining: bool,
    /// The game has stopped answering (see [`Game::freeze`]): lines the brain sends are kept in
    /// `seen` and nothing more.
    pub frozen: bool,
    /// The moves that went while another was in flight.
    pub ahead: Vec<String>,
}

/// A direction command, as a pipelined move is.
pub fn is_direction(text: &str) -> bool {
    [
        "n",
        "s",
        "e",
        "w",
        "ne",
        "nw",
        "se",
        "sw",
        "u",
        "d",
        "north",
        "south",
        "east",
        "west",
        "northeast",
        "northwest",
        "southeast",
        "southwest",
        "up",
        "down",
    ]
    .contains(&text)
}

impl Game {
    /// A brain attached to a session whose history shows `start` and a prompt.
    pub async fn start(state: SessionState, start: Key) -> (Game, Run) {
        Game::start_with(state, room(start)).await
    }

    /// A brain attached to a session whose history is `history` (ending in a prompt).
    pub async fn start_with(state: SessionState, history: String) -> (Game, Run) {
        let (mut g, run) = Game::start_lost(state, history).await;
        g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
            .await;
        (g, run)
    }

    /// The same, for a history that need not show a room: no map view is waited for.
    pub async fn start_lost(state: SessionState, history: String) -> (Game, Run) {
        let clock = FixedClock(UNIX_EPOCH + Duration::from_secs(T0));
        let brain = Brain::new(world(), Arc::new(clock), None).with_settings(test_settings(""));
        Game::start_brain(state, history, brain).await
    }

    /// `brain` attached to a session whose history is `history`, once the catch-up's whole
    /// snapshot is out.
    pub async fn start_brain(
        state: SessionState,
        history: String,
        mut brain: Brain,
    ) -> (Game, Run) {
        let (to_gateway, rx) = mpsc::channel(4096);
        let (tx, from_gateway) = mpsc::channel(4096);
        let run = tokio::spawn(async move {
            let mut link = ChannelLink::new(to_gateway, from_gateway);
            let result = brain.run(&mut link).await;
            (brain, result)
        });
        let mut g = Game {
            tx: Some(tx),
            rx,
            log: Vec::new(),
            seen: Vec::new(),
            script: VecDeque::new(),
            after_next: None,
            after_cmd: None,
            pending: VecDeque::new(),
            state,
            played: Vec::new(),
            pipelining: false,
            frozen: false,
            ahead: Vec::new(),
        };
        g.log.push(Chunk {
            seq: 1,
            text: history,
        });
        let hello = g.rx.recv().await;
        assert!(matches!(hello, Some(ToGateway::Hello { .. })), "{hello:?}");
        g.send(ToClient::Welcome {
            protocol: wayfarer_proto::PROTOCOL,
            server_build: "test".into(),
            sessions: vec![info(state, 1)],
        })
        .await;
        // The catch-up's whole snapshot.
        g.until(|m| matches!(m, ToGateway::Character { .. }).then_some(()))
            .await;
        (g, run)
    }

    pub async fn send(&self, msg: ToClient) {
        if let Some(tx) = &self.tx {
            tx.send(msg).await.unwrap();
        }
    }

    pub async fn output(&mut self, text: &str) {
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

    /// What the player typed: the gateway's `sent`, then the echo.
    pub async fn typed(&mut self, text: &str) {
        self.send(ToClient::Sent {
            session: ID.into(),
            seq: self.log.len() as u64,
            text: text.into(),
        })
        .await;
    }

    pub fn expect(&mut self, command: &str, answer: impl Into<String>) {
        self.script.push_back((command.into(), answer.into()));
    }

    pub async fn walk_to(&self, to: Key) {
        self.walk_to_short(to, None).await;
    }

    /// `walk_to` with its `stop_short`.
    pub async fn walk_to_short(&self, to: Key, stop_short: Option<bool>) {
        self.send(ToClient::WalkTo {
            session: ID.into(),
            map: to.0,
            room: to.1,
            stop_short,
        })
        .await;
    }

    pub async fn set_state(&mut self, state: SessionState) {
        self.state = state;
        self.send(ToClient::SessionState {
            session: ID.into(),
            state,
        })
        .await;
    }

    /// Act on the brain's next message as the gateway and the game would; `None` once the
    /// brain has gone.
    pub async fn next(&mut self) -> Option<ToGateway> {
        let msg = match self.pending.pop_front() {
            Some(m) => m,
            None => self.rx.recv().await?,
        };
        self.seen.push(msg.clone());
        match &msg {
            ToGateway::HistoryRequest { after_seq, .. } => {
                let after = after_seq.unwrap_or(0);
                let chunks: Vec<Chunk> =
                    self.log.iter().filter(|c| c.seq > after).cloned().collect();
                let from_seq = chunks.first().map_or(self.log.len() as u64 + 1, |c| c.seq);
                self.send(ToClient::History {
                    session: ID.into(),
                    from_seq,
                    chunks,
                })
                .await;
            }
            ToGateway::InputLine { text, .. } => {
                assert_eq!(
                    self.state,
                    SessionState::Realm,
                    "{text:?} sent outside the realm"
                );
                self.send(ToClient::Sent {
                    session: ID.into(),
                    seq: self.log.len() as u64,
                    text: text.clone(),
                })
                .await;
                let after = match &self.after_cmd {
                    Some((cmd, _)) if cmd == text => self.after_cmd.take().map(|(_, m)| m),
                    _ => self.after_next.take(),
                };
                if let Some(msg) = after {
                    if let ToClient::SessionState { state, .. } = msg {
                        self.state = state;
                    }
                    self.send(msg).await;
                }
                // Let the brain do all it would before the answer: a second line now would be
                // two in flight.
                tokio::time::sleep(Duration::from_millis(10)).await;
                while let Ok(m) = self.rx.try_recv() {
                    if let ToGateway::InputLine { text: next, .. } = &m {
                        let waiting = self
                            .pending
                            .iter()
                            .any(|p| matches!(p, ToGateway::InputLine { .. }));
                        assert!(
                            self.pipelining && !waiting && is_direction(text) && is_direction(next),
                            "{m:?} sent while {text:?} was in flight"
                        );
                        self.ahead.push(next.clone());
                    }
                    self.pending.push_back(m);
                }
                if self.frozen {
                    return Some(msg.clone());
                }
                let answer = match self.script.front() {
                    Some((cmd, _)) if cmd == text => {
                        self.played.push(text.clone());
                        self.script.pop_front().unwrap().1
                    }
                    _ if RESYNC_COMMANDS.contains(&text.as_str()) => PROMPT.to_owned(),
                    other => {
                        let said: Vec<_> = self
                            .seen
                            .iter()
                            .filter_map(|m| match m {
                                ToGateway::Notice { text, .. }
                                | ToGateway::Warning { text, .. } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect();
                        let sent: Vec<_> = self
                            .seen
                            .iter()
                            .filter_map(|m| match m {
                                ToGateway::InputLine { text, .. } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect();
                        panic!(
                            "the brain sent {text:?}; the script wants {other:?}; it said {said:?}; \
                             it sent {sent:?}"
                        )
                    }
                };
                // `NOTHING`: its answer came with the one before (as two pipelined moves'
                // rooms can, in one chunk).
                if answer != NOTHING {
                    self.output(&format!("{text}\r\n{answer}")).await;
                }
            }
            _ => {}
        }
        Some(msg)
    }

    /// The loop never rests (no respawn clock), so a test that has seen what it came for stops
    /// the game answering: what the brain sends from here on is only recorded in `seen`, not
    /// in `played`, and its walks time out until the brain is closed.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub async fn until<T>(&mut self, mut want: impl FnMut(&ToGateway) -> Option<T>) -> T {
        loop {
            let msg = self.next().await.expect("the brain went away");
            if let Some(t) = want(&msg) {
                return t;
            }
        }
    }

    /// The next notice or warning for the session.
    pub async fn narration(&mut self) -> String {
        self.until(|m| match m {
            ToGateway::Notice { text, .. } | ToGateway::Warning { text, .. } => Some(text.clone()),
            _ => None,
        })
        .await
    }

    /// Read until the narration is `text`; every narration on the way.
    pub async fn narrated(&mut self, text: &str) -> Vec<String> {
        let mut all = Vec::new();
        loop {
            let n = self.narration().await;
            all.push(n.clone());
            if n == text {
                return all;
            }
        }
    }

    /// The next narration that is not the bot's own about a journey (a Go to turns the bot on:
    /// it re-syncs, says where it goes, and plays where it ends up): the walker's lines.
    pub async fn walk_narration(&mut self) -> String {
        loop {
            let n = self.narration().await;
            if !is_bots_own(&n) {
                return n;
            }
        }
    }

    /// Read until the walker's narration is `text`; the walker's lines on the way.
    pub async fn walk_narrated(&mut self, text: &str) -> Vec<String> {
        let mut all = Vec::new();
        loop {
            let n = self.walk_narration().await;
            all.push(n.clone());
            if n == text {
                return all;
            }
        }
    }

    /// Let the clock run (the paused clock jumps), answering what the brain sends meanwhile.
    pub async fn idle(&mut self, d: Duration) {
        let end = tokio::time::Instant::now() + d;
        loop {
            tokio::select! {
                msg = self.next() => if msg.is_none() { return },
                () = tokio::time::sleep_until(end) => return,
            }
        }
    }

    /// Hang up; everything the brain sent, and the walker's commands.
    pub async fn close(mut self, run: Run) -> (Vec<ToGateway>, Vec<String>) {
        self.tx.take();
        while self.next().await.is_some() {}
        let (_, result) = run.await.unwrap();
        result.unwrap();
        assert!(self.script.is_empty(), "never sent: {:?}", self.script);
        (self.seen, self.played)
    }
}

pub fn key(k: Key) -> RoomKey {
    RoomKey {
        map: k.0,
        room: k.1,
    }
}

/// The snapshot UIs end up with: the last whole one with every later patch applied.
pub fn as_ui_sees(seen: &[ToGateway]) -> CharacterState {
    let mut ui = serde_json::Value::Null;
    for msg in seen {
        match msg {
            ToGateway::Character { state, .. } => ui = serde_json::to_value(state).unwrap(),
            ToGateway::CharacterPatch { patch, .. } => wayfarer_brain::apply_patch(&mut ui, patch),
            _ => {}
        }
    }
    serde_json::from_value(ui).unwrap()
}

/// Every `map_view`: (center, route).
pub fn views(seen: &[ToGateway]) -> Vec<(RoomKey, Option<Vec<RoomKey>>)> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::MapView { center, route, .. } => Some((*center, route.clone())),
            _ => None,
        })
        .collect()
}

impl Game {
    /// The brain's snapshot's position, by a `state_request`.
    pub async fn position(&mut self) -> wayfarer_proto::Position {
        self.send(ToClient::StateRequest {
            session: Some(ID.into()),
        })
        .await;
        self.until(|m| match m {
            ToGateway::Character { state, .. } => state.position.clone(),
            _ => None,
        })
        .await
    }
}

/// A line of the bot's own on a journey, not the walker's: "Going to ...", "...: re-syncing",
/// "re-synced", "Waiting here", "Not going to ...", "Playing from ...".
pub fn is_bots_own(line: &str) -> bool {
    line.starts_with("Going to ")
        || line.starts_with("Not going to ")
        || line.starts_with("Playing from ")
        || line.ends_with(": re-syncing")
        || line == "re-synced"
        || line == "Waiting here"
}
