//! Walking (docs/brain.md "Walking") against a fake gateway whose session answers each command
//! like the game: rooms cut from the `room-walk` capture where it has them, rooms made from the
//! world pack where it doesn't, and the game's refusal and door texts. On a paused clock.
//!
//! The world is the pack built from `tests/fixtures/world` (shared with wayfarer-world's tests,
//! `common::pack_path`); each test process loads it once (about half a second: the game
//! texts' patterns and the room graph).

#[path = "../../wayfarer-world/tests/common/mod.rs"]
mod common;

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use wayfarer_brain::{
    Brain, BrainError, ChannelLink, FixedClock, MAX_BASHES, RESYNC_COMMANDS, World,
};
use wayfarer_proto::{
    CharacterState, Chunk, ExitKind, RoomKey, SessionInfo, SessionState, ToClient, ToGateway,
};
use wayfarer_world::{Key, WorldPack};

const ID: &str = "aldric";
const PROMPT: &str = "\x1b[79D\x1b[K\x1b[0;37m[HP=35\x1b[0;37m]:";

const VILLAGE_ENTRANCE: Key = (1, 2140);
const WEAPONS_SHOP: Key = (1, 2141);
const NARROW_PATH: Key = (1, 2143);
const NARROW_ROAD: Key = (1, 2146);
const ARENA: Key = (1, 2150);
/// North of the Arena, through its door.
const BEYOND_ARENA: Key = (1, 2152);

fn world() -> World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD
        .get_or_init(|| {
            let pack = WorldPack::open(common::pack_path()).unwrap();
            World::from_pack(pack).unwrap()
        })
        .clone()
}

fn atlas() -> Arc<wayfarer_brain::Atlas> {
    world().atlas.unwrap()
}

fn capture() -> &'static str {
    static RAW: OnceLock<String> = OnceLock::new();
    RAW.get_or_init(|| {
        let raw = std::fs::read(common::repo("tests/fixtures/captures/room-walk/raw.bin")).unwrap();
        wayfarer_world::cp437::decode(&raw)
    })
}

/// The rooms taken from the capture (each shown there under a name no other room there has).
const CAPTURED: [Key; 5] = [
    VILLAGE_ENTRANCE,
    WEAPONS_SHOP,
    NARROW_PATH,
    NARROW_ROAD,
    ARENA,
];

