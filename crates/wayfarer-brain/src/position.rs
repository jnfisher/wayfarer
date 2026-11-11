//! Where a character is: the position [`Belief`] over the room graph, fed by every room the
//! game shows and every move sent (by the brain or the player), the [`Traveller`] its routes
//! are planned for, with the exits the game refused this session, the doors it showed open,
//! shut or locked, the hidden exits it found, and the rooms visited.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{CharacterState, ExitKind, MapExit, MapRoom, RoomKey, ToGateway};
use wayfarer_world::{
    Belief, DirSet, Key, LockState, SegmentKind, Traveller, layout, mm, pocket_exits,
};

use crate::brain::Atlas;

/// How long a hidden exit found by a `search` shows: it hides again after one step of 300
/// fast ticks, about five minutes (`kb/navigation.md` "Hidden"). After that the walker
/// searches for it again.
/// A reach of this many rooms or fewer is a sealed pocket: its Hidden exits out are taken as known.
pub const POCKET_ROOMS: usize = 60;
pub const HIDDEN_FOR: Duration = Duration::from_secs(5 * 60);
/// How long an exit the walker gave up on (a door that would not bash open, a refused move, a
/// hidden exit not found or not opened) stays out of the routes (C-091). A door that failed its
/// bashes was unlucky, not shut for good; a guard moves on; a lever is pulled by someone else.
pub const BLOCKED_FOR: Duration = Duration::from_secs(10 * 60);

/// A room as the game showed it: its title (when it was not lost behind a prompt), the
/// directions of its `Obvious exits` (when that line came), or too dark to see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoomSeen {
    pub title: Option<String>,
    pub exits: Option<DirSet>,
    pub dark: bool,
}

/// The directions of an `Obvious exits` list; exits shown by their own text are left out.
fn dir_set(exits: &[wayfarer_parse::helpers::Exit]) -> DirSet {
    exits
        .iter()
        .filter_map(|e| mm::direction(e.dir.as_deref()?))
        .map(|d| d as u8)
        .collect()
}

#[derive(Debug, Default)]
pub(crate) struct Position {
    pub belief: Belief,
    pub traveller: Traveller,
    /// The last room title seen.
    pub title: Option<String>,
    /// The directions of the last room's `Obvious exits`; `None` when it showed none (dark, or
    /// lost behind a prompt).
    pub exits: Option<DirSet>,
    /// Rooms the character was certainly in since the brain started.
    pub visited: HashSet<Key>,
    /// When each hidden exit in the traveller's `found` was found.
    found_at: BTreeMap<(Key, u8), Instant>,
    /// When each exit in the traveller's `blocked` was first seen blocked by [`Position::refresh`]
    /// (C-091): it is taken again [`BLOCKED_FOR`] after.
    blocked_at: BTreeMap<(Key, u8), Instant>,
    /// The rooms whose pockets were last looked at ([`POCKET_ROOMS`]).
    pocket_at: BTreeSet<Key>,
    /// Moves that went to the game and whose rooms have not come yet, oldest first: one, or
    /// two with a move pipelined ahead of the first.
    moving: VecDeque<String>,
    /// The direction of an `open` or `bash` that went to the game, until its answer.
    door_cmd: Option<u8>,
    /// A room title waiting for its exits.
    title_pending: Option<String>,
    /// Entries for the position log (C-085), until the session sends them.
    pub traces: Vec<serde_json::Value>,
    /// The next room seen with a title is placed afresh ([`Position::afresh`]).
    afresh: bool,
    /// The phrase of the move in flight came back as speech (`You say "go hole"`): the
    /// character did not move (C-086). Cleared by the next move sent.
    spoken: Option<String>,
}

impl Position {
    /// Whether `text` moves the character from where it may be: a direction, or the phrase of
    /// a Text exit of a candidate room.
    pub fn is_move(&self, atlas: &Atlas, text: &str) -> bool {
        mm::direction(text).is_some()
            || self
                .belief
                .candidates()
                .iter()
                .any(|&k| atlas.graph.edges(k).iter().any(|e| e.taken_by(text)))
    }

