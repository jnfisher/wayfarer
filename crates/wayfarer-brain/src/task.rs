//! Task (docs/brain.md "Running a loop", docs/plan/03-engine.md section 4): the bot runs a loop
//! the player chose. It walks to the loop's stop nearest the character, then on to each next
//! stop in loop order, passing by the stops the player unticked and those whose respawn delay
//! has not passed since it last cleared them. At a stop the stop is home, so Fight and Recover
//! work there as in "fight here"; once nothing Fight would attack is left (or what is left has
//! been quiet for [`STOP_QUIET`]) the stop is cleared and Task moves on. With every stop
//! waiting on its delay it rests if it must, else waits where it is.
//!
//! Task is below Flee, Recover and Fight: it has control only when none of them wants it. It
//! asks the walker for each leg ([`Step::walk`]); a fight on the way stops the leg (the
//! session sees to that), and Task walks on from wherever it is once the fight is over.
//!
//! A stop no route reaches from where the character is (behind a door it cannot open, a
//! hidden exit it does not know) is passed by for the round without a leg, and tried again
//! the next round; it does not count towards [`GIVE_UPS`]. With no ticked stop reachable at
//! all, the bot stops itself.

use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_world::{Category, Key, Traveller, distances, route};

use crate::brain::Atlas;
use crate::executor::{Action, Outcome};
use crate::flee::dir_name;
use crate::modes::{Mode, Step, View};
use crate::policy::{REST_UNTIL, hostile};
use crate::walker::describe;

/// The stock respawn delay, minutes: a room whose `regendelay` is 0 waits this long
/// (`regenmins`, `kb/progression.md`).
pub const STOCK_REGEN: i64 = 5;
/// A stop where hostile monsters Fight will not attack are left (refused targets) is cleared
/// once the character's own combat has been quiet this long there.
pub const STOP_QUIET: Duration = Duration::from_secs(20);
/// Legs to one stop that end elsewhere, with no fight on the way, before the stop is passed by
/// this time round (its delay then runs as if it were cleared, an arena's too).
pub const LEG_TRIES: u32 = 3;
/// Stops passed by in a row, with no stop reached and nothing killed between, before the bot
/// stops itself: the loop cannot go round.
pub const GIVE_UPS: u32 = 3;

/// Whether a stop has respawned: an arena always has (no delay after a kill); any other once
/// `regen` minutes (0: [`STOCK_REGEN`]) have passed since the bot cleared it, or if it never
/// has.
pub fn stop_ready(now: Instant, cleared_at: Option<Instant>, regen: i64, is_arena: bool) -> bool {
    ready_in(now, cleared_at, regen, is_arena).is_zero()
}

/// How long until a stop has respawned ([`stop_ready`]); zero when it has.
pub fn ready_in(now: Instant, cleared_at: Option<Instant>, regen: i64, is_arena: bool) -> Duration {
    let Some(cleared) = cleared_at.filter(|_| !is_arena) else {
        return Duration::ZERO;
    };
    let minutes = if regen > 0 { regen } else { STOCK_REGEN };
    let delay = Duration::from_secs(u64::try_from(minutes).unwrap_or(0) * 60);
    (cleared + delay).saturating_duration_since(now)
}

/// Why no route reaches `to` from `from`: the first exit the traveller may not take on the way
/// a traveller with no strength, no refusals and every hidden exit would go.
fn why_unreachable(v: &View, from: Key, to: Key) -> String {
    let g = &v.atlas.graph;
    let t = v.traveller;
    let mut open = Traveller {
        strength: None,
        picklocks: None,
        blocked: BTreeSet::new(),
        ..t.clone()
    };
    open.allow.insert(Category::Hidden);
    let room = |k: Key| {
        g.room(k)
            .map_or_else(|| format!("{}/{}", k.0, k.1), |r| r.name.clone())
    };
    let stuck = route(g, from, to, &open).and_then(|r| {
        r.into_iter().find_map(|s| {
            let e = g
                .edges(s.from)
                .iter()
                .find(|e| e.dir == s.dir && e.to == s.to)?;
            (!t.can_take(e)).then(|| (e.clone(), room(s.from)))
        })
    });
    let Some((e, at)) = stuck else {
        return "no route known".to_owned();
    };
    let dir = dir_name(e.dir);
    let door = if e.kind == wayfarer_world::ExitType::Gate {
        "gate"
    } else {
        "door"
    };
    if t.door_odds(&e) == Some(0) {
        format!("the {door} {dir} of {at} cannot be opened")
    } else if e.door.is_some() && t.blocked.contains(&(e.from, e.dir)) {
        format!("the {door} {dir} of {at} would not open")
    } else if t.blocked.contains(&(e.from, e.dir)) {
        format!("the exit {dir} of {at} was refused")
    } else if e.hidden() {
        format!("the hidden exit {dir} of {at} is not known")
    } else {
        "no route known".to_owned()
    }
}

