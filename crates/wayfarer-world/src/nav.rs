//! The room graph: every room and exit of the pack in memory, with what it takes to go through
//! each exit (`kb/navigation.md` "Exit types and how to take them"). It holds no state: doors
//! the character opened, hidden exits found and exits the game refused belong to the caller
//! ([`crate::route::Traveller`]).

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Range;

use crate::monsters::Monsters;
use crate::pack::{Room, WorldPack};
use crate::{Error, mm};

pub use crate::walk::Key;

/// The game's exit types, in `exittypes` code order (`mm::EXIT_TYPES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExitType {
    Normal,
    Spell,
    Key,
    Item,
    Toll,
    Action,
    Hidden,
    Door,
    MapChange,
    Trap,
    Text,
    Gate,
    RemoteAction,
    Class,
    Race,
    Level,
    Timed,
    Ticket,
    UserCount,
    BlockGuard,
    Alignment,
    Delay,
    Cast,
    Ability,
    SpellTrap,
}

const EXIT_TYPES: [ExitType; 25] = {
    use ExitType::*;
    [
        Normal,
        Spell,
        Key,
        Item,
        Toll,
        Action,
        Hidden,
        Door,
        MapChange,
        Trap,
        Text,
        Gate,
        RemoteAction,
        Class,
        Race,
        Level,
        Timed,
        Ticket,
        UserCount,
        BlockGuard,
        Alignment,
        Delay,
        Cast,
        Ability,
        SpellTrap,
    ]
};

/// Whether an exit shows in `Obvious exits` (`kb/screen-patterns.md` section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listing {
    Always,
    /// Shown or not depending on state: a Trap or Timed exit, or a Hidden one once found.
    Maybe,
    /// Text and Remote Action exits are never listed.
    Never,
}

impl ExitType {
    /// The pack's name for a type: `"Normal"`, `"Map Change"`, ...
    pub fn from_name(name: &str) -> Option<ExitType> {
        mm::EXIT_TYPES
            .iter()
            .position(|n| *n == name)
            .map(|i| EXIT_TYPES[i])
    }

    pub fn name(self) -> &'static str {
        mm::EXIT_TYPES[self as usize]
    }

    pub fn listing(self) -> Listing {
        match self {
            ExitType::Text | ExitType::RemoteAction => Listing::Never,
            ExitType::Hidden | ExitType::Trap | ExitType::SpellTrap | ExitType::Timed => {
                Listing::Maybe
            }
            _ => Listing::Always,
        }
    }

    /// Doors, gates and key doors: they have a lock state.
    pub fn is_door(self) -> bool {
        matches!(self, ExitType::Door | ExitType::Gate | ExitType::Key)
    }
}

/// A door's lock state (`LOCK_OPEN/CLOSED/LOCKED`): `exitpar1` of a Door or Gate, `exitpar2`
/// of a Key exit. The pack holds the export's state; a cold start locks every door.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockState {
    Open,
    Closed,
    Locked,
}

impl LockState {
    fn from_param(p: i64) -> LockState {
        match p {
            0 => LockState::Open,
            1 => LockState::Closed,
            _ => LockState::Locked,
        }
    }
}

/// Exit kinds that need something the character may not have; routes skip them unless the
/// [`crate::route::Traveller`] allows the category. Remote Action exits are never walked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// Every Hidden exit, found or not (otherwise only the found ones).
    Hidden,
    Key,
    Spell,
    Item,
    Toll,
    Cast,
    Ability,
}

/// What an exit asks of the character, from its parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    None,
    /// `exitpar1` minimum, `exitpar2` maximum; 0 is no limit.
    Level {
        min: i64,
        max: i64,
    },
    /// `exitpar1` the one class allowed, else `exitpar2` the one barred.
    Class {
        only: i64,
        barred: i64,
    },
    Race {
        only: i64,
        barred: i64,
    },
    /// Evil points between `exitpar1` (as a signed int) and `exitpar2`.
    Alignment {
        min: i64,
        max: i64,
    },
    Item {
        item: i64,
    },
    Toll {
        crowns: i64,
    },
    Spell,
    Cast {
        spell: i64,
    },
    Key,
    Ability {
        ability: i64,
        min: i64,
        max: i64,
    },
}

/// One exit, as the graph keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: Key,
    pub to: Key,
    /// 0-9: N S E W NE NW SE SW U D.
    pub dir: u8,
    pub kind: ExitType,
    /// What to type: `n` for a direction, the phrase for a Text exit (empty for a Text exit
    /// whose message has no text, which can't be taken).
    pub command: String,
    /// Every phrase a Text exit takes (its message's texts); empty for other exits.
    pub phrases: Vec<String>,
    pub gate: Gate,
    /// Doors, gates and key doors, with the pack's lock state.
    pub door: Option<LockState>,
    /// `exitpar1`..`exitpar4`, raw.
    pub params: [i64; 4],
    pub(crate) to_ix: u32,
}

