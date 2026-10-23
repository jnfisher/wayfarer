//! The party coordinator (docs/plan/10-party-play.md, docs/plan/03-engine.md section 8): the
//! parties the player defined, kept in the store, and above the sessions what each member
//! should do about them. Every tick it reads each session's state and its party as the game
//! last showed it ([`GameParty`]) and plans the lines that form a party up (`invite`,
//! `follow`), keep it checked (`par`) and take a member out (`leave`), and each session's
//! [`PartyRole`], which its modes read. It sends nothing itself: the brain puts each line
//! through that session's executor and says which went.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_proto::{MemberState, MemberStatus, PartyDef, PartyStatus};
use wayfarer_store::PartyRecord;
use wayfarer_world::Key;

use crate::party_game::GameParty;

/// The most followers a party takes (the game's six members).
pub const MAX_FOLLOWERS: usize = 5;
/// How long before the same `invite` or `follow` is tried again.
const ASK_EVERY: Duration = Duration::from_secs(10);
/// How often the leader checks its party with `par` while it has members.
const PAR_EVERY: Duration = Duration::from_secs(30);
/// The least time between two of the leader's `par`s.
const PAR_GAP: Duration = Duration::from_secs(5);
/// How long a joined follower may be out of the leader's room before the leader checks with
/// `par`: the game moves followers in its fast cycle, well within this, and a follower an exit
/// refused drops without a word (`kb/grouping.md` §2a).
const APART: Duration = Duration::from_secs(3);

/// What a session is in a party, for its modes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum PartyRole {
    #[default]
    None,
    Leader {
        party: String,
        followers: Vec<String>,
    },
    Follower {
        party: String,
        /// The leader's session id, and the name to say it by.
        leader: String,
        leader_name: String,
        /// In the leader's party in the game.
        joined: bool,
    },
}

impl PartyRole {
    /// The leader's name, when this is a follower: its own modes do not walk it.
    pub fn following(&self) -> Option<&str> {
        match self {
            PartyRole::Follower { leader_name, .. } => Some(leader_name),
            _ => None,
        }
    }
}

/// What the coordinator reads of one session.
#[derive(Debug, Clone, Default)]
pub(crate) struct MemberInfo {
    /// In the realm, and the brain following it live.
    pub realm: bool,
    /// Its bot is running.
    pub running: bool,
    /// The in-game name (`Penny Pax`), once `st` has shown it.
    pub name: Option<String>,
    pub room: Option<Key>,
    pub game: GameParty,
}

/// One tick's plan: the lines to send, each session's role, and the parties for the UIs when
/// they changed.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub lines: Vec<(String, String)>,
    pub roles: BTreeMap<String, PartyRole>,
    pub statuses: Option<Vec<PartyStatus>>,
}

#[derive(Debug, Default)]
pub(crate) struct Parties {
    defs: Vec<PartyDef>,
    /// Followers' last `follow`, and the `invite` last sent for each.
    followed: HashMap<String, Instant>,
    invited: HashMap<String, Instant>,
    /// Leaders' last `par`.
    par_at: HashMap<String, Instant>,
    /// Joined followers seen out of their leader's room, since when.
    apart: HashMap<String, Instant>,
    /// Sessions taken out of a party that still owe the game a `leave`.
    leaving: BTreeSet<String>,
    /// The last statuses sent, to send again only on a change.
    sent: Option<Vec<PartyStatus>>,
}

/// The first word of a name: what the game's party lines and commands use.
fn first(name: &str) -> &str {
    name.split_whitespace().next().unwrap_or(name)
}

impl Parties {
    pub fn from_records(records: Vec<PartyRecord>) -> Parties {
        Parties {
            defs: records
                .into_iter()
                .map(|r| PartyDef {
                    id: r.id,
                    leader: r.leader,
                    followers: r.followers,
                })
                .collect(),
            ..Parties::default()
        }
    }

    #[cfg(test)]
    pub fn defs(&self) -> &[PartyDef] {
        &self.defs
    }

    pub fn has_parties(&self) -> bool {
        !self.defs.is_empty()
    }

