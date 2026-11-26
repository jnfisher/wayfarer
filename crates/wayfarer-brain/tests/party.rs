//! Parties (docs/brain.md "Parties", card C-039): two characters in the Arena behind a fake
//! gateway whose game keeps a party as MajorMUD does (`invite`, `follow`, `par`, `leave`, with
//! the lines of `tests/fixtures/party/`). On a paused clock.

mod game;

use std::collections::{BTreeMap, BTreeSet};

use game::*;
use tokio::sync::mpsc;
use wayfarer_brain::{Brain, ChannelLink, FixedClock, Store};
use wayfarer_proto::{BotAction, MemberState, PartyDef, PartyRank, PartyStatus};

const LEADER: &str = "penny";
const FOLLOWER: &str = "lauren";
/// Newhaven's docks, whose skiff (`borrow skiff`, a Text exit) rows to the Small Pier: a way
/// through the game's follow does not take a follower (C-121).
const NEWHAVEN_DOCKS: Key = (1, 2149);
const SMALL_PIER: Key = (1, 2335);

fn name(session: &str) -> &'static str {
    if session == LEADER {
        "Penny Pax"
    } else {
        "Lauren Phillips"
    }
}

fn first(session: &str) -> &'static str {
    name(session).split(' ').next().unwrap()
}

fn said(text: &str) -> String {
    format!("\x1b[79D\x1b[K\x1b[0;37m{text}\r\n{PROMPT}")
}

const STASH_WORDS: [&str; 4] = ["silver", "gold", "copper", "platinum"];

/// The game behind the gateway: where each character is, and the party between them.
#[derive(Default)]
struct World {
    invited: bool,
    following: bool,
    rooms: BTreeMap<String, Key>,
    /// HP of 35 where a test set it: the prompts of the game's answers show it.
    hp: BTreeMap<String, i32>,
    /// The Arena's north door is locked: the leader's bashes fail, the follower's gives way.
    locked: bool,
    /// Each one's rank, `Midrank` when not here; inviting puts the leader in front.
    ranks: BTreeMap<String, &'static str>,
    /// The status screens are whole (stats, HP and mana), and the follower is a Cleric with
    /// minor healing (2 mana, 20 of it).
    cleric: bool,
    /// Those whose prompts say `(Resting)`.
    resting: BTreeSet<String>,
    /// The server plays Quicksilver 1.20: `help 1.20` has its topic and `guard` is a command.
    qs120: bool,
    /// Those who cannot move: held, as by a spell ("You are held").
    held: BTreeSet<String>,
    /// The coins each carries, by `hide`'s word, where a test gave them: `i` lists them and
    /// `hide` takes them away (C-093).
    purse: BTreeMap<String, BTreeMap<&'static str, u64>>,
    /// The Arena's `look` shows the other one there, in its `Also here:`.
    together: bool,
    /// The game takes a following follower along when its leader moves from their room, as
    /// the real one does: `-- Following your Party leader <dir> --` and the room.
    drag: bool,
    /// `sneak`s still to refuse with `You may not sneak right now!` (C-122).
    sneak_refusals: u32,
    /// Rooms too dark to see: a move into one, or a `look` there, shows only the darkness line
    /// (C-127).
    dark: BTreeSet<Key>,
    /// Those whose every move the game refuses with "There is no exit in that direction!",
    /// wherever they are: a wrong belief in the dark (C-128).
    no_exit: BTreeSet<String>,
    /// The leader is a Mage with speed (level 12, 12 mana) and 60 mana, for a blessing on the
    /// follower (C-131).
    mage: bool,
    /// What the game says to the leader's `sped lauren` in place of casting it.
    speed_refusal: Option<&'static str>,
    /// Those whose lines the game holds, as a room command's `adddelay` does, and until when
    /// (C-121): a line typed meanwhile is answered when the hold ends.
    held_until: BTreeMap<String, tokio::time::Instant>,
    /// The lines held: who typed each, the line, and when it is answered.
    delayed: Vec<(String, String, tokio::time::Instant)>,
    /// Those a room command's script has teleported, with how many lines the brain had sent
    /// by then.
    fired: Vec<(String, usize)>,
}

/// A room too dark to see, as the game shows it.
fn dark_room() -> String {
    said("The room is very dark - you can't see anything")
}

/// The game's name for a coin, by `hide`'s word.
fn coin_name(word: &str) -> &'static str {
    match word {
        "silver" => "silver nobles",
        "gold" => "gold crowns",
        "copper" => "copper farthings",
        _ => "platinum pieces",
    }
}

impl World {
    fn rank(&self, session: &str) -> &'static str {
        self.ranks.get(session).copied().unwrap_or("Midrank")
    }
}

/// The `fight-arena` capture's whole status screen (level 1, Health 50, Willpower 40) for
/// `session`: a Cleric with 20 mana for the follower, a Warrior for the leader.
fn whole_status(session: &str) -> String {
    let screen =
        game::arena::status(35).replace("Aldric Stonebrook", &format!("{:<17}", name(session)));
    if session != FOLLOWER {
        return screen;
    }
    let hits = "Thievery:    \x1b[36m    0\r\n";
    screen.replace("Warrior    ", "Cleric     ").replacen(
        hits,
        &format!(
            "{hits}\x1b[32mMana: \x1b[36m * 20/20    \x1b[32mSpellcasting: \x1b[36m  40     \
             \x1b[32mTraps:       \x1b[36m    0\r\n"
        ),
        1,
    )
}

/// The leader's status screen as a Mage with 60 mana (C-131).
fn mage_status() -> String {
    let hits = "Thievery:    \x1b[36m    0\r\n";
    whole_status(LEADER)
        .replace("Warrior    ", "Mage       ")
        .replacen(
            hits,
            &format!(
                "{hits}\x1b[32mMana: \x1b[36m * 60/60    \x1b[32mSpellcasting: \x1b[36m  40     \
                 \x1b[32mTraps:       \x1b[36m    0\r\n"
            ),
            1,
        )
}

/// A room as `look` shows it; the Arena with nothing in it, so nobody stops to fight.
fn shown(k: Key) -> String {
    if k == ARENA {
        game::arena::arena(&[], 35)
    } else {
        room(k)
    }
}

struct Gateway {
    tx: mpsc::Sender<ToClient>,
    rx: mpsc::Receiver<ToGateway>,
    logs: BTreeMap<String, Vec<Chunk>>,
    world: World,
    /// Every line the brain sent, by session.
    lines: Vec<(String, String)>,
    seen: Vec<ToGateway>,
}

impl Gateway {
    async fn start(brain: Brain) -> (Gateway, Run) {
        Gateway::start_at(brain, ARENA).await
    }

    /// Both characters in `room` to begin with (the Arena unless a test says).
    async fn start_at(brain: Brain, room: Key) -> (Gateway, Run) {
        let (to_gateway, rx) = mpsc::channel(4096);
        let (tx, from_gateway) = mpsc::channel(4096);
        let mut brain = brain;
        let run = tokio::spawn(async move {
            let mut link = ChannelLink::new(to_gateway, from_gateway);
            let result = brain.run(&mut link).await;
            (brain, result)
        });
        let mut g = Gateway {
            tx,
            rx,
            logs: BTreeMap::new(),
            world: World {
                rooms: BTreeMap::from([(LEADER.to_owned(), room), (FOLLOWER.to_owned(), room)]),
                ..World::default()
            },
            lines: Vec::new(),
            seen: Vec::new(),
        };
        for s in [LEADER, FOLLOWER] {
            g.logs.insert(
                s.to_owned(),
                vec![Chunk {
                    seq: 1,
                    text: shown(room),
                }],
            );
        }
        let hello = g.rx.recv().await;
        assert!(matches!(hello, Some(ToGateway::Hello { .. })), "{hello:?}");
        let sessions = [LEADER, FOLLOWER]
            .map(|s| SessionInfo {
                id: s.into(),
                name: s.into(),
                server: "bbs.example.org:23".into(),
                state: SessionState::Realm,
                last_seq: 1,
            })
            .to_vec();
        g.send(ToClient::Welcome {
            protocol: wayfarer_proto::PROTOCOL,
            server_build: "test".into(),
            sessions,
        })
        .await;
        (g, run)
    }

    async fn send(&self, msg: ToClient) {
        self.tx.send(msg).await.unwrap();
    }

    async fn output(&mut self, session: &str, text: String) {
        let log = self.logs.get_mut(session).unwrap();
        let seq = log.len() as u64 + 1;
        log.push(Chunk {
            seq,
            text: text.clone(),
        });
        self.send(ToClient::Output {
            session: session.into(),
            seq,
            text,
        })
        .await;
    }

    /// Game output for `session`, its prompts showing its HP and whether it rests.
    async fn tell(&mut self, session: &str, text: &str) {
        let t = match self.world.hp.get(session) {
            Some(hp) => text.replace("[HP=35", &format!("[HP={hp}")),
            None => text.to_owned(),
        };
        let t = if self.world.resting.contains(session) {
            t.replace("\x1b[0;37m]:", "\x1b[0;37m (Resting) ]:")
        } else {
            t
        };
        self.output(session, t).await;
    }

