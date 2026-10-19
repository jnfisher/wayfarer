//! The bot (docs/brain.md "Fight here", docs/plan/03-engine.md sections 4 and 5): running,
//! paused or stopped, and the mode stack, highest first: Recover, Fight, Idle. On every tick
//! the highest mode that wants control gets it and says what to do next; the session sends its
//! action through the executor (one in flight) or its walk through the walker, and hands the
//! outcome back. Every change of what it is doing is narrated.

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{BotMode, BotState};
use wayfarer_world::{Key, Traveller};

use crate::brain::Atlas;
use crate::executor::{Action, Outcome};
use crate::fight::Fight;
use crate::policy::{self, STOP_BELOW};
use crate::recover::Recover;
use crate::walker::Say;

/// Who is in the room, as the game has shown it: the monsters of the last `Also here:`, with
/// the arrivals, departures and deaths since; and those that attacked the character since.
#[derive(Debug, Default)]
pub(crate) struct Presence {
    pub here: Vec<String>,
    pub attackers: Vec<String>,
    /// The room being shown: its `Also here:` monsters until its exits come.
    shown: Option<Vec<String>>,
}

impl Presence {
    pub fn event(&mut self, atlas: &Atlas, ev: &GameEvent) {
        match ev {
            GameEvent::RoomTitle { .. } => self.shown = Some(Vec::new()),
            GameEvent::AlsoHere { monsters, .. } => match &mut self.shown {
                Some(shown) => shown.clone_from(monsters),
                None => self.here.clone_from(monsters),
            },
            GameEvent::RoomExits { .. } | GameEvent::RoomDark | GameEvent::Prompt { .. } => {
                if let Some(shown) = self.shown.take() {
                    self.here = shown;
                    // A new room, or this one shown again: who attacked is here or gone.
                    let here = &self.here;
                    self.attackers.retain(|a| here.contains(a));
                }
            }
            GameEvent::MonsterArrives { name } => self.here.push(name.clone()),
            GameEvent::MonsterLeaves { name } => {
                remove_first(&mut self.here, |n| n == name);
                remove_first(&mut self.attackers, |n| n == name);
            }
            GameEvent::AttackedBy { name } if !self.attackers.contains(name) => {
                self.attackers.push(name.clone());
            }
            GameEvent::MonsterDeath { monsters } => {
                let dead = |n: &String| policy::is_one_of(&atlas.monsters, n, monsters);
                // Who died is the one attacking, if one of them fits.
                if let Some(i) = self.attackers.iter().position(dead) {
                    let name = self.attackers.remove(i);
                    remove_first(&mut self.here, |n| *n == name);
                } else {
                    remove_first(&mut self.here, dead);
                }
            }
            GameEvent::YouKilled => {
                self.here.clear();
                self.attackers.clear();
            }
            _ => {}
        }
    }
}

fn remove_first(v: &mut Vec<String>, f: impl FnMut(&String) -> bool) {
    if let Some(i) = v.iter().position(f) {
        v.remove(i);
    }
}

/// What the modes see.
pub(crate) struct View<'a> {
    pub atlas: &'a Atlas,
    pub now: Instant,
    /// HP in per cent of the maximum, when both are known.
    pub hp: Option<i32>,
    /// In combat, or not known to be out of it.
    pub engaged: bool,
    pub resting: bool,
    pub poisoned: bool,
    /// The believed room, and whether the belief is sure of it.
    pub room: Option<Key>,
    pub certain: bool,
    pub traveller: &'a Traveller,
    pub presence: &'a Presence,
}

impl View<'_> {
    pub fn room_name(&self, k: Option<Key>) -> String {
        k.and_then(|k| self.atlas.graph.room(k))
            .map_or_else(|| "this room".to_owned(), |r| r.name.clone())
    }
}

/// What a mode wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Do {
    /// Through the executor.
    Send(Action),
    /// Through the walker, step by confirmed step.
    Walk(Key),
}

