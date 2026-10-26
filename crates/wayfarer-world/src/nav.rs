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
    /// Not an exit of the pack: a line of the room's command text that teleports (`ring chime`
    /// on Slum Street, C-052), the phrase typed.
    RoomCommand,
    /// Not an exit of the pack: a phrase a MegaMud path types where the pack has no way
    /// through to the room its next step starts in (C-052).
    PathCommand,
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
        match self {
            ExitType::RoomCommand => "Room Command",
            ExitType::PathCommand => "Path Command",
            _ => mm::EXIT_TYPES[self as usize],
        }
    }

    /// Taken by typing a phrase, not a direction: Text exits, room commands and MegaMud's
    /// crossings.
    pub fn is_phrase(self) -> bool {
        matches!(
            self,
            ExitType::Text | ExitType::RoomCommand | ExitType::PathCommand
        )
    }

    /// Not an exit at all, but a way through C-052 adds: never listed, never drawn, and its
    /// `dir` is past the ten directions.
    pub fn is_command(self) -> bool {
        matches!(self, ExitType::RoomCommand | ExitType::PathCommand)
    }

    pub fn listing(self) -> Listing {
        match self {
            ExitType::Text
            | ExitType::RemoteAction
            | ExitType::RoomCommand
            | ExitType::PathCommand => Listing::Never,
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
    /// `exitpar1` the number of the key item that turns it.
    Key {
        item: i64,
    },
    Ability {
        ability: i64,
        min: i64,
        max: i64,
    },
    /// Into a room the pack has no name for: a player's house (C-053). Never taken.
    Unnamed,
}

/// One exit, as the graph keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: Key,
    pub to: Key,
    /// 0-9: N S E W NE NW SE SW U D; a room command or a MegaMud crossing numbers on from
    /// [`COMMAND_DIRS`].
    pub dir: u8,
    pub kind: ExitType,
    /// What to type: `n` for a direction, the phrase for a Text exit (empty for a Text exit
    /// whose message has no text, or a room command with no phrase the game would read as
    /// one, which can't be taken).
    pub command: String,
    /// Every phrase a Text exit, a room command or a crossing takes; empty for other exits.
    pub phrases: Vec<String>,
    /// A Hidden exit that is blocked, not searched for: the phrases of its own room's Remote
    /// Action exits that take its blocks off, in the order to type them ([`unblocking`]).
    /// Empty for any other exit, and for one its room's Remote Actions can't open.
    pub unblock: Vec<String>,
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
    /// walked by direction, or one of its phrases (whole line, any case) for a Text exit, a
    /// room command or a crossing.
    pub fn taken_by(&self, command: &str) -> bool {
        match self.kind {
            ExitType::RemoteAction => false,
            k if k.is_phrase() => {
                let c = command.trim();
                self.phrases.iter().any(|p| p.eq_ignore_ascii_case(c))
            }
            _ => mm::direction(command) == Some(self.dir as usize),
        }
    }
}

/// The first `dir` of the ways through that are not exits ([`ExitType::is_command`]).
pub const COMMAND_DIRS: u8 = 10;

/// A Hidden exit's flags (`exitpar1`, `EXF_`): found by searching.
const EXF_NEEDSEARCH: i64 = 0x0002;
/// Its first block, `EXF_BLOCKED1`; blocks 2 to 10 are the bits above it.
const EXF_BLOCKED1: i64 = 0x0010;