    /// Act on the brain's messages for `secs` seconds of game time.
    async fn play(&mut self, secs: u64) {
        let wait = async {
            loop {
                self.next().await;
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(secs), wait).await;
    }

    /// The game's answer to `text` from `session`, and what the other one sees.
    fn answer(&mut self, session: &str, text: &str) -> Vec<(String, String)> {
        let other = if session == LEADER { FOLLOWER } else { LEADER };
        let me = |t: String| vec![(session.to_owned(), t)];
        match text {
            "st" if self.world.mage && session == LEADER => me(mage_status()),
            "st" if self.world.cleric || self.world.mage => me(whole_status(session)),
            "spells" if self.world.mage && session == LEADER => me(format!(
                "You have the following spells:\r\nLevel Mana Short Spell Name\r\n  \
                 12  12    sped  speed\r\n{PROMPT}"
            )),
            t if t.starts_with("sped ") => match self.world.speed_refusal {
                Some(line) => me(said(line)),
                None => vec![
                    (
                        session.to_owned(),
                        said(&format!("You cast speed on {}.", first(other))),
                    ),
                    (
                        other.to_owned(),
                        said(&format!(
                            "{} casts speed on you.\r\nYou feel fast!",
                            first(session)
                        )),
                    ),
                ],
            },
            "st" => me(format!(
                "\x1b[0;32mName: \x1b[36m{:<25}\x1b[32mLives/CP:\x1b[36m     9/0\r\n{PROMPT}",
                name(session)
            )),
            "spells" if self.world.cleric && session == FOLLOWER => me(format!(
                "You have the following spells:\r\nLevel Mana Short Spell Name\r\n  \
                 1   2    mihe  minor healing\r\n{PROMPT}"
            )),
            t if t.starts_with("mihe ") => {
                let healed = *self.world.hp.get(other).unwrap_or(&35);
                self.world.hp.insert(other.to_owned(), (healed + 5).min(35));
                vec![
                    (
                        session.to_owned(),
                        said(&format!("You cast minor healing on {}!", first(other))),
                    ),
                    (
                        other.to_owned(),
                        said(&format!("{} casts minor healing on you!", first(session))),
                    ),
                ]
            }
            "look"
                if self.world.together
                    && self.world.rooms[session] == ARENA
                    && self.world.rooms[other] == ARENA =>
            {
                me(game::arena::arena(&[first(other)], 35))
            }
            "look" if self.world.dark.contains(&self.world.rooms[session]) => me(dark_room()),
            "look" => me(shown(self.world.rooms[session])),
            "i" if self.world.purse.contains_key(session) => {
                let list: Vec<String> = self.world.purse[session]
                    .iter()
                    .filter(|(_, n)| **n > 0)
                    .map(|(w, n)| format!("{n} {}", coin_name(w)))
                    .chain(["torch".to_owned()])
                    .collect();
                me(said(&format!("You are carrying {}", list.join(", "))))
            }
            t if t.starts_with("hide ") && self.world.purse.contains_key(session) => {
                let mut words = t.split_whitespace().skip(1);
                let n: u64 = words.next().unwrap().parse().unwrap();
                let word = words.next().unwrap();
                let purse = self.world.purse.get_mut(session).unwrap();
                let have = purse
                    .entry(STASH_WORDS.iter().find(|w| **w == word).unwrap())
                    .or_default();
                if *have < n {
                    me(said(&format!(
                        "You don't have {n} {} to hide!",
                        coin_name(word)
                    )))
                } else {
                    *have -= n;
                    me(said(&format!("You hid {n} {}.", coin_name(word))))
                }
            }
            "n" if self.world.locked && self.world.rooms[session] == ARENA => {
                me(said("The door is closed!"))
            }
            // A room command whose script waits (the Royal Palace kitchen's vortex, five
            // seconds), as quicksilver runs `adddelay`: the teleport at once and no room
            // shown, the character's lines held for the wait, and the party over the moment
            // one of them is teleported away from the other.
            t if game::atlas()
                .graph
                .edges(self.world.rooms[session])
                .iter()
                .any(|e| e.kind.is_phrase() && e.taken_by(t) && e.delay_secs > 0) =>
            {
                let atlas = game::atlas();
                let e = atlas
                    .graph
                    .edges(self.world.rooms[session])
                    .iter()
                    .find(|e| e.kind.is_phrase() && e.taken_by(t))
                    .unwrap();
                self.world.rooms.insert(session.to_owned(), e.to);
                self.world
                    .fired
                    .push((session.to_owned(), self.lines.len()));
                self.world.held_until.insert(
                    session.to_owned(),
                    tokio::time::Instant::now() + Duration::from_secs(u64::from(e.delay_secs)),
                );
                let mut out = me(said(
                    "You step into the swirling vortex, and find yourself... elsewhere.",
                ));
                if self.world.following {
                    self.world.following = false;
                    out.push((
                        FOLLOWER.to_owned(),
                        said(&format!("You are no longer following {}.", first(LEADER))),
                    ));
                }
                out
            }
            "borrow skiff" | "go skiff" | "row skiff"
                if self.world.rooms[session] == NEWHAVEN_DOCKS =>
            {
                // The skiff takes the one who typed it, follow or no follow.
                self.world.rooms.insert(session.to_owned(), SMALL_PIER);
                me(format!(
                    "\x1b[79D\x1b[K\x1b[0;37mYou climb into one of the skiffs, and row to Silvermere.\r\n{}",
                    shown(SMALL_PIER)
                ))
            }
            "bash n" if !self.world.locked => me(said("The door is already open.")),
            "bash n" if session == LEADER => vec![
                (
                    session.to_owned(),
                    said("Your attempts to bash through fail!"),
                ),
                (
                    other.to_owned(),
                    said("You see Penny attempt to bash the door to the north."),
                ),
            ],
            "bash n" => {
                self.world.locked = false;
                vec![
                    (session.to_owned(), said("You bashed the door open.")),
                    (
                        other.to_owned(),
                        said("You see Lauren bash the door to the north."),
                    ),
                ]
            }
            t if is_direction(t) && self.world.held.contains(session) => {
                me(said("You cannot move!"))
            }
            t if is_direction(t) && self.world.no_exit.contains(session) => {
                me(said("There is no exit in that direction!"))
            }
            t if is_direction(t) => {
                let here = self.world.rooms[session];
                let atlas = game::atlas();
                match atlas.graph.edges(here).iter().find(|e| e.command == t) {
                    Some(e) => {
                        let to = e.to;
                        self.world.rooms.insert(session.to_owned(), to);
                        let view = if self.world.dark.contains(&to) {
                            dark_room()
                        } else {
                            shown(to)
                        };
                        let mut out = me(view.clone());
                        if self.world.drag
                            && self.world.following
                            && session == LEADER
                            && self.world.rooms[other] == here
                        {
                            self.world.rooms.insert(other.to_owned(), to);
                            let dir = match t {
                                "n" => "north",
                                "s" => "south",
                                "e" => "east",
                                "w" => "west",
                                "ne" => "northeast",
                                "nw" => "northwest",
                                "se" => "southeast",
                                "sw" => "southwest",
                                "u" => "upwards",
                                "d" => "downwards",
                                t => t,
                            };
                            out.push((
                                other.to_owned(),
                                format!(" -- Following your Party leader {dir} -- \r\n{view}"),
                            ));
                        }
                        out
                    }
                    None => me(said("There is no exit in that direction!")),
                }
            }
            "par" => {
                let row = |s: &str| {
                    format!(
                        "  {:<30} (Witchunter)          [H:100%]   - {}\r\n",
                        name(s),
                        self.world.rank(s)
                    )
                };
                let screen = if self.world.following {
                    format!(
                        "The following people are in your travel party:\r\n{}{}",
                        row(FOLLOWER),
                        row(LEADER)
                    )
                } else {
                    format!(
                        "You are not in a party at the present time.\r\n{}",
                        row(session)
                    )
                };
                me(format!("{screen}{PROMPT}"))
            }
            t if t.starts_with("invite ") => {
                self.world.invited = true;
                self.world.ranks.insert(session.to_owned(), "Frontrank");
                vec![
                    (
                        session.to_owned(),
                        said(&format!("You have invited {} to follow you.", first(other))),
                    ),
                    (
                        other.to_owned(),
                        said(&format!(
                            "{} has invited you to follow her.",
                            first(session)
                        )),
                    ),
                ]
            }
            t if t.starts_with("follow ") => {
                if !self.world.invited {
                    return me(said("You must be invited first!"));
                }
                self.world.following = true;
                vec![
                    (
                        session.to_owned(),
                        said(&format!("You are now following {}", first(other))),
                    ),
                    (
                        other.to_owned(),
                        said(&format!("{} started to follow you.", first(session))),
                    ),
                ]
            }
            "frontrank" | "midrank" | "backrank" => {
                // The game's rules for two (`kb/grouping.md`): the leader never goes back, and
                // someone stays in front.
                let (rank, word, others) = match text {
                    "frontrank" => ("Frontrank", "front", "front rank"),
                    "midrank" => ("Midrank", "middle", "middle"),
                    _ => ("Backrank", "back", "back rank"),
                };
                if rank == "Backrank" && session == LEADER {
                    return me(said("You may not enter the backrank of your own party."));
                }
                if self.world.rank(session) == "Frontrank"
                    && rank != "Frontrank"
                    && self.world.rank(other) != "Frontrank"
                {
                    return me(said(
                        "You are the only one in the front rank.  You may not leave.",
                    ));
                }
                self.world.ranks.insert(session.to_owned(), rank);
                vec![
                    (
                        session.to_owned(),
                        said(&format!(
                            "You have moved to the {word} ranks of your group."
                        )),
                    ),
                    (
                        other.to_owned(),
                        said(&format!(
                            "{} just moved to the {others} in your group.",
                            first(session)
                        )),
                    ),
                ]
            }
            "help 1.20" if self.world.qs120 => me(said(
                "Quicksilver 1.20: how the rules differ from 1.11p\r\n\r\nBackstab",
            )),
            t if self.world.qs120 && t.starts_with("guard ") => {
                let ward = t.split_whitespace().nth(1).unwrap_or_default();
                if !first(other).eq_ignore_ascii_case(ward) {
                    return me(said("You don't see that player here."));
                }
                vec![
                    (
                        session.to_owned(),
                        said(&format!("You are now guarding {}.", first(other))),
                    ),
                    (
                        other.to_owned(),
                        said(&format!("{} is now guarding you.", first(session))),
                    ),
                ]
            }
            "sneak" if self.world.sneak_refusals > 0 => {
                self.world.sneak_refusals -= 1;
                me(said("You may not sneak right now!"))
            }
            "sneak" => me(said("Attempting to sneak...")),
            "leave" => {
                self.world.following = false;
                me(said(&format!(
                    "You are no longer following {}.",
                    first(other)
                )))
            }
            _ => me(PROMPT.to_owned()),
        }
    }

    /// Act on the brain's next message as the gateway and the game would.
    async fn next(&mut self) -> ToGateway {
        let msg = self.rx.recv().await.expect("the brain is running");
        self.seen.push(msg.clone());
        // Lines the game held (a room command's wait) whose hold is over: answered now.
        let now = tokio::time::Instant::now();
        let mut due = Vec::new();
        self.world.delayed.retain(|(s, t, at)| {
            if now >= *at {
                due.push((s.clone(), t.clone()));
                false
            } else {
                true
            }
        });
        for (s, t) in due {
            self.answer_now(&s, &t).await;
        }
        match &msg {
            ToGateway::HistoryRequest {
                session, after_seq, ..
            } => {
                let after = after_seq.unwrap_or(0);
                let chunks: Vec<Chunk> = self.logs[session]
                    .iter()
                    .filter(|c| c.seq > after)
                    .cloned()
                    .collect();
                let from_seq = chunks.first().map_or(1, |c| c.seq);
                self.send(ToClient::History {
                    session: session.clone(),
                    from_seq,
                    chunks,
                })
                .await;
            }
            ToGateway::InputLine { session, text } => {
                self.lines.push((session.clone(), text.clone()));
                let seq = self.logs[session].len() as u64;
                self.send(ToClient::Sent {
                    session: session.clone(),
                    seq,
                    text: text.clone(),
                })
                .await;
                match self.world.held_until.get(session).copied() {
                    Some(at) if now < at => {
                        self.world.delayed.push((session.clone(), text.clone(), at));
                    }
                    _ => self.answer_now(session, text).await,
                }
            }
            _ => {}
        }
        msg
    }

    /// The game's answer to `text` from `session`, told to each one as the game would, the
    /// line echoed before its answer.
    async fn answer_now(&mut self, session: &str, text: &str) {
        let answers = self.answer(session, text);
        for (i, (s, t)) in answers.into_iter().enumerate() {
            let t = if i == 0 { format!("{text}\r\n{t}") } else { t };
            self.tell(&s, &t).await;
        }
    }

    async fn until<T>(&mut self, mut want: impl FnMut(&ToGateway) -> Option<T>) -> T {
        let wait = async {
            loop {
                let m = self.next().await;
                if let Some(t) = want(&m) {
                    return t;
                }
            }
        };
        match tokio::time::timeout(Duration::from_secs(300), wait).await {
            Ok(t) => t,
            Err(_) => panic!("timed out; the brain sent {:?}", self.lines),
        }
    }

    async fn bot(&mut self, session: &str, action: BotAction) {
        self.send(ToClient::BotControl {
            session: session.into(),
            action,
        })
        .await;
    }

    /// Until a `parties` message where `session` is `state`.
    async fn member(&mut self, session: &str, state: MemberState) -> Vec<PartyStatus> {
        self.until(|m| match m {
            ToGateway::Parties { parties }
                if parties.iter().any(|p| {
                    p.members
                        .iter()
                        .any(|ms| ms.session == session && ms.state == state)
                }) =>
            {
                Some(parties.clone())
            }
            _ => None,
        })
        .await
    }
}

fn brain(store: Option<Store>) -> Brain {
    let clock = FixedClock(std::time::UNIX_EPOCH + Duration::from_secs(T0));
    let b = Brain::new(game::world(), Arc::new(clock), None);
    match store {
        Some(s) => b.with_store(s),
        None => b,
    }
}

fn party() -> PartyDef {
    PartyDef {
        id: String::new(),
        leader: LEADER.into(),
        followers: vec![FOLLOWER.into()],
        guards: Vec::new(),
    }
}

/// Both bots running (fight here) and the party saved.
async fn formed(g: &mut Gateway) {
    for s in [LEADER, FOLLOWER] {
        g.bot(s, BotAction::Start).await;
    }
    g.send(ToClient::PartySave { party: party() }).await;
    g.member(FOLLOWER, MemberState::Joined).await;
}

#[tokio::test(start_paused = true)]
async fn a_party_forms_up_in_one_room() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    let party: Vec<_> = g
        .lines
        .iter()
        .filter(|(_, l)| {
            ["par", "invite", "follow", "leave"]
                .iter()
                .any(|w| l.starts_with(w))
        })
        .map(|(s, l)| (s.as_str(), l.as_str()))
        .collect();
    // The leader looks first; the follower tries `follow`, is told to be invited, the leader
    // invites it by its in-game name, and it follows.
    assert_eq!(
        party[..4],
        [
            (LEADER, "par"),
            (FOLLOWER, "follow penny"),
            (LEADER, "invite lauren"),
            (FOLLOWER, "follow penny"),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_follower_does_not_run_a_loop_and_leaves_on_request() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    g.send(ToClient::RunLoop {
        session: FOLLOWER.into(),
        id: "NARNLOO2.mp".into(),
        skip: None,
    })
    .await;
    let refused = g
        .until(|m| match m {
            ToGateway::Warning { text, .. } if text.starts_with("run_loop") => Some(text.clone()),
            _ => None,
        })
        .await;
    assert_eq!(
        refused,
        "run_loop: following Penny: it goes where the party goes"
    );
    g.send(ToClient::PartyLeave {
        session: FOLLOWER.into(),
    })
    .await;
    // It leaves in the game, and the party, with no followers left, is gone.
    let (mut left, mut gone) = (false, false);
    g.until(|m| {
        match m {
            ToGateway::InputLine { session, text } if session == FOLLOWER && text == "leave" => {
                left = true;
            }
            ToGateway::Parties { parties } if parties.is_empty() => gone = true,
            _ => {}
        }
        (left && gone).then_some(())
    })
    .await;
}

/// The follower's next errand warning, or the notice that one started.
async fn follower_said(g: &mut Gateway) -> String {
    g.until(|m| match m {
        ToGateway::Warning { session, text } | ToGateway::Notice { session, text }
            if session.as_deref() == Some(FOLLOWER)
                && ["errand", "Train", "Equip"]
                    .iter()
                    .any(|w| text.starts_with(w)) =>
        {
            Some(text.clone())
        }
        _ => None,
    })
    .await
}

#[tokio::test(start_paused = true)]
async fn a_follower_equips_where_it_stands_in_the_party() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    g.bot(FOLLOWER, BotAction::Pause).await;
    g.bot(FOLLOWER, BotAction::EquipAll).await;
    let started = follower_said(&mut g).await;
    assert!(started.ends_with("started"), "{started}");
    assert!(
        !g.lines.iter().any(|(s, l)| s == FOLLOWER && l == "leave"),
        "{:?}",
        g.lines
    );
}

#[tokio::test(start_paused = true)]
async fn a_follower_sent_to_train_leaves_the_party_first() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    g.bot(FOLLOWER, BotAction::Pause).await;
    g.bot(FOLLOWER, BotAction::Train).await;
    // It leaves in the game and the party, its only follower gone, is deleted; the errand is
    // then its own, not refused for following.
    let (mut left, mut gone, mut said) = (false, false, None);
    g.until(|m| {
        match m {
            ToGateway::InputLine { session, text } if session == FOLLOWER && text == "leave" => {
                left = true;
            }
            ToGateway::Parties { parties } if parties.is_empty() => gone = true,
            ToGateway::Warning { session, text } | ToGateway::Notice { session, text }
                if session.as_deref() == Some(FOLLOWER)
                    && (text.starts_with("Train") || text.starts_with("errand")) =>
            {
                said = Some(text.clone());
            }
            _ => {}
        }
        (left && gone && said.is_some()).then_some(())
    })
    .await;
    let said = said.unwrap();
    assert!(said.starts_with("Train"), "{said}");
}

