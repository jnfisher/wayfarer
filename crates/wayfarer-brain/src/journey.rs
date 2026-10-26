//! Journey (docs/brain.md "The bot's states"): a walk the player asked for, run by the bot. A Go
//! to (`walk_to`) has one stop; a loop walked once (`walk_loop`) has the loop's stops from the
//! one nearest the character round to it again. Like Task, it sits below Flee, Recover and
//! Fight: the bot fights on the way and walks on from wherever the fight left it. A *passive*
//! one (a Go to with `fight` off) attacks nothing on the way: Fight takes only what attacks
//! the character. At the last stop the journey is over and the bot plays there, with that room
//! as its home.
//!
//! A walk the session stopped (a fight, a sneak to make again) is begun again from wherever
//! the character is, [`LEG_TRIES`] times with no fight between; one the walker could not start
//! (no route) or gave up by itself (lost, a door that will not open) ends the journey at once.

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_world::Key;

use crate::executor::{Action, Outcome};
use crate::modes::{Mode, Step, View};
use crate::task::LEG_TRIES;

#[derive(Debug, Clone)]
pub(crate) struct Journey {
    /// "Newhaven, Arena", or "loop Newhaven".
    name: String,
    /// A loop walked once: its stops are narrated.
    looped: bool,
    stops: Vec<(Key, String)>,
    /// The stop it walks to.
    next: usize,
    /// Stop one room short of the last stop, a boss lair (Go to's `stop_short`).
    short: bool,
    /// Legs to the stop at `next` begun since it last arrived or fought.
    tries: u32,
    /// A leg was begun to the stop at `next`.
    walked: bool,
    /// Over: arrived, or it could not get there (why).
    done: Option<Option<String>>,
    notes: Vec<String>,
    /// Attack nothing on the way: fight only what attacks the character.
    passive: bool,
}

impl Journey {
    /// A Go to `to`, named `name`; `short`: `to` is a boss lair to stop one room short of.
    pub fn go_to(to: Key, name: String, short: bool) -> Journey {
        Journey {
            name: name.clone(),
            looped: false,
            stops: vec![(to, name)],
            next: 0,
            short,
            tries: 0,
            walked: false,
            done: None,
            notes: Vec::new(),
            passive: false,
        }
    }

    /// Loop `name` walked once through `stops` (the first again at the end).
    pub fn walk_loop(name: &str, stops: Vec<(Key, String)>) -> Journey {
        Journey {
            name: format!("loop {name}"),
            looped: true,
            stops,
            next: 0,
            short: false,
            tries: 0,
            walked: false,
            done: None,
            notes: Vec::new(),
            passive: false,
        }
    }

    /// The same journey, attacking nothing on the way when `passive`.
    pub fn passive(mut self, passive: bool) -> Journey {
        self.passive = passive;
        self
    }

