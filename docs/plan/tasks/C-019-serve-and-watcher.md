---
id: C-019
title: `wayfarer serve` runs the brain in-process; `scripts/dev/brain.sh` runs it under a watcher; the dev rig doc
tier: C
size: S
wave: 2
depends: [C-008, C-014, C-016]
crate: bin/wayfarer, scripts/
touches:
  - bin/wayfarer/src/main.rs (`serve`, `brain`), scripts/dev/brain.sh (new),
    scripts/dev/gateway.sh (the world pack), docs/dev-rig.md, docs/brain.md
---

## Goal

The two ways the plan says the brain runs both work: in development, `scripts/dev/brain.sh`
runs `wayfarer brain` under a file watcher, restarting on every engine change while the
gateway keeps everyone logged in; in a release, `wayfarer serve` runs the gateway and the
brain in one process over in-process channels, with the same messages.

## Inputs

- `docs/plan/02-architecture.md` "Two processes in development, one in a release".
- `docs/plan/07-development-and-testing.md` "The development machine" (the brain under
  `cargo watch` or the repo's own script), `docs/dev-rig.md` "Brain (from M1)" and "What
  restarts when".
- C-016 (`Brain::run`, `ChannelLink`, `--world`), C-006 (`serve()` and `Gateway::start`),
  C-008 (`scripts/dev/gateway.sh`).

## Behaviour

- **`wayfarer serve`** takes the gateway's arguments plus `--world PATH` and runs both: the
  gateway as now, and the brain attached through a `ChannelLink` to the gateway's connection
  handling (the same `Connection` logic, no socket), so UIs see `brain_state true` and the
  brain's messages exactly as with two processes. Stopping it logs everyone off as `gateway`
  does. With no world pack it says so and runs the gateway alone with a warning, so `serve`
  still works for a plain client.
- **`scripts/dev/brain.sh`**: checks for `cargo-watch` (and says how to install it:
  `cargo install cargo-watch` or `brew install cargo-watch`), builds the world pack into the
  data directory if it is missing (`wayfarer import-world --export tests/fixtures/world`),
  then runs `cargo watch -w crates -w bin -x "run -q -p wayfarer -- brain"` with the
  gateway address and token from the data directory. A `brain` crate change restarts the
  brain within the rebuild time; the gateway and the characters are untouched.
- **`scripts/dev/gateway.sh`** is unchanged except that it also builds the world pack if
  missing, so either script may go first.
- **Docs.** `docs/dev-rig.md`: the "Brain (from M1)" section written for real (the third
  terminal tab), "What restarts when" updated, the sleep note (the brain reattaches after
  wake); `docs/brain.md` gains the `serve` form.

## Design notes

- `serve` builds a `ChannelLink` pair; one end is given to `Brain::run` in its own tokio
  task, the other is handed to the server through the channel-attached connection C-014
  built (the same `Connection::run` over a sink and stream, no socket). Nothing in
  `server.rs` should need more than wiring here; if it does, that is C-014's gap, and this
  card says so in its PR rather than refactoring the server.
- The watcher's restart gap: while the brain is down the gateway and UIs carry on; on attach
  the brain re-syncs (C-016). No double re-sync storms: the brain re-syncs once per attach.

## Done when

- `wayfarer serve --world PACK` with the rig: the UI header fills (C-017) with no separate
  brain process; stopping it logs everyone off.
- `scripts/dev/brain.sh`: touching a file in `crates/wayfarer-brain` restarts the brain; the
  UI shows `brain_state false` then `true`; the character never leaves the realm (check the
  gateway log).
- In-process test: `serve`'s wiring attaches a brain and a UI test client sees
  `brain_state true` and a `character` after fixture output.
- `docs/dev-rig.md` and `docs/brain.md` updated; `scripts/live-check.sh` still passes.

## Tests

- The in-process wiring test above; the rest is the rig, by hand, with the results in the PR.

## Notes for the agent

- Never touch the local silvermere; the gateway and brain scripts only talk to it.
- `cargo watch` is a developer tool: not a dependency of the build, not in CI.