#[tokio::test(start_paused = true)]
async fn parties_survive_a_restart_and_a_formed_party_is_only_checked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (store, _) = Store::open(&path).unwrap();
    let (mut g, run) = Gateway::start(brain(Some(store))).await;
    formed(&mut g).await;
    drop(g);
    let _ = run.await;

    let (store, _) = Store::open(&path).unwrap();
    let (mut g, _run) = Gateway::start(brain(Some(store))).await;
    // The game still has them in a party: the new brain checks with `par` and invites no one.
    g.world.invited = true;
    g.world.following = true;
    for s in [LEADER, FOLLOWER] {
        g.bot(s, BotAction::Start).await;
    }
    g.send(ToClient::PartiesRequest).await;
    let parties = g.member(LEADER, MemberState::Joined).await;
    assert_eq!(parties[0].party.id, "party-1");
    assert_eq!(parties[0].party.followers, [FOLLOWER]);
    g.member(FOLLOWER, MemberState::Joined).await;
    assert!(
        !g.lines.iter().any(|(_, l)| l.starts_with("invite")),
        "{:?}",
        g.lines
    );
}

#[tokio::test(start_paused = true)]
async fn a_follower_the_game_dropped_walks_back_and_joins_again() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    // The player walks Penny up out of the Arena; the game drops Lauren without a word (an
    // exit she could not take, say), so she stays behind.
    g.world.following = false;
    let up = game::atlas()
        .graph
        .edges(ARENA)
        .iter()
        .find(|e| e.to == NARROW_ROAD)
        .expect("the way up")
        .command
        .clone();
    g.world.rooms.insert(LEADER.to_owned(), NARROW_ROAD);
    let seq = g.logs[LEADER].len() as u64;
    g.send(ToClient::Sent {
        session: LEADER.into(),
        seq,
        text: up.clone(),
    })
    .await;
    g.output(LEADER, format!("{up}\r\n{}", shown(NARROW_ROAD)))
        .await;
    // The leader's `par` no longer lists her: separated, and she walks back.
    g.member(FOLLOWER, MemberState::Separated).await;
    g.until(|m| {
        matches!(m, ToGateway::InputLine { session, text } if session == FOLLOWER && *text == up)
            .then_some(())
    })
    .await;
    g.member(FOLLOWER, MemberState::Joined).await;
    assert_eq!(g.world.rooms[FOLLOWER], NARROW_ROAD);
    let back: Vec<_> = g
        .lines
        .iter()
        .filter(|(s, l)| s == FOLLOWER && (l.starts_with("follow") || *l == up))
        .map(|(_, l)| l.as_str())
        .collect();
    assert_eq!(back[back.len() - 2..], [up.as_str(), "follow penny"]);
}

