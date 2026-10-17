//! Walking a MegaMud path through the game's rooms, checking each step's room id.

use std::collections::HashMap;

use crate::mm::{self, MpFile, WILDCARD};

/// A room key: (map, room).
pub type Key = (i64, i64);

/// What the walker needs of a room.
#[derive(Clone, Debug)]
pub struct Node {
    pub mm_id: u32,
    /// (direction, destination) for each exit.
    pub links: Vec<(usize, Key)>,
}

/// The rooms, with lookups by full id and by title hash. Lists are in key order.
#[derive(Default)]
pub struct Graph {
    pub rooms: HashMap<Key, Node>,
    by_id: HashMap<u32, Vec<Key>>,
    by_hash: HashMap<u32, Vec<Key>>,
}

impl Graph {
    pub fn insert(&mut self, key: Key, node: Node) {
        self.by_id.entry(node.mm_id).or_default().push(key);
        self.by_hash.entry(node.mm_id >> 20).or_default().push(key);
        self.rooms.insert(key, node);
    }

    /// Sort the lookup lists so every result is in key order.
    pub fn finish(&mut self) {
        self.by_id.values_mut().for_each(|v| v.sort_unstable());
        self.by_hash.values_mut().for_each(|v| v.sort_unstable());
    }

    /// Rooms whose id equals `id`.
    pub fn with_id(&self, id: u32) -> &[Key] {
        self.by_id.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Rooms whose title hash equals `id`'s.
    pub fn with_hash(&self, id: u32) -> &[Key] {
        self.by_hash.get(&(id >> 20)).map_or(&[], Vec::as_slice)
    }
}

/// How a step's id compares with the room the walker is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// The full 32-bit id is equal.
    Exact,
    /// The title hash is equal, the exit bits differ.
    Title,
    /// Not even the title hash.
    None,
    /// The step's id is `FFFFFFFF`: not checked.
    Wildcard,
}

impl Match {
    pub fn as_str(self) -> &'static str {
        match self {
            Match::Exact => "exact",
            Match::Title => "title",
            Match::None => "none",
            Match::Wildcard => "wildcard",
        }
    }

    fn of(node: &Node, id: u32) -> Match {
        if id == WILDCARD {
            Match::Wildcard
        } else if node.mm_id == id {
            Match::Exact
        } else if node.mm_id >> 20 == id >> 20 {
            Match::Title
        } else {
            Match::None
        }
    }
}

/// One walk of a path from one start room.
#[derive(Clone, Debug)]
pub struct Walk {
    pub start: Key,
    /// For each step reached: the room the command was typed in, and how its id matched.
    pub steps: Vec<(Key, Match)>,
    /// Where the walk ended (the room it broke in, if it broke).
    pub end: Key,
    /// The step (0-based) that couldn't be followed, and why.
    pub broke: Option<(usize, String)>,
}

impl Walk {
    pub fn count(&self, m: Match) -> usize {
        self.steps.iter().filter(|(_, x)| *x == m).count()
    }

    fn score(&self, g: &Graph, to_id: u32) -> (bool, usize, usize, usize, bool) {
        let end_ok = g
            .rooms
            .get(&self.end)
            .is_some_and(|n| n.mm_id >> 20 == to_id >> 20);
        (
            self.broke.is_none(),
            self.steps.len(),
            self.count(Match::Exact),
            self.count(Match::Exact) + self.count(Match::Title),
            end_ok,
        )
    }
}

/// Where a non-direction command led: the room among `cands` the next step's id fits best
/// (full id before title hash; the current room first).
fn pick(g: &Graph, cands: &[Key], next_id: u32) -> Option<Key> {
    let fits = |want: Match| {
        cands
            .iter()
            .find(|k| {
                g.rooms
                    .get(k)
                    .is_some_and(|n| Match::of(n, next_id) == want)
            })
            .copied()
    };
    fits(Match::Exact).or_else(|| fits(Match::Title))
}

