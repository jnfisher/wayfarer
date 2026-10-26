//! Parties (docs/brain.md "Parties", card C-039): two characters in the Arena behind a fake
//! gateway whose game keeps a party as MajorMUD does (`invite`, `follow`, `par`, `leave`, with
//! the lines of `tests/fixtures/party/`). On a paused clock.

mod game;

use std::collections::BTreeMap;

use game::*;
use tokio::sync::mpsc;
use wayfarer_brain::{Brain, ChannelLink, FixedClock, Store};
use wayfarer_proto::{BotAction, MemberState, PartyDef, PartyRank, PartyStatus};

const LEADER: &str = "penny";
const FOLLOWER: &str = "lauren";

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
}

impl World {
    fn rank(&self, session: &str) -> &'static str {
        self.ranks.get(session).copied().unwrap_or("Midrank")
    }
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
                rooms: BTreeMap::from([(LEADER.to_owned(), ARENA), (FOLLOWER.to_owned(), ARENA)]),
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
                    text: shown(ARENA),
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

    /// The game's answer to `text` from `session`, and what the other one sees.
    fn answer(&mut self, session: &str, text: &str) -> Vec<(String, String)> {
        let other = if session == LEADER { FOLLOWER } else { LEADER };
        let me = |t: String| vec![(session.to_owned(), t)];
        match text {
            "st" => me(format!(
                "\x1b[0;32mName: \x1b[36m{:<25}\x1b[32mLives/CP:\x1b[36m     9/0\r\n{PROMPT}",
                name(session)
            )),
            "look" => me(shown(self.world.rooms[session])),
            "n" if self.world.locked && self.world.rooms[session] == ARENA => {
                me(said("The door is closed!"))
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
            t if is_direction(t) => {
                let here = self.world.rooms[session];
                let atlas = game::atlas();
                match atlas.graph.edges(here).iter().find(|e| e.command == t) {
                    Some(e) => {
                        let to = e.to;
                        self.world.rooms.insert(session.to_owned(), to);
                        me(shown(to))
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
                let answers = self.answer(session, text);
                for (i, (s, t)) in answers.into_iter().enumerate() {
                    let t = if i == 0 { format!("{text}\r\n{t}") } else { t };
                    let t = match self.world.hp.get(&s) {
                        Some(hp) => t.replace("[HP=35", &format!("[HP={hp}")),
                        None => t,
                    };
                    self.output(&s, t).await;
                }
            }
            _ => {}
        }
        msg
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
    assert_eq!(bashes.len(), 3);
}