#[tokio::test(start_paused = true)]
async fn a_resync_checks_at_once_and_forms_a_party_the_game_broke() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    ranked(&mut g, FOLLOWER, PartyRank::Front).await;
    // The game dropped Lauren without a word, in the same room: the next routine `par` is
    // up to half a minute off. Out of the party, she is back at Midrank.
    g.world.following = false;
    g.world.ranks.remove(FOLLOWER);
    let before = g.lines.len();
    let at = tokio::time::Instant::now();
    g.send(ToClient::PartyResync { party: None }).await;
    g.until(|m| {
        matches!(m, ToGateway::Notice { session: None, text } if text == "Re-sync: the parties are forming up again")
            .then_some(())
    })
    .await;
    g.member(FOLLOWER, MemberState::NotJoined).await;
    g.member(FOLLOWER, MemberState::Joined).await;
    assert!(at.elapsed() < Duration::from_secs(5), "{:?}", at.elapsed());
    let after: Vec<_> = g.lines[before..]
        .iter()
        .map(|(s, l)| (s.as_str(), l.as_str()))
        .collect();
    assert!(after.contains(&(LEADER, "par")), "{after:?}");
    assert!(after.contains(&(FOLLOWER, "par")), "{after:?}");
    assert!(after.contains(&(FOLLOWER, "follow penny")), "{after:?}");
    // And put back in her rank.
    ranked(&mut g, FOLLOWER, PartyRank::Front).await;
    let after: Vec<_> = g.lines[before..]
        .iter()
        .map(|(s, l)| (s.as_str(), l.as_str()))
        .collect();
    assert!(after.contains(&(FOLLOWER, "frontrank")), "{after:?}");
    assert!(at.elapsed() < Duration::from_secs(15), "{:?}", at.elapsed());
    // A party that is not there.
    g.send(ToClient::PartyResync {
        party: Some("party-9".into()),
    })
    .await;
    g.until(|m| {
        matches!(m, ToGateway::Warning { text, .. } if text.contains("no party \"party-9\""))
            .then_some(())
    })
    .await;
}

/// Until a `parties` message where `session` has `rank`.
async fn ranked(g: &mut Gateway, session: &str, rank: PartyRank) {
    g.until(|m| match m {
        ToGateway::Parties { parties }
            if parties.iter().any(|p| {
                p.members
                    .iter()
                    .any(|ms| ms.session == session && ms.rank == Some(rank))
            }) =>
        {
            Some(())
        }
        _ => None,
    })
    .await;
}

#[tokio::test(start_paused = true)]
async fn each_member_takes_its_party_rank_front_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let b = brain(None).with_settings_dir(wayfarer_settings::SettingsDir::new(dir.path()));
    let (mut g, _run) = Gateway::start(b).await;
    formed(&mut g).await;
    // Joined at Midrank, the follower moves to the front, beside the leader the invite put
    // there.
    ranked(&mut g, FOLLOWER, PartyRank::Front).await;
    let leader = g.seen.iter().rev().find_map(|m| match m {
        ToGateway::Parties { parties } => parties[0]
            .members
            .iter()
            .find(|ms| ms.session == LEADER)
            .map(|ms| ms.rank),
        _ => None,
    });
    assert_eq!(leader, Some(Some(PartyRank::Front)));
    // Set to the back on its character: it moves there.
    g.send(ToClient::SettingsSave {
        session: FOLLOWER.into(),
        layer: "character".into(),
        changes: vec![wayfarer_proto::SettingChange {
            key: "party.rank".into(),
            value: Some(wayfarer_proto::SettingValue::Text("back".into())),
        }],
    })
    .await;
    ranked(&mut g, FOLLOWER, PartyRank::Back).await;
    let sent: Vec<_> = g
        .lines
        .iter()
        .filter(|(_, l)| l.ends_with("rank"))
        .map(|(s, l)| (s.as_str(), l.as_str()))
        .collect();
    assert_eq!(sent, [(FOLLOWER, "frontrank"), (FOLLOWER, "backrank")]);
    // The leader set to the back: it stays in front, and its tile says why, once.
    g.send(ToClient::SettingsSave {
        session: LEADER.into(),
        layer: "character".into(),
        changes: vec![wayfarer_proto::SettingChange {
            key: "party.rank".into(),
            value: Some(wayfarer_proto::SettingValue::Text("back".into())),
        }],
    })
    .await;
    g.until(|m| {
        matches!(m, ToGateway::Warning { session: Some(s), text } if s == LEADER
            && text == "Penny stays in the front rank: a party's leader cannot go to the back rank")
        .then_some(())
    })
    .await;
    assert!(
        !g.lines
            .iter()
            .any(|(s, l)| s == LEADER && l.ends_with("rank"))
    );
}

/// A party rest has every member below its rest-until line rest (C-045): the follower at 71%
/// (rest below 60, until 85) rests for the leader at 40%.
#[tokio::test(start_paused = true)]
async fn a_follower_between_its_lines_rests_with_the_party() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    for (s, hp) in [(LEADER, 14), (FOLLOWER, 25)] {
        g.world.hp.insert(s.to_owned(), hp);
        g.output(
            s,
            said(&format!("Health:    {hp}/35    [{}%]", hp * 100 / 35))
                .replace("[HP=35", &format!("[HP={hp}")),
        )
        .await;
    }
    g.until(|m| match m {
        ToGateway::InputLine { session, text } if session == FOLLOWER && text == "rest" => Some(()),
        _ => None,
    })
    .await;
    // The leader's Recover has it too: it walks the party out of the Arena (it spawns) to
    // rest.
    assert!(
        g.lines
            .iter()
            .any(|(s, t)| s == LEADER && (t == "rest" || t == "u")),
        "{:?}",
        g.lines
    );
}

