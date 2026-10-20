//! Flee (docs/brain.md "Flee", docs/plan/03-engine.md section 4): below the flee line, leave
//! the fight by a confirmed exit toward a room that spawns nothing ([`flee_exits`]), keep
//! running while a monster follows, up to [`FLEE_HOPS`] rooms, and hand over: Recover rests
//! where it got to and walks back home. With no exit to take, or still followed after the last
//! hop, it breaks off the fight and the bot stops itself with a warning.
//!
//! A hop is one move through the executor with a `RoomSeen` expectation, as a walker's step
//! is; a door on the way is opened first (never bashed: a locked door is no candidate). After
//! the room comes it waits [`FLEE_WATCH`] at the prompt for a follower: the follow text, a
//! hostile monster arriving, or an attack.

use std::collections::BTreeSet;

use tokio::time::Instant;
use wayfarer_parse::typed::{GameEvent, Refusal};
use wayfarer_world::{Key, LockState, mm};

use crate::executor::{Action, Confirmed, Expect, Outcome, Refused};
use crate::modes::{Mode, Step, View};
use crate::policy::{
    Candidate, FLEE_BELOW, FLEE_HOPS, FLEE_WATCH, flee_exits, hostile, spawns_here,
};

/// The long name of direction `d`: `north`, `up`.
fn dir_name(d: u8) -> &'static str {
    const LONG: [&str; 10] = [
        "north",
        "south",
        "east",
        "west",
        "northeast",
        "northwest",
        "southeast",
        "southwest",
        "up",
        "down",
    ];
    LONG[usize::from(d)]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doing {
    Open,
    Move,
}

/// The hop in progress: the exit, from where, and what goes next.
#[derive(Debug, Clone, Copy)]
struct Hop {
    from: Key,
    exit: Candidate,
    doing: Doing,
}

#[derive(Debug)]
pub(crate) struct Flee {
    /// The flee line, per cent of the maximum HP.
    below: i32,
    /// The gateway is logging the character off: flee a fight whatever the HP.
    leaving: bool,
    /// Took control, and has not finished.
    active: bool,
    /// Rooms fled through since HP was last at or above the line.
    hops: u32,
    hop: Option<Hop>,
    /// Exits that did not take the character in this flee: the next candidate goes.
    tried: BTreeSet<(Key, u8)>,
    /// Exits the game refused, or doors it said were locked, for the session.
    refused: BTreeSet<(Key, u8)>,
    /// The room the last hop left.
    left: Option<Key>,
    /// A hop landed: the watch for a follower starts at the next prompt.
    landed: bool,
    /// Watching for a follower until then.
    watch: Option<Instant>,
    /// A monster that followed the character in since the hop's room was shown.
    followed: Option<String>,
    /// Monsters that came in since then: one that is hostile followed.
    arrived: Vec<String>,
    /// The monsters of the room fled from, to name the one whose follow text came.
    chasers: Vec<String>,
    /// Finished with HP still under the line: it does not take control again until HP is
    /// back or a monster attacks.
    spent: bool,
    /// Giving up: the bot stops itself, with this reason.
    give_up: Option<String>,
    /// A flee started, at this HP: for the run's statistics.
    started: Option<i32>,
}

impl Default for Flee {
    fn default() -> Self {
        Flee::new(FLEE_BELOW)
    }
}

impl Flee {
    pub fn new(below: i32) -> Flee {
        Flee {
            below,
            leaving: false,
            active: false,
            hops: 0,
            hop: None,
            tried: BTreeSet::new(),
            refused: BTreeSet::new(),
            left: None,
            landed: false,
            watch: None,
            followed: None,
            arrived: Vec::new(),
            chasers: Vec::new(),
            spent: false,
            give_up: None,
            started: None,
        }
    }

    /// A fresh start (`start`, a restore): the line and the exits refused this session stay.
    pub fn restart(&mut self) {
        let refused = std::mem::take(&mut self.refused);
        *self = Flee {
            refused,
            ..Flee::new(self.below)
        };
    }

    /// The gateway is logging off and the character is attacked: flee first, whatever the HP.
    pub fn leave(&mut self) {
        self.leaving = true;
        self.spent = false;
        self.hops = 0;
    }

    /// An HP value: back at or above the line, the next fall is a new flee with its hops.
    pub fn hp(&mut self, hp: Option<i32>) {
        if hp.is_some_and(|hp| hp >= self.below) && !self.active {
            self.spent = false;
            self.hops = 0;
        }
    }

