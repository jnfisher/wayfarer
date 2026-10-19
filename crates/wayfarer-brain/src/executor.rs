//! The executor (docs/plan/03-engine.md section 3): an action is a command plus the events
//! that confirm it and a timeout. One action is in flight per session; it goes at a prompt
//! boundary in the realm (the session decides when that is), and it is done at the first
//! prompt after its answer, or when its time runs out. The hard guards live here, so no mode
//! can get round them.

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::{GameEvent, Refusal};

/// How long an action waits for its answer. A direction command waits a tick before the game
/// takes it (`kb/navigation.md`), well within this.
pub const ACTION_TIMEOUT: Duration = Duration::from_secs(5);

/// What confirms an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// A room is shown: a move, or a look. The movement refusals and a shut door fail it.
    RoomSeen,
    /// `open <dir>`: the door is open now (or was). `The door is locked.` fails it.
    DoorOpened,
    /// `bash <dir>`: bashed open, or the attempt failed.
    BashResult,
    /// Anything: the next prompt.
    Prompt,
    /// An attack: `*Combat Engaged*`. `You do not see <x> here!`, `Your weapon has no
    /// effect` and the other [`ATTACK_REFUSALS`] refuse it.
    Engaged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub command: String,
    pub expect: Expect,
    pub timeout: Duration,
}

impl Action {
    pub fn new(command: impl Into<String>, expect: Expect) -> Action {
        Action {
            command: command.into(),
            expect,
            timeout: ACTION_TIMEOUT,
        }
    }
}

/// A rule the game punishes breaking (docs/plan/01-goals.md "Rules"): the executor refuses
/// the action before it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// An attack command while auto-combat is engaged restarts the swing timer
    /// (`kb/combat.md`).
    AttackInCombat,
    /// The bot never trains stats; the player spends CP.
    TrainStats,
}