/// A follower with Sneak on sneaks (C-048), and once sneaking does not again; the leader,
/// standing in the Arena, does not: it sneaks only before it moves.
#[tokio::test(start_paused = true)]
async fn a_sneaking_follower_sneaks_with_the_party() {
    let over = wayfarer_settings::Overrides::parse("[movement]\nsneak = true\n").unwrap();
    let settings =
        wayfarer_settings::Settings::resolve(&[(wayfarer_settings::Layer::Character, &over)])
            .unwrap();
    let (mut g, _run) = Gateway::start(brain(None).with_settings(settings)).await;
    formed(&mut g).await;
    let wait = async {
        loop {
            g.next().await;
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(30), wait).await;
    let sneaks: Vec<_> = g.lines.iter().filter(|(_, t)| t == "sneak").collect();
    assert_eq!(sneaks, [&(FOLLOWER.to_owned(), "sneak".to_owned())]);
}

/// A follower with its leader bashes the locked door the leader is bashing (MegaMud's *Help
/// Leader Bash Doors*), once it gives way no more, and the leader walks through.
#[tokio::test(start_paused = true)]
async fn a_follower_helps_the_leader_bash_a_door() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    g.world.locked = true;
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: BEYOND_ARENA.0,
        room: BEYOND_ARENA.1,
        stop_short: None,
        fight: None,
    })
    .await;
    // The leader's first `n` finds the door locked; its second goes through.
    let mut tries = 0;
    g.until(|m| match m {
        ToGateway::InputLine { session, text } if session == LEADER && text == "n" => {
            tries += 1;
            (tries == 2).then_some(())
        }
        _ => None,
    })
    .await;
    let bashes: Vec<_> = g
        .lines
        .iter()
        .filter(|(_, t)| t == "bash n")
        .map(|(s, _)| s.as_str())
        .collect();
    // Lauren's one bash opens it; Penny's next finds it open.
    assert_eq!(
        bashes.iter().filter(|s| **s == FOLLOWER).count(),
        1,
        "{:?}",
        g.lines
    );
    assert!(!g.world.locked);
    assert_eq!(bashes.last(), Some(&LEADER), "{:?}", g.lines);
}

/// With *Help the leader bash doors* off, the follower leaves the door to the leader.
#[tokio::test(start_paused = true)]
async fn a_follower_does_not_bash_with_help_off() {
    let over = wayfarer_settings::Overrides::parse("[party]\nhelp_bash = false\n").unwrap();
    let settings =
        wayfarer_settings::Settings::resolve(&[(wayfarer_settings::Layer::Character, &over)])
            .unwrap();
    let (mut g, _run) = Gateway::start(brain(None).with_settings(settings)).await;
    formed(&mut g).await;
    g.world.locked = true;
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: BEYOND_ARENA.0,
        room: BEYOND_ARENA.1,
        stop_short: None,
        fight: None,
    })
    .await;
    g.until(|m| match m {
        ToGateway::Warning { session, text }
            if session.as_deref() == Some(LEADER) && text.starts_with("Stopped walking") =>
        {
            Some(())
        }
        _ => None,
    })
    .await;
    let bashes: Vec<_> = g.lines.iter().filter(|(_, t)| t == "bash n").collect();
    assert!(bashes.iter().all(|(s, _)| s == LEADER), "{:?}", g.lines);
    // As many as the door's odds call for (C-091); the leader's strength is not known here.
    assert_eq!(bashes.len(), arena_door_bashes(false) as usize);
}

/// The follower's casts of minor healing on the leader so far.
fn heals(g: &Gateway) -> usize {
    g.lines
        .iter()
        .filter(|(s, t)| s == FOLLOWER && t == "mihe penny")
        .count()
}

/// John on the live rig (2026-10-26): Faye, the leader, rested to 80% on her own (no party
/// rest: she was over her rest line), and Lauren, a Cleric with her, never healed her: out of
/// a party rest a member was healed only below the heal-a-member line (60%). A member resting
/// on its own is healed up to its rest-until line, as in a party rest; one standing is not.
#[tokio::test(start_paused = true)]
async fn a_healer_heals_a_member_resting_on_its_own_up_to_its_rest_until_line() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    g.world.cleric = true;
    formed(&mut g).await;
    // 25 of 35 is 71%: over the leader's rest line (60) and the heal-a-member line (60), under
    // her rest-until line (85). Standing, she is not healed.
    g.world.hp.insert(LEADER.to_owned(), 25);
    g.tell(LEADER, &said("Health:    25/35    [71%]")).await;
    g.play(30).await;
    assert_eq!(heals(&g), 0, "{:?}", g.lines);
    // Resting: the Cleric heals her, once (5 of minor healing takes her to 30, 85%).
    g.world.resting.insert(LEADER.to_owned());
    g.tell(LEADER, &said("You are now resting.")).await;
    g.until(|m| match m {
        ToGateway::InputLine { session, text } if session == FOLLOWER && text == "mihe penny" => {
            Some(())
        }
        _ => None,
    })
    .await;
    g.play(30).await;
    assert_eq!(heals(&g), 1, "{:?}", g.lines);
    assert!(
        g.seen
            .iter()
            .any(|m| matches!(m, ToGateway::Notice { session: Some(s), text }
            if s == FOLLOWER && text == "Recover: casting minor healing on penny (resting)")),
        "{:?}",
        g.seen
    );
}

/// GUARD (C-068): on a 1.20 server the leader guards the follower once the party is formed,
/// and the parties message says who guards whom. On a 1.11p server nothing is sent.
#[tokio::test(start_paused = true)]
async fn on_1_20_the_leader_guards_the_follower_it_was_told_to() {
    for qs120 in [true, false] {
        let (mut g, _run) = Gateway::start(brain(None)).await;
        g.world.qs120 = qs120;
        let mut def = party();
        def.guards = vec![wayfarer_proto::GuardPair {
            guard: LEADER.into(),
            ward: FOLLOWER.into(),
        }];
        for s in [LEADER, FOLLOWER] {
            g.bot(s, BotAction::Start).await;
        }
        g.send(ToClient::PartySave { party: def }).await;
        g.member(FOLLOWER, MemberState::Joined).await;
        g.play(90).await;
        let guards: Vec<_> = g
            .lines
            .iter()
            .filter(|(_, l)| l.starts_with("guard"))
            .map(|(s, l)| (s.as_str(), l.as_str()))
            .collect();
        if !qs120 {
            assert!(guards.is_empty(), "1.11p: {guards:?}");
            continue;
        }
        assert_eq!(guards, [(LEADER, "guard Lauren")]);
        let status = g
            .seen
            .iter()
            .rev()
            .find_map(|m| match m {
                ToGateway::Parties { parties } => Some(parties.clone()),
                _ => None,
            })
            .expect("a parties message");
        let ms = |s: &str| {
            status[0]
                .members
                .iter()
                .find(|m| m.session == s)
                .cloned()
                .unwrap()
        };
        assert_eq!(ms(LEADER).guarding.as_deref(), Some("Lauren"));
        assert_eq!(ms(FOLLOWER).guarded_by.as_deref(), Some("Penny"));
    }
}

/// The notices a session's brain said, in order.
/// The brain's warnings for `session`'s tile.
fn warnings_of(g: &Gateway, session: &str) -> Vec<String> {
    g.seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Warning {
                session: Some(s),
                text,
                ..
            } if s == session => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn notices_of(g: &Gateway, session: &str) -> Vec<String> {
    g.seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Notice {
                session: Some(s),
                text,
                ..
            } if s == session => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// C-074, Sasha on the Thrag run: a follower in the party whose leader the beliefs put in
/// another room (as around a move), under its flee line and attacked, runs nowhere on its
/// own: it says it asks the leader to run, and fights on.
#[tokio::test(start_paused = true)]
async fn a_follower_in_the_party_never_flees_on_its_own() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    // The leader's belief moves up the stairs; the follower's stays in the Arena, which
    // spawns, so its Flee would have something to run from. The game moved her too (a `look`
    // shows it): only her belief lags, and a follower left behind is another case (C-116).
    g.output(LEADER, shown(NARROW_ROAD)).await;
    g.world.rooms.insert(FOLLOWER.to_owned(), NARROW_ROAD);
    g.world.hp.insert(FOLLOWER.to_owned(), 6);
    let lines_before = g.lines.len();
    // Its maximum HP, as the rest test gives it; then a blow at 6 of 35.
    g.output(
        FOLLOWER,
        said("Health:    6/35    [17%]").replace("[HP=35", "[HP=6"),
    )
    .await;
    g.tell(
        FOLLOWER,
        &said("The large giant rat lunges at you for 3 damage!"),
    )
    .await;
    g.play(15).await;
    let moved: Vec<&String> = g.lines[lines_before..]
        .iter()
        .filter(|(s, t)| s == FOLLOWER && is_direction(t))
        .map(|(_, t)| t)
        .collect();
    assert!(moved.is_empty(), "the follower moved: {moved:?}");
    let said = notices_of(&g, FOLLOWER);
    assert!(
        said.iter()
            .any(|n| n == "Under my flee line: asking Penny to run"),
        "{said:?}"
    );
    assert!(!said.iter().any(|n| n.starts_with("Fleeing")), "{said:?}");
}

/// C-089, Lexi in the Dragon's Teeth Hills: a leader on a Go to holds while a member walks
/// back, and the hold is not a walk that ended elsewhere. Each tick of the hold had counted
/// one, so three ticks in a second ended the Go to ("3 walks ... ended elsewhere") and the
/// party stood where it was.
#[tokio::test(start_paused = true)]
async fn a_go_to_waits_for_a_member_walking_back_and_goes_on() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    // The game drops Lauren in the village; she walks back to the Arena.
    g.world.following = false;
    let far = VILLAGE_ENTRANCE;
    g.world.rooms.insert(FOLLOWER.to_owned(), far);
    g.output(FOLLOWER, shown(far)).await;
    g.member(FOLLOWER, MemberState::Separated).await;
    let to = NARROW_ROAD;
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: to.0,
        room: to.1,
        stop_short: None,
        fight: None,
    })
    .await;
    // She walks back meanwhile, and the leader's hold ticks while she does.
    g.play(60).await;
    let said = notices_of(&g, LEADER);
    assert!(
        !said.iter().any(|n| n.contains("could not get there")),
        "{said:#?}"
    );
    assert!(said.iter().any(|n| n.contains("to catch up")), "{said:#?}");
    assert_eq!(g.world.rooms[LEADER], to, "{said:#?}");
}

