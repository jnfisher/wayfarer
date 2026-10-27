//! What MegaMud's paths know about the exits they take (`docs/plan/03-engine.md` "Areas and
//! loops"): for every step of a path whose verification walk reached the end, the room the
//! walk typed it in and the exit it took. A direction step takes the exit that way, with the
//! actions written before it (`n[search n]`); a phrase (`climb tree`) takes a Text exit to the
//! room the next step starts in. From that: the Hidden exits a path finds, which a traveller on
//! that path may search for, and the phrase a Text exit takes where the pack's message has
//! none. A phrase that takes no Text exit or room command to that room is a crossing (C-052):
//! a way through the pack does not have, which [`RoomGraph::learn_crossings`] adds.
//!
//! A sealed Hidden exit ([`Edge::sealed`]: no search finds it, its own room can't open it) is
//! never searched for. Where a path opens one from elsewhere (the Crypt's two levers before
//! 1/1056 north, `Cry1cry2.mp`), the stretch from the first phrase that opens it to the exit
//! itself is a window ([`Window`]), which [`RoomGraph::learn_windows`] adds as a way through.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::Error;
use crate::mm;
use crate::nav::{COMMAND_DIRS, Edge, ExitType, Gate, Key, RoomGraph};
use crate::pack::WorldPack;

/// One exit a path took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathExit {
    pub from: Key,
    pub dir: u8,
    pub kind: ExitType,
    /// The step's `[actions]`, typed before the move (or after it failed).
    pub actions: Vec<String>,
    /// The phrase typed for a Text exit; `None` for a direction.
    pub phrase: Option<String>,
    /// A sealed Hidden exit ([`Edge::sealed`]): no search finds it.
    pub sealed: bool,
}

/// A stretch of a path that opens a sealed Hidden exit from elsewhere and goes through it: from
/// the first step whose phrase is one of the Remote Actions on that exit, to the exit.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// The path it comes from.
    pub file: String,
    /// Its steps as [`Edge::window`] holds them; the last is the sealed exit.
    pub steps: Vec<Edge>,
}

/// The exits MegaMud's verified paths take, built once from the pack.
#[derive(Clone, Debug, Default)]
pub struct PathKnowledge {
    /// By path file, in step order.
    by_file: HashMap<String, Vec<PathExit>>,
    /// Text exits by (room, direction): the phrases paths type for them, in file order.
    phrases: BTreeMap<(Key, u8), Vec<String>>,
    /// The phrases typed where the pack has no way through to the next step's room: (room,
    /// the room it led to, the phrase), each once, in file order.
    crossings: Vec<(Key, Key, String)>,
    /// The windows, the shortest for each (first room, sealed exit), in key order.
    windows: BTreeMap<(Key, Key, u8), Window>,
}

/// A path step's phrase as the character types it alone: MegaMud's `@party ` prefix (the whole
/// party does it) dropped. `None` for another of MegaMud's own `@` commands.
fn phrase(base: &str) -> Option<String> {
    let b = base.trim();
    let b = b.strip_prefix("@party ").map_or(b, str::trim);
    (!b.is_empty() && !b.starts_with('@')).then(|| b.to_owned())
}