impl Guard {
    pub fn describe(self) -> &'static str {
        match self {
            Guard::AttackInCombat => "no attack command while combat is engaged",
            Guard::TrainStats => "the bot never trains stats",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmed {
    Room,
    DoorOpened,
    Bash { ok: bool },
    Prompt,
    Engaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Move(Refusal),
    /// The move ran into a shut door: `locked` when the game said `closed!` in lower case.
    DoorClosed {
        locked: bool,
    },
    /// `open` on a locked door.
    Locked,
    /// The game would not start the attack: the target is not here, the weapon does nothing
    /// to it, or another of the [`ATTACK_REFUSALS`].
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Confirmed(Confirmed),
    Refused(Refused),
    TimedOut,
}

/// The game's other answers to an attack that start no fight (`kb/screen-patterns.md`, the
/// registry's names): unmapped, the attack would only time out, and go again.
pub const ATTACK_REFUSALS: [&str; 8] = [
    "evil_warning",
    "wrong_weapon",
    "unbalanced",
    "need_rest_attack",
    "too_afraid",
    "lawful_block",
    "guilt",
    "ambiguous",
];

/// The game's command words for an attack (`kb/screen-parsing.md` "Command words": every
/// prefix from the shortest accepted form to the full word): (full word, shortest form).
const ATTACKS: [(&str, usize); 9] = [
    ("attack", 1),
    ("au", 2),
    ("aa", 2),
    ("allout", 2),
    ("force", 4),
    ("bash", 3),
    ("backstab", 5),
    ("bs", 2),
    ("punch", 2),
];

fn abbreviates(word: &str, full: &str, shortest: usize) -> bool {
    word.len() >= shortest && full.starts_with(word)
}

/// Whether `command` starts an attack. `bash <dir>` bashes a door, not a monster.
pub fn is_attack(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    let mut words = lower.split_whitespace();
    let Some(verb) = words.next() else {
        return false;
    };
    let attack = ATTACKS
        .iter()
        .any(|&(full, shortest)| abbreviates(verb, full, shortest))
        || abbreviates(verb, "purg", 3)
        || abbreviates(verb, "killlblo", 3);
    let at_door = abbreviates(verb, "bash", 3)
        && words
            .next()
            .is_some_and(|w| wayfarer_world::mm::direction(w).is_some());
    attack && !at_door
}

/// Whether `command` is `train stats` (or an abbreviation of it).
pub fn is_train_stats(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    let mut words = lower.split_whitespace();
    matches!(
        (words.next(), words.next()),
        (Some(t), Some(s)) if abbreviates(t, "train", 3) && abbreviates(s, "stats", 2)
    )
}

/// Refuse what a guard forbids. `in_combat` unknown counts as engaged: an attack goes only
/// when the character is known to be out of combat.
pub fn guard(command: &str, in_combat: Option<bool>) -> Result<(), Guard> {
    if is_train_stats(command) {
        return Err(Guard::TrainStats);
    }
    if in_combat != Some(false) && is_attack(command) {
        return Err(Guard::AttackInCombat);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum State {
    Idle,
    /// Waiting for a prompt boundary.
    Queued(Action),
    InFlight {
        action: Action,
        deadline: Instant,
        answer: Option<Outcome>,
    },
}

#[derive(Debug)]
pub struct Executor {
    state: State,
}

impl Default for Executor {
    fn default() -> Self {
        Executor { state: State::Idle }
    }
}

impl Executor {
    /// Queue an action to go at the next prompt boundary. A guard refuses it; so does a busy
    /// executor (one action at a time), which is a caller's bug.
    pub fn submit(&mut self, action: Action, in_combat: Option<bool>) -> Result<(), Guard> {
        guard(&action.command, in_combat)?;
        debug_assert_eq!(self.state, State::Idle, "one action at a time");
        self.state = State::Queued(action);
        Ok(())
    }

    /// An action is queued or in flight.
    pub fn busy(&self) -> bool {
        self.state != State::Idle
    }

    pub fn in_flight(&self) -> Option<&Action> {
        match &self.state {
            State::InFlight { action, .. } => Some(action),
            _ => None,
        }
    }

    /// The queued action's command, now in flight: the caller sends it. Only at a prompt
    /// boundary in the realm, which the caller checks.
    pub fn take_ready(&mut self, now: Instant) -> Option<String> {
        let State::Queued(action) = &self.state else {
            return None;
        };
        let action = action.clone();
        let command = action.command.clone();
        self.state = State::InFlight {
            deadline: now + action.timeout,
            action,
            answer: None,
        };
        Some(command)
    }

    /// An event from the game: the answer to the action in flight, if it is one. The first
    /// answer counts.
    pub fn event(&mut self, ev: &GameEvent) {
        let State::InFlight { action, answer, .. } = &mut self.state else {
            return;
        };
        if answer.is_some() {
            return;
        }
        *answer = match (action.expect, ev) {
            (Expect::RoomSeen, GameEvent::MoveRefused { reason }) => {
                Some(Outcome::Refused(Refused::Move(*reason)))
            }
            (Expect::RoomSeen, GameEvent::DoorClosed { locked }) => {
                Some(Outcome::Refused(Refused::DoorClosed { locked: *locked }))
            }
            (Expect::DoorOpened, GameEvent::DoorOpened) => {
                Some(Outcome::Confirmed(Confirmed::DoorOpened))
            }
            (Expect::DoorOpened, GameEvent::OpenRefused) => Some(Outcome::Refused(Refused::Locked)),
            (Expect::BashResult, GameEvent::BashResult { ok }) => {
                Some(Outcome::Confirmed(Confirmed::Bash { ok: *ok }))
            }
            (Expect::BashResult, GameEvent::DoorOpened) => {
                Some(Outcome::Confirmed(Confirmed::Bash { ok: true }))
            }
            (Expect::Prompt, GameEvent::Prompt { .. }) => {
                Some(Outcome::Confirmed(Confirmed::Prompt))
            }
            (Expect::Engaged, GameEvent::CombatEngaged) => {
                Some(Outcome::Confirmed(Confirmed::Engaged))
            }
            (Expect::Engaged, GameEvent::YouDoNotSee { .. } | GameEvent::NoEffect) => {
                Some(Outcome::Refused(Refused::Target))
            }
            (Expect::Engaged, GameEvent::Other(e))
                if ATTACK_REFUSALS.contains(&e.name.as_str()) =>
            {
                Some(Outcome::Refused(Refused::Target))
            }
            _ => None,
        };
    }

    /// A room was shown.
    pub fn room_seen(&mut self) {
        if let State::InFlight {
            action:
                Action {
                    expect: Expect::RoomSeen,
                    ..
                },
            answer: answer @ None,
            ..
        } = &mut self.state
        {
            *answer = Some(Outcome::Confirmed(Confirmed::Room));
        }
    }

    /// The action in flight is done: answered, and the prompt after the answer came; or its
    /// time ran out.
    pub fn finished(&mut self, at_prompt: bool, now: Instant) -> Option<(Action, Outcome)> {
        let State::InFlight {
            action,
            deadline,
            answer,
        } = &self.state
        else {
            return None;
        };
        let outcome = match answer {
            Some(a) if at_prompt => *a,
            _ if now >= *deadline => answer.unwrap_or(Outcome::TimedOut),
            _ => return None,
        };
        let action = action.clone();
        self.state = State::Idle;
        Some((action, outcome))
    }

    /// When the action in flight times out.
    pub fn deadline(&self) -> Option<Instant> {
        match self.state {
            State::InFlight { deadline, .. } => Some(deadline),
            _ => None,
        }
    }

    /// Forget the action (the session left the realm, the brain re-attached).
    pub fn cancel(&mut self) {
        self.state = State::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attacks_are_refused_while_in_combat() {
        for cmd in [
            "a rat", "att rat", "attack", "bash rat", "bas", "bs thug", "aa", "pu rat",
        ] {
            assert!(is_attack(cmd), "{cmd}");
            assert_eq!(guard(cmd, Some(true)), Err(Guard::AttackInCombat), "{cmd}");
            // Not known to be out of combat: engaged, as far as the guard goes.
            assert_eq!(guard(cmd, None), Err(Guard::AttackInCombat), "{cmd}");
            assert_eq!(guard(cmd, Some(false)), Ok(()), "{cmd}");
        }
        // A door, a move, a look: not attacks.
        for cmd in ["bash n", "bash north", "n", "look", "ba", "open n", "st"] {
            assert!(!is_attack(cmd), "{cmd}");
            assert_eq!(guard(cmd, Some(true)), Ok(()), "{cmd}");
            assert_eq!(guard(cmd, None), Ok(()), "{cmd}");
        }
        let mut ex = Executor::default();
        assert_eq!(
            ex.submit(Action::new("a rat", Expect::Engaged), Some(true)),
            Err(Guard::AttackInCombat)
        );
        assert_eq!(
            ex.submit(Action::new("a rat", Expect::Engaged), None),
            Err(Guard::AttackInCombat)
        );
        assert!(!ex.busy());
    }

    #[test]
    fn train_stats_is_refused_always() {
        for cmd in ["train stats", "TRAIN STATS", "tra st", "train  stat"] {
            assert_eq!(guard(cmd, Some(false)), Err(Guard::TrainStats), "{cmd}");
        }
        assert_eq!(guard("train", Some(false)), Ok(()));
        let mut ex = Executor::default();
        assert_eq!(
            ex.submit(Action::new("train stats", Expect::Prompt), Some(false)),
            Err(Guard::TrainStats)
        );
        assert!(!ex.busy());
    }

    #[tokio::test(start_paused = true)]
    async fn one_action_answered_then_the_prompt() {
        let mut ex = Executor::default();
        let now = Instant::now();
        ex.submit(Action::new("n", Expect::RoomSeen), Some(false))
            .unwrap();
        assert!(ex.busy() && ex.in_flight().is_none());
        assert_eq!(ex.take_ready(now).as_deref(), Some("n"));
        assert_eq!(ex.take_ready(now), None);
        assert_eq!(ex.finished(true, now), None, "a prompt before the answer");
        ex.room_seen();
        assert_eq!(ex.finished(false, now), None, "the answer, no prompt yet");
        let (action, outcome) = ex.finished(true, now).unwrap();
        assert_eq!(action.command, "n");
        assert_eq!(outcome, Outcome::Confirmed(Confirmed::Room));
        assert!(!ex.busy());

        ex.submit(Action::new("n", Expect::RoomSeen), Some(false))
            .unwrap();
        ex.take_ready(now);
        ex.event(&GameEvent::DoorClosed { locked: false });
        ex.room_seen();
        assert_eq!(
            ex.finished(true, now).unwrap().1,
            Outcome::Refused(Refused::DoorClosed { locked: false })
        );

        ex.submit(Action::new("bash n", Expect::BashResult), Some(false))
            .unwrap();
        ex.take_ready(now);
        assert_eq!(ex.deadline(), Some(now + ACTION_TIMEOUT));
        assert_eq!(
            ex.finished(true, now + ACTION_TIMEOUT).unwrap().1,
            Outcome::TimedOut
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_attack_is_engaged_or_refused() {
        let mut ex = Executor::default();
        let now = Instant::now();
        ex.submit(Action::new("a rat", Expect::Engaged), Some(false))
            .unwrap();
        ex.take_ready(now);
        ex.event(&GameEvent::CombatEngaged);
        assert_eq!(
            ex.finished(true, now).unwrap().1,
            Outcome::Confirmed(Confirmed::Engaged)
        );
        for refusal in [
            GameEvent::YouDoNotSee {
                target: "rat".into(),
            },
            GameEvent::NoEffect,
        ] {
            ex.submit(Action::new("a rat", Expect::Engaged), Some(false))
                .unwrap();
            ex.take_ready(now);
            ex.event(&refusal);
            assert_eq!(
                ex.finished(true, now).unwrap().1,
                Outcome::Refused(Refused::Target)
            );
        }
    }
}