/// C-089: a Go to waits for a member out of the party no longer than
/// `party.wait_separated_minutes` (5 by default), then ends where the leader is.
#[tokio::test(start_paused = true)]
async fn a_go_to_ends_when_a_member_does_not_catch_up_in_time() {
    // Over a second: six minutes of the paused clock, the leader's `par` every few seconds.
    let (mut g, _run) = Gateway::start(brain(None)).await;
    formed(&mut g).await;
    // The game drops Lauren in the village, and she cannot move: she does not walk back.
    g.world.following = false;
    g.world.held.insert(FOLLOWER.to_owned());
    g.world.rooms.insert(FOLLOWER.to_owned(), VILLAGE_ENTRANCE);
    g.output(FOLLOWER, shown(VILLAGE_ENTRANCE)).await;
    g.member(FOLLOWER, MemberState::Separated).await;
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: NARROW_ROAD.0,
        room: NARROW_ROAD.1,
        stop_short: None,
        fight: None,
    })
    .await;
    g.play(4 * 60).await;
    let said = notices_of(&g, LEADER);
    assert!(
        !said.iter().any(|n| n.starts_with("Not going")),
        "{said:#?}"
    );
    g.play(2 * 60).await;
    let warned: Vec<String> = g
        .seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Warning {
                session: Some(s),
                text,
            } if s == LEADER => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        warned.iter().any(|w| w
            == "Not going to Newhaven, Narrow Road: Lauren did not catch up in 5 minutes; playing here"),
        "{warned:#?} {:#?}",
        notices_of(&g, LEADER)
    );
    assert_eq!(g.world.rooms[LEADER], ARENA);
}

/// A formed party in the Arena with the coins of `purses`, the leader's loop started there:
/// its first stop. Runs until each of `until` has said the narration it starts with.
async fn stash_round(together: bool, until: &[(&str, &str)]) -> Gateway {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    g.world.together = together;
    g.world.purse.insert(LEADER.into(), [("gold", 7)].into());
    g.world.purse.insert(
        FOLLOWER.into(),
        [("silver", 12), ("gold", 3), ("platinum", 2)].into(),
    );
    formed(&mut g).await;
    for (s, coins) in [
        (LEADER, "7 gold crowns"),
        (
            FOLLOWER,
            "12 silver nobles, 3 gold crowns, 2 platinum pieces",
        ),
    ] {
        g.output(s, format!("You are carrying {coins}\r\n{PROMPT}"))
            .await;
    }
    g.send(ToClient::RunLoop {
        session: LEADER.into(),
        id: "1:2140:8:doors:0".into(),
        skip: Some(vec![
            wayfarer_proto::RoomKey {
                map: BEYOND_ARENA.0,
                room: BEYOND_ARENA.1,
            },
            wayfarer_proto::RoomKey { map: 1, room: 2153 },
        ]),
    })
    .await;
    // The fake game's loop has no respawn clock: it would go round and round on the paused
    // clock, so the test stops once each has said what it waits for.
    let mut said = BTreeSet::new();
    let mut lines = 0;
    let done = g
        .until(|m| match m {
            // A spinning loop never moves the paused clock: give up after this many lines.
            ToGateway::InputLine { .. } => {
                lines += 1;
                (lines > 300).then_some(false)
            }
            ToGateway::Notice {
                session: Some(session),
                text,
            } if until
                .iter()
                .any(|(s, t)| s == session && text.starts_with(t)) =>
            {
                said.insert(session.clone());
                (said.len() == until.len()).then_some(true)
            }
            _ => None,
        })
        .await;
    assert!(
        done,
        "leader: {:?}\nfollower: {:?}",
        notices_of(&g, LEADER),
        notices_of(&g, FOLLOWER)
    );
    g
}

fn hides(g: &Gateway, s: &str) -> Vec<String> {
    g.lines
        .iter()
        .filter(|(who, l)| who == s && l.starts_with("hide"))
        .map(|(_, l)| l.clone())
        .collect()
}

/// C-092, C-093: a leader whose loop reaches its first stop, sure of it, hides its silver,
/// gold and copper there, and its follower beside it, with the leader in sight, hides its own
/// too, once. Each reads its coins with `i` first.
#[tokio::test(start_paused = true)]
async fn at_the_leaders_first_stop_every_member_with_it_hides_its_coins() {
    let g = stash_round(true, &[(LEADER, "Stash: done"), (FOLLOWER, "Stash: done")]).await;
    assert_eq!(hides(&g, LEADER), ["hide 7 gold"], "{:?}", g.lines);
    assert_eq!(
        hides(&g, FOLLOWER),
        ["hide 12 silver", "hide 3 gold"],
        "{:?}",
        g.lines
    );
    for s in [LEADER, FOLLOWER] {
        let mine: Vec<&str> = g
            .lines
            .iter()
            .filter(|(who, _)| who == s)
            .map(|(_, l)| l.as_str())
            .collect();
        let first_hide = mine.iter().position(|l| l.starts_with("hide")).unwrap();
        assert_eq!(mine[first_hide - 1], "i", "{s}: {mine:?}");
    }
    assert!(g.world.purse[FOLLOWER]["platinum"] == 2);
    // The leader stays until its follower has read its coins after hiding them: the game would
    // take the follower along, to hide them a room on.
    let at = |who: &str, what: &dyn Fn(&str) -> bool| {
        g.lines.iter().position(|(s, l)| s == who && what(l))
    };
    let follower_hid = g
        .lines
        .iter()
        .rposition(|(s, l)| s == FOLLOWER && l.starts_with("hide"))
        .unwrap();
    let follower_read = follower_hid
        + g.lines[follower_hid..]
            .iter()
            .position(|(s, l)| s == FOLLOWER && l == "i")
            .unwrap();
    if let Some(moved) = at(LEADER, &|l| is_direction(l)) {
        assert!(moved > follower_read, "{:?}", g.lines);
    }
    assert!(
        notices_of(&g, LEADER)
            .iter()
            .any(|n| n == "Waiting for Lauren to hide their coins"),
        "{:?}",
        notices_of(&g, LEADER)
    );
}

/// C-093: a follower that does not see the leader in its room hides nothing, and says why.
#[tokio::test(start_paused = true)]
async fn a_follower_that_does_not_see_its_leader_hides_nothing() {
    let g = stash_round(
        false,
        &[
            (LEADER, "Stash: done"),
            (FOLLOWER, "Stash: not sure this is"),
        ],
    )
    .await;
    assert_eq!(hides(&g, LEADER), ["hide 7 gold"], "{:?}", g.lines);
    assert!(hides(&g, FOLLOWER).is_empty(), "{:?}", g.lines);
}

/// C-104, Riley and Asa on 2026-11-16: a one-stop loop's cleared stop is stepped out of and
/// back into. When the step out did not go (a fight stopped it, or the party's hold for a
/// member's stash), the leader, still in the stop, took itself for arriving there again: a new
/// stash round, the stop "cleared" again and a hold again, many times a second. A round is one
/// per arrival from elsewhere.
#[tokio::test(start_paused = true)]
async fn a_step_out_that_does_not_go_starts_no_new_stash_round() {
    let store = Store::in_memory().unwrap();
    let (mut g, _run) = Gateway::start(brain(Some(store))).await;
    g.world.together = true;
    g.world.drag = true;
    formed(&mut g).await;
    g.send(ToClient::SaveLoop {
        session: LEADER.into(),
        name: "Road".into(),
        base: None,
        stops: vec![wayfarer_proto::RoomKey {
            map: NARROW_ROAD.0,
            room: NARROW_ROAD.1,
        }],
    })
    .await;
    g.send(ToClient::RunLoop {
        session: LEADER.into(),
        id: "custom:road".into(),
        skip: None,
    })
    .await;
    g.until(|m| match m {
        ToGateway::Notice {
            session: Some(s),
            text,
        } if s == LEADER && text == "Arrived: Newhaven, Narrow Road" => Some(()),
        _ => None,
    })
    .await;
    // Held there: its step out does not go.
    g.world.held.insert(LEADER.to_owned());
    let mut notices = 0;
    let mut rounds = 0;
    g.until(|m| match m {
        ToGateway::Notice {
            session: Some(s),
            text,
        } if s == LEADER => {
            notices += 1;
            if text.starts_with("Stash:") {
                rounds += 1;
            }
            (notices > 60).then_some(())
        }
        _ => None,
    })
    .await;
    let said = notices_of(&g, LEADER);
    // The arrival's round, and no other.
    assert_eq!(rounds, 1, "{said:#?}");
}

