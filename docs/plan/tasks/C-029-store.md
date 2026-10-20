---
id: C-029
title: `wayfarer-store`: the bot's state survives a brain restart; measured statistics; a stats tab
tier: B
size: M
wave: 4
depends: [C-025, C-018, C-027]
crate: wayfarer-store (new), wayfarer-brain, wayfarer-proto, ui/
touches:
  - crates/wayfarer-store/ (new: SQLite, `store.sqlite` in the data directory)
  - crates/wayfarer-brain/src/{brain.rs,session.rs,modes.rs} (load at attach, save on change)
  - crates/wayfarer-proto/src/ (`stats` in the snapshot; `stats_request`), ui/src/DevPanel.svelte or a Stats tab
  - docs/brain.md, docs/protocol.md
---

## Goal

Two things the plan puts in the store (`02-architecture.md` "Storage", `03-engine.md` sections
2 and 7): the bot's own state, so a brain restart (the watcher, a crash) resumes what the
character was doing instead of leaving it stopped; and measured statistics, so "how is it
doing" is a number on a tab: exp per hour, kills, deaths, flees, per character, per loop and
per session.

## Inputs

- `docs/plan/02-architecture.md` "Storage" (`store.sqlite` in the data directory), "Crates"
  (`wayfarer-store`: log index and search, measured statistics; a leaf used by brain).
- `docs/plan/03-engine.md` section 2 "Measured statistics" (exp/hr, kills, deaths and flees
  per hunting ground and level, attack results per mode), section 7 "Restarts" (persisted
  state: plan, loop position, pile, pause state; re-sync; "resumes, narrating that it did").
- `docs/plan/07-development-and-testing.md` "Four restart domains" (the brain restarts on
  every engine change with no cost).
- C-025 (`Bot` state, `start`, `pause`), C-024 (loop ids survive restarts by design), C-018
  (the drawer and its tabs), C-020 (`kills` counter).

## Behaviour

- **Store.** `Store::open(path)` (rusqlite, WAL, `store.sqlite` beside `wayfarer.toml`,
  `--store` to override), schema versioned. Tables: `bot_state (session, state, home,
  loop_id, loop_stops (the stop keys, JSON), stop_index, skip (JSON), paused_reason,
  updated_at)`: the stops are stored as well as the id, because a generated id resolves
  against the character's level (`parse_loop_id`) and a level-up can make it name another
  loop or none; on resume the stored stops win, with a notice when the id no longer matches; `runs (id, session, started_at,
  ended_at, loop_id)`; `events (run_id, at, kind, value, room)` for kills (monster, exp),
  deaths, flees, rests, level-ups; `samples (run_id, at, exp, hp)` every minute.
- **Resume.** On attach, for each session with a stored `running` or `paused` state: the
  brain re-syncs, then puts the bot back into that state and mode with the stored home,
  loop and stop, narrating "Resumed: running loop Newhaven at stop 2/3 (brain restarted)".
  A `stopped` state is just stopped. State is saved on every change (start, stop, pause,
  resume, a loop stop reached); the write is cheap and synchronous.
- **Statistics.** A run starts at Start and ends at Stop (or a death). The brain records
  kills (from `MonsterDeath` and `ExpGain`), deaths, flees, rests and level-ups with the room
  and time, and samples exp and HP every minute. The snapshot gains `stats { run_started_at,
  exp_per_hour, kills, deaths, flees, rests, exp_gained }` for the current run, recomputed on
  each event; `stats_request {session}` answers with the last ten runs' summaries (`stats`
  message).
- **Stats tab** in the drawer beside map and dev: the current run's numbers large, the last
  runs as rows (started, loop, minutes, exp/hr, kills, deaths, flees), and a small exp-over-
  time sparkline from the samples. `Alt+S`.
- Nothing personal in the store beyond what the logs already hold; it lives in the data
  directory.

## Design notes

- The brain's time for the store is the clock it already injects, so replay (C-032) can run
  with a throwaway store on the recorded clock.
- Keep the store a leaf: `brain` depends on `store`; `store` on `rusqlite` only.
- A run's `exp_per_hour` is exp gained over wall time since Start, not since the last kill.

## Done when

- In-process: Start, a kill, pause, then a second `Brain` attached to the same store and
  fake gateway resumes paused with the same home and narrates it; stop clears; a run's
  events and samples are in the store; `stats` sums are right; a corrupt or missing store is
  recreated with a warning.
- Live: start the bot on the pool character in the Arena, restart `wayfarer brain` while it
  runs; it comes back running in the Arena; `stats_request` shows the kills so far. Summary
  in the PR.
- Stats tab renders the run and the rows; `Alt+S` in the keys table and docs.
- `cargo nextest run --workspace` under the budget; `npm test` green.

## Tests

- Store round trips and migrations; brain resume on the paused clock; UI tab rendering.

## Notes for the agent

- Loop running itself is C-030; this card persists `loop_id` and `stop_index` as fields the
  Task mode will fill, with tests that they survive, and resumes "fight here" fully.
- The log index and search of `02-architecture.md` are a later card.