/// A room as the game shows it after a move: from the capture for the Newhaven rooms it
/// walks through, else made from the pack (title, a line of description, the exits it lists).
fn room(k: Key) -> String {
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
fn says(line: &str) -> String {
    format!("\x1b[1;31;40m{line}\r\n{PROMPT}")
}

fn info(state: SessionState, last_seq: u64) -> SessionInfo {
    SessionInfo {
        id: ID.into(),
        name: "Aldric".into(),
        server: "bbs.example.org:23".into(),
        state,
        last_seq,
    }
}

type Run = JoinHandle<(Brain, Result<(), BrainError>)>;

/// The gateway, and behind it the game: each command the brain sends is checked against the
/// script and answered with the script's text.
struct Game {
    tx: Option<mpsc::Sender<ToClient>>,
    rx: mpsc::Receiver<ToGateway>,
    log: Vec<Chunk>,
    /// Everything the brain sent, in order.
    seen: Vec<ToGateway>,
    /// (command, answer) in the order the brain must send them.
    script: VecDeque<(String, String)>,
    /// What the gateway says right after the brain's next command goes, before its answer: a
    /// line the player typed (`sent`), a session state.
    after_next: Option<ToClient>,
    /// The brain's messages read while a command was in flight, still to be acted on.
    pending: VecDeque<ToGateway>,
    state: SessionState,
    /// The commands the script answered, in order: the walker's (the re-sync's are answered
    /// with a bare prompt and left out).
    played: Vec<String>,
}

impl Game {
    /// A brain attached to a session whose history shows `start` and a prompt.
    async fn start(state: SessionState, start: Key) -> (Game, Run) {
        let (to_gateway, rx) = mpsc::channel(4096);
        let (tx, from_gateway) = mpsc::channel(4096);
        let clock = FixedClock(UNIX_EPOCH + Duration::from_secs(1_792_411_200));
        let mut brain = Brain::new(world(), Arc::new(clock), None);
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
            pending: VecDeque::new(),
            state,
            played: Vec::new(),
        };
        g.log.push(Chunk {
            seq: 1,
            text: room(start),
        });
        let hello = g.rx.recv().await;
        assert!(matches!(hello, Some(ToGateway::Hello { .. })), "{hello:?}");
        g.send(ToClient::Welcome {
            protocol: wayfarer_proto::PROTOCOL,
            server_build: "test".into(),
            sessions: vec![info(state, 1)],
        })
        .await;
        // The catch-up's whole snapshot, and its map view.
        g.until(|m| matches!(m, ToGateway::Character { .. }).then_some(()))
            .await;
        g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
            .await;
        (g, run)
    }

    async fn send(&self, msg: ToClient) {
        if let Some(tx) = &self.tx {
            tx.send(msg).await.unwrap();
        }
    }

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

    /// What the player typed: the gateway's `sent`, then the echo.
    async fn typed(&mut self, text: &str) {
        self.send(ToClient::Sent {
            session: ID.into(),
            seq: self.log.len() as u64,
            text: text.into(),
        })
        .await;
    }

    fn expect(&mut self, command: &str, answer: impl Into<String>) {
        self.script.push_back((command.into(), answer.into()));
    }

    async fn walk_to(&self, to: Key) {
        self.send(ToClient::WalkTo {
            session: ID.into(),
            map: to.0,
            room: to.1,
        })
        .await;
    }

    async fn set_state(&mut self, state: SessionState) {
        self.state = state;
        self.send(ToClient::SessionState {
            session: ID.into(),
            state,
        })
        .await;
    }

    /// Act on the brain's next message as the gateway and the game would; `None` once the
    /// brain has gone.
    async fn next(&mut self) -> Option<ToGateway> {
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
                if let Some(msg) = self.after_next.take() {
                    if let ToClient::SessionState { state, .. } = msg {
                        self.state = state;
                    }
                    self.send(msg).await;
                }
                // Let the brain do all it would before the answer: a second line now would be
                // two in flight.
                tokio::time::sleep(Duration::from_millis(10)).await;
                while let Ok(m) = self.rx.try_recv() {
                    assert!(
                        !matches!(m, ToGateway::InputLine { .. }),
                        "{m:?} sent while {text:?} was in flight"
                    );
                    self.pending.push_back(m);
                }
                let answer = match self.script.front() {
                    Some((cmd, _)) if cmd == text => {
                        self.played.push(text.clone());
                        self.script.pop_front().unwrap().1
                    }
                    _ if RESYNC_COMMANDS.contains(&text.as_str()) => PROMPT.to_owned(),
                    other => panic!("the brain sent {text:?}; the script wants {other:?}"),
                };
                self.output(&format!("{text}\r\n{answer}")).await;
            }
            _ => {}
        }
        Some(msg)
    }

    async fn until<T>(&mut self, mut want: impl FnMut(&ToGateway) -> Option<T>) -> T {
        loop {
            let msg = self.next().await.expect("the brain went away");
            if let Some(t) = want(&msg) {
                return t;
            }
        }
    }

    /// The next notice or warning for the session.
    async fn narration(&mut self) -> String {
        self.until(|m| match m {
            ToGateway::Notice { text, .. } | ToGateway::Warning { text, .. } => Some(text.clone()),
            _ => None,
        })
        .await
    }

    /// Read until the narration is `text`; every narration on the way.
    async fn narrated(&mut self, text: &str) -> Vec<String> {
        let mut all = Vec::new();
        loop {
            let n = self.narration().await;
            all.push(n.clone());
            if n == text {
                return all;
            }
        }
    }

    /// Let the clock run (the paused clock jumps), answering what the brain sends meanwhile.
    async fn idle(&mut self, d: Duration) {
        let end = tokio::time::Instant::now() + d;
        loop {
            tokio::select! {
                msg = self.next() => if msg.is_none() { return },
                () = tokio::time::sleep_until(end) => return,
            }
        }
    }

    /// Hang up; everything the brain sent, and the walker's commands.
    async fn close(mut self, run: Run) -> (Vec<ToGateway>, Vec<String>) {
        self.tx.take();
        while self.next().await.is_some() {}
        let (_, result) = run.await.unwrap();
        result.unwrap();
        assert!(self.script.is_empty(), "never sent: {:?}", self.script);
        (self.seen, self.played)
    }
}

