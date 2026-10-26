//! The party coordinator (docs/plan/10-party-play.md, docs/plan/03-engine.md section 8): the
//! parties the player defined, kept in the store, and above the sessions what each member
//! should do about them. Every tick it reads each session's state and its party as the game
//! last showed it ([`GameParty`]) and plans the lines that form a party up (`invite`,
//! `follow`), keep it checked (`par`), put each member in its rank (`frontrank`, `midrank`,
//! `backrank`) and take a member out (`leave`), and each session's
//! [`PartyRole`], which its modes read. It sends nothing itself: the brain puts each line
//! through that session's executor and says which went.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::Rank;
use wayfarer_proto::{MemberState, MemberStatus, PartyDef, PartyRank, PartyStatus};
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
/// How long a rank command waits for a `par` that shows it before another goes.
const RANK_EVERY: Duration = Duration::from_secs(10);

/// What a session is in a party, for its modes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum PartyRole {
    #[default]
    None,
    Leader {
        party: String,
        followers: Vec<String>,
        /// Why its bot does not move on: a member is out of the party and walking back
        /// (`Waiting for Lauren to catch up`).
        hold: Option<String>,
        /// The party rests: why (`Resting for Lauren (62%)`). Its Recover rests the party
        /// where the quiet-room rule allows; its loop holds.
        rest: Option<String>,
        /// The members with it, for its heals.
        mates: Vec<Mate>,
    },
    Follower {
        party: String,
        /// The leader's session id, and the name to say it by.
        leader: String,
        leader_name: String,
        /// In the leader's party in the game.
        joined: bool,
        /// Out of the party: walk to the leader's room to join it again.
        rejoin: Option<Key>,
        /// Joined and in the leader's room: it runs when the leader runs (its own Flee waits
        /// for the leader's), and fights the leader's target.
        with_leader: bool,
        /// The monster the leader fights, when it is with the leader.
        assist: Option<String>,
        /// The locked door the leader is bashing, when it is with the leader: the room and
        /// the direction.
        bash: Option<(Key, u8)>,
        /// The party rests (C-045): why. A follower below its own rest-until lines rests with
        /// it, as the leader does.
        rest: Option<String>,
        /// The other members with it (the leader among them), for its heals.
        mates: Vec<Mate>,
    },
}

/// Another member in the room, for a healer (C-043).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mate {
    /// What a heal is cast at: the in-game first name, in lower case.
    pub name: String,
    pub hp: i32,
    pub missing: i64,
    /// Below its own flee line.
    pub flee: bool,
    /// Its rest-until HP line, per cent: what a healer heals it up to in a party rest (C-045).
    pub until: i32,
    /// The HP it is short of that line.
    pub short: i64,
    /// HP per hour it gets back resting, when its stats are known.
    pub hp_rate: Option<i64>,
    /// Seconds until its mana is back at its rest-until mana line (0 for one that does not
    /// cast; `None` when that is not known).
    pub mana_wait: Option<i64>,
}

impl PartyRole {
    /// The leader's name, when this is a follower: its own modes do not walk it.
    pub fn following(&self) -> Option<&str> {
        match self {
            PartyRole::Follower { leader_name, .. } => Some(leader_name),
            _ => None,
        }
    }

    /// The leader's room, when this follower walks back to it.
    pub fn rejoin(&self) -> Option<Key> {
        match self {
            PartyRole::Follower { rejoin, .. } => *rejoin,
            _ => None,
        }
    }

    /// Why this leader's loop waits, when it does: a member walking back, or a party rest.
    pub fn hold(&self) -> Option<&str> {
        match self {
            PartyRole::Leader { hold, rest, .. } => hold.as_deref().or(rest.as_deref()),
            _ => None,
        }
    }

    /// Why the party rests, when it does: the leader rests it, and each member with it rests
    /// with it (C-045).
    pub fn party_rest(&self) -> Option<&str> {
        match self {
            PartyRole::Leader { rest, .. } | PartyRole::Follower { rest, .. } => rest.as_deref(),
            PartyRole::None => None,
        }
    }

    /// A follower with its leader: its own Flee waits for the leader's.
    pub fn with_leader(&self) -> bool {
        matches!(
            self,
            PartyRole::Follower {
                with_leader: true,
                ..
            }
        )
    }

    /// The other members with it, for its heals.
    pub fn mates(&self) -> &[Mate] {
        match self {
            PartyRole::Leader { mates, .. } | PartyRole::Follower { mates, .. } => mates,
            PartyRole::None => &[],
        }
    }

    /// The leader's target, for a follower with it.
    pub fn assist(&self) -> Option<&str> {
        match self {
            PartyRole::Follower { assist, .. } => assist.as_deref(),
            _ => None,
        }
    }

    /// The locked door the leader is bashing, for a follower with it: the room and direction.
    pub fn help_bash(&self) -> Option<(Key, u8)> {
        match self {
            PartyRole::Follower { bash, .. } => *bash,
            _ => None,
        }
    }
}

/// A member's HP and mana against its own lines (`health.*`), as its session sees them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Health {
    pub hp: Option<i32>,
    /// The hit points gone.
    pub missing: Option<i64>,
    /// Below its rest line, for HP or (casting) mana.
    pub low: bool,
    /// At or above its rest-until lines.
    pub rested: bool,
    /// Below its flee line.
    pub flee: bool,
    /// Its rest-until HP line, and the HP it is short of it.
    pub until: i32,
    pub short: Option<i64>,
    /// HP per hour it gets back resting, when its stats are known.
    pub hp_rate: Option<i64>,
    /// Seconds until its mana is back at its rest-until mana line (0 for one that does not
    /// cast).
    pub mana_wait: Option<i64>,
}

/// A follower's walk back to its leader, as its session sees it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WalkBack {
    /// The rooms still to go, while the walker takes it.
    pub steps_left: Option<u32>,
    /// It cannot get there, and why (no route, or every route refused).
    pub stuck: Option<String>,
}