    /// It attacks nothing on the way.
    pub fn is_passive(&self) -> bool {
        self.passive
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where it walks to now.
    pub fn to(&self) -> Key {
        self.stops[self.next.min(self.stops.len() - 1)].0
    }

    /// The wire's `journey`.
    pub fn snapshot(&self) -> wayfarer_proto::Journey {
        let to = self.to();
        wayfarer_proto::Journey {
            name: self.name.clone(),
            to: wayfarer_proto::RoomKey {
                map: to.0,
                room: to.1,
            },
            passive: self.passive,
        }
    }

    /// The journey is over: `Some(None)` arrived, `Some(Some(why))` could not get there.
    pub fn take_done(&mut self) -> Option<Option<String>> {
        self.done.take()
    }

    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    fn total(&self) -> usize {
        if self.stops.len() > 1 {
            self.stops.len() - 1
        } else {
            1
        }
    }

    /// At the stop it walked to: on to the next one, or over.
    fn reached(&mut self) {
        if self.looped {
            let name = &self.stops[self.next].1;
            let loop_name = self.name.trim_start_matches("loop ");
            self.notes.push(
                if self.next + 1 == self.stops.len() && self.stops.len() > 1 {
                    format!("Loop {loop_name} done: back at {name}")
                } else {
                    format!("Loop stop {}/{}: {name}", self.next + 1, self.total())
                },
            );
        } else if !self.walked {
            // Already there: no walk, so the walker said nothing.
            self.notes
                .push(format!("Arrived: {}", self.stops[self.next].1));
        }
        self.next += 1;
        self.tries = 0;
        self.walked = false;
        if self.next >= self.stops.len() {
            self.done = Some(None);
        }
    }

    /// The bot's tick, whichever mode has control.
    pub fn observe(&mut self, v: &View) {
        if self.done.is_none() && self.next < self.stops.len() && v.room == Some(self.to()) {
            self.reached();
        }
    }

    /// The walker ended a leg of the journey at `at`: a Go to stopping short of a boss lair is
    /// there.
    pub fn arrived(&mut self, at: Key) {
        if self.done.is_none() && self.short && self.next + 1 == self.stops.len() && at != self.to()
        {
            self.reached();
        }
    }

    /// The walker could not start the leg (no route), or gave it up (it said why): the
    /// journey is over.
    pub fn walk_failed(&mut self) {
        if self.done.is_none() {
            self.done = Some(Some("the walk stopped".to_owned()));
        }
    }
}

impl Mode for Journey {
    fn wants(&self, _v: &View) -> Option<String> {
        (self.done.is_none() && self.next < self.stops.len())
            .then(|| format!("going to {}", self.stops[self.next].1))
    }

    fn step(&mut self, _v: &View) -> Step {
        if self.tries >= LEG_TRIES {
            self.done = Some(Some(format!(
                "{LEG_TRIES} walks to {} ended elsewhere",
                self.stops[self.next].1
            )));
            return Step::wait("Going: could not get there");
        }
        self.tries += 1;
        self.walked = true;
        let (to, name) = self.stops[self.next].clone();
        let last = self.next + 1 == self.stops.len();
        Step::walk_short(to, last && self.short, format!("Going to {name}"))
            .doing(format!("going to {name}"))
    }

    fn on_event(&mut self, ev: &GameEvent, _now: Instant) {
        // A fight on the way stopped the leg: it does not count.
        if matches!(ev, GameEvent::CombatEngaged | GameEvent::AttackedBy { .. }) {
            self.tries = 0;
        }
    }

    fn outcome(&mut self, _action: &Action, _outcome: Outcome) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loop_walk_names_its_stops_and_ends_back_at_the_first() {
        let a: Key = (1, 1);
        let b: Key = (1, 2);
        let mut t = Journey::walk_loop(
            "Newhaven",
            vec![(a, "Arena".into()), (b, "Road".into()), (a, "Arena".into())],
        );
        t.reached();
        t.reached();
        assert!(t.take_done().is_none());
        t.reached();
        assert_eq!(
            t.take_notes(),
            [
                "Loop stop 1/2: Arena",
                "Loop stop 2/2: Road",
                "Loop Newhaven done: back at Arena"
            ]
        );
        assert_eq!(t.take_done(), Some(None));
    }

    #[test]
    fn a_go_to_where_it_is_already_says_it_arrived() {
        let mut t = Journey::go_to((1, 1), "Arena".into(), false);
        t.reached();
        assert_eq!(t.take_notes(), ["Arrived: Arena"]);
        assert_eq!(t.take_done(), Some(None));
    }

    #[test]
    fn a_go_to_short_of_a_lair_ends_where_the_walker_stopped() {
        let lair: Key = (1, 9);
        let mut t = Journey::go_to(lair, "Lair".into(), true);
        t.arrived((1, 8));
        assert_eq!(t.take_done(), Some(None));
        let mut t = Journey::go_to(lair, "Lair".into(), false);
        t.arrived((1, 8));
        assert!(t.take_done().is_none());
    }
}
