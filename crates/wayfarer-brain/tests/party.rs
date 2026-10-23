//! Parties (docs/brain.md "Parties", card C-039): two characters in the Arena behind a fake
//! gateway whose game keeps a party as MajorMUD does (`invite`, `follow`, `par`, `leave`, with
//! the lines of `tests/fixtures/party/`). On a paused clock.

mod game;

use std::collections::BTreeMap;

use game::*;
use tokio::sync::mpsc;
use wayfarer_brain::{Brain, ChannelLink, FixedClock, Store};
use wayfarer_proto::{BotAction, MemberState, PartyDef, PartyStatus};

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

/// The game behind the gateway: both characters in the Arena, and the party between them.
#[derive(Default)]
struct World {
    invited: bool,
    following: bool,
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
            world: World::default(),
            lines: Vec::new(),
            seen: Vec::new(),
        };
        for s in [LEADER, FOLLOWER] {
            g.logs.insert(
                s.to_owned(),
                vec![Chunk {
                    seq: 1,
                    text: room(ARENA),
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
            "look" => me(room(ARENA)),
            "par" => {
                let row = |s: &str| {
                    format!(
                        "  {:<30} (Witchunter)          [H:100%]   - Midrank\r\n",
                        name(s)
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
