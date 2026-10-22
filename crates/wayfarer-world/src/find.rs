//! Go to: rooms, well-known places and monsters found by part of a name, nearest first, and
//! which rooms are boss lairs (MegaMud's "Stop Before Entering Room", `kb/megamud-manual.md`).
//!
//! [`Finder`] indexes the names once per brain; [`Finder::find`] answers a query with one
//! breadth-first search from the character's room; route lengths beyond [`NEAR_STEPS`] show
//! as "far".

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use crate::Error;
use crate::monsters::Monsters;
use crate::nav::RoomGraph;
use crate::pack::{Monster, Waypoint, WorldPack};
use crate::route::{Traveller, distances_any};
use crate::walk::Key;

/// Results a query returns at most.
pub const MAX_RESULTS: usize = 30;
/// Rooms a result lists at most, nearest first; `count` says how many there are.
pub const MAX_ROOMS: usize = 20;
/// The longest route length given in steps; a room beyond is "far".
pub const NEAR_STEPS: u32 = 200;
/// The shortest query answered.
pub const MIN_QUERY: usize = 2;

/// Room type of a shop, and the shop types a place is named by (`kb/economy.md`).
const ROOM_SHOP: i64 = 1;
const SHOP_BANK: i64 = 7;
const SHOP_TRAINING: i64 = 8;

/// What a well-known place is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlaceKind {
    Shop,
    Bank,
    Trainer,
    Boss,
    Other,
}

/// What a result names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FoundKind {
    Place(PlaceKind),
    Monster,
    /// Every room with this exact name.
    Rooms,
}

/// How far a room is from the character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reach {
    Steps(u32),
    /// Beyond [`NEAR_STEPS`], or the character's room is unknown.
    Far,
    /// The traveller has no way there from the character's room.
    NoRoute,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FoundRoom {
    pub key: Key,
    pub name: String,
    pub reach: Reach,
    /// A boss lair: a walk there stops one room short by default.
    pub boss: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub kind: FoundKind,
    pub name: String,
    /// The nearest of its rooms, at most [`MAX_ROOMS`].
    pub rooms: Vec<FoundRoom>,
    /// How many rooms it names.
    pub count: usize,
    /// The bosses of its boss lairs, by name.
    pub bosses: Vec<String>,
}

impl Found {
    fn reach(&self) -> Reach {
        self.rooms.first().map_or(Reach::NoRoute, |r| r.reach)
    }
}

/// A named set of rooms the finder can match.
struct Entry {
    kind: FoundKind,
    name: String,
    lower: String,
    rooms: Vec<Key>,
    /// For a monster: its records' numbers.
    monsters: Vec<i64>,
}

/// The names to search, built once.
pub struct Finder {
    entries: Vec<Entry>,
    /// Boss lairs: the bosses each holds (none when only a waypoint flags it).
    lairs: HashMap<Key, Vec<i64>>,
}