    /// `party_save`: check `def` against the known sessions and keep it, moving its members
    /// out of any other party. Returns the parties changed (to store), the ids of those left
    /// empty (to delete), or why it was refused.
    pub fn save(
        &mut self,
        mut def: PartyDef,
        known: &BTreeSet<String>,
    ) -> Result<(Vec<PartyDef>, Vec<String>), String> {
        def.followers.dedup();
        if def.followers.is_empty() {
            return Err("a party needs at least one follower".to_owned());
        }
        if def.followers.len() > MAX_FOLLOWERS {
            return Err(format!("a party takes at most {MAX_FOLLOWERS} followers"));
        }
        if def.followers.contains(&def.leader) {
            return Err(format!("{} cannot lead and follow", def.leader));
        }
        let mut seen = BTreeSet::new();
        for s in std::iter::once(&def.leader).chain(&def.followers) {
            if !known.contains(s) {
                return Err(format!("no session {s:?}"));
            }
            if !seen.insert(s) {
                return Err(format!("{s} is in the party twice"));
            }
        }
        if def.id.is_empty() {
            def.id = self.new_id();
        }
        let before: BTreeSet<String> = self.members(&def.id).into_iter().collect();
        let members: BTreeSet<String> = std::iter::once(def.leader.clone())
            .chain(def.followers.iter().cloned())
            .collect();
        // Out of the other parties: a leader moved out takes its party with it.
        let mut changed = Vec::new();
        let mut emptied = Vec::new();
        for other in self.defs.iter_mut().filter(|d| d.id != def.id) {
            if members.contains(&other.leader) {
                emptied.push(other.id.clone());
                continue;
            }
            let n = other.followers.len();
            other.followers.retain(|f| !members.contains(f));
            if other.followers.len() != n {
                if other.followers.is_empty() {
                    emptied.push(other.id.clone());
                } else {
                    changed.push(other.clone());
                }
            }
        }
        for id in &emptied {
            for m in self.members(id) {
                if !members.contains(&m) {
                    self.leaving.insert(m);
                }
            }
        }
        self.defs.retain(|d| !emptied.contains(&d.id));
        // Followers dropped from this party leave it in the game.
        for gone in before.difference(&members) {
            self.leaving.insert(gone.clone());
        }
        match self.defs.iter_mut().find(|d| d.id == def.id) {
            Some(d) => *d = def.clone(),
            None => self.defs.push(def.clone()),
        }
        for m in &members {
            self.leaving.remove(m);
        }
        changed.push(def);
        self.sent = None;
        Ok((changed, emptied))
    }

    /// `party_delete`: whether there was one. Its followers leave the party in the game.
    pub fn delete(&mut self, id: &str) -> bool {
        let Some(i) = self.defs.iter().position(|d| d.id == id) else {
            return false;
        };
        let def = self.defs.remove(i);
        self.leaving.extend(def.followers);
        self.sent = None;
        true
    }

    /// `party_leave`: take `session` out of its party (as a follower; a leader's party is
    /// deleted). Returns the party changed, or the id deleted.
    pub fn leave(&mut self, session: &str) -> Option<Result<PartyDef, String>> {
        let i = self
            .defs
            .iter()
            .position(|d| d.leader == session || d.followers.iter().any(|f| f == session))?;
        if self.defs[i].leader == session {
            let id = self.defs[i].id.clone();
            self.delete(&id);
            return Some(Err(id));
        }
        let d = &mut self.defs[i];
        d.followers.retain(|f| f != session);
        self.leaving.insert(session.to_owned());
        self.sent = None;
        if d.followers.is_empty() {
            let id = d.id.clone();
            self.delete(&id);
            return Some(Err(id));
        }
        Some(Ok(d.clone()))
    }

    /// `party_join`: `session` as a follower of party `id`, moved out of any other.
    pub fn join(
        &mut self,
        session: &str,
        id: &str,
        known: &BTreeSet<String>,
    ) -> Result<(Vec<PartyDef>, Vec<String>), String> {
        let Some(d) = self.defs.iter().find(|d| d.id == id) else {
            return Err(format!("no party {id:?}"));
        };
        let mut def = d.clone();
        if !def.followers.iter().any(|f| f == session) {
            def.followers.push(session.to_owned());
        }
        self.save(def, known)
    }

    fn new_id(&self) -> String {
        (1..)
            .map(|n| format!("party-{n}"))
            .find(|id| !self.defs.iter().any(|d| d.id == *id))
            .expect("a free id")
    }

    fn members(&self, id: &str) -> Vec<String> {
        self.defs
            .iter()
            .filter(|d| d.id == id)
            .flat_map(|d| std::iter::once(d.leader.clone()).chain(d.followers.iter().cloned()))
            .collect()
    }

    /// The UIs asked: the next plan sends the statuses whether or not they changed.
    pub fn resend(&mut self) {
        self.sent = None;
    }