impl PathKnowledge {
    /// Every path in the pack whose walk reached the end (none without MegaMud's paths).
    pub fn load(pack: &WorldPack, graph: &RoomGraph) -> Result<PathKnowledge, Error> {
        let mut k = PathKnowledge::default();
        for p in pack.paths()?.into_iter().filter(|p| p.verification.walked) {
            let steps = pack.path_steps(&p.file)?;
            let mut exits = Vec::new();
            for (i, s) in steps.iter().enumerate() {
                let Some((from, _)) = &s.walk else {
                    continue;
                };
                let next = steps
                    .get(i + 1)
                    .and_then(|n| n.walk.as_ref().map(|(k, _)| *k))
                    .or(p.verification.end);
                let actions = s.actions.clone().unwrap_or_default();
                let exit = match mm::direction(&s.base) {
                    Some(d) => graph.edge(*from, d as u8).map(|e| PathExit {
                        from: *from,
                        dir: e.dir,
                        kind: e.kind,
                        actions,
                        phrase: None,
                        sealed: e.sealed(),
                    }),
                    None => {
                        let Some(ph) = phrase(&s.base) else {
                            continue;
                        };
                        let found = graph
                            .edges(*from)
                            .iter()
                            .find(|e| e.kind.is_phrase() && Some(e.to) == next);
                        if found.is_none()
                            && let Some(to) = next.filter(|to| to != from)
                            && !k
                                .crossings
                                .iter()
                                .any(|c| c.0 == *from && c.1 == to && c.2 == ph)
                        {
                            k.crossings.push((*from, to, ph.clone()));
                        }
                        found.map(|e| PathExit {
                            from: *from,
                            dir: e.dir,
                            kind: e.kind,
                            actions,
                            phrase: Some(ph),
                            sealed: false,
                        })
                    }
                };
                if let Some(e) = exit {
                    if let Some(ph) = &e.phrase {
                        let known = k.phrases.entry((e.from, e.dir)).or_default();
                        if !known.iter().any(|x| x.eq_ignore_ascii_case(ph)) {
                            known.push(ph.clone());
                        }
                    }
                    exits.push(e);
                }
            }
            for w in windows(graph, &p.file, &steps, p.verification.end) {
                let first = w.steps[0].from;
                let last = w.steps.last().expect("a window has its exit");
                match k.windows.entry((first, last.from, last.dir)) {
                    std::collections::btree_map::Entry::Occupied(mut o)
                        if o.get().steps.len() > w.steps.len() =>
                    {
                        o.insert(w);
                    }
                    std::collections::btree_map::Entry::Occupied(_) => {}
                    std::collections::btree_map::Entry::Vacant(v) => {
                        v.insert(w);
                    }
                }
            }
            k.by_file.insert(p.file, exits);
        }
        Ok(k)
    }

    /// The path windows: stretches of the paths that open a sealed Hidden exit and go through
    /// it.
    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.windows.values()
    }

    /// The exits the path `file` takes, in step order (empty for a path it does not know).
    pub fn exits(&self, file: &str) -> &[PathExit] {
        self.by_file.get(file).map_or(&[], Vec::as_slice)
    }

    /// The Hidden exits the path `file` goes through that a search finds or their room opens
    /// (not the sealed ones): (room, direction).
    pub fn hidden_exits(&self, file: &str) -> BTreeSet<(Key, u8)> {
        self.exits(file)
            .iter()
            .filter(|e| e.kind == ExitType::Hidden && !e.sealed)
            .map(|e| (e.from, e.dir))
            .collect()
    }

    /// The Hidden exits any verified path goes through: (room, direction). A way into an area
    /// is usually another path's (the Bandit Keep's is `Ftrehkep.mp`'s), and the exits are as
    /// real for a loop that does not take them. Not the sealed ones: no search finds them.
    pub fn all_hidden_exits(&self) -> BTreeSet<(Key, u8)> {
        self.by_file
            .values()
            .flatten()
            .filter(|e| e.kind == ExitType::Hidden && !e.sealed)
            .map(|e| (e.from, e.dir))
            .collect()
    }

    /// The phrases paths type for the Text exit `dir` of `from`.
    pub fn phrases(&self, from: Key, dir: u8) -> &[String] {
        self.phrases.get(&(from, dir)).map_or(&[], Vec::as_slice)
    }

    /// Every Text exit a path takes, with its phrases.
    pub fn text_exits(&self) -> impl Iterator<Item = ((Key, u8), &[String])> {
        self.phrases.iter().map(|(k, v)| (*k, v.as_slice()))
    }

    /// The crossings the paths know: (room, the room it leads to, the phrase).
    pub fn crossings(&self) -> impl Iterator<Item = (Key, Key, String)> + '_ {
        self.crossings.iter().cloned()
    }

    /// The paths it knows.
    pub fn len(&self) -> usize {
        self.by_file.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_file.is_empty()
    }
}

