//! Errands that go somewhere first (docs/brain.md "Errands"): **Train** walks to the right
//! trainer and trains, and stays there for the player to train stats; **Recover death pile**
//! walks to the room the character last died in and picks up and equips what it dropped.
//!
//! A trip walks to its room with the player's walker, then runs its errands one after the
//! other, each with the first command it needs (`look`, `i`, `train`).

use std::collections::VecDeque;

use wayfarer_parse::typed::GameEvent;
use wayfarer_world::Key;

use crate::errand::{Ctx, Errand, ErrandNext};
use crate::executor::{Action, Outcome};

#[derive(Debug)]
pub(crate) struct Trip {
    /// Names the errand on the tile and in the summary.
    label: &'static str,
    /// Where to go, named for the narration.
    dest: Option<(Key, String)>,
    /// The walk has been asked for.
    walking: bool,
    stages: VecDeque<Errand>,
    current: Option<Errand>,
    said: Vec<String>,
    /// What to add to the summary at the end (where it stopped, what to do next).
    after: Option<String>,
}

impl Trip {
    pub fn new(
        label: &'static str,
        dest: (Key, String),
        stages: Vec<Errand>,
        after: Option<String>,
    ) -> Self {
        Trip {
            label,
            dest: Some(dest),
            walking: false,
            stages: stages.into(),
            current: None,
            said: Vec::new(),
            after,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn narration(&self) -> String {
        match (&self.dest, &self.current) {
            (Some((_, name)), _) => format!("{}: walking to {name}", self.label),
            (None, Some(e)) => e.narration(),
            (None, None) => format!("{}: finishing", self.label),
        }
    }

    pub fn step(&mut self, ctx: &Ctx) -> ErrandNext {
        if ctx.walking {
            return ErrandNext::Wait;
        }
        if let Some((to, name)) = self.dest.clone() {
            if !self.walking {
                self.walking = true;
                return ErrandNext::Walk(to);
            }
            self.walking = false;
            self.dest = None;
            if ctx.here != Some(to) {
                return ErrandNext::Done(format!(
                    "{} stopped: could not get to {name}",
                    self.label
                ));
            }
        }
        loop {
            let errand = match self.current.as_mut() {
                Some(e) => e,
                None => match self.stages.pop_front() {
                    Some(e) => self.current.insert(e),
                    None => return ErrandNext::Done(self.summary()),
                },
            };
            match errand.step(ctx) {
                ErrandNext::Done(text) => {
                    self.said.push(text);
                    self.current = None;
                }
                other => return other,
            }
        }
    }

    pub fn on_event(&mut self, ev: &GameEvent) {
        if let Some(e) = &mut self.current {
            e.on_event(ev);
        }
    }

    pub fn outcome(&mut self, action: &Action, outcome: Outcome) {
        if let Some(e) = &mut self.current {
            e.outcome(action, outcome);
        }
    }

    fn summary(&self) -> String {
        let mut text = self.said.join("; ");
        if text.is_empty() {
            text = format!("{} done", self.label);
        }
        if let Some(after) = &self.after {
            text.push_str(&format!(". {after}"));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errand::ErrandKind;
    use crate::executor::Confirmed;
    use std::collections::BTreeMap;

    fn ctx<'a>(here: Option<Key>, empty: &'a BTreeMap<String, u64>) -> Ctx<'a> {
        Ctx {
            floor: empty,
            floor_items: &[],
            carried: &[],
            coins: empty,
            load: None,
            limit: "heavy",
            here,
            walking: false,
        }
    }

    fn run(
        trip: &mut Trip,
        reach: bool,
        mut answer: impl FnMut(&str) -> Vec<GameEvent>,
    ) -> Vec<String> {
        let empty = BTreeMap::new();
        let mut here = Some((1, 1));
        let mut log = Vec::new();
        for _ in 0..50 {
            match trip.step(&ctx(here, &empty)) {
                ErrandNext::Walk(to) => {
                    log.push(format!("walk {}", to.1));
                    if reach {
                        here = Some(to);
                    }
                }
                ErrandNext::Send(a) => {
                    log.push(a.command.clone());
                    for ev in answer(&a.command) {
                        trip.on_event(&ev);
                    }
                    trip.outcome(&a, Outcome::Confirmed(Confirmed::Prompt));
                }
                ErrandNext::Wait => panic!("waiting"),
                ErrandNext::Done(text) => {
                    log.push(format!("= {text}"));
                    return log;
                }
            }
        }
        panic!("never finished: {log:?}");
    }

    #[test]
    fn a_training_trip_walks_there_trains_each_level_and_says_where_it_stays() {
        let mut trip = Trip::new(
            "Train",
            ((1, 20), "the Training Room".to_owned()),
            vec![Errand::new(ErrandKind::Train)],
            Some("At the Training Room: train stats here when ready".to_owned()),
        );
        let mut left = 2;
        let log = run(&mut trip, true, |cmd| {
            if cmd != "train" {
                return vec![];
            }
            if left > 0 {
                left -= 1;
                vec![GameEvent::Trained { copper: 300 }]
            } else {
                vec![GameEvent::TrainRefused { why: "no_exp" }]
            }
        });
        assert_eq!(
            log,
            [
                "walk 20",
                "train",
                "train",
                "train",
                "= Train done: 2 levels, paid 6 gold; stopped: not enough experience for the next level. At the Training Room: train stats here when ready"
            ]
        );
    }

    #[test]
    fn a_trip_that_cannot_arrive_stops_there_and_trains_nothing() {
        let mut trip = Trip::new(
            "Train",
            ((1, 20), "the Training Room".to_owned()),
            vec![Errand::new(ErrandKind::Train)],
            None,
        );
        let log = run(&mut trip, false, |_| vec![]);
        assert_eq!(
            log,
            [
                "walk 20",
                "= Train stopped: could not get to the Training Room"
            ]
        );
    }

    #[test]
    fn the_pile_trip_picks_up_then_equips() {
        let mut trip = Trip::new(
            "Recover",
            ((1, 7), "where it died".to_owned()),
            vec![
                Errand::new(ErrandKind::PickUp),
                Errand::new(ErrandKind::Equip),
            ],
            None,
        );
        let log = run(&mut trip, true, |_| vec![]);
        assert_eq!(
            log,
            [
                "walk 7",
                "look",
                "i",
                "= Pick up done: nothing; Equip done: nothing"
            ]
        );
    }
}