/// C-121: the game's follow moves a follower through directions only. The leader's Go to
/// from the Arena to the Small Pier ends with the docks' skiff (`borrow skiff`, a Text exit),
/// which took the leader alone and left its followers behind (the Royal Palace kitchen's
/// `go vortex` left Riley's party wandering the Black Wasteland, 2026-11-19). Now the follower
/// with the leader types the same phrase as soon as the leader does, then looks where it
/// took it, and both end up on the pier.
#[tokio::test(start_paused = true)]
async fn a_follower_takes_a_room_command_exit_with_its_leader() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    g.world.together = true;
    g.world.drag = true;
    formed(&mut g).await;
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: SMALL_PIER.0,
        room: SMALL_PIER.1,
        stop_short: None,
        fight: None,
    })
    .await;
    g.until(|m| match m {
        ToGateway::InputLine { session, text } if session == FOLLOWER && text == "borrow skiff" => {
            Some(())
        }
        _ => None,
    })
    .await;
    g.play(5).await;
    assert_eq!(g.world.rooms[LEADER], SMALL_PIER, "{:?}", g.lines);
    assert_eq!(g.world.rooms[FOLLOWER], SMALL_PIER, "{:?}", g.lines);
    // The leader's phrase, then the follower's, then the follower's look.
    let skiff: Vec<&str> = g
        .lines
        .iter()
        .filter(|(_, t)| t == "borrow skiff")
        .map(|(s, _)| s.as_str())
        .collect();
    assert_eq!(skiff, [LEADER, FOLLOWER], "{:?}", g.lines);
    let after: Vec<&str> = g
        .lines
        .iter()
        .skip_while(|(s, t)| !(s == FOLLOWER && t == "borrow skiff"))
        .skip(1)
        .filter(|(s, _)| s == FOLLOWER)
        .map(|(_, t)| t.as_str())
        .collect();
    assert_eq!(
        after.first(),
        Some(&"look"),
        "{after:?} / all: {:?}",
        g.lines
    );
    // The leader's tile says who went with it.
    assert!(
        notices_of(&g, LEADER)
            .iter()
            .any(|n| n == "`borrow skiff` with the party: Lauren"),
        "{:?}",
        notices_of(&g, LEADER)
    );
}

/// The Royal Palace kitchen (3:668): `go vortex` there is a room command whose script holds
/// the character for five seconds (`adddelay 5`) as it teleports it to the Black Wasteland
/// (3:740), whose southeast is 3:719.
const PALACE_KITCHEN: Key = (3, 668);
const WASTELAND_ON: Key = (3, 719);

/// C-121, the vortices (live, 2026-11-25 and 26): the game teleports at the phrase, holds the
/// character's lines for five seconds and drops it from the party; the room comes with the
/// look after. The follower types the phrase once, with the leader; nobody looks or judges
/// before the hold is over; the follower is not walked back through the vortex; the leader
/// waits on the far side instead of walking on alone (Sasha went four rooms on); and the party
/// forms up there and goes on together.
#[tokio::test(start_paused = true)]
async fn a_follower_crosses_a_delayed_room_command_with_its_leader_who_waits_for_it() {
    let (mut g, _run) = Gateway::start_at(brain(None), PALACE_KITCHEN).await;
    g.world.drag = true;
    formed(&mut g).await;
    // Through the vortex's room (a boss lair, the Lord of the Hunt: walked into, not stopped
    // short of) and one room on.
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: WASTELAND_ON.0,
        room: WASTELAND_ON.1,
        stop_short: Some(false),
        fight: None,
    })
    .await;
    g.play(30).await;
    assert_eq!(
        g.world.rooms[LEADER],
        WASTELAND_ON,
        "lines {:?}\nwarnings {:?}\nnotices {:?}",
        g.lines,
        warnings_of(&g, LEADER),
        notices_of(&g, LEADER)
    );
    assert_eq!(g.world.rooms[FOLLOWER], WASTELAND_ON, "{:?}", g.lines);
    assert!(g.world.following, "{:?}", g.lines);
    // Each typed the phrase once, the leader first.
    let vortex: Vec<&str> = g
        .lines
        .iter()
        .filter(|(_, t)| t == "go vortex")
        .map(|(s, _)| s.as_str())
        .collect();
    assert_eq!(vortex, [LEADER, FOLLOWER], "{:?}", g.lines);
    // The looks were typed after the teleport (the hold answers them when it ends).
    for s in [LEADER, FOLLOWER] {
        let fired = g
            .world
            .fired
            .iter()
            .find(|(f, _)| f == s)
            .map(|(_, n)| *n)
            .unwrap();
        let phrase = g
            .lines
            .iter()
            .position(|(f, t)| f == s && t == "go vortex")
            .unwrap();
        let look = g.lines[phrase..]
            .iter()
            .position(|(f, t)| f == s && t == "look")
            .map(|i| phrase + i)
            .unwrap_or_else(|| panic!("{s} never looked: {:?}", g.lines));
        assert!(
            look >= fired,
            "{s} looked at line {look}, teleported at {fired}: {:?}",
            g.lines
        );
    }
    // The leader moved on only once the follower had looked and was following again.
    let phrase = g
        .lines
        .iter()
        .position(|(f, t)| f == FOLLOWER && t == "go vortex")
        .unwrap();
    let after = |who: &str, what: &str| {
        g.lines[phrase..]
            .iter()
            .position(|(f, t)| f == who && t == what)
            .map(|i| phrase + i)
            .unwrap_or_else(|| panic!("{who} never sent {what:?} after the phrase: {:?}", g.lines))
    };
    let (look, follow, on) = (
        after(FOLLOWER, "look"),
        after(FOLLOWER, "follow penny"),
        after(LEADER, "se"),
    );
    assert!(
        look < on && follow < on,
        "the leader went on at line {on}, the follower looked at {look} and followed at {follow}: {:?}",
        g.lines
    );
    // Meanwhile the follower's tile said so.
    assert!(
        g.seen
            .iter()
            .any(|m| matches!(m, ToGateway::Parties { parties }
            if parties.iter().any(|p| p.members.iter().any(|ms| ms.session == FOLLOWER
                && ms.detail.as_deref() == Some("going through `go vortex`"))))),
        "{:?}",
        g.lines
    );
    // Nobody walked back.
    let notices: Vec<String> = [LEADER, FOLLOWER]
        .iter()
        .flat_map(|s| notices_of(&g, s))
        .collect();
    assert!(
        !notices.iter().any(|n| n.starts_with("Walking back")),
        "{notices:?}"
    );
}

/// C-122: a follower refused its sneak right after a kill (`You may not sneak right now!`:
/// the game still counting the fight) tries again two seconds later, not five, so it is not
/// dragged through rooms unsneaked behind its leader.
#[tokio::test(start_paused = true)]
async fn a_follower_refused_a_sneak_tries_again_within_two_seconds() {
    let over = wayfarer_settings::Overrides::parse("[movement]\nsneak = true\n").unwrap();
    let settings =
        wayfarer_settings::Settings::resolve(&[(wayfarer_settings::Layer::Character, &over)])
            .unwrap();
    let (mut g, _run) = Gateway::start(brain(None).with_settings(settings)).await;
    formed(&mut g).await;
    g.play(5).await;
    let before = g.lines.iter().filter(|(_, t)| t == "sneak").count();
    // The sneak ends (a noticed move, say); the next attempt is refused, the one after takes.
    g.world.sneak_refusals = 1;
    g.output(FOLLOWER, said("You are no longer sneaking."))
        .await;
    let mut sneaks = 0;
    let mut first = None;
    g.until(|m| match m {
        ToGateway::InputLine { session, text } if session == FOLLOWER && text == "sneak" => {
            sneaks += 1;
            if sneaks == 1 {
                first = Some(tokio::time::Instant::now());
            }
            (sneaks == 2).then_some(())
        }
        _ => None,
    })
    .await;
    let gap = tokio::time::Instant::now() - first.unwrap();
    assert!(
        gap >= Duration::from_secs(2) && gap < Duration::from_secs(4),
        "{gap:?}"
    );
    // The second attempt took, and it stays sneaking.
    g.play(10).await;
    let after = g.lines.iter().filter(|(_, t)| t == "sneak").count();
    assert_eq!(after - before, 2, "{:?}", g.lines);
}

/// The Dungeon under the Arena: its Entrance is lit, the Small Cavern beyond it too dark to see
/// (light -200).
const DUNGEON_ENTRANCE: Key = (1, 2152);
const SMALL_CAVERN: Key = (1, 2156);

/// Every direction the follower typed itself after `since` lines.
fn moves_of(g: &Gateway, session: &str, since: usize) -> Vec<String> {
    g.lines[since..]
        .iter()
        .filter(|(s, t)| s == session && is_direction(t))
        .map(|(_, t)| t.clone())
        .collect()
}

/// The leader walks the party into the Small Cavern, dragging the follower through the lit
/// Entrance and into the dark; `since` is where the follower's own lines then begin.
async fn into_the_cavern(g: &mut Gateway) -> usize {
    g.world.together = true;
    g.world.drag = true;
    g.world.dark.insert(SMALL_CAVERN);
    formed(g).await;
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: SMALL_CAVERN.0,
        room: SMALL_CAVERN.1,
        stop_short: None,
        fight: None,
    })
    .await;
    g.play(15).await;
    assert_eq!(
        g.world.rooms[LEADER],
        SMALL_CAVERN,
        "{:?} / {:?}",
        notices_of(g, LEADER),
        g.lines
    );
    assert_eq!(g.world.rooms[FOLLOWER], SMALL_CAVERN);
    g.lines.len()
}

