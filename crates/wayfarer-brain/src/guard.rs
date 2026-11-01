//! `guard` on a Quicksilver 1.20 server (C-068, docs/brain.md "Parties"): what one session
//! knows of its guarding, from the game's lines. The party coordinator reads it to decide when
//! a `guard <ward>` (or a plain `guard` to stop) goes; nothing is sent from here. The game ends
//! a guard by itself, with its stop lines, when the two part, either rests or meditates, leaves
//! the party, logs off or dies; no state survives a reconnect.

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;

/// Refusals in a row before the guard is held.
pub(crate) const REFUSALS: u32 = 3;
/// How long a guard refused that often is left alone.
pub(crate) const HOLD: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Guard {
    /// The ward this character guards, as the game named it (`Mirelle`).
    pub guarding: Option<String>,
    /// Who guards this character, as the game named them.
    pub guarded_by: Option<String>,
    /// Refusals in a row since the last start.
    pub refusals: u32,
    /// The last refusal (`not_here`, `not_party`, `self`, `resting`, `not_guarding`).
    pub refused: Option<&'static str>,
    /// No `guard` before this: refused too often.
    pub held_until: Option<Instant>,
}

impl Guard {
    /// One of the game's lines.
    pub fn event(&mut self, ev: &GameEvent, now: Instant) {
        match ev {
            GameEvent::Guarding {
                guard: None,
                ward: Some(ward),
                on,
            } => {
                if *on {
                    self.guarding = Some(ward.clone());
                    self.refusals = 0;
                    self.refused = None;
                    self.held_until = None;
                } else if self
                    .guarding
                    .as_deref()
                    .is_some_and(|w| w.eq_ignore_ascii_case(ward))
                {
                    self.guarding = None;
                }
            }
            GameEvent::Guarding {
                guard: Some(guard),
                ward: None,
                on,
            } => {
                if *on {
                    self.guarded_by = Some(guard.clone());
                } else if self
                    .guarded_by
                    .as_deref()
                    .is_some_and(|g| g.eq_ignore_ascii_case(guard))
                {
                    self.guarded_by = None;
                }
            }
            GameEvent::GuardRefused { why } => {
                self.refused = Some(why);
                // Not guarding anyone is the answer to a `guard` that stops: no refusal.
                if *why == "not_guarding" {
                    self.guarding = None;
                    return;
                }
                self.refusals += 1;
                if self.refusals >= REFUSALS {
                    self.refusals = 0;
                    self.held_until = Some(now + HOLD);
                }
            }
            // Dead: the game ends it (its stop lines may not come before the death's).
            GameEvent::YouKilled => {
                self.guarding = None;
                self.guarded_by = None;
            }
            _ => {}
        }
    }

    /// A new connection, or out of the realm: nothing survives.
    pub fn reset(&mut self) {
        *self = Guard::default();
    }

    /// Whether a `guard` may go now (not held).
    pub fn may_ask(&self, now: Instant) -> bool {
        self.held_until.is_none_or(|t| now >= t)
    }

    /// Whether it guards the member named `first` (an in-game first name, any case).
    pub fn guards(&self, first: &str) -> bool {
        self.guarding
            .as_deref()
            .and_then(|w| w.split_whitespace().next())
            .is_some_and(|w| w.eq_ignore_ascii_case(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guarding(guard: Option<&str>, ward: Option<&str>, on: bool) -> GameEvent {
        GameEvent::Guarding {
            guard: guard.map(Into::into),
            ward: ward.map(Into::into),
            on,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn start_stop_and_the_wards_side() {
        let now = Instant::now();
        let mut g = Guard::default();
        g.event(&guarding(None, Some("Mirelle"), true), now);
        assert!(g.guards("mirelle"));
        // Another's stop changes nothing; its own does.
        g.event(&guarding(None, Some("Penny"), false), now);
        assert!(g.guards("Mirelle"));
        g.event(&guarding(None, Some("Mirelle"), false), now);
        assert_eq!(g.guarding, None);
        g.event(&guarding(Some("Brannoc"), None, true), now);
        assert_eq!(g.guarded_by.as_deref(), Some("Brannoc"));
        g.event(&guarding(Some("Brannoc"), None, false), now);
        assert_eq!(g.guarded_by, None);
    }

    #[tokio::test(start_paused = true)]
    async fn refusals_hold_it_after_three() {
        let now = Instant::now();
        let mut g = Guard::default();
        for _ in 0..REFUSALS - 1 {
            g.event(&GameEvent::GuardRefused { why: "not_here" }, now);
            assert!(g.may_ask(now));
        }
        g.event(&GameEvent::GuardRefused { why: "not_here" }, now);
        assert!(!g.may_ask(now));
        assert!(g.may_ask(now + HOLD));
        // A start clears it.
        g.event(&guarding(None, Some("Mirelle"), true), now);
        assert!(g.may_ask(now));
        // `You are not guarding anyone.` is no refusal.
        g.event(
            &GameEvent::GuardRefused {
                why: "not_guarding",
            },
            now,
        );
        assert_eq!((g.guarding.as_deref(), g.refusals), (None, 0));
    }

    #[tokio::test(start_paused = true)]
    async fn death_and_a_reconnect_end_it() {
        let now = Instant::now();
        let mut g = Guard::default();
        g.event(&guarding(None, Some("Mirelle"), true), now);
        g.event(&GameEvent::YouKilled, now);
        assert_eq!(g.guarding, None);
        g.event(&guarding(None, Some("Mirelle"), true), now);
        g.reset();
        assert_eq!(g, Guard::default());
    }
}