impl Edge {
    pub fn hidden(&self) -> bool {
        self.kind == ExitType::Hidden
    }

    /// Whether typing `command` takes this exit: its direction (any spelling) for an exit
    /// walked by direction, or one of a Text exit's phrases (whole line, any case).
    pub fn taken_by(&self, command: &str) -> bool {
        match self.kind {
            ExitType::RemoteAction => false,
            ExitType::Text => {
                let c = command.trim();
                self.phrases.iter().any(|p| p.eq_ignore_ascii_case(c))
            }
            _ => mm::direction(command) == Some(self.dir as usize),
        }
    }
}

/// A set of directions (bit `d` for direction `d`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DirSet(pub u16);

impl DirSet {
    /// Directions by name or command (`north`, `n`, `up`, ...); `None` if one isn't a direction.
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<DirSet> {
        names
            .into_iter()
            .map(|n| mm::direction(n).map(|d| d as u8))
            .collect()
    }

    pub fn contains(self, dir: u8) -> bool {
        self.0 & (1 << dir) != 0
    }

    pub fn insert(&mut self, dir: u8) {
        self.0 |= 1 << dir;
    }

    pub fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = u8> {
        (0..10u8).filter(move |d| self.contains(*d))
    }
}

impl FromIterator<u8> for DirSet {
    fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> DirSet {
        let mut s = DirSet::default();
        iter.into_iter().for_each(|d| s.insert(d));
        s
    }
}

/// One room in the graph.
#[derive(Clone, Debug)]
pub struct Node {
    pub room: Room,
    /// Monsters come here on their own: the spawn pass generates some (`Monsters::generated`).
    pub spawns: bool,
    edges: Range<u32>,
}

impl Node {
    pub fn key(&self) -> Key {
        (self.room.map, self.room.room)
    }
}

/// Every room and exit, indexed. Nodes are in key order; each node's edges are in direction
/// order in one flat list.
pub struct RoomGraph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    index: HashMap<Key, u32>,
    by_name: HashMap<String, Vec<u32>>,
    /// Exits whose destination isn't a room of the pack (left out).
    pub dangling: usize,
}

impl RoomGraph {
    /// Read every room and exit of the pack once.
    pub fn load(pack: &WorldPack) -> Result<RoomGraph, Error> {
        let monsters = Monsters::load(pack)?;
        let rooms = pack.all_rooms()?;
        let exits = pack.all_exits()?;
        let index: HashMap<Key, u32> = rooms
            .iter()
            .enumerate()
            .map(|(i, r)| ((r.map, r.room), i as u32))
            .collect();
        let mut phrases: HashMap<i64, Vec<String>> = HashMap::new();
        let mut edges = Vec::with_capacity(exits.len());
        let mut nodes = Vec::with_capacity(rooms.len());
        let mut dangling = 0;
        let mut ex = exits.into_iter().peekable();
        for room in rooms {
            let key = (room.map, room.room);
            let start = edges.len() as u32;
            while let Some((from, e)) = ex.next_if(|(from, _)| *from <= key) {
                let kind = ExitType::from_name(&e.exit_type)
                    .ok_or_else(|| Error::Format(format!("exit type {:?} unknown", e.exit_type)))?;
                let to = (e.to_map, e.to_room);
                let (true, Some(&to_ix)) = (from == key, index.get(&to)) else {
                    dangling += 1;
                    continue;
                };
                let p = e.params;
                let texts = if kind == ExitType::Text {
                    match phrases.entry(p[0]) {
                        Entry::Occupied(o) => o.get().clone(),
                        Entry::Vacant(v) => {
                            let texts = pack.message(p[0])?.unwrap_or_default();
                            v.insert(
                                texts
                                    .into_iter()
                                    .map(|t| t.trim().to_owned())
                                    .filter(|t| !t.is_empty())
                                    .collect(),
                            )
                            .clone()
                        }
                    }
                } else {
                    Vec::new()
                };
                let command = match kind {
                    ExitType::Text => texts.first().cloned().unwrap_or_default(),
                    _ => mm::DIRS[e.dir as usize].to_owned(),
                };
                let gate = match kind {
                    ExitType::Level => Gate::Level {
                        min: p[0],
                        max: p[1],
                    },
                    ExitType::Class => Gate::Class {
                        only: p[0],
                        barred: p[1],
                    },
                    ExitType::Race => Gate::Race {
                        only: p[0],
                        barred: p[1],
                    },
                    ExitType::Alignment => Gate::Alignment {
                        min: i64::from(p[0] as u32 as i32),
                        max: p[1],
                    },
                    ExitType::Item => Gate::Item { item: p[0] },
                    ExitType::Toll => Gate::Toll { crowns: p[0] },
                    ExitType::Spell => Gate::Spell,
                    ExitType::Cast => Gate::Cast { spell: p[0] },
                    ExitType::Key => Gate::Key,
                    ExitType::Ability => Gate::Ability {
                        ability: p[0],
                        min: p[1],
                        max: p[2],
                    },
                    _ => Gate::None,
                };
                let door = match kind {
                    ExitType::Door | ExitType::Gate => Some(LockState::from_param(p[0])),
                    ExitType::Key => Some(LockState::from_param(p[1])),
                    _ => None,
                };
                edges.push(Edge {
                    from,
                    to,
                    dir: e.dir,
                    kind,
                    command,
                    phrases: texts,
                    gate,
                    door,
                    params: p,
                    to_ix,
                });
            }
            let spawns = !monsters.generated(&room).is_empty();
            nodes.push(Node {
                room,
                spawns,
                edges: start..edges.len() as u32,
            });
        }
        dangling += ex.count();
        let mut by_name: HashMap<String, Vec<u32>> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            by_name
                .entry(n.room.name.clone())
                .or_default()
                .push(i as u32);
        }
        Ok(RoomGraph {
            nodes,
            edges,
            index,
            by_name,
            dangling,
        })
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn node(&self, key: Key) -> Option<&Node> {
        self.index.get(&key).map(|&i| &self.nodes[i as usize])
    }