    /// A move went to the game ahead of the one whose room has not come yet (pipelining): its
    /// room comes after that one's.
    pub fn sent_ahead(&mut self, text: &str) {
        self.moving.push_back(text.trim().to_owned());
    }

    /// A line went to the game (from the brain or the player).
    pub fn sent(&mut self, atlas: &Atlas, text: &str) {
        if self.is_move(atlas, text) {
            // A move that never got its room is forgotten: it went nowhere.
            self.moving.clear();
            self.spoken = None;
            self.moving.push_back(text.trim().to_owned());
        }
        let mut words = text.split_whitespace();
        if let (Some(verb), Some(dir), None) = (words.next(), words.next(), words.next())
            && ["open", "bash"].contains(&verb.to_ascii_lowercase().as_str())
            && let Some(d) = mm::direction(dir)
        {
            self.door_cmd = Some(d as u8);
        }
    }

    /// The door `dir` of the room the character is sure to be in is in `state`.
    fn door(&mut self, dir: Option<u8>, state: LockState) {
        if let (Some(dir), Some(k), true) = (dir, self.belief.best(), self.belief.certain()) {
            self.traveller.doors.insert((k, dir), state);
        }
    }

    /// A hidden exit was found `now` in the room the character is sure to be in: it shows for
    /// [`HIDDEN_FOR`], and is known to be there for the rest of the session.
    fn found(&mut self, dir: &str, now: Instant) {
        if let (Some(d), Some(k), true) = (
            mm::direction(dir),
            self.belief.best(),
            self.belief.certain(),
        ) {
            let at = (k, d as u8);
            self.traveller.found.insert(at);
            self.traveller.searchable.insert(at);
            self.found_at.insert(at, now);
        }
    }

    /// Forget the hidden exits found [`HIDDEN_FOR`] ago or more: they have hidden again.
    pub fn forget_hidden(&mut self, now: Instant) {
        let gone: Vec<(Key, u8)> = self
            .found_at
            .iter()
            .filter(|&(_, &at)| now.saturating_duration_since(at) >= HIDDEN_FOR)
            .map(|(&k, _)| k)
            .collect();
        for at in gone {
            self.found_at.remove(&at);
            self.traveller.found.remove(&at);
        }
    }

    /// Whether exit `dir` of `room` shows now: in the last exits line, if the character is
    /// sure to be there, or found less than [`HIDDEN_FOR`] ago.
    pub fn showing(&self, room: Key, dir: u8) -> bool {
        self.traveller.found.contains(&(room, dir))
            || (self.belief.certain()
                && self.belief.best() == Some(room)
                && self.exits.is_some_and(|e| e.contains(dir)))
    }

