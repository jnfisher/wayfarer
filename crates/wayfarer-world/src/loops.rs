//! Loops through an area: closed circuits over its hunt rooms generated from the world data,
//! and MegaMud's verified loops that pass through it (`docs/plan/03-engine.md` section 4,
//! "Areas and loops"; the prototype's `World.patrol` and `plan_ground`).
//!
//! An area is the ball of rooms within a radius of an anchor room, by exits the traveller may
//! take. Its hunt rooms are the rooms whose spawn pool holds a monster worth hunting. A
//! generated loop visits them in nearest-neighbour order, improved by 2-opt, each leg the
//! traveller's cheapest route between them through the area.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::Error;
use crate::monsters::{Monsters, huntable};
use crate::nav::{Key, RoomGraph};
use crate::pack::{MmPath, WorldPack};
use crate::route::{Route, Step, Traveller, distances, route, route_within};

/// Room types (`kb/game-data.md` "Enumerations").
const ROOM_ARENA: i64 = 2;
const ROOM_LAIR: i64 = 3;
/// 2-opt passes over a tour at most (the prototype's three, and a margin).
const TWO_OPT_PASSES: usize = 5;
/// A leg that cannot be walked.
const NO_WAY: usize = usize::MAX / 4;

/// The rooms within `radius` steps of `anchor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Area {
    pub anchor: Key,
    pub radius: u32,
}

impl Area {
    /// The area's rooms with their distance from the anchor, by exits the traveller may take
    /// (a gated exit it can't pass, or a door when it takes none, leaves what is behind out).
    pub fn rooms(&self, graph: &RoomGraph, t: &Traveller) -> HashMap<Key, u32> {
        distances(graph, self.anchor, self.radius, t)
    }
}

/// A room of the area that spawns a monster worth hunting.
#[derive(Clone, Debug, PartialEq)]
pub struct HuntRoom {
    pub key: Key,
    pub name: String,
    /// The huntable monsters of its spawn pool: (number, name).
    pub monsters: Vec<(i64, String)>,
    /// An Arena: monsters come back with no wait after a kill (`kb/progression.md`).
    pub is_arena: bool,
    /// A Lair.
    pub is_lair: bool,
    /// `regendelay`: minutes nothing spawns after a kill (arenas exempt).
    pub regen: i64,
}

/// A closed circuit through hunt rooms.
#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    pub name: String,
    /// The stops in walking order; the loop goes back to the first after the last.
    pub rooms: Vec<Key>,
    /// The route: from the first stop through every other and back to it.
    pub steps: Vec<Step>,
    /// Steps in `steps`.
    pub length: usize,
    /// Stops: rooms that spawn.
    pub spawns: usize,
}

/// Any room as a loop's stop: what it spawns that is worth hunting (nothing, for a room a player
/// adds to a loop that spawns nothing), and whether it is an arena or a lair. `None` for a
/// room the world does not have.
pub fn stop_room(graph: &RoomGraph, monsters: &Monsters, key: Key) -> Option<HuntRoom> {
    let room = graph.room(key)?;
    let found: Vec<(i64, String)> = monsters
        .spawn_pool(room)
        .into_iter()
        .filter(|m| huntable(m))
        .map(|m| (m.number, m.name.clone()))
        .collect();
    Some(HuntRoom {
        key,
        name: room.name.clone(),
        monsters: found,
        is_arena: room.room_type == ROOM_ARENA,
        is_lair: room.room_type == ROOM_LAIR,
        regen: room.regen_delay,
    })
}

/// The area's hunt rooms, in key order.
pub fn hunt_rooms(
    graph: &RoomGraph,
    monsters: &Monsters,
    area: Area,
    t: &Traveller,
) -> Vec<HuntRoom> {
    hunt_rooms_in(graph, monsters, &area.rooms(graph, t))
}

fn hunt_rooms_in(
    graph: &RoomGraph,
    monsters: &Monsters,
    rooms: &HashMap<Key, u32>,
) -> Vec<HuntRoom> {
    let mut keys: Vec<Key> = rooms.keys().copied().collect();
    keys.sort_unstable();
    keys.into_iter()
        .filter_map(|k| {
            let room = graph.room(k)?;
            let found: Vec<(i64, String)> = monsters
                .spawn_pool(room)
                .into_iter()
                .filter(|m| huntable(m))
                .map(|m| (m.number, m.name.clone()))
                .collect();
            (!found.is_empty()).then(|| HuntRoom {
                key: k,
                name: room.name.clone(),
                monsters: found,
                is_arena: room.room_type == ROOM_ARENA,
                is_lair: room.room_type == ROOM_LAIR,
                regen: room.regen_delay,
            })
        })
        .collect()
}