/// Whole minutes, rounded up: "3 m".
fn minutes(d: Duration) -> String {
    format!("{} m", d.as_secs().div_ceil(60).max(1))
}

/// The loop the bot runs, as the store keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TaskLoop {
    pub id: String,
    /// The stops when the loop was started; they win over what `id` resolves to now.
    pub stops: Vec<Key>,
    /// The stop it is at or walking to.
    pub index: usize,
    /// Stops to pass by this time round.
    pub skip: Vec<usize>,
}

/// One stop of the loop, with what its respawn needs.
#[derive(Debug, Clone)]
struct Stop {
    key: Key,
    name: String,
    /// `regendelay`, minutes (0: the stock delay).
    regen: i64,
    arena: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase {
    /// On the way to the stop at `index` (or about to be).
    Going,
    /// At the stop: hunting it. `quiet_from`: when the character last fought there.
    At {
        kills: Vec<String>,
        quiet_from: Instant,
    },
    /// At the stop, cleared, with no other stop ready: waiting for respawns.
    Waiting,
}

#[derive(Debug)]
pub(crate) struct Task {
    id: String,
    name: String,
    stops: Vec<Stop>,
    /// The stop it is at or walking to.
    index: usize,
    skip: BTreeSet<usize>,
    /// When the bot last cleared each stop.
    cleared: Vec<Option<Instant>>,
    /// When the bot last passed each stop by, not getting there: its delay runs from then
    /// whether or not it is an arena, so the bot does not try again at once.
    passed: Vec<Option<Instant>>,
    /// Stops passed by in a row since it last reached a stop or killed something.
    give_ups: u32,
    /// Why the bot should stop itself, once [`GIVE_UPS`] stops are passed by in a row.
    gave_up: Option<String>,
    phase: Phase,
    /// Legs to the stop at `index` begun since it last arrived or fought.
    tries: u32,
    /// Lines to narrate before the step's own: the stops passed by.
    notes: Vec<String>,
    /// Waiting for respawns with HP to top up: Recover rests first.
    rest: bool,
    /// The last "skipping unreachable stops" line said, so it is said once until it changes.
    unreachable_said: Option<String>,
}

impl Task {
    /// The loop `id` named `name` with these stops, going first to stop `first`, passing by
    /// the stops `skip`. The stops' respawn delays and arenas come from the world pack.
    pub fn new(
        atlas: &Atlas,
        id: String,
        name: String,
        stops: &[Key],
        first: usize,
        skip: BTreeSet<usize>,
    ) -> Task {
        let stops: Vec<Stop> = stops
            .iter()
            .map(|&key| {
                let room = atlas.graph.room(key);
                Stop {
                    key,
                    name: room.map_or_else(|| format!("{}/{}", key.0, key.1), |r| r.name.clone()),
                    regen: room.map_or(0, |r| r.regen_delay),
                    arena: room.is_some_and(|r| r.room_type == 2),
                }
            })
            .collect();
        let n = stops.len();
        Task {
            id,
            name,
            cleared: vec![None; n],
            passed: vec![None; n],
            give_ups: 0,
            gave_up: None,
            index: first.min(n.saturating_sub(1)),
            skip: skip.into_iter().filter(|&i| i < n).collect(),
            stops,
            phase: Phase::Going,
            tries: 0,
            notes: Vec::new(),
            rest: false,
            unreachable_said: None,
        }
    }

    /// A stored loop, put back after a restart: on the way to its stop again, nothing cleared.
    pub fn restore(atlas: &Atlas, name: String, t: &TaskLoop) -> Task {
        Task::new(
            atlas,
            t.id.clone(),
            name,
            &t.stops,
            t.index,
            t.skip.iter().copied().collect(),
        )
    }