fn key(k: Key) -> RoomKey {
    RoomKey {
        map: k.0,
        room: k.1,
    }
}

/// The snapshot UIs end up with: the last whole one with every later patch applied.
fn as_ui_sees(seen: &[ToGateway]) -> CharacterState {
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
fn views(seen: &[ToGateway]) -> Vec<(RoomKey, Option<Vec<RoomKey>>)> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::MapView { center, route, .. } => Some((*center, route.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn walks_a_route_confirming_each_step() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", room(ARENA));
    g.walk_to(ARENA).await;
    assert_eq!(g.narration().await, "Walking to Newhaven, Arena: 3 steps");
    assert_eq!(g.narration().await, "Arrived: Newhaven, Arena");
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w", "w", "d"]);

    // The walk showed on the character, and is gone at the end.
    assert!(seen.iter().any(|m| matches!(
        m,
        ToGateway::CharacterPatch { patch, .. }
            if patch["walking"]["to"] == serde_json::json!({"map": 1, "room": 2150})
    )));
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.walking, None);
    let pos = ui.position.unwrap();
    assert_eq!(
        (pos.map, pos.room, pos.candidates),
        (Some(1), Some(2150), 1)
    );
    assert_eq!(pos.title.as_deref(), Some("Newhaven, Arena"));

    // The catch-up's view, one when the route starts, one after each room, centred on it; the
    // route in it while walking. Arriving ends the route in the last room's view.
    let v = views(&seen);
    let centers: Vec<RoomKey> = v.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        centers,
        [
            VILLAGE_ENTRANCE,
            VILLAGE_ENTRANCE,
            NARROW_PATH,
            NARROW_ROAD,
            ARENA
        ]
        .map(key)
    );
    assert_eq!(v[0].1, None, "the catch-up's view");
    assert_eq!(
        v[1].1,
        Some(
            [VILLAGE_ENTRANCE, NARROW_PATH, NARROW_ROAD, ARENA]
                .map(key)
                .to_vec()
        )
    );
    assert_eq!(v[3].1, Some([NARROW_ROAD, ARENA].map(key).to_vec()));
    assert_eq!(v[4].1, None);
}

#[tokio::test(start_paused = true)]
async fn the_map_view_puts_the_believed_room_at_the_center() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.send(ToClient::MapRequest {
        session: ID.into(),
        radius: Some(4),
    })
    .await;
    let (rooms, exits) = g
        .until(|m| match m {
            ToGateway::MapView { rooms, exits, .. } => Some((rooms.clone(), exits.clone())),
            _ => None,
        })
        .await;
    let at = |k: Key| rooms.iter().find(|r| (r.map, r.room) == k).unwrap();
    let c = &rooms[0];
    assert_eq!((c.map, c.room, c.x, c.y, c.level), (1, 2140, 0, 0, 0));
    assert!(c.candidate && c.visited);
    assert_eq!(c.name, "Newhaven, Village Entrance");
    let shop = at(WEAPONS_SHOP);
    assert_eq!((shop.x, shop.y), (0, -1));
    assert_eq!(shop.kind, Some(wayfarer_proto::RoomKind::Shop));
    assert!(!shop.visited && !shop.candidate);
    assert_eq!((at(NARROW_PATH).x, at(NARROW_PATH).y), (-1, 0));
    let arena = at(ARENA);
    assert_eq!((arena.x, arena.level), (-2, -1));
    assert_eq!(arena.kind, Some(wayfarer_proto::RoomKind::Arena));
    // Within four steps only.
    assert!(rooms.iter().all(|r| r.x.abs() <= 4 && r.y.abs() <= 4));
    assert!(
        exits.iter().any(|e| {
            e.from == key(VILLAGE_ENTRANCE) && e.to == key(WEAPONS_SHOP) && e.dir == "n"
        })
    );
    assert!(
        exits.iter().any(|e| e.kind == ExitKind::Door),
        "the Arena's door"
    );
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_position_follows_a_character_played_by_hand() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.typed("n").await;
    g.output(&format!("n\r\n{}", room(WEAPONS_SHOP))).await;
    let center = g
        .until(|m| match m {
            ToGateway::MapView { center, .. } => Some(*center),
            _ => None,
        })
        .await;
    assert_eq!(center, key(WEAPONS_SHOP));
    let (_, played) = g.close(run).await;
    assert_eq!(played, Vec::<String>::new());
}