/// The loops through the area's hunt rooms: one over all of them, and when the area splits
/// (hunt rooms that can't reach each other inside it: a river, a level exit, a one-way drop)
/// one per part of two or more rooms, whose legs stay inside the area. The loop over all of
/// them may leave the area to get from one part to another; it is left out when it can't
/// close. Named after the anchor's place (`Newhaven` for "Newhaven, Village Entrance"); a
/// part's loop adds its first stop's name (`Newhaven: The Silver River, Silvermere Docks`).
pub fn generate(graph: &RoomGraph, monsters: &Monsters, area: Area, t: &Traveller) -> Vec<Loop> {
    let rooms = area.rooms(graph, t);
    let hunt = hunt_rooms_in(graph, monsters, &rooms);
    circuits(graph, area, &rooms, &hunt, t)
}

/// [`generate`] over hunt rooms already found.
pub fn circuits(
    graph: &RoomGraph,
    area: Area,
    rooms: &HashMap<Key, u32>,
    hunt: &[HuntRoom],
    t: &Traveller,
) -> Vec<Loop> {
    if hunt.is_empty() {
        return Vec::new();
    }
    let base = graph.room(area.anchor).map_or_else(
        || format!("{}/{}", area.anchor.0, area.anchor.1),
        |r| place(&r.name),
    );
    let inside: HashSet<Key> = rooms.keys().copied().collect();
    let mut legs = Legs {
        graph,
        t,
        inside: &inside,
        within: HashMap::new(),
        outside: HashMap::new(),
    };
    let stops: Vec<Key> = hunt.iter().map(|h| h.key).collect();
    let parts = parts(&stops, &mut legs);
    let near = |k: &Key| (rooms.get(k).copied().unwrap_or(u32::MAX), *k);
    let mut out = Vec::new();
    let split = parts.len() > 1;
    if let Some(l) = tour(&stops, near, &mut legs, split) {
        out.push(l.named(base.clone()));
    }
    if split {
        let mut names: HashMap<String, usize> = HashMap::new();
        for part in parts.iter().filter(|p| p.len() > 1) {
            let Some(l) = tour(part, near, &mut legs, false) else {
                continue;
            };
            let first = graph.room(l.rooms[0]).map_or("", |r| r.name.as_str());
            let mut name = format!("{base}: {first}");
            let n = names.entry(name.clone()).or_default();
            *n += 1;
            if *n > 1 {
                name = format!("{name} {n}");
            }
            out.push(l.named(name));
        }
    }
    out
}

/// The place a room's name starts with: `Newhaven` for "Newhaven, Arena".
fn place(name: &str) -> String {
    name.split_once(", ").map_or(name, |(p, _)| p).to_owned()
}

/// Routes between stops, worked out once each.
struct Legs<'a> {
    graph: &'a RoomGraph,
    t: &'a Traveller,
    inside: &'a HashSet<Key>,
    within: HashMap<(Key, Key), Option<Route>>,
    outside: HashMap<(Key, Key), Option<Route>>,
}

impl Legs<'_> {
    /// The leg from `a` to `b` inside the area, or with `open` anywhere when there is none
    /// inside.
    fn get(&mut self, a: Key, b: Key, open: bool) -> Option<&Route> {
        let (graph, t, inside) = (self.graph, self.t, self.inside);
        let within = self
            .within
            .entry((a, b))
            .or_insert_with(|| route_within(graph, a, b, t, |k| inside.contains(&k)));
        if within.is_some() || !open {
            return self.within[&(a, b)].as_ref();
        }
        self.outside
            .entry((a, b))
            .or_insert_with(|| route(graph, a, b, t))
            .as_ref()
    }

    fn len(&mut self, a: Key, b: Key, open: bool) -> usize {
        self.get(a, b, open).map_or(NO_WAY, Vec::len)
    }
}

/// The stops grouped by which reach each other (both ways) inside the area, each part in key
/// order, the parts by their first stop.
fn parts(stops: &[Key], legs: &mut Legs) -> Vec<Vec<Key>> {
    let mut part: BTreeMap<Key, usize> = BTreeMap::new();
    let mut out: Vec<Vec<Key>> = Vec::new();
    for &a in stops {
        if part.contains_key(&a) {
            continue;
        }
        let mut p = vec![a];
        for &b in stops {
            if b != a
                && !part.contains_key(&b)
                && legs.len(a, b, false) < NO_WAY
                && legs.len(b, a, false) < NO_WAY
            {
                p.push(b);
            }
        }
        for &k in &p {
            part.insert(k, out.len());
        }
        out.push(p);
    }
    out
}

struct Tour {
    rooms: Vec<Key>,
    steps: Vec<Step>,
}

impl Tour {
    fn named(self, name: String) -> Loop {
        Loop {
            name,
            length: self.steps.len(),
            spawns: self.rooms.len(),
            rooms: self.rooms,
            steps: self.steps,
        }
    }
}

/// The closed tour's length in steps.
fn cost(order: &[Key], legs: &mut Legs, open: bool) -> usize {
    if order.len() < 2 {
        return 0;
    }
    (0..order.len())
        .map(|i| legs.len(order[i], order[(i + 1) % order.len()], open))
        .fold(0, usize::saturating_add)
}

