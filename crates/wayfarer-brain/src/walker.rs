//! The walker: a route to a room over the world graph, walked one confirmed step at a time
//! (docs/brain.md "Walking"). It decides what to send next from each action's outcome; the
//! session sends it through the executor. It never fights: combat stops it.

use wayfarer_parse::typed::Refusal;
use wayfarer_proto::{RoomKey, Walking};
use wayfarer_world::{Key, Route, mm, route};

use crate::brain::Atlas;
use crate::executor::{Action, Confirmed, Expect, Outcome, Refused};
use crate::position::Position;

/// `bash` commands sent at one locked door before the walker gives up on it.
pub const MAX_BASHES: u32 = 3;
/// "No exit" answers in one walk: the first relocates (look, then replan), the second stops.
pub const MAX_RELOCATIONS: u32 = 2;
/// Routes planned again in one walk before it gives up (the prototype's five attempts).
pub const MAX_REPLANS: u32 = 5;

/// A line for the dev panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Say {
    Notice(String),
    Warning(String),
}

/// What the walker wants done after a call.
#[derive(Debug, Default)]
pub(crate) struct Next {
    /// The action to submit to the executor.
    pub action: Option<Action>,
    pub says: Vec<Say>,
    /// The route started, changed or ended.
    pub route_changed: bool,
}

impl Next {
    fn notice(&mut self, text: impl Into<String>) {
        self.says.push(Say::Notice(text.into()));
    }

    fn warning(&mut self, text: impl Into<String>) {
        self.says.push(Say::Warning(text.into()));
    }
}

/// What the action in flight is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doing {
    Move,
    Open,
    Bash,
    Look,
}

#[derive(Debug)]
struct Walk {
    to: Key,
    name: String,
    route: Route,
    /// The step of `route` in progress.
    next: usize,
    doing: Doing,
    /// Moves confirmed over the whole walk.
    confirmed: u32,
    bashes: u32,
    relocations: u32,
    replans: u32,
    /// Rooms that were not the one the route expected.
    drifts: u32,
    /// Stop when the action in flight is done, and why.
    stopping: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Walker {
    walk: Option<Walk>,
    /// A `walk_to` that came while an action of the walk it replaces was in flight.
    then: Option<Key>,
}

fn room_name(atlas: &Atlas, k: Key) -> String {
    atlas
        .graph
        .room(k)
        .map_or_else(|| format!("{}/{}", k.0, k.1), |r| r.name.clone())
}

fn plural(n: usize, one: &str) -> String {
    match n {
        1 => format!("one {one}"),
        n => format!("{n} {one}s"),
    }
}

/// "6 steps, one door".
fn describe(route: &Route) -> String {
    let doors = route.iter().filter(|s| s.door.is_some()).count();
    let mut s = plural(route.len(), "step");
    if doors > 0 {
        s = format!("{s}, {}", plural(doors, "door"));
    }
    s
}

impl Walker {
    pub fn walking(&self) -> bool {
        self.walk.is_some()
    }

    /// The wire's `walking`.
    pub fn snapshot(&self) -> Option<Walking> {
        let w = self.walk.as_ref()?;
        Some(Walking {
            to: RoomKey {
                map: w.to.0,
                room: w.to.1,
            },
            name: w.name.clone(),
            steps_left: u32::try_from(w.route.len() - w.next).unwrap_or(u32::MAX),
            step: w.confirmed + 1,
        })
    }

    /// The rooms still to pass, from the one the step in progress leaves.
    pub fn route_keys(&self) -> Option<Vec<Key>> {
        let w = self.walk.as_ref()?;
        let rest = &w.route[w.next..];
        let first = rest.first()?.from;
        Some(
            std::iter::once(first)
                .chain(rest.iter().map(|s| s.to))
                .collect(),
        )
    }