/// How well `name` matches `query` (both lower case): the whole name, its start, the start of
/// a word, anywhere; `None` when it does not.
fn quality(name: &str, query: &str) -> Option<u8> {
    if name == query {
        return Some(0);
    }
    if name.starts_with(query) {
        return Some(1);
    }
    let mut anywhere = None;
    for (i, _) in name.match_indices(query) {
        let word = name[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if word {
            return Some(2);
        }
        anywhere = Some(3);
    }
    anywhere
}

impl Finder {
    pub fn load(pack: &WorldPack, graph: &RoomGraph, monsters: &Monsters) -> Result<Finder, Error> {
        let shops: HashMap<i64, i64> = pack.shop_types()?.into_iter().collect();
        Ok(Finder::new(graph, monsters, &pack.waypoints()?, &shops))
    }

    /// `shops`: each shop's type, by number.
    pub fn new(
        graph: &RoomGraph,
        monsters: &Monsters,
        waypoints: &[Waypoint],
        shops: &HashMap<i64, i64>,
    ) -> Finder {
        let mut lairs: HashMap<Key, Vec<i64>> = HashMap::new();
        let mut spawns: BTreeMap<String, (String, Vec<i64>, Vec<Key>)> = BTreeMap::new();
        let mut names: BTreeMap<&str, Vec<Key>> = BTreeMap::new();
        let mut shop_rooms: BTreeMap<(&str, PlaceKind), Vec<Key>> = BTreeMap::new();
        for n in graph.nodes() {
            let (k, r) = (n.key(), &n.room);
            names.entry(r.name.as_str()).or_default().push(k);
            if r.room_type == ROOM_SHOP && r.shop_num != 0 {
                let kind = match shops.get(&r.shop_num) {
                    Some(&SHOP_BANK) => PlaceKind::Bank,
                    Some(&SHOP_TRAINING) => PlaceKind::Trainer,
                    _ => PlaceKind::Shop,
                };
                shop_rooms.entry((&r.name, kind)).or_default().push(k);
            }
            for m in monsters.spawn_pool(r) {
                if monsters.is_boss(m.number) {
                    lairs.entry(k).or_default().push(m.number);
                }
                let e = spawns
                    .entry(m.name.to_lowercase())
                    .or_insert_with(|| (m.name.clone(), Vec::new(), Vec::new()));
                if !e.1.contains(&m.number) {
                    e.1.push(m.number);
                }
                if e.2.last() != Some(&k) {
                    e.2.push(k);
                }
            }
        }

        let mut entries = Vec::new();
        for w in waypoints {
            if w.rooms.is_empty() {
                continue;
            }
            let kind = if w.is_boss {
                PlaceKind::Boss
            } else if w.is_trainer {
                PlaceKind::Trainer
            } else if w.is_bank {
                PlaceKind::Bank
            } else if w.is_shop {
                PlaceKind::Shop
            } else {
                PlaceKind::Other
            };
            if w.is_boss {
                for &k in &w.rooms {
                    lairs.entry(k).or_default();
                }
            }
            entries.push(Entry {
                kind: FoundKind::Place(kind),
                lower: w.name.to_lowercase(),
                name: w.name.clone(),
                rooms: w.rooms.clone(),
                monsters: Vec::new(),
            });
        }
        for ((name, kind), rooms) in &shop_rooms {
            entries.push(Entry {
                kind: FoundKind::Place(*kind),
                name: (*name).to_owned(),
                lower: name.to_lowercase(),
                rooms: rooms.clone(),
                monsters: Vec::new(),
            });
        }
        for (lower, (name, numbers, rooms)) in spawns {
            entries.push(Entry {
                kind: FoundKind::Monster,
                name,
                lower,
                rooms,
                monsters: numbers,
            });
        }
        for (name, rooms) in names {
            // A shop is listed as a place already.
            if shop_rooms.keys().any(|(n, _)| *n == name) {
                continue;
            }
            entries.push(Entry {
                kind: FoundKind::Rooms,
                name: name.to_owned(),
                lower: name.to_lowercase(),
                rooms,
                monsters: Vec::new(),
            });
        }

        Finder { entries, lairs }
    }

    /// The bosses whose lair this room is (empty when only a MegaMud waypoint flagged boss
    /// says so); `None` when it is not a boss lair.
    pub fn boss_lair<'m>(&self, room: Key, monsters: &'m Monsters) -> Option<Vec<&'m Monster>> {
        self.lairs
            .get(&room)
            .map(|v| v.iter().filter_map(|&n| monsters.get(n)).collect())
    }

    pub fn is_boss_lair(&self, room: Key) -> bool {
        self.lairs.contains_key(&room)
    }

    /// Everything whose name holds `query` (two characters or more, any case), best first:
    /// those `from` cannot reach last, then the better match (the whole name, its start, a
    /// word's start, anywhere), then the nearer by route length for the traveller, then places
    /// before monsters before room names. At most [`MAX_RESULTS`].
    pub fn find(
        &self,
        graph: &RoomGraph,
        monsters: &Monsters,
        query: &str,
        from: Option<Key>,
        traveller: &Traveller,
    ) -> Vec<Found> {
        let query = query.trim().to_lowercase();
        if query.chars().count() < MIN_QUERY {
            return Vec::new();
        }
        if let Some(key) = room_key(&query, from)
            && let Some(room) = graph.room(key)
        {
            let reach = match from.map(|f| distances_any(graph, f, u32::MAX, traveller)) {
                None => Reach::Far,
                Some(r) => match r.get(&key) {
                    Some(&d) if d <= NEAR_STEPS => Reach::Steps(d),
                    Some(_) => Reach::Far,
                    None => Reach::NoRoute,
                },
            };
            return vec![Found {
                kind: FoundKind::Rooms,
                name: room.name.clone(),
                rooms: vec![FoundRoom {
                    key,
                    name: room.name.clone(),
                    reach,
                    boss: self.is_boss_lair(key),
                }],
                count: 1,
                bosses: Vec::new(),
            }];
        }
        let mut hits: Vec<(&Entry, u8)> = self
            .entries
            .iter()
            .filter_map(|e| quality(&e.lower, &query).map(|q| (e, q)))
            .collect();
        if !hits.iter().any(|(e, _)| e.kind == FoundKind::Monster) {
            // "thin giant rat" finds the giant rats.
            let named: Vec<String> = monsters
                .by_name(&query)
                .iter()
                .map(|m| m.name.to_lowercase())
                .collect();
            hits.extend(
                self.entries
                    .iter()
                    .filter(|e| e.kind == FoundKind::Monster && named.contains(&e.lower))
                    .map(|e| (e, 3)),
            );
        }
        // Every room the traveller can reach: one breadth-first pass over at most the whole
        // graph (some milliseconds), so "no route" is true and not a guess.
        let reached = match from {
            Some(f) if !hits.is_empty() => Some(distances_any(graph, f, u32::MAX, traveller)),
            _ => None,
        };
        let reach = |k: Key| -> Reach {
            match reached.as_ref().map(|r| r.get(&k)) {
                None => Reach::Far,
                Some(Some(&d)) if d <= NEAR_STEPS => Reach::Steps(d),
                Some(Some(_)) => Reach::Far,
                Some(None) => Reach::NoRoute,
            }
        };
        let mut found: Vec<(u8, Found)> = hits
            .into_iter()
            .map(|(e, q)| {
                let mut rooms: Vec<FoundRoom> = e
                    .rooms
                    .iter()
                    .map(|&k| FoundRoom {
                        key: k,
                        name: graph.room(k).map_or_else(String::new, |r| r.name.clone()),
                        reach: reach(k),
                        boss: self.is_boss_lair(k),
                    })
                    .collect();
                rooms.sort_by_key(|r| (r.reach, r.key));
                let count = rooms.len();
                rooms.truncate(MAX_ROOMS);
                let mut bosses: Vec<String> = Vec::new();
                let numbers = if e.kind == FoundKind::Monster {
                    e.monsters.clone()
                } else {
                    rooms
                        .iter()
                        .filter_map(|r| self.lairs.get(&r.key))
                        .flatten()
                        .copied()
                        .collect()
                };
                for n in numbers {
                    if let Some(m) = monsters.get(n).filter(|m| monsters.is_boss(m.number))
                        && !bosses.contains(&m.name)
                    {
                        bosses.push(m.name.clone());
                    }
                }
                let found = Found {
                    kind: e.kind,
                    name: e.name.clone(),
                    rooms,
                    count,
                    bosses,
                };
                (q, found)
            })
            .collect();
        found.sort_by(|(qa, a), (qb, b)| rank(*qa, a, *qb, b));
        found.truncate(MAX_RESULTS);
        found.into_iter().map(|(_, f)| f).collect()
    }
}

