//! Where a character is: the position [`Belief`] over the room graph, fed by every room the
//! game shows and every move sent (by the brain or the player), the [`Traveller`] its routes
//! are planned for, with the exits the game refused this session, the doors it showed open,
//! shut or locked, the hidden exits it found, and the rooms visited.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{CharacterState, ExitKind, MapExit, MapRoom, RoomKey, ToGateway};
use wayfarer_world::{Belief, DirSet, Key, LockState, SegmentKind, Traveller, layout, mm};

use crate::brain::Atlas;

/// How long a hidden exit found by a `search` shows: it hides again after one step of 300
/// fast ticks, about five minutes (`kb/navigation.md` "Hidden"). After that the walker
/// searches for it again.
pub const HIDDEN_FOR: Duration = Duration::from_secs(5 * 60);

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
    /// Moves that went to the game and whose rooms have not come yet, oldest first: one, or
    /// two with a move pipelined ahead of the first.
    moving: VecDeque<String>,
    /// The direction of an `open` or `bash` that went to the game, until its answer.
    door_cmd: Option<u8>,
    /// A room title waiting for its exits.
    title_pending: Option<String>,
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
            GameEvent::StatusStat { stats } => {
                if let Some((_, n)) = stats.iter().find(|(label, _)| label == "Picklocks") {
                    self.traveller.picklocks = Some(*n);
                }
                return None;
            }
            _ => return None,
        };
        if let Some(cmd) = self.moving.pop_front() {
            self.belief.moved(&atlas.graph, &cmd);
        }
        self.belief
            .observe(&atlas.graph, seen.title.as_deref(), seen.exits, seen.dark);
        if seen.title.is_some() {
            self.title.clone_from(&seen.title);
        }
        self.exits = seen.exits;
        if let GameEvent::RoomExits { exits } = ev {
            self.doors_shown(exits);
        }
        if self.belief.certain()
            && let Some(k) = self.belief.best()
        {
            self.visited.insert(k);
        }
        Some(seen)
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
    pub fn refused(&mut self, atlas: &Atlas, command: &str) {
        let exits = self.belief.refused(&atlas.graph, command);
        self.traveller.blocked.extend(exits);
    }

    /// Mark one exit blocked (a door that would not bash open).
    pub fn block(&mut self, from: Key, dir: u8) {
        self.traveller.blocked.insert((from, dir));
    }

    /// Plan for the character as the model knows it at `now`: level, class, race and strength;
    /// the hidden exits found long enough ago have hidden again.
    pub fn refresh(&mut self, atlas: &Atlas, c: &CharacterState, now: Instant) {
        self.forget_hidden(now);
        self.traveller.strength = c.stats.as_ref().and_then(|s| s.strength).map(i64::from);
        self.traveller.level = c.level.map(i64::from);
        self.traveller.class = c.class.as_ref().and_then(|n| atlas.classes.get(n).copied());
        self.traveller.race = c.race.as_ref().and_then(|n| atlas.races.get(n).copied());
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
