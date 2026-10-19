//! A grid layout of the rooms around a position, for the map panel.

use std::collections::{HashMap, VecDeque};

use crate::nav::{ExitType, Key, RoomGraph};

/// One room on the grid. x grows east, y grows south; `level` goes up with Up exits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub key: Key,
    pub x: i32,
    pub y: i32,
    pub level: i32,
    /// Steps from the center.
    pub steps: u32,
    /// Its cell was already taken by another room: drawn there, but nothing is placed from it.
    pub collides: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    Plain,
    Door,
    Hidden,
    Text,
}

/// An exit between two placed rooms. A two-way pair comes back once, from the room placed
/// first; `one_way` when the far room has no exit back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub from: Key,
    pub to: Key,
    pub dir: u8,
    pub kind: SegmentKind,
    pub one_way: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// Breadth first from the center, which is first at (0, 0, 0).
    pub rooms: Vec<Placed>,
    pub segments: Vec<Segment>,
}

impl Layout {
    pub fn collisions(&self) -> impl Iterator<Item = &Placed> {
        self.rooms.iter().filter(|p| p.collides)
    }
}

/// The grid step of each direction: (dx, dy, dlevel).
pub const OFFSETS: [(i32, i32, i32); 10] = [
    (0, -1, 0),
    (0, 1, 0),
    (1, 0, 0),
    (-1, 0, 0),
    (1, -1, 0),
    (-1, -1, 0),
    (1, 1, 0),
    (-1, 1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

fn kind(t: ExitType) -> SegmentKind {
    match t {
        ExitType::Door | ExitType::Gate | ExitType::Key => SegmentKind::Door,
        ExitType::Hidden => SegmentKind::Hidden,
        ExitType::Text => SegmentKind::Text,
        _ => SegmentKind::Plain,
    }
}

/// Place `center` at (0, 0, 0) and each room within `radius` steps one cell from the room it
/// was reached from, in its exit's direction (breadth first, exits in direction order).
/// Remote Action exits don't count. A room whose cell is taken is still returned, flagged
/// `collides`, and its branch stops there.
pub fn layout(graph: &RoomGraph, center: Key, radius: u32) -> Layout {
    let mut out = Layout::default();
    if !graph.contains(center) {
        return out;
    }
    let mut placed: HashMap<Key, usize> = HashMap::from([(center, 0)]);
    let mut cells: HashMap<(i32, i32, i32), Key> = HashMap::from([((0, 0, 0), center)]);
    out.rooms.push(Placed {
        key: center,
        x: 0,
        y: 0,
        level: 0,
        steps: 0,
        collides: false,
    });
    let mut queue = VecDeque::from([0usize]);
    while let Some(i) = queue.pop_front() {
        let p = out.rooms[i].clone();
        if p.collides || p.steps >= radius {
            continue;
        }
        for e in graph.edges(p.key) {
            if e.kind == ExitType::RemoteAction || placed.contains_key(&e.to) {
                continue;
            }
            let (dx, dy, dl) = OFFSETS[e.dir as usize];
            let cell = (p.x + dx, p.y + dy, p.level + dl);
            let collides = cells.contains_key(&cell);
            if !collides {
                cells.insert(cell, e.to);
            }
            placed.insert(e.to, out.rooms.len());
            queue.push_back(out.rooms.len());
            out.rooms.push(Placed {
                key: e.to,
                x: cell.0,
                y: cell.1,
                level: cell.2,
                steps: p.steps + 1,
                collides,
            });
        }
    }
    // Segments between placed rooms, a two-way pair once.
    for p in &out.rooms {
        for e in graph.edges(p.key) {
            if e.kind == ExitType::RemoteAction || e.to == p.key {
                continue;
            }
            let Some(&j) = placed.get(&e.to) else {
                continue;
            };
            let back = graph
                .edges(e.to)
                .iter()
                .find(|b| b.to == p.key && b.kind != ExitType::RemoteAction);
            if back.is_some() && placed[&p.key] > j {
                // drawn from the room placed first
                continue;
            }
            out.segments.push(Segment {
                from: p.key,
                to: e.to,
                dir: e.dir,
                kind: back.map_or(kind(e.kind), |b| kind(e.kind).max_with(kind(b.kind))),
                one_way: back.is_none(),
            });
        }
    }
    out
}

impl SegmentKind {
    /// The kind to draw for a pair: a door wins, then hidden, then text.
    fn max_with(self, other: SegmentKind) -> SegmentKind {
        let rank = |k: SegmentKind| match k {
            SegmentKind::Plain => 0,
            SegmentKind::Text => 1,
            SegmentKind::Hidden => 2,
            SegmentKind::Door => 3,
        };
        if rank(other) > rank(self) {
            other
        } else {
            self
        }
    }
}
