//! Shortest routes over the room graph, with the commands to type (`kb/navigation.md`
//! "Routing"; the prototype's `World.path`).

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, VecDeque};

use crate::nav::{Category, Edge, ExitType, Gate, Key, LockState, RoomGraph};

/// Extra costs on top of 1 per step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Costs {
    /// Entering a room that spawns monsters (not the destination).
    pub spawn: u32,
    /// Going through a door, gate or key door.
    pub door: u32,
}

impl Default for Costs {
    fn default() -> Costs {
        Costs { spawn: 40, door: 2 }
    }
}

/// Who is walking: what the gated exits check, and what this session has learned.
#[derive(Clone, Debug, Default)]
pub struct Traveller {
    /// `None` is unknown, and the gate is not checked.
    pub level: Option<i64>,
    pub class: Option<i64>,
    pub race: Option<i64>,
    /// Evil points.
    pub evil: Option<i64>,
    /// Exits the game refused this session: (room, direction).
    pub blocked: BTreeSet<(Key, u8)>,
    /// Hidden exits found (`search`); they hide again after about five minutes.
    pub found: BTreeSet<(Key, u8)>,
    /// Exit categories allowed beyond the default.
    pub allow: BTreeSet<Category>,
    /// Doors, gates and key doors are not taken (a loop generated without doors).
    pub no_doors: bool,
    pub costs: Costs,
}

impl Traveller {
    /// Anyone through anything but a Remote Action exit, with no extra costs: how rooms are
    /// connected at all (the belief's search radius).
    pub fn anyone() -> Traveller {
        Traveller {
            allow: [
                Category::Hidden,
                Category::Key,
                Category::Spell,
                Category::Item,
                Category::Toll,
                Category::Cast,
                Category::Ability,
            ]
            .into(),
            costs: Costs { spawn: 0, door: 0 },
            ..Traveller::default()
        }
    }

    fn allows(&self, c: Category) -> bool {
        self.allow.contains(&c)
    }

    /// Whether this traveller may take the exit.
    pub fn can_take(&self, e: &Edge) -> bool {
        if self.blocked.contains(&(e.from, e.dir)) || (self.no_doors && e.door.is_some()) {
            return false;
        }
        let in_range = |v: Option<i64>, lo: i64, hi: i64| {
            v.is_none_or(|v| (lo == 0 || v >= lo) && (hi == 0 || v <= hi))
        };
        let one_of = |v: Option<i64>, only: i64, barred: i64| {
            v.is_none_or(|v| if only != 0 { v == only } else { v != barred })
        };
        let gate_ok = match e.gate {
            Gate::None => true,
            Gate::Level { min, max } => in_range(self.level, min, max),
            Gate::Class { only, barred } => one_of(self.class, only, barred),
            Gate::Race { only, barred } => one_of(self.race, only, barred),
            Gate::Alignment { min, max } => self.evil.is_none_or(|v| v >= min && v <= max),
            Gate::Item { .. } => self.allows(Category::Item),
            Gate::Toll { .. } => self.allows(Category::Toll),
            Gate::Spell => self.allows(Category::Spell),
            Gate::Cast { .. } => self.allows(Category::Cast),
            Gate::Key => self.allows(Category::Key),
            Gate::Ability { .. } => self.allows(Category::Ability),
        };
        gate_ok
            && match e.kind {
                ExitType::RemoteAction => false,
                ExitType::Hidden => {
                    self.allows(Category::Hidden) || self.found.contains(&(e.from, e.dir))
                }
                ExitType::Text => !e.command.is_empty(),
                _ => true,
            }
    }
}

/// One step of a route.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub command: String,
    pub from: Key,
    pub to: Key,
    pub dir: u8,
    pub exit_type: ExitType,
    /// A door on the way, with the pack's lock state (`open`, then `bash` if locked).
    pub door: Option<LockState>,
}

pub type Route = Vec<Step>;

/// The cheapest route from `from` to `to` (Dijkstra): 1 per step, plus `costs.spawn` for
/// entering a room that spawns monsters (the destination exempt) and `costs.door` per door.
/// `None` when there is no way; an empty route when `from` is `to`.
pub fn route(graph: &RoomGraph, from: Key, to: Key, t: &Traveller) -> Option<Route> {
    route_within(graph, from, to, t, |_| true)
}

/// [`route`] through the rooms `within` accepts only (`from` and `to` need not be).
pub fn route_within(
    graph: &RoomGraph,
    from: Key,
    to: Key,
    t: &Traveller,
    within: impl Fn(Key) -> bool,
) -> Option<Route> {
    let (src, dst) = (graph.ix(from)?, graph.ix(to)?);
    let mut best: HashMap<u32, u32> = HashMap::from([(src, 0)]);
    let mut prev: HashMap<u32, &Edge> = HashMap::new();
    let mut heap = BinaryHeap::from([Reverse((0u32, src))]);
    while let Some(Reverse((cost, cur))) = heap.pop() {
        if cur == dst {
            break;
        }
        if cost > best[&cur] {
            continue;
        }
        for e in graph.edges_at(cur) {
            if !t.can_take(e) || (e.to_ix != dst && !within(e.to)) {
                continue;
            }
            let mut c = cost + 1;
            if e.to_ix != dst && graph.node_at(e.to_ix).spawns {
                c += t.costs.spawn;
            }
            if e.door.is_some() {
                c += t.costs.door;
            }
            if best.get(&e.to_ix).is_none_or(|&b| c < b) {
                best.insert(e.to_ix, c);
                prev.insert(e.to_ix, e);
                heap.push(Reverse((c, e.to_ix)));
            }
        }
    }
    if src != dst && !prev.contains_key(&dst) {
        return None;
    }
    let mut steps = Vec::new();
    let mut at = dst;
    while at != src {
        let e = prev[&at];
        steps.push(Step {
            command: e.command.clone(),
            from: e.from,
            to: e.to,
            dir: e.dir,
            exit_type: e.kind,
            door: e.door,
        });
        at = graph
            .ix(e.from)
            .expect("an edge leaves a room of the graph");
    }
    steps.reverse();
    Some(steps)
}

/// Steps to every room within `radius` steps of `from` (breadth first) by exits the
/// traveller may take.
pub fn distances(graph: &RoomGraph, from: Key, radius: u32, t: &Traveller) -> HashMap<Key, u32> {
    let mut out = HashMap::new();
    let Some(src) = graph.ix(from) else {
        return out;
    };
    let mut seen: HashMap<u32, u32> = HashMap::from([(src, 0)]);
    let mut queue = VecDeque::from([src]);
    while let Some(cur) = queue.pop_front() {
        let d = seen[&cur];
        if d >= radius {
            continue;
        }
        for e in graph.edges_at(cur) {
            if t.can_take(e) && !seen.contains_key(&e.to_ix) {
                seen.insert(e.to_ix, d + 1);
                queue.push_back(e.to_ix);
            }
        }
    }
    out.extend(seen.into_iter().map(|(i, d)| (graph.key_at(i), d)));
    out
}