    pub fn room(&self, key: Key) -> Option<&Room> {
        self.node(key).map(|n| &n.room)
    }

    pub fn contains(&self, key: Key) -> bool {
        self.index.contains_key(&key)
    }

    /// Every room, in key order.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }

    /// A room's exits in direction order (none for a key that isn't a room).
    pub fn edges(&self, key: Key) -> &[Edge] {
        self.index.get(&key).map_or(&[], |&i| self.edges_at(i))
    }

    /// Every exit.
    pub fn all_edges(&self) -> &[Edge] {
        &self.edges
    }

    /// A room's exit in one direction.
    pub fn edge(&self, key: Key, dir: u8) -> Option<&Edge> {
        self.edges(key).iter().find(|e| e.dir == dir)
    }

    /// Give the Text exits whose message has no text the phrases MegaMud's paths type for
    /// them, so they can be taken; returns how many it gave one.
    pub fn learn_phrases(&mut self, k: &crate::knowledge::PathKnowledge) -> usize {
        let mut learned = 0;
        for ((from, dir), phrases) in k.text_exits() {
            let Some(&i) = self.index.get(&from) else {
                continue;
            };
            let range = self.nodes[i as usize].edges.clone();
            let edges = &mut self.edges[range.start as usize..range.end as usize];
            if let Some(e) = edges.iter_mut().find(|e| e.dir == dir)
                && e.kind == ExitType::Text
                && e.command.is_empty()
            {
                e.command = phrases[0].clone();
                e.phrases = phrases.to_vec();
                learned += 1;
            }
        }
        learned
    }

    /// The rooms with exactly this name, in key order.
    pub fn rooms_named(&self, name: &str) -> Vec<Key> {
        self.by_name.get(name).map_or_else(Vec::new, |v| {
            v.iter().map(|&i| self.nodes[i as usize].key()).collect()
        })
    }

    /// The exits a room shows in `Obvious exits`: (always shown, shown or not).
    pub fn listed_exits(&self, key: Key) -> (DirSet, DirSet) {
        let (mut always, mut maybe) = (DirSet::default(), DirSet::default());
        for e in self.edges(key) {
            match e.kind.listing() {
                Listing::Always => always.insert(e.dir),
                Listing::Maybe => maybe.insert(e.dir),
                Listing::Never => {}
            }
        }
        (always, maybe)
    }

    /// Whether `observed` can be this room's `Obvious exits`.
    pub fn exits_fit(&self, key: Key, observed: DirSet) -> bool {
        let (always, maybe) = self.listed_exits(key);
        observed.0 & always.0 == always.0 && observed.0 & !(always.0 | maybe.0) == 0
    }

    pub(crate) fn ix(&self, key: Key) -> Option<u32> {
        self.index.get(&key).copied()
    }

    pub(crate) fn key_at(&self, ix: u32) -> Key {
        self.nodes[ix as usize].key()
    }

    pub(crate) fn node_at(&self, ix: u32) -> &Node {
        &self.nodes[ix as usize]
    }

    pub(crate) fn edges_at(&self, ix: u32) -> &[Edge] {
        let r = &self.nodes[ix as usize].edges;
        &self.edges[r.start as usize..r.end as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_types_round_trip_by_name() {
        for (i, name) in mm::EXIT_TYPES.iter().enumerate() {
            let t = ExitType::from_name(name).unwrap();
            assert_eq!(t as usize, i);
            assert_eq!(t.name(), *name);
        }
        assert_eq!(ExitType::from_name("Portal"), None);
    }

    #[test]
    fn dir_sets_from_names() {
        let s = DirSet::from_names(["north", "up", "se"]).unwrap();
        assert_eq!(s.iter().collect::<Vec<_>>(), [0, 6, 8]);
        assert_eq!(s.len(), 3);
        assert_eq!(DirSet::from_names(["north", "hole"]), None);
        assert!(DirSet::from_names([]).unwrap().is_empty());
    }
}