/// A room named by its number: `1:1217`, or `1217` for a room on the character's own map
/// (map 1 when its place is not known).
fn room_key(query: &str, from: Option<Key>) -> Option<Key> {
    let num = |t: &str| t.trim().parse().ok();
    match query.split_once(':') {
        Some((map, room)) => Some((num(map)?, num(room)?)),
        None => Some((from.map_or(1, |f| f.0), num(query)?)),
    }
}

fn rank(qa: u8, a: &Found, qb: u8, b: &Found) -> Ordering {
    let lost = |f: &Found| f.reach() == Reach::NoRoute;
    lost(a)
        .cmp(&lost(b))
        .then(qa.cmp(&qb))
        .then(a.reach().cmp(&b.reach()))
        .then(a.kind.cmp(&b.kind))
        .then_with(|| a.name.cmp(&b.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_is_named_by_its_number() {
        assert_eq!(room_key("1:1217", Some((9, 5))), Some((1, 1217)));
        assert_eq!(room_key("1217", Some((9, 5))), Some((9, 1217)));
        assert_eq!(room_key("1217", None), Some((1, 1217)));
        assert_eq!(room_key("slum", None), None);
        assert_eq!(room_key("1:x", None), None);
    }

    #[test]
    fn quality_prefers_the_whole_name_then_its_start_then_a_word() {
        assert_eq!(quality("arena", "arena"), Some(0));
        assert_eq!(quality("arena stands", "arena"), Some(1));
        assert_eq!(quality("newhaven, arena", "arena"), Some(2));
        assert_eq!(quality("minotaur chieftain", "chief"), Some(2));
        assert_eq!(quality("great arena of khazarad", "rena"), Some(3));
        assert_eq!(quality("wooden passage", "xyz"), None);
    }

    fn found(kind: FoundKind, name: &str, reach: Reach) -> Found {
        Found {
            kind,
            name: name.into(),
            rooms: vec![FoundRoom {
                key: (1, 1),
                name: name.into(),
                reach,
                boss: false,
            }],
            count: 1,
            bosses: Vec::new(),
        }
    }

    #[test]
    fn ranking_puts_no_route_last_then_the_better_match_then_the_nearer() {
        let mut v = [
            (0, found(FoundKind::Rooms, "Arena", Reach::NoRoute)),
            (
                2,
                found(FoundKind::Rooms, "Newhaven, Arena", Reach::Steps(9)),
            ),
            (2, found(FoundKind::Rooms, "Old Arena", Reach::Steps(3))),
            (0, found(FoundKind::Rooms, "Arena", Reach::Far)),
            (
                2,
                found(
                    FoundKind::Place(PlaceKind::Shop),
                    "Arena Shop",
                    Reach::Steps(3),
                ),
            ),
        ];
        v.sort_by(|(qa, a), (qb, b)| rank(*qa, a, *qb, b));
        let order: Vec<(&str, Reach)> = v
            .iter()
            .map(|(_, f)| (f.name.as_str(), f.reach()))
            .collect();
        assert_eq!(
            order,
            [
                ("Arena", Reach::Far),
                ("Arena Shop", Reach::Steps(3)),
                ("Old Arena", Reach::Steps(3)),
                ("Newhaven, Arena", Reach::Steps(9)),
                ("Arena", Reach::NoRoute),
            ]
        );
    }
}
