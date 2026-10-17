---
id: C-009
title: scripts/live-check.sh: pinned silvermere, seeded, fixture snapshot, logon/log-off scenario
tier: B
size: S
wave: 1
depends: [C-005, C-006]
crate: scripts/, tests/live/
touches:
  - scripts/live-check.sh
  - tests/live/ (scenario runner and fixture definitions)
  - tests/live/fixtures.toml (accounts and characters John approves)
---

## Goal

The test that needs a game server, run in an agent's own cloud container before it pushes,
never in GitHub Actions: start a pinned silvermere with a fixed seed and a ready-made
database of test characters, run `wayfarer gateway` against it, and check the M0 scenario
(log on, reach the realm, type a command, log off cleanly) in a minute or two.

## Inputs

- `docs/plan/07-development-and-testing.md` "Live tests in the agent's container" and
  "Test layers".
- `silvermere` CLI: `serve --seed N`, `import --from`, `account create` (password on stdin),
  `backup` and `restore` (`silvermere/bin/silvermere/src/cli.rs`).
- `mmud-exploration/dist/WCCMMUD_1.11p_DOS_MBBSEmu_v2.zip` (game files; available in the
  session because mmud-exploration is attached).
- `mmud-exploration/kb/connection.md` "Character creation" (editor cursor positions, name
  rules) and the `creation-fsd` capture, for building fixture characters.

## Behaviour

- `scripts/live-check.sh [--keep]`:
  1. Get silvermere: download the pinned release binary into a cache directory (version in
     `tests/live/silvermere.version`); if none is published, fail with a clear message
     naming the prerequisite.
  2. Fixture database: if the cache has a snapshot for this silvermere version and this
     `fixtures.toml`, `silvermere restore` it; otherwise import the world, create the
     accounts, create each character through the game's editor with the stats in
     `fixtures.toml`, take a `silvermere backup` into the cache, and use it.
  3. Start `silvermere serve --seed 1` on free ports; start `wayfarer gateway` with a config
     for the fixture characters.
  4. Run the scenario through the gateway's WebSocket: every character reaches `realm`; an
     `input_line` of `st` produces the status screen; the gateway shuts down and every
     character logs off cleanly (no "disconnected while playing" notice on the next logon,
     checked by logging on once more).
  5. Print a short summary (pass or fail per step, timings) to paste into the PR; stop
     everything unless `--keep`.
- Total time on a warm cache: under two minutes.

## Design notes

- The scenario runner is a small Rust test binary under `tests/live/` (a separate cargo
  package or an `#[ignore]`d integration test run by the script), so CI never runs it.
- `fixtures.toml`: two characters to start (a Warrior and a Mage), race, class, stats and
  names. **The stats are John's choice**; the card proposes values in the PR for him to
  approve. Creating them is test setup, not the bot spending CP (`01-goals.md` "Rules").

## Done when

- The script passes twice in a row in a fresh cloud container, the second run from the
  cached snapshot, and the PR shows both summaries and times.
- Nothing in `.github/workflows/` runs it.

## Tests

- The scenario itself; a unit test for the fixture-snapshot cache key.

## Notes for the agent

- **Prerequisite**: silvermere publishes a Linux x86-64 binary on a tagged release. If it
  does not exist when this card starts, open an issue on `jnfisher/silvermere` describing the
  need (one release workflow job, on tag only) and stop at a draft PR with the rest done;
  building silvermere from source in every container is too slow to be the plan.
- Keep scenarios few in M0; later cards add theirs (walk, fight, rest, flee) here.