/// A closed tour of `stops` from the one nearest the anchor: nearest neighbour, then 2-opt
/// (a reversed stretch kept when the whole tour gets shorter: legs need not be the same
/// length both ways). `None` when it can't close.
fn tour(
    stops: &[Key],
    near: impl Fn(&Key) -> (u32, Key),
    legs: &mut Legs,
    open: bool,
) -> Option<Tour> {
    let first = *stops.iter().min_by_key(|k| near(k))?;
    let mut order = vec![first];
    let mut left: Vec<Key> = stops.iter().copied().filter(|&k| k != first).collect();
    left.sort_unstable();
    while !left.is_empty() {
        let at = *order.last().expect("not empty");
        let (i, _) = left
            .iter()
            .enumerate()
            .min_by_key(|&(_, &k)| legs.len(at, k, open))
            .expect("not empty");
        order.push(left.remove(i));
    }
    let n = order.len();
    let mut best = cost(&order, legs, open);
    for _ in 0..TWO_OPT_PASSES {
        let mut improved = false;
        for i in 1..n.saturating_sub(1) {
            for j in i + 1..n {
                let mut c = order.clone();
                c[i..=j].reverse();
                let cc = cost(&c, legs, open);
                if cc < best {
                    (order, best) = (c, cc);
                    improved = true;
                }
            }
        }
        if !improved {
            break;
        }
    }
    if best >= NO_WAY {
        return None;
    }
    let mut steps = Vec::with_capacity(best);
    if n > 1 {
        for i in 0..n {
            let leg = legs.get(order[i], order[(i + 1) % n], open)?;
            steps.extend(leg.iter().cloned());
        }
    }
    Some(Tour {
        rooms: order,
        steps,
    })
}

/// One of MegaMud's loops with the rooms its verification walk went through.
#[derive(Clone, Debug, PartialEq)]
pub struct MmLoop {
    pub path: MmPath,
    /// The rooms the walk passed, in order, from its start back to it.
    pub rooms: Vec<Key>,
    /// The hunt rooms among them, in path order, each once: the loop's stops.
    pub hunt: Vec<HuntRoom>,
}

impl MmLoop {
    /// The stops a task runs it by: the walk's hunt rooms, in path order, without repeats.
    /// No step of the format waits or attacks, so no other room is a stop.
    pub fn stops(&self) -> Vec<Key> {
        self.hunt.iter().map(|h| h.key).collect()
    }

    /// Why it cannot be run as a task, if it cannot: nothing on it to hunt.
    pub fn not_runnable(&self) -> Option<&'static str> {
        self.hunt.is_empty().then_some("nothing to hunt on it")
    }

    /// What a task calls it: its description, or its file name when that is blank.
    pub fn name(&self) -> &str {
        match self.path.description.trim() {
            "" => &self.path.file,
            d => d,
        }
    }
}

/// Whether a MegaMud loop may be listed or run: its verification walk reached the end and
/// closed in its start room (`docs/plan/03-engine.md`, "Trusting MegaMud's data").
pub fn verified(p: &MmPath) -> bool {
    let v = &p.verification;
    p.is_loop && v.walked && v.closes && v.start.is_some()
}

/// MegaMud's verified loops in the pack, by file name, with the rooms each walk passed and
/// the hunt rooms among them.
pub fn megamud_loops(
    pack: &WorldPack,
    graph: &RoomGraph,
    monsters: &Monsters,
) -> Result<Vec<MmLoop>, Error> {
    let mut out = Vec::new();
    for path in pack.loops()?.into_iter().filter(verified) {
        let mut rooms: Vec<Key> = Vec::new();
        let walked = pack
            .path_steps(&path.file)?
            .into_iter()
            .filter_map(|s| s.walk);
        for k in walked.map(|(k, _)| k).chain(path.verification.end) {
            if rooms.last() != Some(&k) {
                rooms.push(k);
            }
        }
        // Each room once, numbered by where the walk first passed it.
        let mut order: HashMap<Key, u32> = HashMap::new();
        for &k in &rooms {
            let n = order.len() as u32;
            order.entry(k).or_insert(n);
        }
        let mut hunt = hunt_rooms_in(graph, monsters, &order);
        hunt.sort_by_key(|h| order[&h.key]);
        out.push(MmLoop { path, rooms, hunt });
    }
    Ok(out)
}

/// The verified loops whose walk passes a room of the area; never an unverified one.
pub fn megamud_through<'a>(loops: &'a [MmLoop], rooms: &HashMap<Key, u32>) -> Vec<&'a MmLoop> {
    loops
        .iter()
        .filter(|l| verified(&l.path) && l.rooms.iter().any(|k| rooms.contains_key(k)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_the_name_up_to_the_comma() {
        assert_eq!(place("Newhaven, Village Entrance"), "Newhaven");
        assert_eq!(place("Dungeon, Entrance"), "Dungeon");
        assert_eq!(place("Small Cavern"), "Small Cavern");
    }
}