/// C-127: Sasha, dragged eight rooms into the dark Redstone Tunnels behind Rocco, was taken
/// for left behind (her belief never moved, and the darkness line counted as a room seen) and
/// walked off on her own, which really did separate her (silver-mirror, 2026-11-21). A
/// follower the game moves through the dark keeps its place beside its leader.
#[tokio::test(start_paused = true)]
async fn a_follower_dragged_through_the_dark_stays_with_its_leader() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    let since = into_the_cavern(&mut g).await;
    // Well past the left-behind limit: still joined, and it typed no move of its own.
    g.play(40).await;
    assert_eq!(
        moves_of(&g, FOLLOWER, since),
        Vec::<String>::new(),
        "{:?}",
        &g.lines[since..]
    );
    let states: Vec<MemberState> = g
        .seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Parties { parties } => parties
                .iter()
                .flat_map(|p| p.members.iter())
                .find(|ms| ms.session == FOLLOWER)
                .map(|ms| ms.state),
            _ => None,
        })
        .collect();
    assert_eq!(states.last(), Some(&MemberState::Joined), "{states:?}");
    assert!(!states.contains(&MemberState::Separated), "{states:?}");
    g.send(ToClient::StateRequest {
        session: Some(FOLLOWER.into()),
    })
    .await;
    let pos = g
        .until(|m| match m {
            ToGateway::Character { session, state } if session == FOLLOWER => {
                state.position.clone()
            }
            _ => None,
        })
        .await;
    assert_eq!(
        (pos.map, pos.room, pos.candidates),
        (Some(1), Some(SMALL_CAVERN.1), 1)
    );
}

/// C-128: a follower whose belief is wrong in the dark (the game dropped it, its leader went
/// back to the lit Entrance, and its one way there is refused) stops at the first refusal as
/// lost: no move once a second, a look now and then, and the leader told at once.
#[tokio::test(start_paused = true)]
async fn a_follower_lost_in_the_dark_waits_and_the_leader_is_told() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    into_the_cavern(&mut g).await;
    // The game drops the follower without a word, and refuses its every move.
    g.world.following = false;
    g.world.no_exit.insert(FOLLOWER.to_owned());
    g.send(ToClient::WalkTo {
        session: LEADER.into(),
        map: DUNGEON_ENTRANCE.0,
        room: DUNGEON_ENTRANCE.1,
        stop_short: None,
        fight: None,
    })
    .await;
    let parties = g.member(FOLLOWER, MemberState::Separated).await;
    let since = g.lines.len();
    g.play(60).await;
    // One move, refused; then looks only.
    let moves = moves_of(&g, FOLLOWER, since);
    assert!(moves.len() <= 1, "{moves:?}");
    let looks = g.lines[since..]
        .iter()
        .filter(|(s, t)| s == FOLLOWER && t == "look")
        .count();
    assert!(
        (3..=8).contains(&looks),
        "{looks} looks: {:?}",
        &g.lines[since..]
    );
    let detail = g
        .seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Parties { parties } => parties
                .iter()
                .flat_map(|p| p.members.iter())
                .find(|ms| ms.session == FOLLOWER && ms.state == MemberState::Separated)
                .and_then(|ms| ms.detail.clone()),
            _ => None,
        })
        .next_back();
    assert_eq!(detail.as_deref(), Some("lost in the dark"), "{parties:?}");
    let warned: Vec<String> = g
        .seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Warning {
                session: Some(s),
                text,
            } if s == LEADER => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        warned
            .iter()
            .any(|w| w
                .contains("Lauren is lost in the dark: walk it to a lit room, or the party to it")),
        "{warned:#?} {:#?}",
        notices_of(&g, LEADER)
    );
}

/// The leader's casts of speed on the follower so far (C-131).
fn speeds(g: &Gateway) -> usize {
    g.lines
        .iter()
        .filter(|(s, t)| s == LEADER && t == "sped lauren")
        .count()
}

/// A Mage leader with speed ticked for the follower, the party formed (C-131).
async fn speed_on_lauren(g: &mut Gateway) {
    g.world.mage = true;
    g.world.together = true;
    formed(g).await;
    g.send(ToClient::BlessingsSave {
        session: LEADER.into(),
        spells: Vec::new(),
        on_others: BTreeMap::from([("sped".to_owned(), vec!["Lauren".to_owned()])]),
    })
    .await;
}

/// Blessings on a party member (C-131): Penny, a Mage, keeps speed on Lauren. She casts
/// `sped lauren` once Lauren stands with her; Lauren's own session sees "You feel fast!", so
/// nothing is cast again while that holds; her "You slow down." brings the next cast.
#[tokio::test(start_paused = true)]
async fn speed_is_kept_on_a_party_member_from_its_own_lines() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    speed_on_lauren(&mut g).await;
    g.until(|m| match m {
        ToGateway::InputLine { session, text } if session == LEADER && text == "sped lauren" => {
            Some(())
        }
        _ => None,
    })
    .await;
    g.play(60).await;
    assert_eq!(speeds(&g), 1, "{:?}", g.lines);
    assert!(
        g.seen
            .iter()
            .any(|m| matches!(m, ToGateway::Notice { session: Some(s), text }
            if s == LEADER && text == "Blessing: casting speed on Lauren")),
        "{:?}",
        g.seen
    );
    // The panel: speed on Lauren, ticked and on.
    let on = g.seen.iter().any(|m| {
        matches!(m, ToGateway::Blessings { characters }
            if characters.iter().any(|c| c.session == LEADER && c.spells.iter().any(|s|
                s.short == "sped" && !s.chosen && s.others.iter().any(|o| o.name == "Lauren" && o.chosen && o.on))))
    });
    assert!(on, "a blessings message with speed on Lauren: {:?}", g.seen);
    // Lauren's speed wears off: Penny casts it again.
    g.tell(FOLLOWER, &said("You slow down.")).await;
    g.play(30).await;
    assert_eq!(speeds(&g), 2, "{:?}", g.lines);
}

/// A member in another room gets no blessing and no word about it (C-131): the party is
/// saved but not formed, so Lauren is no mate of Penny's.
#[tokio::test(start_paused = true)]
async fn a_member_elsewhere_gets_no_blessing() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    g.world.rooms.insert(FOLLOWER.to_owned(), (1, 9));
    g.logs.get_mut(FOLLOWER).unwrap()[0].text = shown((1, 9));
    g.world.mage = true;
    for s in [LEADER, FOLLOWER] {
        g.bot(s, BotAction::Start).await;
    }
    g.send(ToClient::PartySave { party: party() }).await;
    g.send(ToClient::BlessingsSave {
        session: LEADER.into(),
        spells: Vec::new(),
        on_others: BTreeMap::from([("sped".to_owned(), vec!["Lauren".to_owned()])]),
    })
    .await;
    g.play(60).await;
    assert_eq!(speeds(&g), 0, "{:?}", g.lines);
    assert!(
        !g.seen
            .iter()
            .any(|m| matches!(m, ToGateway::Notice { text, .. } if text.contains("speed"))),
        "{:?}",
        g.seen
    );
}

/// A cast on a member the game refuses ("You attempt to cast speed, but fail.": a roll, or a
/// copy already on it) is tried again after a round and held after three (C-131).
#[tokio::test(start_paused = true)]
async fn a_member_cast_that_keeps_failing_is_held() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    g.world.speed_refusal = Some("You attempt to cast speed, but fail.");
    speed_on_lauren(&mut g).await;
    g.play(120).await;
    assert_eq!(speeds(&g), 3, "{:?}", g.lines);
    g.play(120).await;
    assert_eq!(speeds(&g), 3, "held for five minutes: {:?}", g.lines);
}

/// "You do not see lauren here!" is no failure: the member was not there after all, and the
/// cast waits for the mates list to catch up, with no hold (C-131).
#[tokio::test(start_paused = true)]
async fn a_member_the_game_does_not_see_is_not_held() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    g.world.speed_refusal = Some("You do not see lauren here!");
    speed_on_lauren(&mut g).await;
    g.play(60).await;
    assert!(speeds(&g) > 3, "{:?}", g.lines);
}

/// A spell the game will not cast on a player drops its member targets, said once (C-131).
#[tokio::test(start_paused = true)]
async fn a_spell_the_game_will_not_cast_on_a_player_is_dropped_with_a_notice() {
    let (mut g, _run) = Gateway::start(brain(None)).await;
    g.world.speed_refusal = Some("You may not cast that spell on a user!");
    speed_on_lauren(&mut g).await;
    g.play(120).await;
    assert_eq!(speeds(&g), 1, "{:?}", g.lines);
    let notices: Vec<&String> = g
        .seen
        .iter()
        .filter_map(|m| match m {
            ToGateway::Notice {
                session: Some(s),
                text,
            } if s == LEADER && text == "Blessing: speed cannot be cast on another player" => {
                Some(text)
            }
            _ => None,
        })
        .collect();
    assert_eq!(notices.len(), 1, "{:?}", g.seen);
}
