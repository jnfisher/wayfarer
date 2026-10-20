---
id: C-032
title: Replay: play a recorded session back into the UI and the brain on its recorded clock; the CI replay layer
tier: B
size: M
wave: 4
depends: [C-029, C-030, C-031]
crate: wayfarer-gateway, bin/wayfarer, wayfarer-brain
touches:
  - crates/wayfarer-gateway/src/{replay.rs (new),gateway.rs,session.rs} (a replay session source; `sent.log`)
  - crates/wayfarer-brain/src/{clock.rs and every timer: the clock refactor}
  - bin/wayfarer/src/main.rs (`wayfarer replay`), crates/wayfarer-brain/tests/replay.rs (new)
  - tests/fixtures/replay/ (two short recordings with PROVENANCE.md), docs/replay.md (new), docs/dev-rig.md
---

## Goal

`09-milestones.md` M3: "Replay mode plays back any recorded session". A gateway session log
(`logs/<id>/output.log`, with its `sent.bin`) can be played back as if the game were sending
it, at its recorded pace or faster, so John can watch in the normal UI what a bot did while
he was away, and so CI can feed a recording through the brain with the recorded clock and
check the decisions match (`07-development-and-testing.md` test layer 3).

## Inputs

- `docs/plan/07-development-and-testing.md` "Test layers" (3: replay), "Live tests" (recorded
  sessions played back in the UI's replay mode), `03-engine.md` (the core is deterministic;
  time injected), `02-architecture.md` "Storage" (`logs/`).
- `crates/wayfarer-gateway/README.md` (the log record format: `seq`, `ts`, `text`; `sent.bin`;
  `Connect` connectors, the in-memory pipe the tests use), C-016 (`Clock`, `ChannelLink`),
  C-029 (a throwaway store for a replay).

## Behaviour

- **`wayfarer replay <logs/<id> dir> [--speed N|--instant] [--config …]`**: starts a gateway
  whose one session, named after the directory, is fed from `output.log` on its timestamps
  (scaled by `--speed`, or as fast as the UI takes it with `--instant`), with the session
  state following the text as the tracker does; serves the UI on 7373 (or `--bind`) so the
  browser shows the tiles, headers and events exactly as live; input typed into it is
  recorded as sent and shown but goes nowhere. A brain may attach to it as to any gateway
  (and `--brain` runs one in-process, with a throwaway store and the recorded clock), so the
  dev panel and the map work on the recording. Stopping it logs nothing off.
- **`sent.log`** (gateway): `sent.bin` is raw bytes with no time, order or origin, so the
  gateway gains `logs/<id>/sent.log`, one JSON line per line sent: `seq` (the output `seq` it
  followed), `ts`, `origin` (`ui`, `brain`, `script`), `text` masked as `sent.bin` masks.
  It is what the replay feeds the brain as `sent` and what the CI layer compares against.
- **The brain's clock**: today the brain's timers (`IDLE`, `ANSWER_TIMEOUT`,
  `ACTION_TIMEOUT`, `COMBAT_QUIET`, Recover's waits) use `tokio::time` directly; `Clock` is
  only for timestamps (`clock.rs` says so). This card moves them behind the injected clock
  (`Clock::now()` as the `Instant` source and `Clock::sleep_until` for the wake-ups), so a
  `ReplayClock` driven by `output.log`'s `ts` makes re-sync timing and the 12 s quiet rule
  behave as they did live, in CI and under `wayfarer replay --brain`. The refactor touches
  every brain file, which is why this card runs after the last brain card of the wave.
- **Pace**: `output.log`'s `ts` drives the replay clock, scaled by `--speed`.
- **The CI layer**: `crates/wayfarer-brain/tests/replay.rs` feeds a recording (the fixture
  pair: `output.log` plus `sent.log`, recorded by this card from live-check runs of "fight"
  and "walk" once `sent.log` exists, copied with provenance) through a brain on the recorded
  clock and asserts the commands it wants to send equal, in order, the lines `sent.log` shows
  with origin `brain`. Each under a second; the two fixtures together under 1 MB. When a later
  card changes a decision on purpose it re-records the fixtures (say so in `docs/replay.md`).
- `docs/replay.md`: how to record (every gateway session is already recorded), how to replay,
  what the UI shows; `docs/dev-rig.md` links it.

## Design notes

- The gateway gets a `Connect` implementation that reads the log instead of a socket
  (`replay.rs`), so sessions, logs, history and the WebSocket server are untouched; the replay
  writes its own logs into a temp data directory, never over the recording.
- The brain's `Clock` trait already exists; `ReplayClock` advances with the `ts` of the chunk
  being fed.
- Keep the fixture recordings free of anything personal (test accounts on 127.0.0.1 only).

## Done when

- `wayfarer replay` on a live-check recording shows the session in the browser with the
  header filling (brain attached) and the dev panel's events; `--instant` finishes in seconds.
- The CI replay test passes on both fixtures, fails when a brain decision changes (prove it
  once by changing `IDLE` and reverting), and runs under a second each.
- `docs/replay.md` written; `docs/README.md` links it.

## Tests

- The replay connector's pacing on a paused clock; the CI replay layer.

## Notes for the agent

- A recording's `sent.bin` masks passwords already; the replay must not need credentials.
- Do not add a UI "replay" page: the normal page on a replay gateway is the feature.