/// What the coordinator reads of one session.
/// Whether two members may be in one room: the same believed room, or a room both beliefs
/// hold. Unknown for either is not.
fn same_room(a: &MemberInfo, b: &MemberInfo) -> bool {
    match (a.room, b.room) {
        (Some(x), Some(y)) => x == y || a.rooms.iter().any(|k| b.rooms.contains(k)),
        _ => false,
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MemberInfo {
    /// In the realm, and the brain following it live.
    pub realm: bool,
    /// Its bot is running.
    pub running: bool,
    /// The in-game name (`Penny Pax`), once `st` has shown it.
    pub name: Option<String>,
    /// Its believed room: the lowest of `rooms` when the belief is not sure.
    pub room: Option<Key>,
    /// Every room the belief has it in (empty: only `room` is known). Slum Street has dozens of
    /// rooms of one name, so a leader and a follower side by side may each be unsure, with
    /// different lowest keys (C-053).
    pub rooms: Vec<Key>,
    pub game: GameParty,
    /// Its walk back to the leader.
    pub walk: WalkBack,
    /// How long it waits, as a leader, for a separated member (`party.wait_separated_minutes`).
    pub wait_separated: Duration,
    /// How long it holds, as a leader, for a party rest (`party.wait_rest_minutes`).
    pub wait_rest: Duration,
    pub health: Health,
    /// The monster it fights, when it fights one.
    pub target: Option<String>,
    /// The locked door its walk is bashing: the room and direction.
    pub bashing: Option<(Key, u8)>,
    /// The rank it should have in its party (`party.rank`).
    pub rank: Option<Rank>,
    /// Resting or fighting: a rank command would end the rest, so it waits.
    pub busy: bool,
}

/// One tick's plan: the lines to send, each session's role, and the parties for the UIs when
/// they changed.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub lines: Vec<(String, String)>,
    pub roles: BTreeMap<String, PartyRole>,
    pub statuses: Option<Vec<PartyStatus>>,
    /// Leaders that waited too long for a member: stop the loop, and why.
    pub give_up: Vec<(String, String)>,
    /// Leaders to flee now, for a member under its flee line, and why.
    pub flee: Vec<(String, String)>,
    /// A word for a member's tile: a rank it cannot have.
    pub notes: Vec<(String, String)>,
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
    /// Followers that were in the party in the game since they were added.
    was_joined: BTreeSet<String>,
    /// Followers that walk to their leader to join it: separated ones, and those added with
    /// `party_join`.
    walk_in: BTreeSet<String>,
    /// Since when each follower walking in has been out of the party.
    out_since: HashMap<String, Instant>,
    /// Followers the leader gave up waiting for (its loop stopped), until they are back.
    gave_up: BTreeSet<String>,
    /// Parties resting, since when, and whether the leader gave up on it.
    resting: HashMap<String, (Instant, bool)>,
    /// Followers whose flee line set the leader fleeing, until they are above it again.
    fled_for: BTreeSet<String>,
    /// The last statuses sent, to send again only on a change.
    sent: Option<Vec<PartyStatus>>,
    ranks: Ranks,
    /// Members a `party_resync` asked to check their party with `par`, until it goes.
    resync: BTreeSet<String>,
}

/// The rank commands sent and the ranks refused (`party.rank`).
#[derive(Debug, Default)]
struct Ranks {
    /// When each member's last rank command went.
    sent: HashMap<String, Instant>,
    /// The rank each member was refused, or cannot have, until it wants another (or a re-sync).
    refused: HashMap<String, Rank>,
}

fn rank_command(r: Rank) -> &'static str {
    match r {
        Rank::Front => "frontrank",
        Rank::Mid => "midrank",
        Rank::Back => "backrank",
    }
}

fn rank_word(r: Rank) -> &'static str {
    match r {
        Rank::Front => "front",
        Rank::Mid => "middle",
        Rank::Back => "back",
    }
}

fn wire_rank(r: Rank) -> PartyRank {
    match r {
        Rank::Front => PartyRank::Front,
        Rank::Mid => PartyRank::Mid,
        Rank::Back => PartyRank::Back,
    }
}

impl Ranks {
    fn forget(&mut self, session: &str) {
        self.sent.remove(session);
        self.refused.remove(session);
    }