    /// Read one typed event at `now`; the room, when this event completed one. A room seen
    /// moves the belief through the move in flight first (if there was one), then narrows it.
    pub fn event(&mut self, atlas: &Atlas, ev: &GameEvent, now: Instant) -> Option<RoomSeen> {
        let seen = match ev {
            GameEvent::RoomTitle { title } => {
                self.title_pending = Some(title.clone());
                return None;
            }
            GameEvent::RoomExits { exits } => RoomSeen {
                title: self.title_pending.take(),
                exits: Some(dir_set(exits)),
                dark: false,
            },
            GameEvent::RoomDark => RoomSeen {
                title: self.title_pending.take(),
                exits: None,
                dark: true,
            },
            // A title the prompt came straight after: no exits line was shown.
            GameEvent::Prompt { .. } if self.title_pending.is_some() => RoomSeen {
                title: self.title_pending.take(),
                exits: None,
                dark: false,
            },
            GameEvent::MoveRefused { .. } => {
                self.moving.pop_front();
                return None;
            }
            GameEvent::DoorClosed { locked } => {
                let dir = self.moving.pop_front().and_then(|m| mm::direction(&m));
                let state = if *locked {
                    LockState::Locked
                } else {
                    LockState::Closed
                };
                self.door(dir.map(|d| d as u8), state);
                return None;
            }
            GameEvent::DoorOpened | GameEvent::BashResult { ok: true } => {
                let dir = self.door_cmd.take();
                self.door(dir, LockState::Open);
                return None;
            }
            GameEvent::OpenRefused => {
                let dir = self.door_cmd.take();
                self.door(dir, LockState::Locked);
                return None;
            }
            GameEvent::SearchResult { found: true, dir } => {
                self.found(dir, now);
                return None;
            }
            // The status screen's picklocks skill: the odds of picking a lock.
            // The phrase in flight was only speech: no way through here (C-086).
            GameEvent::Other(ev) if ev.name == "you_say" => {
                let said = ev.text("msg").unwrap_or("").trim();
                if self
                    .moving
                    .front()
                    .is_some_and(|m| m.eq_ignore_ascii_case(said))
                {
                    let phrase = self.moving.pop_front();
                    self.traces.push(serde_json::json!({
                        "kind": "speech",
                        "phrase": phrase,
                        "cands": crate::walker::cands_str(&self.belief),
                    }));
                    self.spoken = phrase;
                }
                return None;
            }
            GameEvent::StatusStat { stats } => {
                if let Some((_, n)) = stats.iter().find(|(label, _)| label == "Picklocks") {
                    self.traveller.picklocks = Some(*n);
                }
                return None;
            }
            _ => return None,
        };
        let moved = self.moving.pop_front();
        if let Some(cmd) = &moved {
            self.belief.moved(&atlas.graph, cmd);
        }
        if self.afresh && seen.title.is_some() && !seen.dark {
            self.afresh = false;
            self.belief = Belief::unknown();
        }
        let how = self
            .belief
            .observe(&atlas.graph, seen.title.as_deref(), seen.exits, seen.dark);
        const NAMES: [&str; 10] = ["n", "s", "e", "w", "ne", "nw", "se", "sw", "u", "d"];
        self.traces.push(serde_json::json!({
            "kind": "room",
            "move": moved,
            "title": seen.title,
            "exits": seen.exits.map(|x| {
                x.iter().map(|d| NAMES[usize::from(d)]).collect::<Vec<_>>().join(",")
            }),
            "how": how.as_str(),
            "count": self.belief.candidates().len(),
            "cands": crate::walker::cands_str(&self.belief),
        }));
        if seen.title.is_some() {
            self.title.clone_from(&seen.title);
        }
        self.exits = seen.exits;
        if let GameEvent::RoomExits { exits } = ev {
            self.blanks_shown(atlas, exits);
            self.doors_shown(exits);
            self.hidden_shown(atlas, now);
        }
        if self.belief.certain()
            && let Some(k) = self.belief.best()
        {
            self.visited.insert(k);
        }
        Some(seen)
    }

    /// The empty slots of an exits line (`north, , west`) are the room's opened Hidden exits
    /// whose name is blank (C-076): only a room with as many of them, not otherwise listed, can
    /// show the line, which narrows the belief (the Labyrinth's rooms share a name and exits),
    /// and once it is sure the slots count as those exits, so the line shows them.
    fn blanks_shown(&mut self, atlas: &Atlas, exits: &[wayfarer_parse::helpers::Exit]) {
        let slots = exits
            .iter()
            .filter(|e| e.dir.is_none() && e.state.as_deref() == Some(""))
            .count();
        let Some(listed) = self.exits.filter(|_| slots > 0) else {
            return;
        };
        let blank = |k: Key| -> Vec<u8> {
            atlas
                .graph
                .edges(k)
                .iter()
                .filter(|e| e.lists_blank && !listed.contains(e.dir))
                .map(|e| e.dir)
                .collect()
        };
        self.belief.narrow(|k| blank(k).len() == slots);
        if let (Some(k), true, Some(shown)) = (
            self.belief.best(),
            self.belief.certain(),
            self.exits.as_mut(),
        ) {
            let dirs = blank(k);
            if dirs.len() == slots {
                for d in dirs {
                    shown.insert(d);
                }
            }
        }
    }

