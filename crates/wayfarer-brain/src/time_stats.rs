//! Where a run's time goes (docs/brain.md "Where the time goes"): fighting, fleeing, resting,
//! walking, upkeep (looting, blessings, sneaking), waiting for something to do, paused, and
//! away from the realm. The session says what the character is doing after every message and
//! timer; the time since it last said is put to what it was doing then. Time the brain was not
//! running is not counted anywhere. The [`Ledger`](crate::stats::Ledger) keeps it with the run.

use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use wayfarer_proto::TimeSpent;

/// What the character is doing, for the time it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Activity {
    Fight,
    Flee,
    Rest,
    Walk,
    /// Looting, casting a blessing, sneaking.
    Upkeep,
    /// The bot running with nothing to do: waiting for a monster, for the party.
    Wait,
    /// The bot paused.
    Paused,
    /// Not in the realm, or the connection is down.
    Away,
}

/// Milliseconds spent at each, since the run started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Spent {
    pub fight: u64,
    pub flee: u64,
    pub rest: u64,
    pub walk: u64,
    pub upkeep: u64,
    pub wait: u64,
    pub paused: u64,
    pub away: u64,
}

impl Spent {
    fn add(&mut self, a: Activity, ms: u64) {
        let slot = match a {
            Activity::Fight => &mut self.fight,
            Activity::Flee => &mut self.flee,
            Activity::Rest => &mut self.rest,
            Activity::Walk => &mut self.walk,
            Activity::Upkeep => &mut self.upkeep,
            Activity::Wait => &mut self.wait,
            Activity::Paused => &mut self.paused,
            Activity::Away => &mut self.away,
        };
        *slot += ms;
    }
}

/// The run's time, and what the character has been doing since when; `dirty` until the store
/// has it.
#[derive(Debug, Default)]
pub(crate) struct TimeTally {
    spent: Spent,
    current: Option<(Activity, Instant)>,
    dirty: bool,
}

impl TimeTally {
    /// From the store's JSON; nothing spent when there is none or it does not read.
    pub fn load(json: Option<&str>) -> Self {
        Self {
            spent: json
                .and_then(|j| serde_json::from_str(j).ok())
                .unwrap_or_default(),
            ..Self::default()
        }
    }

    /// The character is doing `a` from `now`: the time since the last call goes to what it was
    /// doing then.
    pub fn observe(&mut self, a: Activity, now: Instant) {
        if let Some((was, since)) = self.current {
            let ms =
                u64::try_from(now.saturating_duration_since(since).as_millis()).unwrap_or(u64::MAX);
            if ms > 0 {
                self.spent.add(was, ms);
                self.dirty = true;
            }
        }
        self.current = Some((a, now));
    }

    /// Count up to `now`, going on with the same activity.
    pub fn flush(&mut self, now: Instant) {
        if let Some((a, _)) = self.current {
            self.observe(a, now);
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.spent).expect("time spent serializes")
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn saved(&mut self) {
        self.dirty = false;
    }

    /// The wire's `time`, in whole seconds.
    pub fn to_wire(&self) -> TimeSpent {
        let s = |ms: u64| ms / 1000;
        let t = &self.spent;
        TimeSpent {
            fight_secs: s(t.fight),
            flee_secs: s(t.flee),
            rest_secs: s(t.rest),
            walk_secs: s(t.walk),
            upkeep_secs: s(t.upkeep),
            wait_secs: s(t.wait),
            paused_secs: s(t.paused),
            away_secs: s(t.away),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn time_goes_to_what_the_character_was_doing() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut t = TimeTally::default();
        t.observe(Activity::Wait, at(0));
        t.observe(Activity::Fight, at(10));
        // Said again: still fighting.
        t.observe(Activity::Fight, at(25));
        t.observe(Activity::Rest, at(40));
        t.flush(at(100));
        let w = t.to_wire();
        assert_eq!((w.wait_secs, w.fight_secs, w.rest_secs), (10, 30, 60));
        assert!(t.dirty());
        // Through the store and back: the counts, not what it is doing.
        let mut back = TimeTally::load(Some(&t.to_json()));
        assert_eq!(back.to_wire(), w);
        back.flush(at(200));
        assert_eq!(back.to_wire(), w, "nothing counted before it is told");
        assert_eq!(
            TimeTally::load(Some("not json")).to_wire(),
            TimeSpent::default()
        );
    }
}