    /// A planned line went out through the session's executor.
    pub fn sent(&mut self, session: &str, line: &str, now: Instant) {
        match line.split_whitespace().next() {
            Some("follow") => {
                self.followed.insert(session.to_owned(), now);
            }
            Some("invite") => {
                if let Some(who) = line.split_whitespace().nth(1) {
                    self.invited.insert(who.to_owned(), now);
                }
            }
            Some("par") => {
                self.par_at.insert(session.to_owned(), now);
            }
            Some("leave") => {
                self.leaving.remove(session);
            }
            _ => {}
        }
    }

    /// Plan this tick from every session's state.
    pub fn plan(&mut self, m: &BTreeMap<String, MemberInfo>, now: Instant) -> Plan {
        let mut plan = Plan::default();
        let ago = |t: Option<&Instant>, d: Duration| t.is_none_or(|t| now >= *t + d);
        // Taken out of a party: `leave`, while the game still has it following.
        for s in &self.leaving {
            if m.get(s)
                .is_some_and(|i| i.realm && i.game.following.is_some())
            {
                plan.lines.push((s.clone(), "leave".to_owned()));
            }
        }
        self.leaving
            .retain(|s| m.get(s).is_some_and(|i| i.game.following.is_some()));
        for s in m.keys() {
            plan.roles.insert(s.clone(), PartyRole::None);
        }
        let mut statuses = Vec::new();
        for def in &self.defs {
            let blank = MemberInfo::default();
            let l = m.get(&def.leader).unwrap_or(&blank);
            let lfirst = l.name.as_deref().map(first);
            plan.roles.insert(
                def.leader.clone(),
                PartyRole::Leader {
                    party: def.id.clone(),
                    followers: def.followers.clone(),
                },
            );
            let mut members = vec![MemberStatus {
                session: def.leader.clone(),
                state: if l.realm {
                    MemberState::Joined
                } else {
                    MemberState::Away
                },
                detail: None,
            }];
            // A whole `par` screen since every member's last join or drop.
            let fresh = |f: &MemberInfo| {
                !l.game.stale
                    && l.game
                        .as_of
                        .is_some_and(|t| f.game.changed.is_none_or(|c| c <= t))
            };
            let mut owe_par = l.realm && (l.game.as_of.is_none() || l.game.stale);
            let mut counted = 0;
            for f in &def.followers {
                let fi = m.get(f).unwrap_or(&blank);
                let ffirst = fi.name.as_deref().map(first);
                let follows = matches!(
                    (fi.game.following.as_deref(), lfirst),
                    (Some(a), Some(b)) if a.eq_ignore_ascii_case(b)
                );
                let joined =
                    follows && (!fresh(fi) || fi.name.as_deref().is_some_and(|n| l.game.lists(n)));
                plan.roles.insert(
                    f.clone(),
                    PartyRole::Follower {
                        party: def.id.clone(),
                        leader: def.leader.clone(),
                        leader_name: lfirst.map_or_else(|| def.leader.clone(), str::to_owned),
                        joined,
                    },
                );
                let (state, detail) = if !fi.realm || !fi.running || !l.realm {
                    let why = if !fi.realm {
                        "not in the realm"
                    } else if !fi.running {
                        "its bot is not running"
                    } else {
                        "the leader is not in the realm"
                    };
                    (MemberState::Away, Some(why.to_owned()))
                } else if joined {
                    counted += 1;
                    if fi.room.is_some() && fi.room != l.room {
                        let since = *self.apart.entry(f.clone()).or_insert(now);
                        if now >= since + APART {
                            owe_par = true;
                        }
                    } else {
                        self.apart.remove(f);
                    }
                    (MemberState::Joined, None)
                } else {
                    counted += 1;
                    self.apart.remove(f);
                    let together = fi.room.is_some() && fi.room == l.room;
                    // Forming waits for a `par` that shows the party as it is (after a
                    // restart, for one): a formed party gets no invites.
                    if together
                        && l.game.as_of.is_some()
                        && !owe_par
                        && let (Some(lfirst), Some(ffirst)) = (lfirst, ffirst)
                    {
                        let lead = lfirst.to_lowercase();
                        let invited = fi
                            .game
                            .invited_by
                            .as_deref()
                            .is_some_and(|b| b.eq_ignore_ascii_case(lfirst));
                        let refused = fi
                            .game
                            .must_invite
                            .is_some_and(|t| self.followed.get(f).is_none_or(|s| t >= *s));
                        if invited {
                            if ago(self.followed.get(f), Duration::from_secs(2)) {
                                plan.lines.push((f.clone(), format!("follow {lead}")));
                            }
                        } else if refused {
                            let who = ffirst.to_lowercase();
                            if ago(self.invited.get(&who), ASK_EVERY) {
                                plan.lines
                                    .push((def.leader.clone(), format!("invite {who}")));
                            }
                        } else if ago(self.followed.get(f), ASK_EVERY) {
                            // A follower dropped at an exit is still on the leader's list:
                            // `follow` alone re-joins it (`kb/grouping.md` §10).
                            plan.lines.push((f.clone(), format!("follow {lead}")));
                        }
                    }
                    let detail = if together {
                        "forming up"
                    } else if fi.room.is_none() || l.room.is_none() {
                        "where it is is not known"
                    } else {
                        "in another room"
                    };
                    (MemberState::NotJoined, Some(detail.to_owned()))
                };
                if fi
                    .game
                    .changed
                    .is_some_and(|c| l.game.as_of.is_none_or(|t| c > t))
                {
                    owe_par = true;
                }
                members.push(MemberStatus {
                    session: f.clone(),
                    state,
                    detail,
                });
            }
            if l.realm && counted > 0 && l.game.as_of.is_none_or(|t| now >= t + PAR_EVERY) {
                owe_par = true;
            }
            if owe_par && ago(self.par_at.get(&def.leader), PAR_GAP) {
                plan.lines.push((def.leader.clone(), "par".to_owned()));
            }
            statuses.push(PartyStatus {
                party: def.clone(),
                members,
            });
        }
        if self.sent.as_ref() != Some(&statuses) {
            self.sent = Some(statuses.clone());
            plan.statuses = Some(statuses);
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::party_game::ParMember;

    fn known() -> BTreeSet<String> {
        ["penny", "lauren", "faye", "manuel"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn def(leader: &str, followers: &[&str]) -> PartyDef {
        PartyDef {
            id: String::new(),
            leader: leader.into(),
            followers: followers.iter().map(|f| (*f).to_owned()).collect(),
        }
    }

    fn member(name: &str, room: Key) -> MemberInfo {
        MemberInfo {
            realm: true,
            running: true,
            name: Some(name.into()),
            room: Some(room),
            game: GameParty::default(),
        }
    }

    fn row(name: &str, invited: bool) -> ParMember {
        ParMember {
            name: name.into(),
            class: None,
            mana: None,
            hp: Some(100),
            resting: false,
            rank: None,
            invited,
        }
    }

    /// Penny and Lauren in one room; Penny's `par` (at `now`) lists `listed`.
    fn two(now: Instant, listed: &[&str]) -> BTreeMap<String, MemberInfo> {
        let mut penny = member("Penny Pax", (1, 1));
        penny.game.as_of = Some(now);
        penny.game.members = listed.iter().map(|n| row(n, false)).collect();
        penny.game.members.push(row("Penny Pax", false));
        let lauren = member("Lauren Phillips", (1, 1));
        BTreeMap::from([("penny".to_owned(), penny), ("lauren".to_owned(), lauren)])
    }

    fn lines(plan: &Plan) -> Vec<(&str, &str)> {
        plan.lines
            .iter()
            .map(|(s, l)| (s.as_str(), l.as_str()))
            .collect()
    }

    #[test]
    fn saving_checks_the_party() {
        let mut p = Parties::default();
        let k = known();
        assert!(p.save(def("penny", &[]), &k).is_err());
        assert!(p.save(def("penny", &["penny"]), &k).is_err());
        assert!(p.save(def("penny", &["nobody"]), &k).is_err());
        assert!(
            p.save(def("penny", &["lauren", "lauren", "faye"]), &k)
                .is_ok()
        );
        assert_eq!(p.defs()[0].followers, ["lauren", "faye"]);
        assert_eq!(p.defs()[0].id, "party-1");
        let six = def("penny", &["a", "b", "c", "d", "e", "f"]);
        assert!(p.save(six, &k).unwrap_err().contains("at most 5"));
    }

    #[test]
    fn a_follower_saved_into_another_party_is_moved() {
        let mut p = Parties::default();
        let k = known();
        p.save(def("penny", &["lauren", "faye"]), &k).unwrap();
        let (changed, emptied) = p.save(def("manuel", &["faye"]), &k).unwrap();
        assert!(emptied.is_empty());
        assert_eq!(changed[0].followers, ["lauren"]);
        assert_eq!(p.defs().len(), 2);
        // The last follower moved out: that party goes.
        let mut manuel = def("manuel", &["faye", "lauren"]);
        manuel.id = "party-2".into();
        let (_, emptied) = p.save(manuel, &k).unwrap();
        assert_eq!(emptied, ["party-1"]);
        assert_eq!(p.defs().len(), 1);
    }

    #[tokio::test]
    async fn forming_up_follows_then_invites() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &[]);
        // Not in the party: the follower tries `follow` first.
        let plan = p.plan(&m, now);
        assert_eq!(lines(&plan), [("lauren", "follow penny")]);
        assert_eq!(
            plan.roles["lauren"],
            PartyRole::Follower {
                party: "party-1".into(),
                leader: "penny".into(),
                leader_name: "Penny".into(),
                joined: false
            }
        );
        p.sent("lauren", "follow penny", now);
        // `You must be invited first!`: the leader invites, by the in-game name.
        let later = now + Duration::from_secs(1);
        m.get_mut("lauren").unwrap().game.must_invite = Some(later);
        assert_eq!(lines(&p.plan(&m, later)), [("penny", "invite lauren")]);
        p.sent("penny", "invite lauren", later);
        assert!(p.plan(&m, later).lines.is_empty());
        // Invited: it follows at once.
        m.get_mut("lauren").unwrap().game.invited_by = Some("Penny".into());
        let later = later + Duration::from_secs(3);
        assert_eq!(lines(&p.plan(&m, later)), [("lauren", "follow penny")]);
    }

    #[tokio::test]
    async fn joined_when_the_game_says_so() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips"]);
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        let plan = p.plan(&m, now);
        assert!(plan.lines.is_empty(), "{:?}", plan.lines);
        let st = plan.statuses.unwrap();
        assert_eq!(st[0].members[1].state, MemberState::Joined);
        // A later `par` that does not list her: dropped (an exit refused her silently).
        let later = now + Duration::from_secs(40);
        let penny = m.get_mut("penny").unwrap();
        penny.game.members.retain(|r| r.name == "Penny Pax");
        penny.game.as_of = Some(later);
        let plan = p.plan(&m, later);
        assert_eq!(lines(&plan), [("lauren", "follow penny")]);
        assert!(matches!(
            plan.roles["lauren"],
            PartyRole::Follower { joined: false, .. }
        ));
    }

    #[tokio::test]
    async fn par_after_a_change_every_30_s_and_before_forming_after_a_restart() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        // A fresh brain: no `par` yet. It checks before inviting anyone.
        let mut m = two(now, &[]);
        m.get_mut("penny").unwrap().game = GameParty::default();
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        assert_eq!(lines(&p.plan(&m, now)), [("penny", "par")]);
        p.sent("penny", "par", now);
        assert!(p.plan(&m, now + Duration::from_secs(1)).lines.is_empty());
        // The screen came, listing her: nothing to do until 30 s on.
        let mut m = two(now, &["Lauren Phillips"]);
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        assert!(p.plan(&m, now + Duration::from_secs(10)).lines.is_empty());
        assert_eq!(
            lines(&p.plan(&m, now + Duration::from_secs(31))),
            [("penny", "par")]
        );
        // A drop line since the screen: checked again.
        let t = now + Duration::from_secs(40);
        p.sent("penny", "par", now + Duration::from_secs(31));
        let lauren = m.get_mut("lauren").unwrap();
        lauren.game.following = None;
        lauren.game.changed = Some(t);
        assert!(lines(&p.plan(&m, t)).contains(&("penny", "par")));
    }