    /// The members of `def` in the party in the game: each one's rank as the leader's screen
    /// shows it, into its status; and when the screen is current (`ready`), the commands that
    /// bring them to their `party.rank` in an order the game allows (`kb/grouping.md`): the
    /// leader never goes back, someone stays in front, and the back never outnumbers the front.
    /// Everyone for the front moves at once (more in front blocks nobody), then one at a time
    /// to the middle and then the back, each after a screen that shows the last. A move the
    /// game would refuse is not sent: the member's tile is told once.
    fn plan(
        &mut self,
        def: &PartyDef,
        m: &BTreeMap<String, MemberInfo>,
        ready: bool,
        now: Instant,
        plan: &mut Plan,
        members: &mut [MemberStatus],
    ) {
        let blank = MemberInfo::default();
        let l = m.get(&def.leader).unwrap_or(&blank);
        let ranked: Vec<(String, Rank)> = members
            .iter()
            .filter(|ms| matches!(ms.state, MemberState::Joined | MemberState::Resting))
            .filter_map(|ms| {
                let n = m.get(&ms.session)?.name.as_deref()?;
                Some((ms.session.clone(), l.game.rank_of(n).filter(|_| l.realm)?))
            })
            .collect();
        // A leader alone is in no party yet: ranks start once a follower has joined.
        if !ranked.iter().any(|(s, _)| *s != def.leader) {
            return;
        }
        for ms in members.iter_mut() {
            ms.rank = ranked
                .iter()
                .find(|(s, _)| *s == ms.session)
                .map(|(_, r)| wire_rank(*r));
        }
        if !ready {
            return;
        }
        let name = |s: &str| {
            m.get(s)
                .and_then(|i| i.name.as_deref())
                .map_or_else(|| s.to_owned(), |n| first(n).to_owned())
        };
        // The game's answer to the last command.
        for (s, _) in &ranked {
            let Some(i) = m.get(s) else { continue };
            if self.refused.get(s).is_some_and(|r| Some(*r) != i.rank) {
                self.refused.remove(s);
            }
            if let (Some(t), Some(want)) = (self.sent.get(s).copied(), i.rank)
                && i.game.rank_refused.is_some_and(|r| r >= t)
            {
                self.sent.remove(s);
                self.refused.insert(s.clone(), want);
                plan.notes.push((
                    s.clone(),
                    format!(
                        "{} stays where it is: the game would not move it to the {} rank",
                        name(s),
                        rank_word(want)
                    ),
                ));
            }
        }
        // A move not shown on a screen yet: wait for it.
        let shown = |t: &Instant| l.game.as_of.is_some_and(|a| a > *t);
        if ranked.iter().any(|(s, _)| {
            self.sent
                .get(s)
                .is_some_and(|t| now < *t + RANK_EVERY && !shown(t))
        }) {
            return;
        }
        let front = ranked.iter().filter(|(_, r)| *r == Rank::Front).count();
        let back = ranked.iter().filter(|(_, r)| *r == Rank::Back).count();
        let wants: Vec<(String, Rank, Rank)> = ranked
            .iter()
            .filter_map(|(s, now_at)| {
                let i = m.get(s)?;
                let want = i.rank?;
                (want != *now_at
                    && i.realm
                    && i.running
                    && !i.busy
                    && self.refused.get(s) != Some(&want))
                .then(|| (s.clone(), *now_at, want))
            })
            .collect();
        let to_front: Vec<&String> = wants
            .iter()
            .filter(|w| w.2 == Rank::Front)
            .map(|w| &w.0)
            .collect();
        if !to_front.is_empty() {
            for s in to_front {
                plan.lines
                    .push((s.clone(), rank_command(Rank::Front).to_owned()));
            }
            return;
        }
        for target in [Rank::Mid, Rank::Back] {
            for (s, at, want) in wants.iter().filter(|w| w.2 == target) {
                let front_others = front - usize::from(*at == Rank::Front);
                let back_others = back - usize::from(*at == Rank::Back);
                let why = if *want == Rank::Back && *s == def.leader {
                    Some("a party's leader cannot go to the back rank")
                } else if *at == Rank::Front && front_others == 0 {
                    Some("someone must stay in the front rank")
                } else if *want == Rank::Back && front_others < back_others {
                    Some("the back rank may not outnumber the front")
                } else {
                    None
                };
                match why {
                    Some(why) => {
                        self.refused.insert(s.clone(), *want);
                        plan.notes.push((
                            s.clone(),
                            format!("{} stays in the {} rank: {why}", name(s), rank_word(*at)),
                        ));
                    }
                    None => {
                        plan.lines.push((s.clone(), rank_command(*want).to_owned()));
                        return;
                    }
                }
            }
        }
    }
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