#[tokio::test(start_paused = true)]
async fn no_exit_looks_and_replans_from_where_the_game_says() {
    // Pitfall 34: the brain thinks it is at the Village Entrance; it is on the Narrow Path.
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", says("There is no exit in that direction!"));
    g.expect("look", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", room(ARENA));
    g.walk_to(ARENA).await;
    let said = g.narrated("Arrived: Newhaven, Arena").await;
    assert_eq!(
        said,
        [
            "Walking to Newhaven, Arena: 3 steps",
            "No exit `w`: looking to relocate",
            "Replanned (relocated): 2 steps",
            "Arrived: Newhaven, Arena",
        ]
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w", "look", "w", "d"]);
}

#[tokio::test(start_paused = true)]
async fn a_second_no_exit_stops_the_walk() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", says("There is no exit in that direction!"));
    g.expect("look", room(VILLAGE_ENTRANCE));
    g.expect("w", says("There is no exit in that direction!"));
    g.walk_to(ARENA).await;
    let said = g.narrated("Stopped walking: no exit `w` twice: lost").await;
    assert_eq!(said.len(), 4, "{said:?}");
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w", "look", "w"]);
    assert_eq!(as_ui_sees(&seen).walking, None);
}

#[tokio::test(start_paused = true)]
async fn a_shut_door_is_opened_then_walked_through() {
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is now open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    let said = g.narrated(&format!("Arrived: {name}")).await;
    assert_eq!(said[0], format!("Walking to {name}: one step, one door"));
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "open n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn a_locked_door_is_bashed_at_most_three_times() {
    // Bashed open on the second try.
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("Your attempts to bash through fail!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    g.narrated(&format!("Arrived: {name}")).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "bash n", "bash n", "n"]);

    // Three failures: the door is marked, and there is no other way.
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is closed!"));
    for _ in 0..MAX_BASHES {
        g.expect("bash n", says("Your attempts to bash through fail!"));
    }
    g.walk_to(BEYOND_ARENA).await;
    let said = g.narration().await;
    assert!(said.starts_with("Walking to"), "{said}");
    let said = g.narration().await;
    assert!(
        said.starts_with("Stopped walking: the door would not bash open; no way to"),
        "{said}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "bash n", "bash n", "bash n"]);
}

#[tokio::test(start_paused = true)]
async fn a_locked_door_open_refused_then_bashed() {
    // `open` on a locked door (the reply the room-walk capture has), then bash.
    let (mut g, run) = Game::start(SessionState::Realm, ARENA).await;
    g.expect("n", says("The door is Closed!"));
    g.expect("open n", says("The door is locked."));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.walk_to(BEYOND_ARENA).await;
    let name = atlas().graph.room(BEYOND_ARENA).unwrap().name.clone();
    g.narrated(&format!("Arrived: {name}")).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["n", "open n", "bash n", "n"]);
}

#[tokio::test(start_paused = true)]
async fn a_refusal_marks_the_exit_and_the_replan_avoids_it() {
    // The Healer's level exit (Narrow Road west, levels 1-3), refused: the brain doesn't know
    // the level yet, so it plans through; then it can't.
    let healer = (1, 2190);
    let (mut g, run) = Game::start(SessionState::Realm, NARROW_ROAD).await;
    g.expect(
        "w",
        says("You have progressed too far to go through this exit!"),
    );
    g.walk_to(healer).await;
    let said = g.narration().await;
    assert_eq!(said, "Walking to Newhaven, Healer: one step");
    let said = g.narration().await;
    assert!(
        said.starts_with("Stopped walking: `w` refused: level too high; no way to"),
        "{said}"
    );
    // The exit stays marked for the session: a second walk doesn't try it.
    g.walk_to(healer).await;
    let said = g.narration().await;
    assert!(
        said.starts_with("Not walking to Newhaven, Healer: no way there"),
        "{said}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
}

#[tokio::test(start_paused = true)]
async fn a_refused_exit_is_walked_round() {
    // Silvermere Docks 1:27 west to 1:33 refused: round by River Street (s, w, n).
    let (from, to) = ((1, 27), (1, 33));
    let a = atlas();
    let around = wayfarer_world::route(
        &a.graph,
        from,
        to,
        &wayfarer_world::Traveller {
            blocked: [(from, 3)].into(),
            ..Default::default()
        },
    )
    .unwrap();
    let (mut g, run) = Game::start(SessionState::Realm, from).await;
    // Two neighbouring rooms are "Docks" with the same exits: the lower key is believed.
    let pos = g.position().await;
    assert_eq!((pos.room, pos.candidates), (Some(27), 2), "{pos:?}");
    g.expect("w", says("You may not go through this exit!"));
    for s in &around {
        g.expect(&s.command, room(s.to));
    }
    g.walk_to(to).await;
    let name = a.graph.room(to).unwrap().name.clone();
    let said = g.narrated(&format!("Arrived: {name}")).await;
    assert!(
        said[1].starts_with("Replanned (`w` refused: forbidden)"),
        "{said:?}"
    );
    let (_, played) = g.close(run).await;
    let mut want = vec!["w".to_owned()];
    want.extend(around.iter().map(|s| s.command.clone()));
    assert_eq!(played, want);
}

#[tokio::test(start_paused = true)]
async fn combat_mid_walk_stops_after_the_move_in_flight() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    let path = room(NARROW_PATH);
    let cut = path.rfind(PROMPT).unwrap();
    g.expect(
        "w",
        format!("{}\x1b[1;31m*Combat Engaged*\r\n{PROMPT}", &path[..cut]),
    );
    g.walk_to(ARENA).await;
    g.narrated("Stopped walking: combat started").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.walking, None);
    assert_eq!(ui.position.unwrap().room, Some(NARROW_PATH.1));
}

#[tokio::test(start_paused = true)]
async fn a_typed_line_stops_the_walk() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.after_next = Some(ToClient::Sent {
        session: ID.into(),
        seq: 1,
        text: "say hello".into(),
    });
    g.walk_to(ARENA).await;
    g.narrated("Stopped walking: you typed a line").await;
    // Nothing more from the walker, however long it waits.
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
}

