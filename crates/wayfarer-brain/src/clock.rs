//! The brain's time, injected (`docs/plan/03-engine.md`: the core is deterministic): wall
//! time for the wire's timestamps (`started_at`, `synced_at`) and the store's, and the
//! monotonic moment every timer counts from (`IDLE`, `ANSWER_TIMEOUT`, `ACTION_TIMEOUT`,
//! `COMBAT_QUIET`, Recover's and Task mode's waits), with the wake-ups. Nothing in the brain
//! reads `tokio::time` for itself, so [`ReplayClock`] plays a recording back on its own time.

use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;
use tokio::time::Instant;

/// A wake-up: [`Clock::sleep_until`]'s future.
pub type Sleep<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait Clock: Send + Sync {
    /// Wall time: what the timestamps say.
    fn now(&self) -> SystemTime;

    /// The moment the brain's timers count from: tokio's clock unless the clock keeps its
    /// own, so a test's paused clock pauses them.
    fn instant(&self) -> Instant {
        Instant::now()
    }

    /// Wake once [`Clock::instant`] has reached `at`.
    fn sleep_until(&self, at: Instant) -> Sleep<'_> {
        Box::pin(tokio::time::sleep_until(at))
    }
}

/// The system's clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// A clock stopped at one moment (tests).
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub SystemTime);

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

/// Wall time that moves with tokio's clock from the moment it is made: on a paused test clock
/// the minutes a test lets pass pass here too (tests).
#[derive(Debug, Clone, Copy)]
pub struct InstantClock {
    start: SystemTime,
    at: Instant,
}

impl InstantClock {
    pub fn new(start: SystemTime) -> Self {
        Self {
            start,
            at: Instant::now(),
        }
    }
}

impl Clock for InstantClock {
    fn now(&self) -> SystemTime {
        self.start + self.at.elapsed()
    }
}

/// A recording's time (`wayfarer replay`, the CI replay layer, `docs/replay.md`): wall time
/// starts at the recording's first `ts`, and the timers run at the recording's pace, so the
/// re-sync's idle wait and the quiet rule fall where they fell live.
///
/// - [`ReplayClock::paced`]: `speed` recorded seconds pass per second of tokio's clock (on a
///   paused test clock, exactly the recorded gaps). The replay's feeder sleeps on the same
///   scale, so output arrives when it arrived, and a timer between two chunks fires between
///   them.
/// - [`ReplayClock::following`]: as fast as the output goes (`--instant`). The time is the
///   `ts` of the chunk last fed, which the feeder publishes; timers fire as it passes them.
#[derive(Debug, Clone)]
pub struct ReplayClock {
    /// The recording's first `ts`.
    origin: SystemTime,
    /// The moment `origin` maps to.
    start: Instant,
    pace: Pace,
}

#[derive(Debug, Clone)]
enum Pace {
    Speed(f64),
    /// Milliseconds since `origin`.
    Follow(watch::Receiver<u64>),
}

impl ReplayClock {
    /// `origin` at `start` on tokio's clock, then `speed` (above 0) times as fast.
    pub fn paced(origin: SystemTime, start: Instant, speed: f64) -> Self {
        assert!(speed > 0.0, "a replay's speed is above 0");
        Self {
            origin,
            start,
            pace: Pace::Speed(speed),
        }
    }

    /// `origin` plus the milliseconds `elapsed` says, as the feeder moves it on.
    pub fn following(origin: SystemTime, elapsed: watch::Receiver<u64>) -> Self {
        Self {
            origin,
            start: Instant::now(),
            pace: Pace::Follow(elapsed),
        }
    }

    fn elapsed(&self) -> Duration {
        match &self.pace {
            Pace::Speed(speed) => self.start.elapsed().mul_f64(*speed),
            Pace::Follow(rx) => Duration::from_millis(*rx.borrow()),
        }
    }
}

impl Clock for ReplayClock {
    fn now(&self) -> SystemTime {
        self.origin + self.elapsed()
    }

    fn instant(&self) -> Instant {
        self.start + self.elapsed()
    }

    fn sleep_until(&self, at: Instant) -> Sleep<'_> {
        Box::pin(async move {
            match &self.pace {
                Pace::Speed(speed) => loop {
                    let now = self.instant();
                    if now >= at {
                        return;
                    }
                    // Rounding may wake it a moment early: then it sleeps the rest.
                    let wait = (at - now).div_f64(*speed).max(Duration::from_micros(1));
                    tokio::time::sleep(wait).await;
                },
                Pace::Follow(rx) => {
                    let ms = at.saturating_duration_since(self.start).as_millis();
                    let mut rx = rx.clone();
                    // The feeder gone, the time stops, and so does the wait.
                    if rx.wait_for(|&v| u128::from(v) >= ms).await.is_err() {
                        std::future::pending::<()>().await;
                    }
                }
            }
        })
    }
}

/// `2026-10-19T12:00:05Z`: RFC 3339 in UTC, to the second.
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Howard Hinnant's civil_from_days.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_seconds() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let t = UNIX_EPOCH + Duration::from_secs(1_792_411_205);
        assert_eq!(rfc3339(t), "2026-10-19T12:00:05Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(rfc3339(leap), "2000-02-29T00:00:00Z");
    }

    #[tokio::test(start_paused = true)]
    async fn a_paced_replay_clock_runs_at_its_speed() {
        let origin = UNIX_EPOCH + Duration::from_secs(1_792_411_200);
        let start = Instant::now();
        let clock = ReplayClock::paced(origin, start, 4.0);
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(clock.instant(), start + Duration::from_secs(4));
        assert_eq!(clock.now(), origin + Duration::from_secs(4));
        // A timer 8 recorded seconds on wakes after 2 s of tokio's.
        let t = Instant::now();
        clock.sleep_until(start + Duration::from_secs(12)).await;
        assert_eq!(t.elapsed(), Duration::from_secs(2));
    }

    #[tokio::test(start_paused = true)]
    async fn a_following_replay_clock_moves_with_the_feeder() {
        let (tx, rx) = watch::channel(0);
        let clock = ReplayClock::following(UNIX_EPOCH, rx);
        let start = clock.instant();
        let woken = tokio::spawn({
            let clock = clock.clone();
            async move { clock.sleep_until(start + Duration::from_secs(2)).await }
        });
        tx.send_replace(1_500);
        tokio::task::yield_now().await;
        assert!(!woken.is_finished());
        assert_eq!(clock.now(), UNIX_EPOCH + Duration::from_millis(1_500));
        tx.send_replace(2_000);
        woken.await.unwrap();
    }
}