/// The phrases that open a blocked Hidden exit with parameters `params`, in the order to type
/// them, from `actions`, the (action, phrase) of its room's Remote Actions on it; empty when it
/// is searched for, or some block has no phrase (`kb/navigation.md` "Remote Action"). The
/// game blocks it again with `|exitpar2|` blocks, 1 to n; action 0 takes them all off,
/// action n block n, and with `exitpar2` 0 or more only once block n+1 is off, so they are
/// typed from n down.
fn unblocking(params: [i64; 4], actions: &[(i64, String)]) -> Vec<String> {
    let flags = params[0];
    if flags & EXF_NEEDSEARCH != 0 {
        return Vec::new();
    }
    let set = (1..=10)
        .filter(|b| flags & (EXF_BLOCKED1 << (b - 1)) != 0)
        .max();
    let again = Some(params[1].abs()).filter(|n| (1..=10).contains(n));
    let Some(n) = set.max(again) else {
        return Vec::new();
    };
    if let Some((_, all)) = actions.iter().find(|(a, _)| *a == 0) {
        return vec![all.clone()];
    }
    (1..=n)
        .rev()
        .map(|b| {
            actions
                .iter()
                .find(|(a, _)| *a == b)
                .map(|(_, p)| p.clone())
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

/// The clauses a room command line may have and still be a way through: what they do can be
/// told beforehand (`kb/navigation.md` "Text blocks"). Anything else (a `cast`, a `random`, a
/// skill test, an item taken or checked, an ability or alignment check) leaves the line out.
const PLAIN_CLAUSES: [&str; 6] = [
    "message",
    "adddelay",
    "minlevel",
    "maxlevel",
    "nomonsters",
    "teleport",
];

/// The ways through a room's command text gives (C-052): for each line that is the first with
/// its pattern, a plain pattern (no wildcard) and only [`PLAIN_CLAUSES`] with one `teleport
/// <room> <map>`, the room it puts the character in (on `map`, its own map when 0 or left
/// out), gated by `minlevel` / `maxlevel`. Lines to the same room with the same gate are one
/// way with every pattern as a phrase, in text order.
pub fn room_commands(map: i64, text: &str) -> Vec<(Key, Gate, Vec<String>)> {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<(Key, Gate, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let mut clauses = line.split(':');
        let pattern = clauses.next().unwrap_or("").trim().to_lowercase();
        if pattern.is_empty() || seen.contains(&pattern) {
            continue;
        }
        seen.push(pattern.clone());
        if pattern.contains(['*', '?']) {
            continue;
        }
        let (mut to, mut min, mut max, mut plain) = (None, 0, 0, true);
        for c in clauses {
            let mut words = c.split_whitespace();
            let Some(kw) = words.next().map(str::to_lowercase) else {
                continue;
            };
            let num = |w: Option<&str>| w.and_then(|w| w.parse::<i64>().ok());
            match kw.as_str() {
                "teleport" if to.is_none() => match (num(words.next()), num(words.next())) {
                    (Some(room), m) => to = Some((m.filter(|&m| m != 0).unwrap_or(map), room)),
                    _ => plain = false,
                },
                "minlevel" => min = num(words.next()).unwrap_or(0),
                "maxlevel" => max = num(words.next()).unwrap_or(0),
                k if PLAIN_CLAUSES.contains(&k) && k != "teleport" => {}
                _ => plain = false,
            }
        }
        let (Some(to), true) = (to, plain) else {
            continue;
        };
        let gate = if min == 0 && max == 0 {
            Gate::None
        } else {
            Gate::Level { min, max }
        };
        match out.iter_mut().find(|(t, g, _)| *t == to && *g == gate) {
            Some((_, _, phrases)) => phrases.push(pattern),
            None => out.push((to, gate, vec![pattern])),
        }
    }
    out
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
    /// Exits into a room the pack has no name for, from a room it has one for: the doors of
    /// players' houses (map 15), whose titles the game makes up as it runs. They stay (they show
    /// in `Obvious exits`) behind [`Gate::Unnamed`], which no route, rest or flee goes through.
    pub into_unnamed: usize,
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
        let commands: HashMap<Key, String> = pack
            .room_commands()?
            .into_iter()
            .map(|(m, r, t)| ((m, r), t))
            .collect();
        let mut edges = Vec::with_capacity(exits.len());
        let mut nodes = Vec::with_capacity(rooms.len());
        let mut dangling = 0;
        let unnamed: std::collections::HashSet<Key> = rooms
            .iter()
            .filter(|r| r.name.trim().is_empty())
            .map(|r| (r.map, r.room))
            .collect();
        let mut into_unnamed = 0;
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
                let house = unnamed.contains(&to) && !unnamed.contains(&key);
                into_unnamed += usize::from(house);
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
                    ExitType::Key => Gate::Key { item: p[0] },
                    ExitType::Ability => Gate::Ability {
                        ability: p[0],
                        min: p[1],
                        max: p[2],
                    },
                    _ => Gate::None,
                };
                let gate = if house { Gate::Unnamed } else { gate };
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
                    unblock: Vec::new(),
                    gate,
                    door,
                    params: p,
                    to_ix,
                });
            }
            // The ways through the room's command text, after its exits.
            let ways = commands
                .get(&key)
                .map_or_else(Vec::new, |t| room_commands(key.0, t));
            for (dir, (to, gate, phrases)) in (COMMAND_DIRS..).zip(ways) {
                let Some(&to_ix) = index.get(&to) else {
                    dangling += 1;
                    continue;
                };
                edges.push(Edge {
                    from: key,
                    to,
                    dir,
                    kind: ExitType::RoomCommand,
                    command: phrases[0].clone(),
                    phrases,
                    unblock: Vec::new(),
                    gate,
                    door: None,
                    params: [0; 4],
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
        // The blocked Hidden exits: the phrases of their room's own Remote Actions (one that
        // asks for an item, `exitpar4`, is left out).
        let mut remote_phrases: HashMap<i64, Option<String>> = HashMap::new();
        for node in &nodes {
            let here = &edges[node.edges.start as usize..node.edges.end as usize];
            let mut found: Vec<(usize, Vec<String>)> = Vec::new();
            for (i, e) in here.iter().enumerate() {
                if e.kind != ExitType::Hidden {
                    continue;
                }
                let mut phrases: Vec<(i64, String)> = Vec::new();
                for r in here.iter().filter(|r| {
                    r.kind == ExitType::RemoteAction
                        && r.to == r.from
                        && r.params[1] >= 0
                        && r.params[1] % 10 == i64::from(e.dir)
                        && r.params[3] == 0
                }) {
                    let first = match remote_phrases.entry(r.params[0]) {
                        Entry::Occupied(o) => o.get().clone(),
                        Entry::Vacant(v) => v
                            .insert(
                                pack.message(r.params[0])?
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|t| t.trim().to_owned())
                                    .find(|t| !t.is_empty()),
                            )
                            .clone(),
                    };
                    if let Some(p) = first {
                        phrases.push((r.params[1] / 10, p));
                    }
                }
                let unblock = unblocking(e.params, &phrases);
                if !unblock.is_empty() {
                    found.push((node.edges.start as usize + i, unblock));
                }
            }
            for (i, unblock) in found {
                edges[i].unblock = unblock;
            }
        }
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
            into_unnamed,
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

    /// Give each room command the first of its phrases `reads_as_command` turns down: the game
    /// tries a room's command text only after its command parser found no command in the line
    /// (`use chime` is the USE command; `ring chime` reaches the script). With none, the room
    /// command can't be taken. Returns how many have a phrase.
    pub fn choose_phrases(&mut self, reads_as_command: impl Fn(&str) -> bool) -> usize {
        let mut usable = 0;
        for e in self
            .edges
            .iter_mut()
            .filter(|e| e.kind == ExitType::RoomCommand)
        {
            e.command = e
                .phrases
                .iter()
                .find(|p| !reads_as_command(p))
                .cloned()
                .unwrap_or_default();
            usable += usize::from(!e.command.is_empty());
        }
        usable
    }

    /// Add MegaMud's crossings (`PathKnowledge::crossings`) as ways through where the room has
    /// none to that room taking the phrase: each a [`ExitType::PathCommand`] numbered after the
    /// room's other ways. Returns how many it added.
    pub fn learn_crossings(&mut self, k: &crate::knowledge::PathKnowledge) -> usize {
        let mut added: HashMap<u32, Vec<Edge>> = HashMap::new();
        for (from, to, phrase) in k.crossings() {
            let (Some(&i), Some(&to_ix)) = (self.index.get(&from), self.index.get(&to)) else {
                continue;
            };
            let have =
                |e: &Edge| e.to == to && (e.taken_by(&phrase) || e.kind == ExitType::RoomCommand);
            let mine = added.entry(i).or_default();
            if self.edges_at(i).iter().any(have) || mine.iter().any(have) {
                continue;
            }
            let dir = self
                .edges_at(i)
                .iter()
                .chain(mine.iter())
                .map(|e| e.dir + 1)
                .max()
                .unwrap_or(0)
                .max(COMMAND_DIRS);
            mine.push(Edge {
                from,
                to,
                dir,
                kind: ExitType::PathCommand,
                command: phrase.clone(),
                phrases: vec![phrase],
                unblock: Vec::new(),
                gate: Gate::None,
                door: None,
                params: [0; 4],
                to_ix,
            });
        }
        let count = added.values().map(Vec::len).sum();
        if count == 0 {
            return 0;
        }
        let old = std::mem::take(&mut self.edges);
        for (i, node) in self.nodes.iter_mut().enumerate() {
            let start = self.edges.len() as u32;
            self.edges
                .extend_from_slice(&old[node.edges.start as usize..node.edges.end as usize]);
            if let Some(more) = added.remove(&(i as u32)) {
                self.edges.extend(more);
            }
            node.edges = start..self.edges.len() as u32;
        }
        count
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
        assert_eq!(ExitType::RoomCommand.name(), "Room Command");
        assert_eq!(ExitType::PathCommand.listing(), Listing::Never);
    }

    #[test]
    fn a_room_command_that_only_teleports_is_a_way_through() {
        // Slum Street, 1:1182 (text 4087): both lines to the Entrance, one way.
        let chime = "use chime:message 3177:teleport 65 1:message 837\n\
                     ring chime:message 3177:teleport 65 1:message 837\n\n";
        assert_eq!(
            room_commands(1, chime),
            [(
                (1, 65),
                Gate::None,
                vec!["use chime".into(), "ring chime".into()]
            )]
        );
        // The vortex at 3:25: gated by level; the map left out is the room's own.
        let vortex = "go vortex:adddelay 5:minlevel 20 1220:message 1205:teleport 681 3:message 1221\n\
                      enter vortex:adddelay 5:minlevel 20 1220:message 1205:teleport 681\n";
        assert_eq!(
            room_commands(3, vortex),
            [(
                (3, 681),
                Gate::Level { min: 20, max: 0 },
                vec!["go vortex".into(), "enter vortex".into()]
            )]
        );
    }

    #[test]
    fn a_room_command_that_does_more_is_left_out() {
        // A cast, a random, a wildcard, and a second line for a pattern already seen.
        let text = "dive pool:message 1943:teleport 121 12:cast 512\n\
                    orc invasion:random 4659:teleport 164 1\n\
                    jump *:teleport 5 1\n\
                    push panel:failability 3:message 1\n\
                    push panel:teleport 9 1\n";
        assert!(room_commands(1, text).is_empty());
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