/// A mode's decision: what to do (nothing, to wait), and the narration.
#[derive(Debug)]
pub(crate) struct Step {
    pub act: Option<Do>,
    pub narration: String,
}

impl Step {
    pub fn wait(narration: impl Into<String>) -> Step {
        Step {
            act: None,
            narration: narration.into(),
        }
    }

    pub fn send(action: Action, narration: impl Into<String>) -> Step {
        Step {
            act: Some(Do::Send(action)),
            narration: narration.into(),
        }
    }

    pub fn walk(to: Key, narration: impl Into<String>) -> Step {
        Step {
            act: Some(Do::Walk(to)),
            narration: narration.into(),
        }
    }
}

/// One mode of the stack.
pub(crate) trait Mode {
    /// Why it wants control, if it does.
    fn wants(&self, v: &View) -> Option<String>;
    /// It has control, nothing is in flight and the game is at a prompt: what next.
    fn step(&mut self, v: &View) -> Step;
    /// An event from the game, whether or not the mode has control.
    fn on_event(&mut self, ev: &GameEvent, now: Instant);
    /// The outcome of an action this mode asked for.
    fn outcome(&mut self, action: &Action, outcome: Outcome);
}

#[derive(Debug, Default)]
pub(crate) struct Bot {
    state: BotState,
    mode: BotMode,
    narration: String,
    reason: Option<String>,
    fight: Fight,
    recover: Recover,
    /// The mode whose action is in flight.
    acting: Option<BotMode>,
    /// Pause when the action in flight is done, and why.
    pausing: Option<String>,
}

impl Bot {
    pub fn running(&self) -> bool {
        self.state == BotState::Running
    }

    /// Running, and not about to pause.
    pub fn active(&self) -> bool {
        self.running() && self.pausing.is_none()
    }

    /// The wire's `bot`.
    pub fn snapshot(&self) -> wayfarer_proto::Bot {
        wayfarer_proto::Bot {
            state: self.state,
            mode: self.mode,
            narration: self.narration.clone(),
            reason: self.reason.clone(),
        }
    }

    fn narrate(&mut self, text: String, says: &mut Vec<Say>) {
        if text != self.narration {
            says.push(Say::Notice(text.clone()));
            self.narration = text;
        }
    }

    /// `start`: play from `home`, the room the character is in. The session re-syncs first.
    pub fn start(&mut self, home: Option<Key>, room_name: &str) -> Vec<Say> {
        let mut says = Vec::new();
        self.state = BotState::Running;
        self.mode = BotMode::Idle;
        self.reason = None;
        self.pausing = None;
        self.fight = Fight::default();
        self.recover = Recover::new(home);
        self.narrate(format!("Starting in {room_name}: re-syncing"), &mut says);
        says
    }

    /// `stop`, or the bot stopping itself (`warning`).
    pub fn stop(&mut self, why: &str, warning: bool) -> Vec<Say> {
        let mut says = Vec::new();
        if self.state == BotState::Stopped {
            return says;
        }
        self.state = BotState::Stopped;
        self.mode = BotMode::Idle;
        self.pausing = None;
        self.reason = Some(why.to_owned());
        // "Stopped at 24% HP: ...", "Stopped: asked to stop".
        let sep = if why.starts_with("at ") { " " } else { ": " };
        self.narration = format!("Stopped{sep}{why}");
        says.push(if warning {
            Say::Warning(self.narration.clone())
        } else {
            Say::Notice(self.narration.clone())
        });
        says
    }

    /// Pause after the action in flight (`busy`), or now.
    pub fn pause(&mut self, why: &str, busy: bool) -> Vec<Say> {
        let mut says = Vec::new();
        if self.state != BotState::Running || self.pausing.is_some() {
            return says;
        }
        if busy {
            self.pausing = Some(why.to_owned());
        } else {
            self.paused(why, &mut says);
        }
        says
    }

