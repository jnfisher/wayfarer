//! Shortest routes over the room graph, with the commands to type (`kb/navigation.md`
//! "Routing"; the prototype's `World.path`).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};

use crate::nav::{Category, Edge, ExitType, Gate, Key, LockState, RoomGraph};

/// What entering a room the traveller avoids costs by default: more than any detour, so
/// another way wins whenever there is one, and still a way when there is not.
pub const AVOID_COST: u32 = 1000;

/// Extra costs on top of 1 per step. The default is the shortest route (John, 2026-10-19):
/// no penalty for rooms that spawn monsters; a door costs one extra step, the `open`, and a
/// hidden exit two, its `search`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Costs {
    /// Entering a room that spawns monsters (not the destination).
    pub spawn: u32,
    /// Going through a door, gate or key door; a locked one costs this per expected try
    /// ([`Costs::door_cost`]).
    pub door: u32,
    /// Entering a room in [`Traveller::avoid`] (not the destination).
    pub avoid: u32,
    /// Going through a Hidden exit not showing now (not in [`Traveller::found`]): the `search`.
    pub hidden: u32,
}

impl Default for Costs {
    fn default() -> Costs {
        Costs {
            spawn: 0,
            door: 1,
            avoid: AVOID_COST,
            hidden: 2,
        }
    }
}

