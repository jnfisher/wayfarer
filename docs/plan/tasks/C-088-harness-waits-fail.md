---
id: C-088
title: "A test's wait for what never comes fails, with the brain's last words, instead of running on"
tier: C
size: S
wave: 16
depends: []
crate: wayfarer-brain (tests)
touches:
  - crates/wayfarer-brain/tests/game/mod.rs (`until`, `narrated`, `walk_narrated`, `WAIT_LIMIT`)
  - crates/wayfarer-brain/tests/brain.rs (`until`)
  - crates/wayfarer-brain/tests/walk.rs
  - docs/plan/tasks/C-085-wrong-copy-of-the-hills.md (a wrong note corrected)
---

## Goal

Twice on 2026-11-10 (C-085, C-086) a broken brain test hung, at full CPU, instead of failing.
The fake game's `until` read the brain's messages until one matched. On the paused clock the
bot's own timers keep it talking ("Waiting here", the status check), so a wait for something
that never came ran on for ever in game time. C-085's notes took six such hangs for a bot that
"spins" with no position. That was wrong: the bot re-synced a second time and the scripts got
out of step.

## Behaviour

- `Game::until`, `narrated`, `walk_narration` and `walk_narrated` fail the test after
  `WAIT_LIMIT` (an hour) of the paused clock. The failure says what the test waited for, the
  brain's last 30 notices and warnings, and its last 30 lines sent:
  `waited 3600s of the paused clock for the walker's line "…"; it never came. The brain's last
  words: […]; its last lines sent: […]`.
- `narrated` and `walk_narrated` share one limit across their reads.
- `brain.rs`'s own `until` gets the same limit. (`party.rs`'s already had 300 s.)
- The limit is an hour because the longest wait of a passing test is over ten minutes
  (`task.rs`, an arena's delay three times). Reaching it takes 1–2 s of real time.

## Done when

- `walk.rs` `a_wait_for_what_never_comes_fails_instead_of_running_on`: a Go to that stops short
  of a lair, and a wait for a walker line that never comes, panics with "it never came".
- The whole suite passes as before.
- The experiment from C-085 (forgetting the belief when `look` goes), run again, makes the
  `fight.rs` tests fail in seconds with "Starting in an unknown room: re-syncing" and a second
  `st, exp, i, look` in their messages, rather than hang. Not committed.