    /// What the store keeps.
    pub fn stored(&self) -> TaskLoop {
        TaskLoop {
            id: self.id.clone(),
            stops: self.stops.iter().map(|s| s.key).collect(),
            index: self.index,
            skip: self.skip.iter().copied().collect(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The wire's `task`.
    pub fn snapshot(&self) -> wayfarer_proto::Task {
        let key = |k: Key| wayfarer_proto::RoomKey {
            map: k.0,
            room: k.1,
        };
        wayfarer_proto::Task {
            loop_id: self.id.clone(),
            loop_name: self.name.clone(),
            stop: u32::try_from(self.index).unwrap_or(u32::MAX),
            stops: self.stops.iter().map(|s| key(s.key)).collect(),
            skip: self.skip.iter().map(|&i| key(self.stops[i].key)).collect(),
        }
    }

    /// The stops to pass by from now on (a `run_loop` for this loop again).
    pub fn set_skip(&mut self, skip: BTreeSet<usize>) {
        self.skip = skip.into_iter().filter(|&i| i < self.stops.len()).collect();
    }

    /// The stop it is walking to is one the player unticked.
    pub fn heading_to_skipped(&self) -> bool {
        self.phase == Phase::Going && self.skip.contains(&self.index)
    }

    /// Where Recover rests from and walks back to: the stop it is at or walking to.
    pub fn home(&self) -> Key {
        self.stops[self.index].key
    }

    /// The bot's tick, whichever mode has control: arriving at the stop it walks to starts the
    /// hunt there, so what Fight kills on arrival counts for the stop.
    pub fn observe(&mut self, v: &View) {
        if self.phase == Phase::Going && !self.stops.is_empty() && v.room == Some(self.home()) {
            self.tries = 0;
            self.give_ups = 0;
            self.passed[self.index] = None;
            self.phase = Phase::At {
                kills: Vec::new(),
                quiet_from: v.now,
            };
        }
    }

    /// A monster died: one more for the stop, if it is at one.
    pub fn killed(&mut self, name: &str, now: Instant) {
        self.give_ups = 0;
        match &mut self.phase {
            Phase::At { kills, quiet_from } => {
                kills.push(name.to_owned());
                *quiet_from = now;
            }
            Phase::Waiting => {
                self.phase = Phase::At {
                    kills: vec![name.to_owned()],
                    quiet_from: now,
                }
            }
            Phase::Going => {}
        }
    }

    /// The bot left the stop it was at or heading to for a monster it does not fight (a flee
    /// from, a stalemate): that stop is passed by this time round, not cleared, and the loop
    /// goes on to the next stop that is ticked and ready (or waits for one).
    pub fn move_on(&mut self, v: &View) {
        let n = self.stops.len();
        if n == 0 {
            return;
        }
        let left = self.index;
        self.passed[left] = Some(v.now);
        self.tries = 0;
        self.notes.push(format!(
            "Stop {} {}: leaving it for now",
            self.nth(left),
            self.stops[left].name
        ));
        let next = (1..n)
            .map(|k| (left + k) % n)
            .find(|&j| !self.skip.contains(&j) && self.ready_in(v.now, j).is_zero());
        match next {
            Some(j) => {
                self.index = j;
                self.phase = Phase::Going;
            }
            None => self.phase = Phase::Waiting,
        }
    }

    /// The lines to narrate before the step's own.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// Why the bot should stop itself, if the loop cannot go round.
    pub fn take_give_up(&mut self) -> Option<String> {
        self.gave_up.take()
    }

    /// Whether it wants Recover to rest before the wait for respawns.
    pub fn take_rest(&mut self) -> bool {
        std::mem::take(&mut self.rest)
    }

    /// When a wait ends, for the clock to wake the bot: the next stop to respawn, or a stop's
    /// quiet time running out.
    pub fn wake(&self, now: Instant) -> Option<Instant> {
        let quiet = match &self.phase {
            Phase::At { quiet_from, .. } => Some(*quiet_from + STOP_QUIET),
            _ => None,
        };
        let respawn = (0..self.stops.len())
            .filter(|i| !self.skip.contains(i))
            .map(|i| self.ready_in(now, i))
            .filter(|d| !d.is_zero())
            .min()
            .map(|d| now + d);
        quiet.into_iter().chain(respawn).min()
    }

    fn ready_in(&self, now: Instant, i: usize) -> Duration {
        let s = &self.stops[i];
        // Passed by: the delay runs from then, an arena's too.
        ready_in(now, self.cleared[i], s.regen, s.arena).max(ready_in(
            now,
            self.passed[i],
            s.regen,
            false,
        ))
    }

    /// "stop 3/9".
    fn nth(&self, i: usize) -> String {
        format!("{}/{}", i + 1, self.stops.len())
    }

    /// Head for the stop at `i`: a leg through the walker, or nothing to do when already there.
    fn leg(&mut self, v: &View, i: usize) -> Step {
        self.index = i;
        self.phase = Phase::Going;
        self.tries += 1;
        let s = &self.stops[i];
        let steps = v
            .room
            .and_then(|from| route(&v.atlas.graph, from, s.key, v.traveller))
            .map_or_else(|| "no route known".to_owned(), |r| describe(&r));
        Step::walk(
            s.key,
            format!(
                "Loop {}: to stop {} {} ({steps})",
                self.name,
                self.nth(i),
                s.name
            ),
        )
    }

    /// The ticked stops no route reaches from the character's room (none when the room is
    /// unknown), said once while they and the reason stay the same: "Skipping 31 stops it
    /// cannot reach this round: the door east of Darkwood Forest, Hidden Keep cannot be
    /// opened".
    fn unreachable(&mut self, v: &View) -> BTreeSet<usize> {
        let Some(from) = v.room else {
            return BTreeSet::new();
        };
        let reach: HashMap<Key, u32> = distances(&v.atlas.graph, from, u32::MAX, v.traveller);
        let out: BTreeSet<usize> = (0..self.stops.len())
            .filter(|i| !self.skip.contains(i) && !reach.contains_key(&self.stops[*i].key))
            .collect();
        let Some(&first) = out.first() else {
            self.unreachable_said = None;
            return out;
        };
        let stops = match out.len() {
            1 => "one stop".to_owned(),
            n => format!("{n} stops"),
        };
        let why = why_unreachable(v, from, self.stops[first].key);
        let text = format!("Skipping {stops} it cannot reach this round: {why}");
        if self.unreachable_said.as_ref() != Some(&text) {
            self.notes.push(text.clone());
            self.unreachable_said = Some(text);
        }
        out
    }

    /// The stop at `index` is cleared (or was waited at): on to the next stop in loop order that
    /// is ticked and ready; with none, rest if HP is short, else wait here.
    fn next_leg(&mut self, v: &View) -> Step {
        let n = self.stops.len();
        let unreachable = self.unreachable(v);
        if !unreachable.is_empty() && unreachable.len() + self.skip.len() == n {
            self.gave_up = Some(format!(
                "loop {}: none of its stops can be reached from here",
                self.name
            ));
            return Step::wait(format!("Loop {}: no stop can be reached", self.name));
        }
        let mut notes = Vec::new();
        for k in 1..=n {
            let j = (self.index + k) % n;
            let s = &self.stops[j];
            if self.skip.contains(&j) {
                notes.push(format!("Skipping {} (unticked)", s.name));
                continue;
            }
            if unreachable.contains(&j) {
                continue;
            }
            let wait = self.ready_in(v.now, j);
            if !wait.is_zero() {
                notes.push(format!(
                    "Stop {} not ready ({})",
                    self.nth(j),
                    minutes(wait)
                ));
                continue;
            }
            if let Some(name) = v.shunned(s.key).filter(|_| Some(s.key) != v.room) {
                // A monster to keep away from was last seen there: passed by this time round.
                notes.push(format!(
                    "Skipping stop {} {}: {name} was last seen there",
                    self.nth(j),
                    s.name
                ));
                self.passed[j] = Some(v.now);
                continue;
            }
            if Some(s.key) == v.room {
                // Ready, and here (an arena, or the loop's only stop ready): wait here.
                self.index = j;
                break;
            }
            self.notes.extend(notes);
            return self.leg(v, j);
        }
        let entering = self.phase != Phase::Waiting;
        self.phase = Phase::Waiting;
        if entering {
            // Why it waits, said once.
            self.notes.extend(notes);
        }
        if entering && v.hp.is_some_and(|hp| hp < REST_UNTIL) {
            self.rest = true;
            return Step::wait("Waiting for respawns: resting first");
        }
        // Away from the stop it was heading to (passed by): its own wait counts too.
        let here = Some(self.stops[self.index].key) == v.room;
        let next = (0..n)
            .filter(|&j| !(here && j == self.index) && !self.skip.contains(&j))
            .map(|j| self.ready_in(v.now, j))
            .filter(|d| !d.is_zero())
            .min();
        match next {
            Some(d) => Step::wait(format!("Waiting for respawns: next in {}", minutes(d))),
            None if self.skip.len() == n => Step::wait("Waiting: every stop is unticked"),
            None => Step::wait(format!(
                "Waiting for respawns in {}",
                self.stops[self.index].name
            )),
        }
    }

    /// "Stop 3/9 cleared: 2 minotaurs".
    fn cleared_text(&self, kills: &[String]) -> String {
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for k in kills {
            match counts.iter_mut().find(|(n, _)| n == k) {
                Some((_, c)) => *c += 1,
                None => counts.push((k, 1)),
            }
        }
        let what = if counts.is_empty() {
            "nothing there".to_owned()
        } else {
            counts
                .iter()
                .map(|(n, c)| match c {
                    1 => format!("1 {n}"),
                    c => format!("{c} {n}s"),
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!("Stop {} cleared: {what}", self.nth(self.index))
    }
}

impl Mode for Task {
    fn wants(&self, _v: &View) -> Option<String> {
        Some(format!("running loop {}", self.name))
    }

    fn step(&mut self, v: &View) -> Step {
        if self.stops.is_empty() {
            return Step::wait(format!("Loop {}: no stops", self.name));
        }
        let here = Some(self.home()) == v.room;
        if self.heading_to_skipped() && !here {
            // Unticked on the way: on to the next one.
            return self.next_leg(v);
        }
        if !here {
            match self.phase {
                // Waiting away from the loop's stops: for one to be ready.
                Phase::Waiting => return self.next_leg(v),
                // Away from the stop it was at (a rest, a flee): back to it.
                Phase::At { .. } => {
                    self.phase = Phase::Going;
                    self.tries = 0;
                }
                Phase::Going => {}
            }
            if self.tries >= LEG_TRIES {
                let s = &self.stops[self.index];
                let text = format!(
                    "Stop {} {}: could not get there; passing it by this time round",
                    self.nth(self.index),
                    s.name
                );
                self.passed[self.index] = Some(v.now);
                self.tries = 0;
                self.give_ups += 1;
                if self.give_ups >= GIVE_UPS {
                    self.gave_up = Some(format!(
                        "loop {}: could not get to a stop {GIVE_UPS} times in a row",
                        self.name
                    ));
                    return Step::wait(text);
                }
                let step = self.next_leg(v);
                self.notes.insert(0, text);
                return step;
            }
            if self.unreachable(v).contains(&self.index) {
                // No route there now: passed by this round, not a failed leg.
                return self.next_leg(v);
            }
            return self.leg(v, self.index);
        }
        self.observe(v);
        if let Phase::At { kills, quiet_from } = &self.phase {
            // Fight wants nothing here; what hostile monsters remain, it will not attack.
            let lingering = v
                .presence
                .here
                .iter()
                .any(|n| hostile(&v.atlas.monsters, n));
            if lingering && v.now < *quiet_from + STOP_QUIET {
                return Step::wait(format!(
                    "Stop {}: waiting for it to go quiet",
                    self.nth(self.index)
                ));
            }
            let text = self.cleared_text(kills);
            self.cleared[self.index] = Some(v.now);
            self.notes.push(text);
        }
        self.next_leg(v)
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        let at_stop = matches!(self.phase, Phase::At { .. } | Phase::Waiting);
        match ev {
            GameEvent::CombatEngaged | GameEvent::AttackedBy { .. } if at_stop => {
                if let Phase::At { quiet_from, .. } = &mut self.phase {
                    *quiet_from = now;
                } else {
                    self.phase = Phase::At {
                        kills: Vec::new(),
                        quiet_from: now,
                    };
                }
            }
            // A fight on the way: the leg was not a failed one.
            GameEvent::CombatEngaged => self.tries = 0,
            _ => {}
        }
    }

    fn outcome(&mut self, _action: &Action, _outcome: Outcome) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_stop_is_ready_once_its_delay_has_passed() {
        let t0 = Instant::now();
        let min = Duration::from_secs(60);
        // Never cleared: ready.
        assert!(stop_ready(t0, None, 3, false));
        // Cleared now: three minutes to wait.
        assert!(!stop_ready(t0, Some(t0), 3, false));
        assert_eq!(ready_in(t0, Some(t0), 3, false), 3 * min);
        assert!(!stop_ready(
            t0 + 3 * min - Duration::from_secs(1),
            Some(t0),
            3,
            false
        ));
        assert!(stop_ready(t0 + 3 * min, Some(t0), 3, false));
        // 0 is the stock five minutes.
        assert_eq!(ready_in(t0, Some(t0), 0, false), 5 * min);
        assert!(stop_ready(t0 + 5 * min, Some(t0), 0, false));
        // An arena: always.
        assert!(stop_ready(t0, Some(t0), 3, true));
        assert_eq!(ready_in(t0, Some(t0), 0, true), Duration::ZERO);
    }

    #[test]
    fn waits_are_whole_minutes_rounded_up() {
        assert_eq!(minutes(Duration::from_secs(1)), "1 m");
        assert_eq!(minutes(Duration::from_secs(60)), "1 m");
        assert_eq!(minutes(Duration::from_secs(61)), "2 m");
        assert_eq!(minutes(Duration::ZERO), "1 m");
    }
}