    /// HP is under the line and a flee would start now: what is in flight should give way.
    pub fn urgent(&self, hp: Option<i32>) -> bool {
        !self.active && !self.spent && hp.is_some_and(|hp| hp < self.below)
    }

    /// Fleeing: it has control until it finishes.
    pub fn busy(&self) -> bool {
        self.active
    }

    /// Why the bot must stop, once: no exit, or followed through every hop.
    pub fn take_give_up(&mut self) -> Option<String> {
        self.give_up.take()
    }

    /// The HP a flee started at, once.
    pub fn take_started(&mut self) -> Option<i32> {
        self.started.take()
    }

    /// When the watch for a follower ends, for the clock to wake the bot.
    pub fn wake(&self) -> Option<Instant> {
        self.watch
    }

    fn finish(&mut self, narration: String) -> Step {
        self.active = false;
        self.spent = true;
        self.hop = None;
        self.watch = None;
        self.landed = false;
        self.tried.clear();
        self.chasers.clear();
        Step::wait(narration)
    }

    /// Give up: `break` off a fight first; the bot then stops itself with `why`.
    fn corner(&mut self, v: &View, why: String) -> Step {
        let hp = v.hp.map_or_else(String::new, |hp| format!("at {hp}% HP: "));
        let step = self.finish(format!("Flee: {why}"));
        self.give_up = Some(format!("{hp}{why}; fight or run by hand"));
        if v.engaged {
            return Step::send(
                Action::new("break", Expect::Prompt),
                format!("Flee: {why}; breaking off"),
            )
            .warning();
        }
        step.warning()
    }

    /// The hop's next action.
    fn send(&self, v: &View, why: &str) -> Step {
        let hop = self.hop.expect("a hop");
        let dir = dir_name(hop.exit.dir);
        match hop.doing {
            Doing::Open => Step::send(
                Action::new(format!("open {dir}"), Expect::DoorOpened),
                format!("{why}: opening the door {dir}"),
            ),
            Doing::Move => Step::send(
                Action::new(mm::DIRS[usize::from(hop.exit.dir)], Expect::RoomSeen),
                format!("{why}: {dir} to {}", v.room_name(Some(hop.exit.to))),
            ),
        }
    }
}

impl Mode for Flee {
    fn wants(&self, v: &View) -> Option<String> {
        if self.active {
            return Some("fleeing".to_owned());
        }
        if self.leaving {
            // Once: the session asked because the character was attacked.
            return (!self.spent).then(|| "logging off under attack".to_owned());
        }
        let hp = v.hp.filter(|&hp| hp < self.below)?;
        let attacked = !v.presence.attackers.is_empty();
        // Fled already: Recover rests, unless a monster attacks again. Nothing to run from
        // in a quiet room either (HP lost to poison, say).
        let threat = v.engaged || attacked || spawns_here(v.atlas, v.room, v.certain);
        if (self.spent && !attacked) || !threat {
            return None;
        }
        Some(format!("HP {hp}%"))
    }