    /// The Hidden exits an exits line shows (found lately, by anyone: `secret passage south`)
    /// are known to be there: routes may take them, and the walker moves through while they
    /// show. When they were found is not known, so they are not counted as found. A sealed one
    /// (opened from elsewhere, a lever) is never searched for: it counts as found from when a
    /// line first shows it until a line of its room does not, and routes take it meanwhile.
    fn hidden_shown(&mut self, atlas: &Atlas, now: Instant) {
        let (Some(k), true, Some(shown)) = (self.belief.best(), self.belief.certain(), self.exits)
        else {
            return;
        };
        for e in atlas.graph.edges(k).iter().filter(|e| e.hidden()) {
            let at = (k, e.dir);
            match (e.sealed(), shown.contains(e.dir)) {
                (true, true) => {
                    self.traveller.found.insert(at);
                    self.found_at.entry(at).or_insert(now);
                }
                (true, false) => {
                    self.traveller.found.remove(&at);
                    self.found_at.remove(&at);
                }
                (false, true) => {
                    self.traveller.searchable.insert(at);
                }
                (false, false) => {}
            }
        }
    }

    /// The doors an exits line shows: open, or closed (locked or not: it does not say, so a
    /// door seen open before goes back to what the pack says).
    fn doors_shown(&mut self, exits: &[wayfarer_parse::helpers::Exit]) {
        let (Some(k), true) = (self.belief.best(), self.belief.certain()) else {
            return;
        };
        for e in exits {
            let Some(d) = e.dir.as_deref().and_then(mm::direction) else {
                continue;
            };
            let at = (k, d as u8);
            let doors = &mut self.traveller.doors;
            match e.state.as_deref() {
                Some("open") => {
                    doors.insert(at, LockState::Open);
                }
                Some("closed") if doors.get(&at) == Some(&LockState::Open) => {
                    doors.remove(&at);
                }
                _ => {}
            }
        }
    }