    /// `walk_to`. `busy`: an action of the walk in progress is in flight; the new walk starts
    /// when it is done.
    pub fn start(&mut self, pos: &mut Position, atlas: &Atlas, to: Key, busy: bool) -> Next {
        let mut next = Next::default();
        if !atlas.graph.contains(to) {
            next.warning(format!("walk: no room {}/{} in the world pack", to.0, to.1));
            return next;
        }
        if let Some(w) = &mut self.walk {
            if busy {
                w.stopping = Some(format!("walking to {} instead", room_name(atlas, to)));
                self.then = Some(to);
                return next;
            }
            self.walk = None;
            next.route_changed = true;
        }
        let name = room_name(atlas, to);
        let Some(from) = pos.best() else {
            next.warning(format!("Not walking to {name}: the position is unknown"));
            return next;
        };
        let Some(r) = route(&atlas.graph, from, to, &pos.traveller) else {
            next.warning(format!(
                "Not walking to {name}: no way there from {}",
                room_name(atlas, from)
            ));
            return next;
        };
        if r.is_empty() {
            next.notice(format!("Arrived: {name}"));
            return next;
        }
        next.notice(format!("Walking to {name}: {}", describe(&r)));
        let mut w = Walk {
            to,
            name,
            route: r,
            next: 0,
            doing: Doing::Move,
            confirmed: 0,
            bashes: 0,
            relocations: 0,
            replans: 0,
            drifts: 0,
            stopping: None,
        };
        next.action = Some(w.send(Doing::Move));
        next.route_changed = true;
        self.walk = Some(w);
        next
    }

    /// Stop after the move in flight (`walk_stop`, combat, a typed line). `busy`: an action of
    /// the walk is queued or in flight.
    pub fn stop(&mut self, why: &str, busy: bool) -> Next {
        let mut next = Next::default();
        self.then = None;
        match &mut self.walk {
            None => {}
            Some(w) if busy => {
                if w.stopping.is_none() {
                    w.stopping = Some(why.to_owned());
                }
            }
            Some(_) => self.end(&mut next, Say::Notice(format!("Stopped walking: {why}"))),
        }
        next
    }

    /// Stop at once: the action in flight will never be answered (the session left the realm).
    pub fn abandon(&mut self, why: &str) -> Next {
        let mut next = Next::default();
        self.then = None;
        if self.walk.is_some() {
            self.end(&mut next, Say::Notice(format!("Stopped walking: {why}")));
        }
        next
    }

    fn end(&mut self, next: &mut Next, say: Say) {
        self.walk = None;
        next.says.push(say);
        next.route_changed = true;
    }

    /// The walk's action is done. The position has already read the room it brought, if any.
    pub fn outcome(
        &mut self,
        pos: &mut Position,
        atlas: &Atlas,
        action: &Action,
        outcome: Outcome,
    ) -> Next {
        let mut next = Next::default();
        let Some(w) = &mut self.walk else {
            return next;
        };
        let doing = w.doing;
        let step = w.route[w.next].clone();
        let result = match (doing, outcome) {
            (_, Outcome::TimedOut) => Err(format!("no answer to `{}`", action.command)),
            (Doing::Move, Outcome::Confirmed(Confirmed::Room)) => {
                if pos.belief.contains(step.to) {
                    w.next += 1;
                    w.confirmed += 1;
                    w.bashes = 0;
                    if w.next == w.route.len() {
                        let name = w.name.clone();
                        self.end(&mut next, Say::Notice(format!("Arrived: {name}")));
                        return self.then_start(pos, atlas, next);
                    }
                    Ok(Doing::Move)
                } else {
                    w.drifts += 1;
                    let seen = pos.title.clone().unwrap_or_else(|| "unknown".to_owned());
                    if w.drifts > 1 {
                        Err(format!(
                            "lost the way: expected {}, the room was {seen}",
                            room_name(atlas, step.to)
                        ))
                    } else {
                        w.replan(pos, atlas, &mut next, &format!("the room was {seen}"))
                    }
                }
            }
            (Doing::Move, Outcome::Refused(Refused::DoorClosed { locked: false })) => {
                Ok(Doing::Open)
            }
            (Doing::Move, Outcome::Refused(Refused::DoorClosed { locked: true }))
            | (Doing::Open, Outcome::Refused(Refused::Locked)) => Ok(Doing::Bash),
            (Doing::Move, Outcome::Refused(Refused::Move(Refusal::NoExit))) => {
                // Pitfall 34: the believed position is wrong. Look, and plan again from
                // where the game says the character is.
                w.relocations += 1;
                if w.relocations >= MAX_RELOCATIONS {
                    Err(format!("no exit `{}` twice: lost", step.command))
                } else {
                    next.notice(format!("No exit `{}`: looking to relocate", step.command));
                    Ok(Doing::Look)
                }
            }
            (Doing::Move, Outcome::Refused(Refused::Move(Refusal::Combat))) => {
                Err("in combat".to_owned())
            }
            (Doing::Move, Outcome::Refused(Refused::Move(reason))) => {
                pos.refused(atlas, &step.command);
                w.replan(
                    pos,
                    atlas,
                    &mut next,
                    &format!("`{}` refused: {}", step.command, reason.describe()),
                )
            }
            (Doing::Open, Outcome::Confirmed(Confirmed::DoorOpened))
            | (Doing::Bash, Outcome::Confirmed(Confirmed::Bash { ok: true })) => Ok(Doing::Move),
            (Doing::Bash, Outcome::Confirmed(Confirmed::Bash { ok: false })) => {
                if w.bashes < MAX_BASHES {
                    Ok(Doing::Bash)
                } else {
                    pos.block(step.from, step.dir);
                    w.replan(pos, atlas, &mut next, "the door would not bash open")
                }
            }
            (Doing::Look, Outcome::Confirmed(Confirmed::Room)) => {
                w.replan(pos, atlas, &mut next, "relocated")
            }
            (_, other) => Err(format!("`{}`: unexpected {other:?}", action.command)),
        };
        let w = self.walk.as_mut().expect("still walking");
        match result {
            Err(why) => {
                self.end(&mut next, Say::Warning(format!("Stopped walking: {why}")));
                self.then_start(pos, atlas, next)
            }
            Ok(_) if w.stopping.is_some() => {
                let why = w.stopping.take().expect("stopping");
                self.end(&mut next, Say::Notice(format!("Stopped walking: {why}")));
                self.then_start(pos, atlas, next)
            }
            Ok(_) if w.next == w.route.len() => {
                // Replanned from the destination itself.
                let name = w.name.clone();
                self.end(&mut next, Say::Notice(format!("Arrived: {name}")));
                next
            }
            Ok(doing) => {
                next.action = Some(w.send(doing));
                next
            }
        }
    }