    fn step(&mut self, v: &View) -> Step {
        let here = v.room_name(v.room);
        let line = match v.hp {
            _ if self.leaving => "Logging off under attack: fleeing first".to_owned(),
            Some(hp) => format!("Fleeing: HP {hp}%"),
            None => "Fleeing".to_owned(),
        };
        if !self.active {
            self.active = true;
            // A flee before a log-off is counted when the log-off starts, before the run ends.
            self.started = (!self.leaving).then(|| v.hp.unwrap_or(0));
        }
        if self.hop.is_some() {
            // The door is open now (or shut, not locked): on with the hop.
            return self.send(v, &line);
        }
        if self.landed {
            self.landed = false;
            self.watch = Some(v.now + FLEE_WATCH);
        }
        let mut why = line;
        if let Some(until) = self.watch {
            let follower = self
                .followed
                .clone()
                .or_else(|| {
                    let hostile = |n: &&String| hostile(&v.atlas.monsters, n);
                    self.arrived.iter().find(hostile).cloned()
                })
                .or_else(|| v.presence.attackers.first().cloned());
            if follower.is_none() && v.now < until {
                return Step::wait(format!("Flee: in {here}, watching for followers"));
            }
            self.watch = None;
            match follower {
                None if !spawns_here(v.atlas, v.room, v.certain) => {
                    return self.finish(format!("Safe in {here}"));
                }
                None if self.hops >= FLEE_HOPS => {
                    return self.finish(format!(
                        "Fled {FLEE_HOPS} rooms: nothing followed into {here}"
                    ));
                }
                None => why = format!("Fleeing on: {here} spawns"),
                Some(name) if self.hops >= FLEE_HOPS => {
                    return self.corner(v, format!("followed by {name} through {FLEE_HOPS} rooms"));
                }
                Some(name) => why = format!("Followed by {name}"),
            }
        } else if !v.engaged
            && v.presence.attackers.is_empty()
            && !spawns_here(v.atlas, v.room, v.certain)
        {
            // Nothing to run from, and nothing spawns here.
            return self.finish(format!("Safe in {here}"));
        } else if self.hops >= FLEE_HOPS {
            return self.corner(v, format!("attacked again after fleeing {FLEE_HOPS} rooms"));
        }
        let Some(from) = v.room.filter(|_| v.certain) else {
            return self.corner(v, "cornered: not sure which room this is".to_owned());
        };
        let blocked: BTreeSet<(Key, u8)> = v
            .traveller
            .blocked
            .iter()
            .chain(&self.refused)
            .chain(&self.tried)
            .copied()
            .collect();
        let mut exits = flee_exits(v.atlas, Some(from), true, &blocked, v.doors);
        // Not back where it came from, if there is anywhere else as good.
        exits.sort_by_key(|c| {
            let tier = (c.door.is_some(), !c.quiet);
            (tier, Some(c.to) == self.left)
        });
        let Some(&exit) = exits.first() else {
            return self.corner(v, format!("cornered in {here}: no exit to flee by"));
        };
        let p = v.presence;
        for name in p.attackers.iter().chain(&p.here) {
            if !self.chasers.contains(name) {
                self.chasers.push(name.clone());
            }
        }
        self.followed = None;
        self.arrived.clear();
        let doing = match exit.door {
            Some(LockState::Open) | None => Doing::Move,
            Some(_) => Doing::Open,
        };
        self.hop = Some(Hop { from, exit, doing });
        self.send(v, &why)
    }

    fn on_event(&mut self, ev: &GameEvent, _now: Instant) {
        if !self.active {
            return;
        }
        match ev {
            // The hop's room: who strikes or comes in from here on followed.
            GameEvent::RoomTitle { .. } if self.hop.is_some_and(|h| h.doing == Doing::Move) => {
                self.followed = None;
                self.arrived.clear();
            }
            // `The giant rat creeps in after you!`: the monster's own follow text, or the
            // generic pattern's. Named as one of those left behind, if it is one.
            GameEvent::Other(e) if e.name.starts_with("monster_follows") => {
                let line = e.line.to_ascii_lowercase();
                let name = self
                    .chasers
                    .iter()
                    .find(|c| line.contains(&c.to_ascii_lowercase()))
                    .cloned()
                    .or_else(|| e.text("mon").map(str::to_owned))
                    .unwrap_or_else(|| "a monster".to_owned());
                self.followed = Some(name);
            }
            GameEvent::MonsterArrives { name } => self.arrived.push(name.clone()),
            _ => {}
        }
    }

    fn outcome(&mut self, action: &Action, outcome: Outcome) {
        let Some(hop) = self.hop else {
            return;
        };
        let exit = (hop.from, hop.exit.dir);
        let mut give_way = |permanent: bool| {
            self.tried.insert(exit);
            if permanent {
                self.refused.insert(exit);
            }
            self.hop = None;
        };
        match (action.expect, outcome) {
            (Expect::DoorOpened, Outcome::Confirmed(Confirmed::DoorOpened)) => {
                self.hop = Some(Hop {
                    doing: Doing::Move,
                    ..hop
                });
            }
            (Expect::DoorOpened, Outcome::Refused(Refused::Locked)) => give_way(true),
            (Expect::RoomSeen, Outcome::Confirmed(Confirmed::Room)) => {
                self.hop = None;
                self.hops += 1;
                self.landed = true;
                self.left = Some(hop.from);
                self.tried.clear();
            }
            (Expect::RoomSeen, Outcome::Refused(Refused::DoorClosed { locked: false })) => {
                self.hop = Some(Hop {
                    doing: Doing::Open,
                    ..hop
                });
            }
            // Locked, or refused (`You may not enter that room while in combat.`, a guard):
            // the next candidate, at once.
            (Expect::RoomSeen, Outcome::Refused(Refused::DoorClosed { locked: true })) => {
                give_way(true)
            }
            // In combat now, maybe not later; any other refusal holds for the session.
            (Expect::RoomSeen, Outcome::Refused(Refused::Move(r))) => {
                give_way(r != Refusal::Combat)
            }
            (Expect::Prompt, _) => {}
            _ => give_way(false),
        }
    }
}
