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

/// How a room seen moved the belief ([`Belief::observe`]), for the position log (C-085).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// Too dark to see: nothing told.
    Dark,
    /// No title: the exits narrowed the candidates, if any fitted.
    Exits,
    /// No room has that name, and no exits came: not a title.
    NotATitle,
    /// A whole room the pack lacks: the position is unknown.
    NotOnMap,
    /// Candidates fitted, and were kept.
    Followed,
    /// Every room of that name (and exits, when seen) in the world.
    Anywhere,
    /// None of the candidates fitted: every room of that name and exits in the world.
    Relocated,
    /// The rooms near the old candidates that fit, the exits not seen.
    Near,
    /// The rooms near the old candidates of that name: no room has those exits.
    NearByName,
    /// Every room of that name: no room has those exits.
    AnywhereByName,
}

impl How {
    pub fn as_str(self) -> &'static str {
        match self {
            How::Dark => "dark",
            How::Exits => "exits",
            How::NotATitle => "not_a_title",
            How::NotOnMap => "not_on_map",
            How::Followed => "followed",
            How::Anywhere => "anywhere",
            How::Relocated => "relocated",
            How::Near => "near",
            How::NearByName => "near_by_name",
            How::AnywhereByName => "anywhere_by_name",
        }
    }
}

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

    /// Keep only the candidates `keep` accepts, when any are left; else the belief stays as it
    /// was (a clue that rules out every candidate is not trusted over the room seen).
    pub fn narrow(&mut self, keep: impl Fn(Key) -> bool) {
        let kept: BTreeSet<Key> = self.cands.iter().copied().filter(|k| keep(*k)).collect();
        if !kept.is_empty() {
            self.set(kept);
        }
    }

    fn set(&mut self, cands: BTreeSet<Key>) {
        if !cands.is_empty() {
            self.last = cands.clone();
        }
        self.cands = cands;
    }

    /// A room was shown. `title` is `None` when it was lost (behind a prompt, or replaced by
    /// "You hear movement"); `exits` is the `Obvious exits` list, `None` when not seen; `dark`
    /// when the room was too dark to see, which tells nothing. Returns how the belief moved.
    ///
    /// Candidates are kept whose name is the title and whose listed exits can be the ones
    /// seen ([`RoomGraph::exits_fit`]). From an unknown belief the candidates are every room
    /// of that name and exits. When nothing fits and the exits were seen, every room in the
    /// world of that name and exits (C-085: the rooms near the old candidates may be a copy of
    /// the same stretch of maze, and the belief would settle on the wrong one). Without exits,
    /// or when no room has those: the rooms within [`WIDEN_RADIUS`] steps of the old
    /// candidates by name, then the world by name. Without a title, the exits narrow the
    /// candidates if any fit. A title no room has changes nothing on its own (it may be a line
    /// misread as one), but with its `Obvious exits` it is a whole room the pack lacks, a
    /// player's house (C-053): the position is unknown, and the next room the pack has
    /// relocates it from the old one.
    pub fn observe(
        &mut self,
        graph: &RoomGraph,
        title: Option<&str>,
        exits: Option<DirSet>,
        dark: bool,
    ) -> How {
        if dark {
            return How::Dark;
        }
        let fits_exits = |k: &Key| exits.is_none_or(|x| graph.exits_fit(*k, x));
        let Some(title) = title else {
            let kept: BTreeSet<Key> = self.cands.iter().copied().filter(fits_exits).collect();
            if !kept.is_empty() {
                self.set(kept);
            }
            return How::Exits;
        };
        let all = graph.rooms_named(title);
        if all.is_empty() {
            if exits.is_some() {
                // A whole room the pack does not have: nowhere it knows.
                self.cands.clear();
                return How::NotOnMap;
            }
            // Otherwise no room has that name: not a title after all.
            return How::NotATitle;
        }
        let named = |k: &Key| graph.room(*k).is_some_and(|r| r.name == title);
        let fits = |k: &Key| named(k) && fits_exits(k);
        let was_unknown = self.cands.is_empty();
        if !was_unknown {
            let kept: BTreeSet<Key> = self.cands.iter().copied().filter(fits).collect();
            if !kept.is_empty() {
                self.set(kept);
                return How::Followed;
            }
        }
        let anywhere: BTreeSet<Key> = all.iter().copied().filter(fits).collect();
        if exits.is_some() && !anywhere.is_empty() {
            self.set(anywhere);
            return if was_unknown {
                How::Anywhere
            } else {
                How::Relocated
            };
        }
        if !self.last.is_empty() {
            let anyone = Traveller::anyone();
            let mut near = BTreeSet::new();
            for &c in self.last.iter().take(WIDEN_FROM) {
                near.extend(distances(graph, c, WIDEN_RADIUS, &anyone).into_keys());
            }
            for (pick, how) in [
                (&fits as &dyn Fn(&Key) -> bool, How::Near),
                (&named, How::NearByName),
            ] {
                let found: BTreeSet<Key> = near.iter().copied().filter(|k| pick(k)).collect();
                if !found.is_empty() {
                    self.set(found);
                    return how;
                }
            }
        }
        if anywhere.is_empty() {
            self.set(all.into_iter().collect());
            How::AnywhereByName
        } else {
            self.set(anywhere);
            How::Anywhere
        }
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
