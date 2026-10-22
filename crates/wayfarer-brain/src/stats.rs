//! What the brain keeps in the store for a session (docs/brain.md "Statistics" and "Restarts"):
//! the bot's state, saved whenever it changes, and the run from Start to Stop (or a death) with
//! its kills, deaths, flees, rests and level-ups, and a sample of exp and HP every minute. The
//! numbers of the run in progress are also kept here, for the snapshot's `run`. Without a store
//! the run is still counted, in memory only.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::time::Instant;
use wayfarer_proto::{ExpSample, RunStats, RunSummary};
use wayfarer_store::{
    BotRecord, BotState, CustomLoop, Event, EventKind, Place, PlaceKind, Run, Sample, Store, Totals,
};
use wayfarer_world::Key;

use crate::clock::{Clock, rfc3339};

/// How often a run samples the character's exp and HP.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(60);
/// How many runs a `stats` message lists.
pub const STATS_RUNS: usize = 10;
/// How many places the snapshot lists.
pub const PLACES_SHOWN: usize = 10;
/// The shortest time a run's exp per hour is taken over, so the first kill of a run is not
/// thousands an hour.
const RATE_FLOOR: i64 = 60;

/// Unix seconds.
pub(crate) fn unix(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

pub(crate) fn rfc3339_at(secs: i64) -> String {
    rfc3339(UNIX_EPOCH + Duration::from_secs(u64::try_from(secs).unwrap_or(0)))
}

/// Exp gained over the run's wall time, [`RATE_FLOOR`] at least.
fn per_hour(exp: i64, secs: i64) -> u64 {
    u64::try_from(exp.max(0) * 3600 / secs.max(RATE_FLOOR)).unwrap_or(0)
}

/// The run in progress.
#[derive(Debug)]
struct OpenRun {
    /// The store's row (0 without a store).
    id: i64,
    started_at: i64,
    totals: Totals,
    /// The last kill's row, until exp is put on it; `Some(0)` without a store.
    kill: Option<i64>,
}

pub(crate) struct Ledger {
    store: Option<Arc<Mutex<Store>>>,
    clock: Arc<dyn Clock>,
    session: String,
    /// The bot as last saved, or as found in the store.
    saved: Option<BotRecord>,
    run: Option<OpenRun>,
    /// The run's numbers as last computed, for the snapshot.
    stats: Option<RunStats>,
    next_sample: Option<Instant>,
    /// Bosses killed since the brain attached, by lower-case name (the store has the rest).
    bosses: std::collections::BTreeMap<String, i64>,
    /// Custom loops made since the brain attached, by id (the store has the rest).
    custom: std::collections::BTreeMap<String, CustomLoop>,
    /// The last places of the character, newest first: the store's, and those since.
    places: Vec<Place>,
}

impl Ledger {
    /// The session's ledger, with what the store holds of its bot.
    pub fn new(store: Option<Arc<Mutex<Store>>>, clock: Arc<dyn Clock>, session: &str) -> Self {
        let mut ledger = Self {
            store,
            clock,
            session: session.to_owned(),
            saved: None,
            run: None,
            stats: None,
            next_sample: None,
            bosses: std::collections::BTreeMap::new(),
            custom: std::collections::BTreeMap::new(),
            places: Vec::new(),
        };
        ledger.saved = ledger.with(|s, id| s.bot(id)).flatten();
        ledger.places = ledger
            .with(|s, id| s.places(id, PLACES_SHOWN))
            .unwrap_or_default();
        ledger
    }

    /// The bot as the store had it when the session was first seen.
    pub fn stored(&self) -> Option<&BotRecord> {
        self.saved.as_ref()
    }

    fn lock(&self) -> Option<MutexGuard<'_, Store>> {
        self.store
            .as_ref()
            .map(|s| s.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    /// Run `f` on the store; `None` without one or when it fails (logged: the bot plays on).
    fn with<T>(&self, f: impl FnOnce(&Store, &str) -> wayfarer_store::Result<T>) -> Option<T> {
        let store = self.lock()?;
        match f(&store, &self.session) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::warn!(session = %self.session, "the store: {e}");
                None
            }
        }
    }

    fn now(&self) -> i64 {
        unix(self.clock.now())
    }

    /// The time now, unix seconds.
    pub fn unix_now(&self) -> i64 {
        self.now()
    }

    /// The character killed the boss `name` just now (a run or not): kept for "should be up".
    pub fn boss_killed(&mut self, name: &str) {
        let at = self.now();
        self.bosses.insert(name.to_lowercase(), at);
        self.with(|s, id| s.boss_killed(id, name, at));
    }

    /// Keep a custom loop, replacing the one with its id.
    pub fn save_custom_loop(&mut self, l: CustomLoop) {
        let at = self.now();
        self.with(|s, _| s.save_custom_loop(&l, at));
        self.custom.insert(l.id.clone(), l);
    }

    /// Forget a custom loop (by id, any case); whether there was one.
    pub fn delete_custom_loop(&mut self, id: &str) -> bool {
        let found = self
            .custom_loops()
            .into_iter()
            .find(|c| c.id.eq_ignore_ascii_case(id));
        let Some(c) = found else { return false };
        self.custom.remove(&c.id);
        self.with(|s, _| s.delete_custom_loop(&c.id));
        true
    }

    /// The custom loops, by name: the store's, and those made since the brain attached over
    /// them.
    pub fn custom_loops(&self) -> Vec<CustomLoop> {
        let mut loops: std::collections::BTreeMap<String, CustomLoop> = self
            .with(|s, _| s.custom_loops())
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.id.clone(), c))
            .collect();
        loops.extend(self.custom.clone());
        let mut loops: Vec<CustomLoop> = loops.into_values().collect();
        loops.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        loops
    }

    /// Remember where the character died, fled or was stopped, a run or not. The same thing
    /// in the same room as the last time is not listed twice.
    pub fn place(&mut self, kind: PlaceKind, detail: Option<&str>, room: Option<Key>) {
        let Some(room) = room else { return };
        if self
            .places
            .first()
            .is_some_and(|p| p.kind == kind && p.room == room)
        {
            return;
        }
        let place = Place {
            at: self.now(),
            kind,
            room,
            detail: detail.map(str::to_owned),
        };
        self.with(|s, id| s.add_place(id, &place));
        self.places.insert(0, place);
        self.places.truncate(PLACES_SHOWN);
    }

    /// The last places, newest first.
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    /// Whether saving `bot` is the bot stopping, from running or paused.
    pub fn stopping(&self, bot: &BotRecord) -> bool {
        bot.state == BotState::Stopped
            && self
                .saved
                .as_ref()
                .is_some_and(|b| b.state != BotState::Stopped)
    }

    /// The bosses the character killed, by lower-case name, with when (unix seconds): the
    /// store's, and those since the brain attached over them.
    pub fn boss_kills(&self) -> Vec<(String, i64)> {
        let mut kills: std::collections::BTreeMap<String, i64> = self
            .with(|s, id| s.boss_kills(id))
            .unwrap_or_default()
            .into_iter()
            .collect();
        kills.extend(self.bosses.clone());
        kills.into_iter().collect()
    }

    /// A restarted brain takes up the stored bot: its run goes on (a new one if the store has
    /// none open).
    pub fn adopt(&mut self, bot: BotRecord, now: Instant) {
        let at = self.now();
        let run = self.with(|s, id| s.open_run(id)).flatten();
        let run = match run {
            Some(run) => {
                let totals = self.with(|s, _| s.totals(run.id)).unwrap_or_default();
                Some((run, totals))
            }
            None => None,
        };
        self.run = Some(match run {
            Some((run, totals)) => OpenRun {
                id: run.id,
                started_at: run.started_at,
                totals,
                kill: None,
            },
            None => self.start_run(at, bot.loop_id.as_deref()),
        });
        let keep = self.run.as_ref().map(|r| r.id);
        self.with(|s, id| s.end_other_runs(id, keep, at));
        self.saved = Some(bot);
        self.next_sample = Some(now + SAMPLE_EVERY);
        self.refresh();
    }

    fn start_run(&self, at: i64, loop_id: Option<&str>) -> OpenRun {
        let id = self
            .with(|s, id| s.start_run(id, at, loop_id))
            .map_or(0, |r: Run| r.id);
        OpenRun {
            id,
            started_at: at,
            totals: Totals::default(),
            kill: None,
        }
    }

    /// Save the bot if it changed: a run starts when it leaves `stopped` and ends when it
    /// goes back. Whether the run's numbers changed.
    pub fn save(&mut self, bot: BotRecord, now: Instant) -> bool {
        if self.saved.as_ref() == Some(&bot) {
            return false;
        }
        let at = self.now();
        self.with(|s, id| s.save_bot(id, &bot, at));
        let mut changed = false;
        if bot.state == BotState::Stopped {
            if self.run.take().is_some() {
                changed = true;
            }
            // Also any run a brain before this one left open.
            self.with(|s, id| s.end_other_runs(id, None, at));
            self.next_sample = None;
            self.stats = None;
        } else if self.run.is_none() {
            self.run = Some(self.start_run(at, bot.loop_id.as_deref()));
            self.next_sample = Some(now + SAMPLE_EVERY);
            self.refresh();
            changed = true;
        }
        self.saved = Some(bot);
        changed
    }

    /// Something the run counts, if one is in progress.
    pub fn event(&mut self, kind: EventKind, value: i64, detail: Option<&str>, room: Option<Key>) {
        let at = self.now();
        let Some(run) = &self.run else { return };
        let run_id = run.id;
        let ev = Event {
            at,
            kind,
            value,
            detail: detail.map(str::to_owned),
            room,
        };
        let row = self.with(|s, _| s.record(run_id, &ev)).unwrap_or(0);
        let run = self.run.as_mut().expect("a run");
        run.totals.add(kind, value);
        if kind == EventKind::Kill {
            run.kill = Some(row);
        }
        self.refresh();
    }

    /// Exp gained: the last kill's, if it has none yet; otherwise exp on its own.
    pub fn exp(&mut self, amount: i64, room: Option<Key>) {
        let Some(run) = &mut self.run else { return };
        match run.kill.take() {
            Some(row) => {
                run.totals.exp_gained += amount;
                if row != 0 {
                    self.with(|s, _| s.add_value(row, amount));
                }
                self.refresh();
            }
            None => self.event(EventKind::Exp, amount, None, room),
        }
    }

    /// End the run in progress (it stays in the history) and start another now, its numbers at
    /// zero. `false` when there is no run to reset.
    pub fn reset(&mut self, now: Instant) -> bool {
        if self.run.take().is_none() {
            return false;
        }
        let at = self.now();
        let loop_id = self.saved.as_ref().and_then(|b| b.loop_id.clone());
        self.with(|s, id| s.end_other_runs(id, None, at));
        self.run = Some(self.start_run(at, loop_id.as_deref()));
        self.next_sample = Some(now + SAMPLE_EVERY);
        self.refresh();
        true
    }

    /// When the next sample is due.
    pub fn deadline(&self) -> Option<Instant> {
        self.next_sample
    }

    /// Take a sample if one is due.
    pub fn sample(&mut self, exp: Option<i64>, hp: Option<i64>, now: Instant) -> bool {
        let (Some(run), Some(due)) = (&self.run, self.next_sample) else {
            return false;
        };
        if now < due {
            return false;
        }
        let run_id = run.id;
        let s = Sample {
            at: self.now(),
            exp,
            hp,
        };
        self.with(|st, _| st.sample(run_id, &s));
        self.next_sample = Some(now + SAMPLE_EVERY);
        self.refresh();
        true
    }

    fn refresh(&mut self) {
        let now = self.now();
        self.stats = self.run.as_ref().map(|r| RunStats {
            run_started_at: rfc3339_at(r.started_at),
            exp_per_hour: per_hour(r.totals.exp_gained, now - r.started_at),
            kills: r.totals.kills,
            deaths: r.totals.deaths,
            flees: r.totals.flees,
            rests: r.totals.rests,
            exp_gained: r.totals.exp_gained,
        });
    }

    /// The run's numbers, for the snapshot.
    pub fn snapshot(&self) -> Option<RunStats> {
        self.stats.clone()
    }

    /// The last runs, newest first, and the newest one's samples (the `stats` message). Without
    /// a store, the run in progress only.
    pub fn summaries(&self) -> (Vec<RunSummary>, Vec<ExpSample>) {
        let now = self.now();
        let summary = |started_at: i64, ended_at: Option<i64>, loop_id, t: &Totals| {
            let secs = ended_at.unwrap_or(now) - started_at;
            RunSummary {
                started_at: rfc3339_at(started_at),
                ended_at: ended_at.map(rfc3339_at),
                loop_id,
                minutes: u64::try_from(secs / 60).unwrap_or(0),
                exp_per_hour: per_hour(t.exp_gained, secs),
                kills: t.kills,
                deaths: t.deaths,
                flees: t.flees,
                rests: t.rests,
                exp_gained: t.exp_gained,
            }
        };
        if self.store.is_none() {
            let runs = self.run.iter().map(|r| {
                let loop_id = self.saved.as_ref().and_then(|b| b.loop_id.clone());
                summary(r.started_at, None, loop_id, &r.totals)
            });
            return (runs.collect(), Vec::new());
        }
        let runs = self
            .with(|s, id| s.runs(id, STATS_RUNS))
            .unwrap_or_default();
        let samples = runs
            .first()
            .and_then(|r| self.with(|s, _| s.samples(r.run.id)))
            .unwrap_or_default()
            .into_iter()
            .map(|s| ExpSample {
                at: rfc3339_at(s.at),
                exp: s.exp,
                hp: s.hp.and_then(|hp| i32::try_from(hp).ok()),
            })
            .collect();
        let runs = runs
            .into_iter()
            .map(|r| summary(r.run.started_at, r.run.ended_at, r.run.loop_id, &r.totals))
            .collect();
        (runs, samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_per_hour_is_over_a_minute_at_least() {
        assert_eq!(per_hour(9, 10), 540);
        assert_eq!(per_hour(9, 60), 540);
        assert_eq!(per_hour(90, 1800), 180);
        assert_eq!(per_hour(-5, 600), 0);
    }
}