    fn paused(&mut self, why: &str, says: &mut Vec<Say>) {
        self.state = BotState::Paused;
        self.pausing = None;
        self.reason = Some(why.to_owned());
        self.narrate(format!("Paused: {why}"), says);
    }

    /// `resume`: the session re-syncs first.
    pub fn resume(&mut self) -> Vec<Say> {
        let mut says = Vec::new();
        if self.state != BotState::Paused {
            return says;
        }
        self.state = BotState::Running;
        self.reason = None;
        self.narrate("Resuming: re-syncing".to_owned(), &mut says);
        says
    }

    /// An event from the game: the modes follow it whatever the state; a death stops the bot.
    pub fn event(&mut self, ev: &GameEvent, now: Instant) -> Vec<Say> {
        self.fight.on_event(ev, now);
        self.recover.on_event(ev, now);
        match ev {
            GameEvent::YouKilled => self.stop("the character was killed", true),
            _ => Vec::new(),
        }
    }

    /// An HP value, wherever it came from (pitfall 27: every one is checked, mid-walk and
    /// mid-action too): below [`STOP_BELOW`] a running bot stops itself, with a warning.
    pub fn hp(&mut self, hp: Option<i32>) -> Vec<Say> {
        match hp {
            Some(hp) if hp < STOP_BELOW && self.running() => {
                let why = format!("at {hp}% HP: flee is not built yet; fight or run by hand");
                self.stop(&why, true)
            }
            _ => Vec::new(),
        }
    }

    /// The outcome of the bot's action: to the mode that asked for it. A pause asked for
    /// meanwhile takes effect now.
    pub fn outcome(&mut self, action: &Action, outcome: Outcome) -> Vec<Say> {
        match self.acting.take() {
            Some(BotMode::Fight) => self.fight.outcome(action, outcome),
            Some(BotMode::Recover) => self.recover.outcome(action, outcome),
            _ => {}
        }
        self.settle()
    }

    /// The action in flight is done (the bot's or another's): a pending pause takes effect.
    pub fn settle(&mut self) -> Vec<Say> {
        let mut says = Vec::new();
        if let Some(why) = self.pausing.take() {
            self.paused(&why, &mut says);
        }
        says
    }

    /// When a wait ends, for the clock to wake the bot.
    pub fn wake(&self) -> Option<Instant> {
        self.recover.wake()
    }

    /// The bot's walk ended, or its action was dropped, with no outcome to hand on: the mode
    /// decides again at the next tick.
    pub fn forget(&mut self) {
        self.acting = None;
    }

    /// Decide: nothing is in flight, nothing is walking, and the game is at a prompt.
    pub fn tick(&mut self, v: &View) -> (Option<Do>, Vec<Say>) {
        let mut says = Vec::new();
        if self.state != BotState::Running || self.pausing.is_some() {
            return (None, says);
        }
        let stopped = self.hp(v.hp);
        if !stopped.is_empty() {
            return (None, stopped);
        }
        // A mode that finishes in its step (rested, back home) hands over at once: the stack
        // is asked again, once.
        for _ in 0..2 {
            let (mode, reason) = if let Some(r) = self.recover.wants(v) {
                (BotMode::Recover, r)
            } else if let Some(r) = self.fight.wants(v) {
                (BotMode::Fight, r)
            } else {
                (BotMode::Idle, "nothing to do".to_owned())
            };
            if mode != self.mode {
                tracing::info!(?mode, %reason, "the bot's mode");
                self.mode = mode;
            }
            let step = match mode {
                BotMode::Recover => self.recover.step(v),
                BotMode::Fight => self.fight.step(v),
                BotMode::Idle => Step::wait("Waiting here"),
            };
            let finished = step.act.is_none() && mode == BotMode::Recover && !self.recover.busy();
            if step.act.is_some() {
                self.acting = Some(mode);
            }
            self.narrate(step.narration, &mut says);
            if !finished {
                return (step.act, says);
            }
        }
        (None, says)
    }
}