/// Follow `mp`'s steps from `start`. A direction moves through that exit, whatever its type;
/// any other command stays or moves to the neighbour (or, for teleports, the one room in the
/// world) that the next step's id names.
pub fn walk(g: &Graph, mp: &MpFile, start: Key) -> Walk {
    let mut cur = start;
    let mut steps = Vec::with_capacity(mp.steps.len());
    for (i, step) in mp.steps.iter().enumerate() {
        let Some(node) = g.rooms.get(&cur) else {
            return Walk {
                start,
                steps,
                end: cur,
                broke: Some((i, "missing room".into())),
            };
        };
        steps.push((cur, Match::of(node, step.id)));
        let next_id = mp.steps.get(i + 1).map_or(mp.to_id, |s| s.id);
        let base = step.base.to_ascii_lowercase();
        let base = base.strip_prefix("@party ").unwrap_or(&base);
        let next = if let Some(d) = mm::direction(base) {
            match node.links.iter().find(|(dd, _)| *dd == d) {
                Some((_, k)) => Ok(*k),
                None => Err(format!("no exit {}", mm::DIRS[d])),
            }
        } else if next_id == WILDCARD {
            Ok(cur)
        } else {
            let mut cands = vec![cur];
            cands.extend(node.links.iter().map(|(_, k)| *k));
            pick(g, &cands, next_id)
                .or_else(|| match g.with_id(next_id) {
                    [one] => Some(*one),
                    _ => None,
                })
                .or_else(|| match g.with_hash(next_id) {
                    [one] => Some(*one),
                    _ => None,
                })
                .ok_or_else(|| format!("unresolved {:?}", step.command))
        };
        match next {
            Ok(k) => cur = k,
            Err(why) => {
                return Walk {
                    start,
                    steps,
                    end: cur,
                    broke: Some((i, why)),
                };
            }
        }
    }
    Walk {
        start,
        steps,
        end: cur,
        broke: None,
    }
}

/// The best walk of `mp`, with the number of start rooms tried. Starts are the rooms whose id
/// equals the first step's; when there are none, the rooms with its title hash. Best is: walks
/// to the end, then most steps, most exact ids, most title matches, ends at TOID's title.
pub fn best_walk(g: &Graph, mp: &MpFile) -> (Option<Walk>, usize) {
    let Some(first) = mp.steps.first() else {
        return (None, 0);
    };
    let starts = match g.with_id(first.id) {
        [] => g.with_hash(first.id),
        exact => exact,
    };
    let mut best: Option<(Walk, _)> = None;
    for &k in starts {
        let w = walk(g, mp, k);
        let s = w.score(g, mp.to_id);
        if best.as_ref().is_none_or(|(_, bs)| s > *bs) {
            best = Some((w, s));
        }
    }
    (best.map(|(w, _)| w), starts.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mm::parse_mp;

    // A (1/1) -e-> B (1/2); B's "go hole" leads to C (1/3), which is no neighbour of B.
    fn graph() -> Graph {
        let mut g = Graph::default();
        g.insert(
            (1, 1),
            Node {
                mm_id: 0x1000_0010,
                links: vec![(2, (1, 2))],
            },
        );
        g.insert(
            (1, 2),
            Node {
                mm_id: 0x2000_0040,
                links: vec![(3, (1, 1))],
            },
        );
        g.insert(
            (1, 3),
            Node {
                mm_id: 0x3000_0000,
                links: vec![],
            },
        );
        g.finish();
        g
    }

    #[test]
    fn walks_directions_and_resolves_a_teleport_by_the_next_id() {
        let mp = parse_mp(
            "T.mp",
            b"[][]\n10000010:30000000:3:-1:0:::\n10000010:0000:e\n20000041:0000:go hole\n",
        )
        .unwrap();
        let (w, starts) = best_walk(&graph(), &mp);
        let w = w.unwrap();
        assert_eq!(starts, 1);
        assert_eq!(w.broke, None);
        assert_eq!(w.end, (1, 3));
        // The second step's exit bits differ from B's: a title-only match.
        assert_eq!(w.steps, [((1, 1), Match::Exact), ((1, 2), Match::Title)]);
    }

    #[test]
    fn a_missing_exit_breaks_the_walk() {
        let mp = parse_mp(
            "T.mp",
            b"[][]\n10000010:10000010:1:-1:0:::\n10000010:0000:n\n",
        )
        .unwrap();
        let w = walk(&graph(), &mp, (1, 1));
        assert_eq!(w.broke, Some((0, "no exit n".to_string())));
        assert_eq!(w.end, (1, 1));
    }
}