#[tokio::test(start_paused = true)]
async fn walk_stop_stops_after_the_move_in_flight() {
    let (mut g, run) = Game::start(SessionState::Realm, VILLAGE_ENTRANCE).await;
    g.expect("w", room(NARROW_PATH));
    g.walk_to(ARENA).await;
    g.send(ToClient::WalkStop { session: ID.into() }).await;
    g.narrated("Stopped walking: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
    assert_eq!(
        as_ui_sees(&seen).position.unwrap().room,
        Some(NARROW_PATH.1)
    );
}

#[tokio::test(start_paused = true)]
async fn nothing_goes_outside_the_realm() {
    let (mut g, run) = Game::start(SessionState::Menu, VILLAGE_ENTRANCE).await;
    g.walk_to(ARENA).await;
    assert_eq!(
        g.narration().await,
        "walk_to: the character is not in the realm"
    );
    // Leaving the realm mid-walk stops it; nothing more is sent.
    g.set_state(SessionState::Realm).await;
    g.expect("w", room(NARROW_PATH));
    g.walk_to(ARENA).await;
    assert_eq!(g.narration().await, "Walking to Newhaven, Arena: 3 steps");
    // The session leaves the realm while the first move is in flight.
    g.after_next = Some(ToClient::SessionState {
        session: ID.into(),
        state: SessionState::Menu,
    });
    g.narrated("Stopped walking: the session left the realm")
        .await;
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["w"]);
}

impl Game {
    /// The brain's snapshot's position, by a `state_request`.
    async fn position(&mut self) -> wayfarer_proto::Position {
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
