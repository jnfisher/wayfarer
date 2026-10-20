//! What MegaMud's paths know about the exits they take (`docs/plan/03-engine.md` "Areas and
//! loops"): for every step of a path whose verification walk reached the end, the room the
//! walk typed it in and the exit it took. A direction step takes the exit that way, with the
//! actions written before it (`n[search n]`); a phrase (`climb tree`) takes a Text exit to the
//! room the next step starts in. From that: the Hidden exits a path finds, which a traveller on
//! that path may search for, and the phrase a Text exit takes where the pack's message has
//! none.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::Error;
use crate::mm;
use crate::nav::{ExitType, Key, RoomGraph};
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
}

/// The exits MegaMud's verified paths take, built once from the pack.
#[derive(Clone, Debug, Default)]
pub struct PathKnowledge {
    /// By path file, in step order.
    by_file: HashMap<String, Vec<PathExit>>,
    /// Text exits by (room, direction): the phrases paths type for them, in file order.
    phrases: BTreeMap<(Key, u8), Vec<String>>,
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
                    }),
                    None => graph
                        .edges(*from)
                        .iter()
                        .find(|e| e.kind == ExitType::Text && Some(e.to) == next)
                        .map(|e| PathExit {
                            from: *from,
                            dir: e.dir,
                            kind: e.kind,
                            actions,
                            phrase: Some(s.base.trim().to_owned()),
                        }),
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
            k.by_file.insert(p.file, exits);
        }
        Ok(k)
    }

    /// The exits the path `file` takes, in step order (empty for a path it does not know).
    pub fn exits(&self, file: &str) -> &[PathExit] {
        self.by_file.get(file).map_or(&[], Vec::as_slice)
    }

    /// The Hidden exits the path `file` goes through: (room, direction).
    pub fn hidden_exits(&self, file: &str) -> BTreeSet<(Key, u8)> {
        self.exits(file)
            .iter()
            .filter(|e| e.kind == ExitType::Hidden)
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

    /// The paths it knows.
    pub fn len(&self) -> usize {
        self.by_file.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_file.is_empty()
    }
}