    #[tokio::test]
    async fn apart_from_the_leader_makes_it_check() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips"]);
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        m.get_mut("penny").unwrap().room = Some((1, 2));
        assert!(p.plan(&m, now).lines.is_empty());
        assert_eq!(
            lines(&p.plan(&m, now + Duration::from_secs(6))),
            [("penny", "par")]
        );
    }

    #[tokio::test]
    async fn away_members_are_skipped() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &[]);
        m.get_mut("lauren").unwrap().running = false;
        let plan = p.plan(&m, now);
        assert!(plan.lines.is_empty());
        assert_eq!(
            plan.statuses.unwrap()[0].members[1].state,
            MemberState::Away
        );
    }

    #[tokio::test]
    async fn leave_sends_leave_while_the_game_has_it_following() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren", "faye"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips"]);
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        assert_eq!(p.leave("lauren").unwrap().unwrap().followers, ["faye"]);
        let plan = p.plan(&m, now);
        assert!(lines(&plan).contains(&("lauren", "leave")));
        assert_eq!(plan.roles["lauren"], PartyRole::None);
        // Left in the game: nothing more.
        m.get_mut("lauren").unwrap().game.following = None;
        assert!(!lines(&p.plan(&m, now)).contains(&("lauren", "leave")));
    }

    #[test]
    fn statuses_go_out_only_on_a_change() {
        let now = Instant::now;
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let m = two(now(), &[]);
        assert!(p.plan(&m, now()).statuses.is_some());
        assert!(p.plan(&m, now()).statuses.is_none());
        p.resend();
        assert!(p.plan(&m, now()).statuses.is_some());
    }
}