    /// The game refused `command` (anything but "no exit"): mark the exits it would have taken
    /// from the candidates, so routes avoid them for the rest of the session.
    /// Whether `phrase`, the last move sent, came back as speech (C-086).
    pub fn spoken(&self, phrase: &str) -> bool {
        self.spoken
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case(phrase.trim()))
    }

    /// Place the next room seen afresh (C-085): among every room of its name and exits, not
    /// only the candidates so far. The re-sync's `look` does this, so a guess made from the
    /// output before it can't stay certain only because the room fits. The belief stands
    /// until that room comes.
    pub fn afresh(&mut self, why: &str) {
        self.afresh = true;
        self.traces
            .push(serde_json::json!({"kind": "afresh", "why": why}));
    }

    /// A `sysop goto` landed: the character is in `room` (C-080). The game shows no room for
    /// it, so the belief is told; the re-sync's `look` checks it.
    pub fn teleported(&mut self, room: Key) {
        self.belief = Belief::at(room);
        self.moving.clear();
        self.door_cmd = None;
        self.title_pending = None;
        self.visited.insert(room);
    }

    pub fn refused(&mut self, atlas: &Atlas, command: &str) {
        let exits = self.belief.refused(&atlas.graph, command);
        self.traveller.blocked.extend(exits);
    }

    /// Mark one exit blocked (a door that would not bash open), for [`BLOCKED_FOR`].
    pub fn block(&mut self, from: Key, dir: u8) {
        self.traveller.blocked.insert((from, dir));
    }

    /// Forget every exit blocked so far: the player started a walk or a loop (C-091), and a
    /// few unlucky bashes earlier must not leave it with no route.
    pub fn clear_blocks(&mut self) {
        self.traveller.blocked.clear();
        self.blocked_at.clear();
    }

    /// Time the blocks: each new one from `now`, and the ones [`BLOCKED_FOR`] old are taken
    /// again.
    fn expire_blocks(&mut self, now: Instant) {
        for &at in &self.traveller.blocked {
            self.blocked_at.entry(at).or_insert(now);
        }
        let gone: Vec<(Key, u8)> = self
            .blocked_at
            .iter()
            .filter(|&(_, &at)| now.saturating_duration_since(at) >= BLOCKED_FOR)
            .map(|(&k, _)| k)
            .collect();
        for at in gone {
            self.blocked_at.remove(&at);
            self.traveller.blocked.remove(&at);
        }
    }

    /// Plan for the character as the model knows it at `now`: level, class, race and strength;
    /// the hidden exits found long enough ago have hidden again.
    pub fn refresh(&mut self, atlas: &Atlas, c: &CharacterState, now: Instant) {
        self.forget_hidden(now);
        self.expire_blocks(now);
        self.traveller.strength = c.stats.as_ref().and_then(|s| s.strength).map(i64::from);
        self.traveller.level = c.level.map(i64::from);
        self.traveller.class = c.class.as_ref().and_then(|n| atlas.classes.get(n).copied());
        self.traveller.race = c.race.as_ref().and_then(|n| atlas.races.get(n).copied());
        // Shut in with only a hidden way out (a room entered some other way): it is searched for.
        // Unsure which of a few rooms: each of them is looked at.
        if self.belief.candidates() != &self.pocket_at && self.belief.candidates().len() <= 8 {
            self.pocket_at = self.belief.candidates().clone();
            for &k in &self.pocket_at {
                let exits = pocket_exits(&atlas.graph, k, POCKET_ROOMS, &self.traveller);
                self.traveller.searchable.extend(exits);
            }
        }
    }

    /// The believed room: the lowest key among the candidates.
    pub fn best(&self) -> Option<Key> {
        self.belief.best()
    }

    /// The `map_view` around the believed room, with the walk's `route` if there is one;
    /// `None` when the position is unknown.
    pub fn map_view(
        &self,
        atlas: &Atlas,
        session: &str,
        radius: u32,
        route: Option<Vec<Key>>,
    ) -> Option<ToGateway> {
        let center = self.belief.best()?;
        let lay = layout(&atlas.graph, center, radius);
        let key = |k: Key| RoomKey {
            map: k.0,
            room: k.1,
        };
        let rooms = lay
            .rooms
            .iter()
            .map(|p| {
                let node = atlas.graph.node(p.key);
                MapRoom {
                    map: p.key.0,
                    room: p.key.1,
                    name: node.map_or_else(String::new, |n| n.room.name.clone()),
                    x: p.x,
                    y: p.y,
                    level: p.level,
                    spawns: node.is_some_and(|n| n.spawns),
                    kind: atlas.kinds.get(&p.key).copied(),
                    visited: self.visited.contains(&p.key),
                    candidate: self.belief.contains(p.key),
                    collides: p.collides,
                }
            })
            .collect();
        let exits = lay
            .segments
            .iter()
            .map(|s| MapExit {
                from: key(s.from),
                to: key(s.to),
                dir: mm::DIRS[usize::from(s.dir)].to_owned(),
                kind: match s.kind {
                    SegmentKind::Plain => ExitKind::Plain,
                    SegmentKind::Door => ExitKind::Door,
                    SegmentKind::Hidden => ExitKind::Hidden,
                    SegmentKind::Text => ExitKind::Text,
                },
                one_way: s.one_way,
            })
            .collect();
        Some(ToGateway::MapView {
            session: session.to_owned(),
            center: key(center),
            level: 0,
            rooms,
            exits,
            route: route.map(|r| r.into_iter().map(key).collect()),
        })
    }

    /// The wire's view of it.
    pub fn snapshot(&self) -> wayfarer_proto::Position {
        let best = self.belief.best();
        wayfarer_proto::Position {
            map: best.map(|k| k.0),
            room: best.map(|k| k.1),
            candidates: u32::try_from(self.belief.candidates().len()).unwrap_or(u32::MAX),
            title: self.title.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_blocked_exit_is_taken_again_after_ten_minutes_or_at_once_when_cleared() {
        // C-091: a door that failed its bashes is unlucky, not shut for good.
        let mut p = Position::default();
        let t0 = Instant::now();
        p.block((1, 2), 0);
        p.expire_blocks(t0);
        p.expire_blocks(t0 + BLOCKED_FOR - Duration::from_secs(1));
        assert!(p.traveller.blocked.contains(&((1, 2), 0)));
        p.expire_blocks(t0 + BLOCKED_FOR);
        assert!(p.traveller.blocked.is_empty());
        // Cleared by a walk the player asks for.
        p.block((1, 3), 1);
        p.expire_blocks(t0 + BLOCKED_FOR);
        p.clear_blocks();
        assert!(p.traveller.blocked.is_empty() && p.blocked_at.is_empty());
    }
}
