//! The executor (docs/plan/03-engine.md section 3): an action is a command plus the events
//! that confirm it and a timeout. One action is in flight per session; it goes at a prompt
//! boundary in the realm (the session decides when that is), and it is done at the first
//! prompt after its answer, or when its time runs out. The one exception is a move along a
//! known route: a second move may go *ahead* of the move in flight (pipelining), and each is
//! answered by the rooms in the order they went; nothing but a move ever goes ahead of a move,
//! so two non-move actions never overlap. The hard guards live here, so no mode can get round
//! them.

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::{GameEvent, Refusal};
use wayfarer_world::Belief;

use crate::commands;

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
    /// `search <dir>`: an exit found there, or nothing.
    SearchResult,
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
    Search { found: bool },
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
    /// `search` in a room too dark to see: `The room is very dark - you can't see anything`.
    TooDark,
    /// The game would not start the attack: the target is not here, the weapon does nothing
    /// to it, or another of the [`ATTACK_REFUSALS`].
    Target,
    /// The attack spell did not go: not enough mana, or a spell already cast this round.
    Spell,
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

/// Whether `command` starts an attack: its first word is one of the game's attack commands
/// ([`commands::Kind::Attack`], every abbreviation the game accepts). `bash <dir>` bashes a
/// door, not a monster.
pub fn is_attack(command: &str) -> bool {
    commands::classify(command).kind == commands::Kind::Attack
}

fn abbreviates(word: &str, full: &str, shortest: usize) -> bool {
    word.len() >= shortest && full.starts_with(word)
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
    InFlight(Flight),
}

#[derive(Debug, Clone, PartialEq)]
struct Flight {
    action: Action,
    deadline: Instant,
    answer: Option<Outcome>,
    /// The position belief as the room that answered it left it: with a move ahead, the
    /// belief may have moved on with that move's room before this one is handed on.
    belief: Option<Belief>,
}

impl Flight {
    fn new(action: Action, now: Instant) -> Flight {
        Flight {
            deadline: now + action.timeout,
            action,
            answer: None,
            belief: None,
        }
    }
}

/// A move sent ahead of the move in flight.
#[derive(Debug, Clone, PartialEq)]
enum Ahead {
    /// To go as soon as the one before it has.
    Queued(Action),
    InFlight(Flight),
}

#[derive(Debug)]
pub struct Executor {
    state: State,
    ahead: Option<Ahead>,
    /// The belief the last finished action's room left ([`Executor::take_belief`]).
    finished_belief: Option<Belief>,
}

impl Default for Executor {
    fn default() -> Self {
        Executor {
            state: State::Idle,
            ahead: None,
            finished_belief: None,
        }
    }
}

