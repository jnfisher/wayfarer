//! Wall-clock time for the wire's timestamps (`started_at`, `synced_at`) and the store's.
//! Timers and timeouts use `tokio::time`, so tests pause them; this is only what the
//! timestamps say.

use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
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
/// the minutes a test lets pass pass here too (tests, replay).
#[derive(Debug, Clone, Copy)]
pub struct InstantClock {
    start: SystemTime,
    at: tokio::time::Instant,
}

impl InstantClock {
    pub fn new(start: SystemTime) -> Self {
        Self {
            start,
            at: tokio::time::Instant::now(),
        }
    }
}

impl Clock for InstantClock {
    fn now(&self) -> SystemTime {
        self.start + self.at.elapsed()
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
    use std::time::Duration;

    use super::*;

    #[test]
    fn formats_utc_seconds() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let t = UNIX_EPOCH + Duration::from_secs(1_792_411_205);
        assert_eq!(rfc3339(t), "2026-10-19T12:00:05Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(rfc3339(leap), "2000-02-29T00:00:00Z");
    }
}