impl Costs {
    /// What a door costs at `odds` percent a try of opening it ([`Traveller::door_odds`]):
    /// `door` per expected try, `door * ceil(100 / odds)`, so a door at 90 % costs two doors
    /// and one at 20 % five. `None` (not locked, or the odds unknown) is one try.
    pub fn door_cost(&self, odds: Option<u32>) -> u32 {
        match odds {
            Some(p) => self.door * 100u32.div_ceil(p.clamp(1, 100)),
            None => self.door,
        }
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
    /// The character's strength and picklocks skill: what a locked door's odds are
    /// ([`Traveller::door_odds`]). `None` is unknown, and every locked door may be bashed.
    pub strength: Option<i64>,
    pub picklocks: Option<i64>,
    /// Exits the game refused this session: (room, direction).
    pub blocked: BTreeSet<(Key, u8)>,
    /// Hidden exits showing now: found by a `search`, they hide again after about five
    /// minutes (the caller forgets them).
    pub found: BTreeSet<(Key, u8)>,
    /// Hidden exits known to be there, to be searched for when not showing: found once this
    /// session, or named by a loop's path.
    pub searchable: BTreeSet<(Key, u8)>,
    /// Doors and gates as the game showed them this session, over the pack's lock states.
    pub doors: BTreeMap<(Key, u8), LockState>,
    /// Exit categories allowed beyond the default.
    pub allow: BTreeSet<Category>,
    /// Doors, gates and key doors are not taken (a loop generated without doors).
    pub no_doors: bool,
    /// Rooms to keep out of when there is another way: where a monster the bot avoids was
    /// last seen. Costly, not impassable ([`Costs::avoid`]).
    pub avoid: BTreeSet<Key>,
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
            costs: Costs {
                spawn: 0,
                door: 0,
                avoid: 0,
                hidden: 0,
            },
            ..Traveller::default()
        }
    }

    fn allows(&self, c: Category) -> bool {
        self.allow.contains(&c)
    }

    /// A door's lock state: as the game last showed it, else the pack's.
    pub fn lock_state(&self, e: &Edge) -> Option<LockState> {
        e.door
            .map(|pack| *self.doors.get(&(e.from, e.dir)).unwrap_or(&pack))
    }

    /// The odds, in percent a try, of opening a locked door or gate whatever its state: the
    /// better of a bash, `strength + exitpar2`, and a pick, `picklocks + exitpar2` with
    /// picklocks above 0 (`kb/navigation.md` "Door / Gate"), from 0 to 100. `None` for an exit
    /// that is not a Door or Gate (a Key exit is opened with its key), or with the strength
    /// unknown.
    pub fn lock_odds(&self, e: &Edge) -> Option<u32> {
        if !matches!(e.kind, ExitType::Door | ExitType::Gate) {
            return None;
        }
        let bash = self.strength? + e.params[1];
        let pick = self
            .picklocks
            .filter(|&p| p > 0)
            .map_or(i64::MIN, |p| p + e.params[1]);
        Some(u32::try_from(bash.max(pick).clamp(0, 100)).expect("0 to 100"))
    }

    /// [`Traveller::lock_odds`] for a door that is locked ([`Traveller::lock_state`]); `None`
    /// for one that opens with `open`, or is open.
    pub fn door_odds(&self, e: &Edge) -> Option<u32> {
        (self.lock_state(e) == Some(LockState::Locked))
            .then(|| self.lock_odds(e))
            .flatten()
    }

    /// Whether this traveller may take the exit.
    pub fn can_take(&self, e: &Edge) -> bool {
        if self.blocked.contains(&(e.from, e.dir))
            || (self.no_doors && e.door.is_some())
            || self.door_odds(e) == Some(0)
        {
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
                    let at = (e.from, e.dir);
                    self.allows(Category::Hidden)
                        || self.found.contains(&at)
                        || self.searchable.contains(&at)
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
    /// A door on the way, with its lock state as the traveller knows it (the game's this
    /// session, else the pack's): `open`, then `bash` if locked.
    pub door: Option<LockState>,
}

pub type Route = Vec<Step>;

/// The cheapest route from `from` to `to` (Dijkstra): 1 per step, plus `costs.spawn` for
/// entering a room that spawns monsters and `costs.avoid` for one the traveller avoids (the
/// destination exempt from both), [`Costs::door_cost`] per door and `costs.hidden` per Hidden
/// exit to search for. Doors the traveller cannot open are left out ([`Traveller::can_take`]).
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
            if e.to_ix != dst && t.avoid.contains(&e.to) {
                c += t.costs.avoid;
            }
            if e.door.is_some() {
                c += t.costs.door_cost(t.door_odds(e));
            }
            if e.kind == ExitType::Hidden && !t.found.contains(&(e.from, e.dir)) {
                c += t.costs.hidden;
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
            door: t.lock_state(e),
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

/// The Hidden exits out of a small pocket: when everything the traveller can reach from `from`
/// is `cap` rooms or fewer, the Hidden exits that lead out of it, (room, direction). Such a
/// pocket (a room behind a hidden door that was entered some other way) has no other way out, so
/// those exits are as good as known. Empty when the reach is larger than `cap`.
pub fn pocket_exits(
    graph: &RoomGraph,
    from: Key,
    cap: usize,
    t: &Traveller,
) -> BTreeSet<(Key, u8)> {
    let mut out = BTreeSet::new();
    let Some(src) = graph.ix(from) else {
        return out;
    };
    let mut seen = HashSet::from([src]);
    let mut queue = VecDeque::from([src]);
    while let Some(cur) = queue.pop_front() {
        for e in graph.edges_at(cur) {
            if t.can_take(e) && seen.insert(e.to_ix) {
                if seen.len() > cap {
                    return out;
                }
                queue.push_back(e.to_ix);
            }
        }
    }
    for &ix in &seen {
        for e in graph.edges_at(ix) {
            if e.hidden() && !seen.contains(&e.to_ix) {
                out.insert((graph.key_at(ix), e.dir));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locked_door_costs_a_door_per_expected_try() {
        let c = Costs::default();
        assert_eq!(c.door_cost(None), 1);
        assert_eq!(c.door_cost(Some(100)), 1);
        assert_eq!(c.door_cost(Some(90)), 2);
        assert_eq!(c.door_cost(Some(50)), 2);
        assert_eq!(c.door_cost(Some(34)), 3);
        assert_eq!(c.door_cost(Some(20)), 5);
        assert_eq!(c.door_cost(Some(1)), 100);
        // Nil odds never reach a cost (the door is left out); they cost as 1 %.
        assert_eq!(c.door_cost(Some(0)), 100);
        assert!(c.door_cost(Some(20)) > c.door_cost(Some(90)));
        // No door cost at all: none per try either.
        let free = Traveller::anyone().costs;
        assert_eq!(free.door_cost(Some(20)), 0);
        assert_eq!((free.hidden, Costs::default().hidden), (0, 2));
    }
}
