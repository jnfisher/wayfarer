//! Where the character might be: a set of candidate rooms, advanced through each move and
//! narrowed by each room seen (`docs/plan/03-engine.md` section 2, `kb/navigation.md`
//! "Position tracking"; the prototype's `progress.py` `update`).

use std::collections::BTreeSet;

use crate::nav::{DirSet, Key, RoomGraph};
use crate::route::{Traveller, distances};

/// How far from the old candidates to look when nothing fits.
pub const WIDEN_RADIUS: u32 = 12;
/// Old candidates searched around when widening, at most (the lowest keys).
const WIDEN_FROM: usize = 50;

/// The candidate rooms. Empty is unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Belief {
    cands: BTreeSet<Key>,
    /// The last non-empty candidates, to widen from when a move left none.
    last: BTreeSet<Key>,
}

impl Belief {
    pub fn unknown() -> Belief {
        Belief::default()
    }

    /// Certain of one room.
    pub fn at(key: Key) -> Belief {
        Belief::of([key])
    }

    pub fn of(keys: impl IntoIterator<Item = Key>) -> Belief {
        let cands: BTreeSet<Key> = keys.into_iter().collect();
        Belief {
            last: cands.clone(),
            cands,
        }
    }

    pub fn candidates(&self) -> &BTreeSet<Key> {
        &self.cands
    }

    pub fn is_unknown(&self) -> bool {
        self.cands.is_empty()
    }

    pub fn certain(&self) -> bool {
        self.cands.len() == 1
    }

    /// The lowest key among the candidates.
    pub fn best(&self) -> Option<Key> {
        self.cands.first().copied()
    }

    pub fn contains(&self, key: Key) -> bool {
        self.cands.contains(&key)
    }

    fn set(&mut self, cands: BTreeSet<Key>) {
        if !cands.is_empty() {
            self.last = cands.clone();
        }
        self.cands = cands;
    }

    /// A room was shown. `title` is `None` when it was lost (behind a prompt, or replaced by
    /// "You hear movement"); `exits` is the `Obvious exits` list, `None` when not seen; `dark`
    /// when the room was too dark to see, which tells nothing.
    ///
    /// Candidates are kept whose name is the title and whose listed exits can be the ones
    /// seen ([`RoomGraph::exits_fit`]). From an unknown belief the candidates are every room
    /// of that name. When nothing fits: the rooms within [`WIDEN_RADIUS`] steps of the old
    /// candidates by name and exits, then by name alone, then the world by name and exits,
    /// then by name alone. Without a title, the exits narrow the candidates if any fit. A title
    /// no room has changes nothing.
    pub fn observe(
        &mut self,
        graph: &RoomGraph,
        title: Option<&str>,
        exits: Option<DirSet>,
        dark: bool,
    ) {
        if dark {
            return;
        }
        let fits_exits = |k: &Key| exits.is_none_or(|x| graph.exits_fit(*k, x));
        let Some(title) = title else {
            let kept: BTreeSet<Key> = self.cands.iter().copied().filter(fits_exits).collect();
            if !kept.is_empty() {
                self.set(kept);
            }
            return;
        };
        let all = graph.rooms_named(title);
        if all.is_empty() {
            // no room has that name: not a title after all
            return;
        }
        let named = |k: &Key| graph.room(*k).is_some_and(|r| r.name == title);
        let fits = |k: &Key| named(k) && fits_exits(k);
        if !self.cands.is_empty() {
            let kept: BTreeSet<Key> = self.cands.iter().copied().filter(fits).collect();
            if !kept.is_empty() {
                return self.set(kept);
            }
        }
        if !self.last.is_empty() {
            let anyone = Traveller::anyone();
            let mut near = BTreeSet::new();
            for &c in self.last.iter().take(WIDEN_FROM) {
                near.extend(distances(graph, c, WIDEN_RADIUS, &anyone).into_keys());
            }
            for pick in [&fits as &dyn Fn(&Key) -> bool, &named] {
                let found: BTreeSet<Key> = near.iter().copied().filter(|k| pick(k)).collect();
                if !found.is_empty() {
                    return self.set(found);
                }
            }
        }
        let found: BTreeSet<Key> = all.iter().copied().filter(fits).collect();
        self.set(if found.is_empty() {
            all.into_iter().collect()
        } else {
            found
        });
    }

    /// A move was typed and the game took it: every candidate follows the exit that
    /// `command` takes from it; candidates with no such exit drop out.
    pub fn moved(&mut self, graph: &RoomGraph, command: &str) {
        let next = self
            .cands
            .iter()
            .flat_map(|&k| graph.edges(k).iter().filter(|e| e.taken_by(command)))
            .map(|e| e.to)
            .collect();
        self.set(next);
    }

    /// The game refused a move: the exits `command` would take from the candidates, to mark
    /// blocked in the [`Traveller`]. "There is no exit in that direction!" for an exit the
    /// data has means the position is wrong (`kb/pitfalls.md` 34): look, and [`observe`]
    /// relocates. The candidates are left as they were.
    ///
    /// [`observe`]: Belief::observe
    pub fn refused(&self, graph: &RoomGraph, command: &str) -> Vec<(Key, u8)> {
        self.cands
            .iter()
            .flat_map(|&k| graph.edges(k).iter().filter(|e| e.taken_by(command)))
            .map(|e| (e.from, e.dir))
            .collect()
    }
}