/// The answer `ev` gives an action expecting `expect`, if it is one.
fn answer(expect: Expect, ev: &GameEvent) -> Option<Outcome> {
    match (expect, ev) {
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
        (Expect::SearchResult, GameEvent::SearchResult { found, .. }) => {
            Some(Outcome::Confirmed(Confirmed::Search { found: *found }))
        }
        (Expect::SearchResult, GameEvent::RoomDark) => Some(Outcome::Refused(Refused::TooDark)),
        (Expect::Prompt, GameEvent::Prompt { .. }) => Some(Outcome::Confirmed(Confirmed::Prompt)),
        (Expect::Engaged, GameEvent::CombatEngaged) => Some(Outcome::Confirmed(Confirmed::Engaged)),
        (Expect::Engaged, GameEvent::YouDoNotSee { .. } | GameEvent::NoEffect) => {
            Some(Outcome::Refused(Refused::Target))
        }
        (Expect::Engaged, GameEvent::NoMana | GameEvent::AlreadyCast) => {
            Some(Outcome::Refused(Refused::Spell))
        }
        (Expect::Engaged, GameEvent::Other(e)) if ATTACK_REFUSALS.contains(&e.name.as_str()) => {
            Some(Outcome::Refused(Refused::Target))
        }
        _ => None,
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

    /// Queue a move to go ahead of the move queued or in flight, without waiting for its
    /// answer or a prompt (pipelining). Only a move ([`Expect::RoomSeen`]) goes ahead, only of
    /// a move, and one at a time: whether it was taken.
    pub fn submit_ahead(&mut self, action: Action, in_combat: Option<bool>) -> bool {
        let first_moves = match &self.state {
            State::Queued(a) => a.expect == Expect::RoomSeen,
            State::InFlight(f) => f.action.expect == Expect::RoomSeen,
            State::Idle => false,
        };
        if !first_moves
            || self.ahead.is_some()
            || action.expect != Expect::RoomSeen
            || guard(&action.command, in_combat).is_err()
        {
            return false;
        }
        self.ahead = Some(Ahead::Queued(action));
        true
    }

    /// An action is queued or in flight.
    pub fn busy(&self) -> bool {
        self.state != State::Idle
    }

    pub fn in_flight(&self) -> Option<&Action> {
        match &self.state {
            State::InFlight(f) => Some(&f.action),
            _ => None,
        }
    }

    /// A move is queued or in flight ahead of the one in flight.
    pub fn ahead(&self) -> bool {
        self.ahead.is_some()
    }

    /// The queued action's command, now in flight: the caller sends it. Only at a prompt
    /// boundary in the realm, which the caller checks.
    pub fn take_ready(&mut self, now: Instant) -> Option<String> {
        let State::Queued(action) = &self.state else {
            return None;
        };
        let action = action.clone();
        let command = action.command.clone();
        self.state = State::InFlight(Flight::new(action, now));
        Some(command)
    }

    /// The move queued ahead, now in flight behind the one already in flight: the caller sends
    /// it at once, prompt or not.
    pub fn take_ahead(&mut self, now: Instant) -> Option<String> {
        if !matches!(self.state, State::InFlight(_)) {
            return None;
        }
        let Some(Ahead::Queued(action)) = &self.ahead else {
            return None;
        };
        let action = action.clone();
        let command = action.command.clone();
        self.ahead = Some(Ahead::InFlight(Flight::new(action, now)));
        Some(command)
    }

    /// The flight the game's next answer is for: the one in flight until it has its answer,
    /// then the move ahead of it.
    fn answering(&mut self) -> Option<&mut Flight> {
        match (&mut self.state, &mut self.ahead) {
            (State::InFlight(f), _) if f.answer.is_none() => Some(f),
            (State::InFlight(_), Some(Ahead::InFlight(f))) if f.answer.is_none() => Some(f),
            _ => None,
        }
    }

    /// An event from the game: the answer to the action in flight, if it is one. The first
    /// answer counts; once it has one, the next is the answer of the move ahead.
    pub fn event(&mut self, ev: &GameEvent) {
        if let Some(f) = self.answering() {
            f.answer = answer(f.action.expect, ev);
        }
    }

    /// A room was shown.
    pub fn room_seen(&mut self) {
        self.room_seen_in(None);
    }

    /// A room was shown, and `belief` is the position belief it left.
    pub fn room_seen_in(&mut self, belief: Option<Belief>) {
        if let Some(f) = self.answering()
            && f.action.expect == Expect::RoomSeen
        {
            f.answer = Some(Outcome::Confirmed(Confirmed::Room));
            f.belief = belief;
        }
    }

    /// The belief the room of the action [`Executor::finished`] last handed on left, if one
    /// was given ([`Executor::room_seen_in`]).
    pub fn take_belief(&mut self) -> Option<Belief> {
        self.finished_belief.take()
    }

    /// The action in flight is done: answered, and the prompt after the answer came; or its
    /// time ran out. The move ahead of it, if any, is the one in flight now.
    pub fn finished(&mut self, at_prompt: bool, now: Instant) -> Option<(Action, Outcome)> {
        let State::InFlight(f) = &self.state else {
            return None;
        };
        // With the move ahead answered too, the prompt after the first answer has gone by.
        let ahead_answered = matches!(&self.ahead, Some(Ahead::InFlight(a)) if a.answer.is_some());
        let outcome = match f.answer {
            Some(a) if at_prompt || ahead_answered => a,
            _ if now >= f.deadline => f.answer.unwrap_or(Outcome::TimedOut),
            _ => return None,
        };
        let action = f.action.clone();
        self.finished_belief = f.belief.clone();
        self.state = match self.ahead.take() {
            Some(Ahead::InFlight(f)) => State::InFlight(f),
            Some(Ahead::Queued(a)) => State::Queued(a),
            None => State::Idle,
        };
        Some((action, outcome))
    }

    /// When the action in flight times out.
    pub fn deadline(&self) -> Option<Instant> {
        match &self.state {
            State::InFlight(f) => Some(f.deadline),
            _ => None,
        }
    }

    /// Forget the action (the session left the realm, the brain re-attached), and the move
    /// ahead of it.
    pub fn cancel(&mut self) {
        self.state = State::Idle;
        self.ahead = None;
    }

    /// Drop the move queued ahead if it has not gone yet: whether one was dropped.
    pub fn drop_ahead(&mut self) -> bool {
        if matches!(self.ahead, Some(Ahead::Queued(_))) {
            self.ahead = None;
            return true;
        }
        false
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