    /// `session` follows a leader in one of the parties.
    pub fn is_follower(&self, session: &str) -> bool {
        self.defs
            .iter()
            .any(|d| d.followers.iter().any(|f| f == session))
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
        let gone: Vec<String> = before.difference(&members).cloned().collect();
        for g in &gone {
            self.forget(g);
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
        for f in &def.followers {
            self.forget(f);
        }
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
        self.forget(session);
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
        let saved = self.save(def, known)?;
        // Joined from wherever it is: it walks to the leader.
        self.walk_in.insert(session.to_owned());
        Ok(saved)
    }

    /// What the coordinator remembers of a follower taken out of its party.
    fn forget(&mut self, session: &str) {
        self.ranks.forget(session);
        self.was_joined.remove(session);
        self.walk_in.remove(session);
        self.out_since.remove(session);
        self.gave_up.remove(session);
        self.apart.remove(session);
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

    /// `party_resync`: form party `id` (`None`: every party) up again now. Each member owes
    /// a `par` at once, the `invite`s and `follow`s go without waiting out [`ASK_EVERY`], and
    /// the leader's waits start afresh. Returns the members, for their sessions to doubt
    /// what they know of their party; `None` when there is no party `id`.
    pub fn resync(&mut self, id: Option<&str>) -> Option<Vec<String>> {
        let members: Vec<String> = self
            .defs
            .iter()
            .filter(|d| id.is_none_or(|id| d.id == id))
            .flat_map(|d| std::iter::once(d.leader.clone()).chain(d.followers.iter().cloned()))
            .collect();
        if members.is_empty() && id.is_some() {
            return None;
        }
        for s in &members {
            self.ranks.forget(s);
            self.followed.remove(s);
            self.par_at.remove(s);
            self.apart.remove(s);
            self.out_since.remove(s);
            self.gave_up.remove(s);
            self.resync.insert(s.clone());
        }
        // Invites are kept by the invitee's first name, which the coordinator does not know
        // here: they all go again.
        self.invited.clear();
        self.sent = None;
        Some(members)
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
                self.resync.remove(session);
            }
            Some("leave") => {
                self.leaving.remove(session);
            }
            Some("frontrank" | "midrank" | "backrank") => {
                self.ranks.sent.insert(session.to_owned(), now);
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
                    hold: None,
                    rest: None,
                    mates: Vec::new(),
                },
            );
            // The followers walking back, by name, for the leader's hold.
            let mut waiting: Vec<String> = Vec::new();
            let mut members = vec![MemberStatus {
                session: def.leader.clone(),
                state: if l.realm {
                    MemberState::Joined
                } else {
                    MemberState::Away
                },
                detail: None,
                rank: None,
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
                let mut rejoin = None;
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
                    self.was_joined.insert(f.clone());
                    self.walk_in.remove(f);
                    self.out_since.remove(f);
                    self.gave_up.remove(f);
                    if fi.room.is_some() && !same_room(fi, l) {
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
                    let together = same_room(fi, l);
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
                    // Dropped by the game (an exit refused it, a flee, a stun), or added with
                    // `party_join`: it walks back to the leader, and the leader waits.
                    if self.was_joined.contains(f) {
                        self.walk_in.insert(f.clone());
                    }
                    if self.walk_in.contains(f) {
                        let name = ffirst.map_or_else(|| f.clone(), str::to_owned);
                        waiting.push(name.clone());
                        let since = *self.out_since.entry(f.clone()).or_insert(now);
                        if now >= since + l.wait_separated && self.gave_up.insert(f.clone()) {
                            let minutes = l.wait_separated.as_secs() / 60;
                            plan.give_up.push((
                                def.leader.clone(),
                                format!("{name} did not catch up in {minutes} minutes"),
                            ));
                        }
                        let leader = lfirst.unwrap_or(&def.leader);
                        if together {
                            (MemberState::NotJoined, Some("forming up".to_owned()))
                        } else if let Some(why) = &fi.walk.stuck {
                            (MemberState::Unreachable, Some(why.clone()))
                        } else if l.room.is_none() {
                            (
                                MemberState::Separated,
                                Some(format!("where {leader} is is not known")),
                            )
                        } else {
                            rejoin = l.room;
                            match fi.walk.steps_left {
                                Some(n) => (
                                    MemberState::Walking,
                                    Some(format!("{n} room{}", if n == 1 { "" } else { "s" })),
                                ),
                                None => (MemberState::Separated, None),
                            }
                        }
                    } else {
                        let detail = if together {
                            "forming up"
                        } else if fi.room.is_none() || l.room.is_none() {
                            "where it is is not known"
                        } else {
                            "in another room"
                        };
                        (MemberState::NotJoined, Some(detail.to_owned()))
                    }
                };
                plan.roles.insert(
                    f.clone(),
                    PartyRole::Follower {
                        party: def.id.clone(),
                        leader: def.leader.clone(),
                        leader_name: lfirst.map_or_else(|| def.leader.clone(), str::to_owned),
                        joined,
                        rejoin,
                        with_leader: joined && same_room(fi, l),
                        assist: (joined && same_room(fi, l))
                            .then(|| l.target.clone())
                            .flatten(),
                        bash: (joined && same_room(fi, l)).then_some(l.bashing).flatten(),
                        rest: None,
                        mates: Vec::new(),
                    },
                );
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
                    rank: None,
                });
            }
            if l.realm && counted > 0 && l.game.as_of.is_none_or(|t| now >= t + PAR_EVERY) {
                owe_par = true;
            }
            if owe_par && ago(self.par_at.get(&def.leader), PAR_GAP) {
                plan.lines.push((def.leader.clone(), "par".to_owned()));
            }
            if !waiting.is_empty()
                && let Some(PartyRole::Leader { hold, .. }) = plan.roles.get_mut(&def.leader)
            {
                *hold = Some(format!("Waiting for {} to catch up", waiting.join(" and ")));
            }
            // Party rest and flee, over the leader and the followers with it.
            let together: Vec<(&String, &MemberInfo)> = std::iter::once((&def.leader, l))
                .chain(def.followers.iter().filter_map(|f| {
                    let fi = m.get(f)?;
                    plan.roles[f].with_leader().then_some((f, fi))
                }))
                .filter(|(_, i)| i.realm && i.running)
                .collect();
            let name = |id: &String, i: &MemberInfo| {
                i.name
                    .as_deref()
                    .map_or_else(|| id.clone(), |n| first(n).to_owned())
            };
            for (f, fi) in together.iter().skip(1) {
                if fi.health.flee {
                    if self.fled_for.insert((*f).clone()) {
                        let hp = fi
                            .health
                            .hp
                            .map_or_else(String::new, |h| format!(" at {h}%"));
                        plan.flee
                            .push((def.leader.clone(), format!("{}{hp}", name(f, fi))));
                    }
                } else {
                    self.fled_for.remove(*f);
                }
            }
            // Each one's mates, for its heals: the others with the leader, HP known.
            for (id, _) in &together {
                let mates: Vec<Mate> = together
                    .iter()
                    .filter(|(o, _)| o != id)
                    .filter_map(|(_, i)| {
                        Some(Mate {
                            name: first(i.name.as_deref()?).to_lowercase(),
                            hp: i.health.hp?,
                            missing: i.health.missing?,
                            flee: i.health.flee,
                            until: i.health.until,
                            short: i.health.short.unwrap_or(0),
                            hp_rate: i.health.hp_rate,
                            mana_wait: i.health.mana_wait,
                        })
                    })
                    .collect();
                if let Some(
                    PartyRole::Leader { mates: m, .. } | PartyRole::Follower { mates: m, .. },
                ) = plan.roles.get_mut(*id)
                {
                    *m = mates;
                }
            }
            let any_low = together.iter().any(|(_, i)| i.health.low);
            let unrested: Vec<(&String, &MemberInfo)> = together
                .iter()
                .filter(|(_, i)| !i.health.rested)
                .copied()
                .collect();
            let resting = match self.resting.get(&def.id) {
                Some(_) if unrested.is_empty() => {
                    self.resting.remove(&def.id);
                    false
                }
                Some(_) => true,
                None if any_low && l.realm => {
                    self.resting.insert(def.id.clone(), (now, false));
                    true
                }
                None => false,
            };
            if resting {
                let who: Vec<String> = unrested
                    .iter()
                    .map(|(id, i)| match i.health.hp {
                        Some(hp) => format!("{} ({hp}%)", name(id, i)),
                        None => name(id, i),
                    })
                    .collect();
                let why = format!("Resting for {}", who.join(", "));
                for (id, _) in &together {
                    if let Some(PartyRole::Leader { rest, .. } | PartyRole::Follower { rest, .. }) =
                        plan.roles.get_mut(*id)
                    {
                        *rest = Some(why.clone());
                    }
                }
                for (id, i) in &unrested {
                    if let Some(ms) = members.iter_mut().find(|ms| ms.session == **id)
                        && ms.state == MemberState::Joined
                    {
                        ms.state = MemberState::Resting;
                        ms.detail = i.health.hp.map(|h| format!("{h}%"));
                    }
                }
                let entry = self.resting.get_mut(&def.id).expect("resting");
                if !entry.1 && now >= entry.0 + l.wait_rest {
                    entry.1 = true;
                    let minutes = l.wait_rest.as_secs() / 60;
                    plan.give_up.push((
                        def.leader.clone(),
                        format!(
                            "the party rested {minutes} minutes; not recovered: {}",
                            who.join(", ")
                        ),
                    ));
                }
            }
            // Ranks, once the leader's screen shows the party as it is.
            let ready = l.realm && l.game.as_of.is_some() && !l.game.stale && !owe_par;
            self.ranks.plan(def, m, ready, now, &mut plan, &mut members);
            statuses.push(PartyStatus {
                party: def.clone(),
                members,
            });
        }
        // A `party_resync`: each member in the realm checks its party, once.
        let defs = &self.defs;
        self.resync.retain(|s| {
            m.get(s).is_some_and(|i| i.realm)
                && defs
                    .iter()
                    .any(|d| d.leader == *s || d.followers.contains(s))
        });
        for s in &self.resync {
            if !plan.lines.iter().any(|(id, l)| id == s && l == "par") {
                plan.lines.push((s.clone(), "par".to_owned()));
            }
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
            rooms: Vec::new(),
            game: GameParty::default(),
            walk: WalkBack::default(),
            wait_separated: Duration::from_secs(300),
            wait_rest: Duration::from_secs(600),
            health: Health {
                hp: Some(100),
                missing: Some(0),
                low: false,
                rested: true,
                flee: false,
                until: 85,
                short: Some(0),
                hp_rate: Some(600),
                mana_wait: Some(0),
            },
            target: None,
            bashing: None,
            rank: None,
            busy: false,
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
                joined: false,
                rejoin: None,
                with_leader: false,
                assist: None,
                bash: None,
                rest: None,
                mates: Vec::new(),
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
    async fn unsure_side_by_side_is_not_apart() {
        // C-053: both on a Slum Street of many; each belief has the other's lowest room among
        // its candidates. Together: no `par` for it, and the follower is with the leader.
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips"]);
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        let penny = m.get_mut("penny").unwrap();
        penny.room = Some((1, 2));
        penny.rooms = vec![(1, 2), (1, 9)];
        let lauren = m.get_mut("lauren").unwrap();
        lauren.room = Some((1, 9));
        lauren.rooms = vec![(1, 9), (1, 12)];
        assert!(p.plan(&m, now).lines.is_empty());
        let plan = p.plan(&m, now + Duration::from_secs(6));
        assert!(plan.lines.is_empty(), "{:?}", plan.lines);
        assert!(plan.roles["lauren"].with_leader());
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

    /// Penny, Lauren joined in her room, then the game dropping Lauren after she walked on.
    fn dropped(p: &mut Parties, now: Instant) -> BTreeMap<String, MemberInfo> {
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips"]);
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        let plan = p.plan(&m, now);
        assert_eq!(
            plan.statuses.unwrap()[0].members[1].state,
            MemberState::Joined
        );
        // Penny went on to (1, 2); her `par` no longer lists Lauren.
        let later = now + Duration::from_secs(1);
        let penny = m.get_mut("penny").unwrap();
        penny.room = Some((1, 2));
        penny.game.members.retain(|r| r.name == "Penny Pax");
        penny.game.as_of = Some(later);
        m
    }

    fn state(plan: &Plan, i: usize) -> (MemberState, Option<String>) {
        let st = plan.statuses.as_ref().expect("statuses");
        let m = &st[0].members[i];
        (m.state, m.detail.clone())
    }

    #[tokio::test]
    async fn a_dropped_follower_walks_back_and_the_leader_waits() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = dropped(&mut p, now);
        let t = now + Duration::from_secs(2);
        let plan = p.plan(&m, t);
        assert_eq!(state(&plan, 1), (MemberState::Separated, None));
        assert_eq!(plan.roles["lauren"].rejoin(), Some((1, 2)));
        assert_eq!(
            plan.roles["penny"].hold(),
            Some("Waiting for Lauren to catch up")
        );
        // On the way: how far, and the leader still waits.
        m.get_mut("lauren").unwrap().walk.steps_left = Some(4);
        let plan = p.plan(&m, t);
        assert_eq!(
            state(&plan, 1),
            (MemberState::Walking, Some("4 rooms".into()))
        );
        // Back in her room: forming up, no walk; joined again, the hold lifts.
        let lauren = m.get_mut("lauren").unwrap();
        lauren.room = Some((1, 2));
        lauren.walk.steps_left = None;
        let plan = p.plan(&m, t);
        assert_eq!(state(&plan, 1).0, MemberState::NotJoined);
        assert_eq!(plan.roles["lauren"].rejoin(), None);
        assert!(plan.roles["penny"].hold().is_some());
        let penny = m.get_mut("penny").unwrap();
        penny.game.members.insert(0, row("Lauren Phillips", false));
        penny.game.as_of = Some(t);
        let plan = p.plan(&m, t);
        assert_eq!(state(&plan, 1).0, MemberState::Joined);
        assert_eq!(plan.roles["penny"].hold(), None);
    }

    #[tokio::test]
    async fn no_way_back_is_unreachable() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = dropped(&mut p, now);
        m.get_mut("lauren").unwrap().walk.stuck = Some("no way to Penny's room is known".into());
        let plan = p.plan(&m, now + Duration::from_secs(2));
        assert_eq!(
            state(&plan, 1),
            (
                MemberState::Unreachable,
                Some("no way to Penny's room is known".into())
            )
        );
        assert_eq!(plan.roles["lauren"].rejoin(), None);
    }

    #[tokio::test]
    async fn the_leader_gives_up_at_the_limit_once() {
        let now = Instant::now();
        let mut p = Parties::default();
        let m = dropped(&mut p, now);
        let t = now + Duration::from_secs(2);
        assert!(p.plan(&m, t).give_up.is_empty());
        assert!(p.plan(&m, t + Duration::from_secs(299)).give_up.is_empty());
        let plan = p.plan(&m, t + Duration::from_secs(300));
        assert_eq!(
            plan.give_up,
            [(
                "penny".to_owned(),
                "Lauren did not catch up in 5 minutes".to_owned()
            )]
        );
        // Said once; the leader still holds (it never walks on without her).
        let plan = p.plan(&m, t + Duration::from_secs(400));
        assert!(plan.give_up.is_empty());
        assert!(plan.roles["penny"].hold().is_some());
    }

    #[tokio::test]
    async fn two_out_at_once_are_waited_for_together() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren", "faye"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips", "Faye Reagan"]);
        m.insert("faye".into(), member("Faye Reagan", (1, 1)));
        for f in ["lauren", "faye"] {
            m.get_mut(f).unwrap().game.following = Some("Penny".into());
        }
        p.plan(&m, now);
        let later = now + Duration::from_secs(1);
        let penny = m.get_mut("penny").unwrap();
        penny.room = Some((1, 2));
        penny.game.members.retain(|r| r.name == "Penny Pax");
        penny.game.as_of = Some(later);
        let plan = p.plan(&m, later);
        assert_eq!(
            plan.roles["penny"].hold(),
            Some("Waiting for Lauren and Faye to catch up")
        );
    }

    #[tokio::test]
    async fn followers_stand_while_the_leader_is_away() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = dropped(&mut p, now);
        m.get_mut("penny").unwrap().realm = false;
        let plan = p.plan(&m, now + Duration::from_secs(2));
        assert_eq!(state(&plan, 1).0, MemberState::Away);
        assert_eq!(plan.roles["lauren"].rejoin(), None);
    }

    #[tokio::test]
    async fn joined_from_elsewhere_walks_to_the_leader() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &[]);
        m.insert("faye".into(), member("Faye Reagan", (1, 9)));
        p.join("faye", "party-1", &known()).unwrap();
        let plan = p.plan(&m, now);
        assert_eq!(plan.roles["faye"].rejoin(), Some((1, 1)));
        // A follower saved with the party, never in it, does not walk.
        m.get_mut("lauren").unwrap().room = Some((1, 7));
        let plan = p.plan(&m, now);
        assert_eq!(plan.roles["lauren"].rejoin(), None);
    }

    /// Penny and Lauren joined in one room.
    fn formed(p: &mut Parties, now: Instant) -> BTreeMap<String, MemberInfo> {
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips"]);
        m.get_mut("lauren").unwrap().game.following = Some("Penny".into());
        m
    }

    #[tokio::test]
    async fn a_resync_checks_every_member_and_asks_again_at_once() {
        let now = Instant::now();
        let mut p = Parties::default();
        p.save(def("penny", &["lauren"]), &known()).unwrap();
        let m = two(now, &[]);
        let plan = p.plan(&m, now);
        assert_eq!(lines(&plan), [("lauren", "follow penny")]);
        p.sent("lauren", "follow penny", now);
        // The follow did nothing: the next waits out ASK_EVERY.
        let later = now + Duration::from_secs(1);
        assert!(lines(&p.plan(&m, later)).is_empty());
        // Re-synced: every member owes a `par`, and the follow goes again at once.
        assert_eq!(p.resync(Some("party-9")), None);
        assert_eq!(p.resync(None).unwrap(), ["penny", "lauren"]);
        let plan = p.plan(&m, later);
        assert_eq!(
            lines(&plan),
            [
                ("lauren", "follow penny"),
                ("lauren", "par"),
                ("penny", "par")
            ]
        );
        assert!(plan.statuses.is_some(), "the panel hears of it");
        // Once each `par` has gone, no more.
        p.sent("penny", "par", later);
        p.sent("lauren", "par", later);
        p.sent("lauren", "follow penny", later);
        assert!(lines(&p.plan(&m, later)).is_empty());
    }

    #[tokio::test]
    async fn a_resync_starts_the_wait_for_a_separated_member_afresh() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        p.plan(&m, now);
        // Dropped and elsewhere: the leader waits, then gives up.
        let l = m.get_mut("lauren").unwrap();
        l.game.following = None;
        l.room = Some((1, 7));
        p.plan(&m, now);
        let late = now + Duration::from_secs(301);
        assert_eq!(p.plan(&m, late).give_up.len(), 1);
        assert!(p.plan(&m, late).give_up.is_empty(), "once");
        p.resync(None);
        assert!(p.plan(&m, late).give_up.is_empty(), "the wait starts again");
        let later = late + Duration::from_secs(301);
        assert_eq!(p.plan(&m, later).give_up.len(), 1);
    }

    /// Penny leading Lauren and Faye, all joined in one room, Penny's `par` at `now` showing
    /// each one's rank; each one's `party.rank` is `want`.
    fn three(
        p: &mut Parties,
        now: Instant,
        at: [Rank; 3],
        want: [Rank; 3],
    ) -> BTreeMap<String, MemberInfo> {
        p.save(def("penny", &["lauren", "faye"]), &known()).unwrap();
        let mut m = two(now, &["Lauren Phillips", "Faye Reagan"]);
        m.insert("faye".into(), member("Faye Reagan", (1, 1)));
        for f in ["lauren", "faye"] {
            m.get_mut(f).unwrap().game.following = Some("Penny".into());
        }
        show(&mut m, now, at);
        for (s, w) in ["penny", "lauren", "faye"].into_iter().zip(want) {
            m.get_mut(s).unwrap().rank = Some(w);
        }
        m
    }

    /// Penny's `par` at `now` shows Penny, Lauren and Faye at `at`.
    fn show(m: &mut BTreeMap<String, MemberInfo>, now: Instant, at: [Rank; 3]) {
        let penny = m.get_mut("penny").unwrap();
        penny.game.as_of = Some(now);
        for (row, rank) in penny.game.members.iter_mut().zip([at[1], at[2], at[0]]) {
            row.rank = Some(rank);
        }
    }

    fn rank_lines(plan: &Plan) -> Vec<(&str, &str)> {
        lines(plan)
            .into_iter()
            .filter(|(_, l)| l.ends_with("rank"))
            .collect()
    }

    use Rank::{Back, Front, Mid};

    #[tokio::test]
    async fn members_go_to_their_rank_the_front_first_then_one_at_a_time() {
        let now = Instant::now();
        let mut p = Parties::default();
        // Lauren (a caster) wants the back, Faye the front; both joined at Midrank.
        let mut m = three(&mut p, now, [Front, Mid, Mid], [Front, Back, Front]);
        let plan = p.plan(&m, now);
        assert_eq!(rank_lines(&plan), [("faye", "frontrank")]);
        let statuses = plan.statuses.unwrap();
        let ranks: Vec<_> = statuses[0].members.iter().map(|ms| ms.rank).collect();
        assert_eq!(
            ranks,
            [
                Some(PartyRank::Front),
                Some(PartyRank::Mid),
                Some(PartyRank::Mid)
            ]
        );
        p.sent("faye", "frontrank", now);
        // Nothing more until a screen shows it.
        let later = now + Duration::from_secs(2);
        assert!(rank_lines(&p.plan(&m, later)).is_empty());
        show(&mut m, later, [Front, Mid, Front]);
        let plan = p.plan(&m, later);
        assert_eq!(rank_lines(&plan), [("lauren", "backrank")]);
        p.sent("lauren", "backrank", later);
        let after = later + Duration::from_secs(2);
        show(&mut m, after, [Front, Back, Front]);
        let plan = p.plan(&m, after);
        assert!(rank_lines(&plan).is_empty());
        assert!(plan.notes.is_empty());
    }

    #[tokio::test]
    async fn a_rank_the_game_forbids_is_not_asked_for_and_the_tile_is_told_once() {
        let now = Instant::now();
        let mut p = Parties::default();
        // The leader cannot go back: it stays in front, and its tile is told; Lauren may go
        // back behind it (one in front, none in back).
        let m = three(&mut p, now, [Front, Mid, Mid], [Back, Back, Mid]);
        let plan = p.plan(&m, now);
        assert_eq!(rank_lines(&plan), [("lauren", "backrank")]);
        assert_eq!(
            plan.notes,
            [(
                "penny".to_owned(),
                "Penny stays in the front rank: a party's leader cannot go to the back rank"
                    .to_owned()
            )]
        );
        let again = p.plan(&m, now);
        assert!(again.notes.is_empty(), "once");
        // The last one in front may not leave it.
        let mut p = Parties::default();
        let m = three(&mut p, now, [Front, Mid, Mid], [Mid, Mid, Mid]);
        let plan = p.plan(&m, now);
        assert!(rank_lines(&plan).is_empty());
        assert!(
            plan.notes[0]
                .1
                .contains("someone must stay in the front rank")
        );
        // The back may not outnumber the front: front 1, back 2 already.
        let mut p = Parties::default();
        let m = three(&mut p, now, [Front, Back, Mid], [Front, Back, Back]);
        let plan = p.plan(&m, now);
        assert_eq!(
            rank_lines(&plan),
            [("faye", "backrank")],
            "1 front, 1 back: allowed"
        );
        let mut p = Parties::default();
        let mut m = three(&mut p, now, [Front, Back, Back], [Front, Back, Back]);
        m.get_mut("lauren").unwrap().rank = Some(Back);
        assert!(rank_lines(&p.plan(&m, now)).is_empty(), "already there");
    }

    #[tokio::test]
    async fn a_refused_rank_stops_until_a_resync_and_a_rest_is_not_broken() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = three(&mut p, now, [Front, Mid, Front], [Front, Back, Front]);
        assert_eq!(rank_lines(&p.plan(&m, now)), [("lauren", "backrank")]);
        p.sent("lauren", "backrank", now);
        let later = now + Duration::from_secs(1);
        m.get_mut("lauren").unwrap().game.rank_refused = Some(later);
        show(&mut m, later + Duration::from_secs(1), [Front, Mid, Front]);
        let plan = p.plan(&m, later + Duration::from_secs(1));
        assert!(rank_lines(&plan).is_empty());
        assert!(
            plan.notes[0]
                .1
                .contains("the game would not move it to the back rank")
        );
        let quiet = p.plan(&m, later + Duration::from_secs(30));
        assert!(rank_lines(&quiet).is_empty() && quiet.notes.is_empty());
        // Re-synced: asked again, unless it is resting.
        p.resync(None);
        m.get_mut("lauren").unwrap().busy = true;
        let t = later + Duration::from_secs(31);
        show(&mut m, t, [Front, Mid, Front]);
        assert!(rank_lines(&p.plan(&m, t)).is_empty(), "resting");
        m.get_mut("lauren").unwrap().busy = false;
        assert_eq!(rank_lines(&p.plan(&m, t)), [("lauren", "backrank")]);
    }

    fn hurt(i: &mut MemberInfo, hp: i32, low: bool, flee: bool) {
        i.health = Health {
            hp: Some(hp),
            missing: Some(i64::from(100 - hp)),
            low,
            rested: false,
            flee,
            until: 85,
            short: Some(i64::from((85 - hp).max(0))),
            hp_rate: Some(600),
            mana_wait: Some(0),
        };
    }

    #[tokio::test]
    async fn one_member_low_rests_the_party_until_all_are_back() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        hurt(m.get_mut("lauren").unwrap(), 40, true, false);
        hurt(m.get_mut("penny").unwrap(), 70, false, false);
        let plan = p.plan(&m, now);
        assert_eq!(
            plan.roles["penny"].party_rest(),
            Some("Resting for Penny (70%), Lauren (40%)")
        );
        assert!(plan.roles["penny"].hold().is_some());
        assert_eq!(state(&plan, 1), (MemberState::Resting, Some("40%".into())));
        // Lauren above her rest line but not her rest-until: still resting.
        hurt(m.get_mut("lauren").unwrap(), 60, false, false);
        m.get_mut("penny").unwrap().health.rested = true;
        let plan = p.plan(&m, now);
        assert_eq!(
            plan.roles["penny"].party_rest(),
            Some("Resting for Lauren (60%)")
        );
        // The follower rests with the party too (C-045): above its rest line, it would
        // otherwise stand and get its HP back at the walking rate.
        assert_eq!(
            plan.roles["lauren"].party_rest(),
            Some("Resting for Lauren (60%)")
        );
        // Everyone rested: the rest ends and the loop goes on.
        m.get_mut("lauren").unwrap().health.rested = true;
        let plan = p.plan(&m, now);
        assert_eq!(plan.roles["penny"].party_rest(), None);
        assert_eq!(plan.roles["lauren"].party_rest(), None);
        assert_eq!(plan.roles["penny"].hold(), None);
    }

    #[tokio::test]
    async fn a_follower_out_of_the_room_does_not_rest_for_the_party() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        hurt(m.get_mut("penny").unwrap(), 40, true, false);
        m.get_mut("lauren").unwrap().room = Some((1, 9));
        let plan = p.plan(&m, now);
        assert!(plan.roles["penny"].party_rest().is_some());
        assert_eq!(plan.roles["lauren"].party_rest(), None);
    }

    #[tokio::test]
    async fn a_rest_that_never_ends_stops_the_loop_once() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        hurt(m.get_mut("lauren").unwrap(), 40, true, false);
        assert!(p.plan(&m, now).give_up.is_empty());
        let plan = p.plan(&m, now + Duration::from_secs(600));
        assert_eq!(
            plan.give_up,
            [(
                "penny".to_owned(),
                "the party rested 10 minutes; not recovered: Lauren (40%)".to_owned()
            )]
        );
        assert!(
            p.plan(&m, now + Duration::from_secs(700))
                .give_up
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_follower_at_its_flee_line_makes_the_leader_flee_once() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        hurt(m.get_mut("lauren").unwrap(), 20, true, true);
        let plan = p.plan(&m, now);
        assert_eq!(
            plan.flee,
            [("penny".to_owned(), "Lauren at 20%".to_owned())]
        );
        assert!(plan.roles["lauren"].with_leader());
        // Still under it: no second flee until she is above it again.
        assert!(p.plan(&m, now).flee.is_empty());
        hurt(m.get_mut("lauren").unwrap(), 50, false, false);
        p.plan(&m, now);
        hurt(m.get_mut("lauren").unwrap(), 20, true, true);
        assert_eq!(p.plan(&m, now).flee.len(), 1);
    }

    #[tokio::test]
    async fn a_follower_out_of_the_leaders_room_flees_on_its_own() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        m.get_mut("lauren").unwrap().room = Some((1, 9));
        hurt(m.get_mut("lauren").unwrap(), 20, true, true);
        let plan = p.plan(&m, now);
        assert!(plan.flee.is_empty());
        assert!(!plan.roles["lauren"].with_leader());
    }

    #[tokio::test]
    async fn a_follower_with_the_leader_goes_for_its_target() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        m.get_mut("penny").unwrap().target = Some("giant rat".into());
        let plan = p.plan(&m, now);
        assert_eq!(plan.roles["lauren"].assist(), Some("giant rat"));
        m.get_mut("lauren").unwrap().room = Some((1, 9));
        assert_eq!(p.plan(&m, now).roles["lauren"].assist(), None);
    }

    #[tokio::test]
    async fn a_follower_with_the_leader_sees_the_door_it_bashes() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        assert_eq!(p.plan(&m, now).roles["lauren"].help_bash(), None);
        m.get_mut("penny").unwrap().bashing = Some(((1, 1), 2));
        let plan = p.plan(&m, now);
        assert_eq!(plan.roles["lauren"].help_bash(), Some(((1, 1), 2)));
        assert_eq!(plan.roles["penny"].help_bash(), None);
        m.get_mut("lauren").unwrap().room = Some((1, 9));
        assert_eq!(p.plan(&m, now).roles["lauren"].help_bash(), None);
    }

    #[tokio::test]
    async fn a_healer_sees_the_members_with_it() {
        let now = Instant::now();
        let mut p = Parties::default();
        let mut m = formed(&mut p, now);
        hurt(m.get_mut("lauren").unwrap(), 40, true, false);
        let plan = p.plan(&m, now);
        assert_eq!(
            plan.roles["penny"].mates(),
            [Mate {
                name: "lauren".into(),
                hp: 40,
                missing: 60,
                flee: false,
                until: 85,
                short: 45,
                hp_rate: Some(600),
                mana_wait: Some(0),
            }]
        );
        assert_eq!(plan.roles["lauren"].mates()[0].name, "penny");
        // Out of the room: no mates.
        m.get_mut("lauren").unwrap().room = Some((1, 9));
        assert!(p.plan(&m, now).roles["penny"].mates().is_empty());
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