    /// Start the walk a `walk_to` asked for while the last one was finishing.
    fn then_start(&mut self, pos: &mut Position, atlas: &Atlas, mut next: Next) -> Next {
        if let Some(to) = self.then.take() {
            let started = self.start(pos, atlas, to, false);
            next.action = started.action;
            next.says.extend(started.says);
            next.route_changed = true;
        }
        next
    }
}

impl Walk {
    /// The action for `doing` at the step in progress.
    fn send(&mut self, doing: Doing) -> Action {
        self.doing = doing;
        let step = &self.route[self.next];
        let dir = mm::DIRS[usize::from(step.dir)];
        match doing {
            Doing::Move => Action::new(step.command.clone(), Expect::RoomSeen),
            Doing::Open => Action::new(format!("open {dir}"), Expect::DoorOpened),
            Doing::Bash => {
                self.bashes += 1;
                Action::new(format!("bash {dir}"), Expect::BashResult)
            }
            // `look`, not an empty line: an empty line redisplays the room only in statline
            // FULL modes (`kb/screen-parsing.md`).
            Doing::Look => Action::new("look", Expect::RoomSeen),
        }
    }

    /// Plan again from the believed room. `Ok(Move)` with the new route in place (empty when
    /// already there), or why not.
    fn replan(
        &mut self,
        pos: &Position,
        atlas: &Atlas,
        next: &mut Next,
        why: &str,
    ) -> Result<Doing, String> {
        self.replans += 1;
        if self.replans > MAX_REPLANS {
            return Err(format!("{why}; gave up after {MAX_REPLANS} replans"));
        }
        let Some(from) = pos.best() else {
            return Err(format!("{why}; the position is unknown"));
        };
        let Some(r) = route(&atlas.graph, from, self.to, &pos.traveller) else {
            return Err(format!(
                "{why}; no way to {} from {}",
                self.name,
                room_name(atlas, from)
            ));
        };
        self.bashes = 0;
        next.route_changed = true;
        if !r.is_empty() {
            next.notice(format!("Replanned ({why}): {}", describe(&r)));
        }
        self.route = r;
        self.next = 0;
        Ok(Doing::Move)
    }
}