/// The windows of one path (`steps`, ending in `end`): for each step through a sealed Hidden
/// exit, the stretch back to the first step since the path last went through it whose phrase is
/// one of the Remote Actions on it (typed in a room the step leaves the character in). Every
/// step of the stretch must be one the graph can follow: a direction to the next step's room, a
/// phrase that stays in its room, or a phrase taking a way through to the next step's room;
/// MegaMud's own `@` commands are left out.
fn windows(
    graph: &RoomGraph,
    file: &str,
    steps: &[crate::pack::MmStep],
    end: Option<Key>,
) -> Vec<Window> {
    let room = |i: usize| steps.get(i).and_then(|s| s.walk.as_ref().map(|(k, _)| *k));
    let after = |i: usize| {
        if i + 1 < steps.len() {
            room(i + 1)
        } else {
            end
        }
    };
    let mut out = Vec::new();
    // Where the path last went through each sealed exit: the stretch before it opened it then.
    let mut passed: HashMap<(Key, u8), usize> = HashMap::new();
    for (j, s) in steps.iter().enumerate() {
        let (Some(from), Some(d)) = (room(j), mm::direction(&s.base)) else {
            continue;
        };
        let Some(exit) = graph.edge(from, d as u8).filter(|e| e.sealed()) else {
            continue;
        };
        let since = passed.insert((from, exit.dir), j + 1).unwrap_or(0);
        let opens = |i: usize| {
            let (Some(at), Some(ph)) = (room(i), phrase(&steps[i].base)) else {
                return false;
            };
            mm::direction(&ph).is_none()
                && after(i) == Some(at)
                && graph.edges(at).iter().any(|r| {
                    r.kind == ExitType::RemoteAction
                        && r.to == exit.from
                        && r.params[1] >= 0
                        && r.params[1] % 10 == i64::from(exit.dir)
                        && r.params[3] == 0
                        && r.phrases.iter().any(|p| p.eq_ignore_ascii_case(&ph))
                })
        };
        let Some(start) = (since..j).find(|&i| opens(i)) else {
            continue;
        };
        let mut stretch = Some(Vec::new());
        for (i, step) in steps.iter().enumerate().take(j + 1).skip(start) {
            let base = &step.base;
            if mm::direction(base).is_none() && phrase(base).is_none() {
                continue; // one of MegaMud's own `@` commands
            }
            let e = room(i)
                .zip(after(i))
                .and_then(|(at, to)| step_edge(graph, at, to, base));
            match (e, &mut stretch) {
                (Some(e), Some(w)) => w.push(e),
                _ => stretch = None,
            }
        }
        if let Some(w) = stretch.filter(|w| w.last().is_some_and(|e| e.to == exit.to)) {
            out.push(Window {
                file: file.to_owned(),
                steps: w,
            });
        }
    }
    out
}

/// The edge a path step `base` typed in `at` takes to `to`: the exit that way, a phrase that
/// stays in `at` (as a Remote Action edge from `at` to itself), or a way through taking the
/// phrase. `None` when the graph has none.
fn step_edge(graph: &RoomGraph, at: Key, to: Key, base: &str) -> Option<Edge> {
    if let Some(d) = mm::direction(base) {
        return graph
            .edge(at, d as u8)
            .filter(|e| e.to == to && e.kind != ExitType::RemoteAction)
            .cloned();
    }
    let ph = phrase(base)?;
    if to == at {
        return Some(Edge {
            from: at,
            to: at,
            dir: COMMAND_DIRS,
            kind: ExitType::RemoteAction,
            command: ph.clone(),
            phrases: vec![ph],
            unblock: Vec::new(),
            gate: Gate::None,
            door: None,
            params: [0; 4],
            window: Vec::new(),
            to_ix: graph.ix(at)?,
        });
    }
    graph
        .edges(at)
        .iter()
        .find(|e| e.to == to && (e.taken_by(&ph) || e.kind == ExitType::RoomCommand))
        .cloned()
}
