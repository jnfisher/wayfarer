---
id: C-033
title: The timed run: `scripts/live-check.sh --scenario soak`: a character runs a loop for N minutes and the numbers come out
tier: C
size: M
wave: 4
depends: [C-030, C-029]
crate: tests/live, scripts/
touches:
  - tests/live/src/{soak.rs (new),main.rs}, scripts/live-check.sh, tests/live/README.md
  - docs/dev-rig.md ("A timed run")
---

## Goal

M3's acceptance run as a command: a named character runs a named loop for a given time, and
the check prints what happened: exp per hour, kills, deaths, flees, rests, relocations,
unclassified-line rate, and how the brain and gateway behaved. John's case (2026-10-19):
Thorgrim, a level 20 Dwarf Paladin he had seeded, on a Wooden Passage loop for 30 minutes,
with the Minotaur Chieftain's room (Large Wooden Room 1:1865) in or out by a switch.

## Inputs

- `docs/plan/09-milestones.md` M3 and its parity bar (exp/hr against the prototype's measured
  baseline, no more deaths, fewer relocations, unclassified under 0.2%), `07-development-and-
  testing.md` "Live tests" (shared, long-lived world; scenarios check behaviour, not numbers;
  the soak layer is "by hand, when wanted").
- `tests/live/README.md` and `src/main.rs` (the scenario runner, `--char`, the pool, the
  summary table), C-030 (`run_loop`, `skip`), C-029 (`stats`, runs), C-024 (`loops_request`,
  ids), C-028 (Flee).
- The character: any account the check has a password for, not only the pool. The harness
  reads `tests/live/.credentials.toml` (`WAYFARER_LIVE_CREDENTIALS`); this card lets that
  file, or the file given, also be a gateway `credentials.toml` (`[character.<id>] user,
  password`), so `--char Thorgrim --credentials "$WAYFARER_DATA_DIR/credentials.toml"` works
  on the rig without copying a password anywhere. The start room is wherever he is.

## Behaviour

- `scripts/live-check.sh --scenario soak --char NAME --minutes N --anchor MAP:ROOM
  [--radius R] [--skip MAP:ROOM ...] [--loop ID]`: logs the character on through the check's
  own gateway, attaches a brain, re-syncs, asks for the loops around the anchor (default: the
  believed room; radius 8), picks the first generated loop (or `--loop`), sends `run_loop`
  with the skips, and watches for N minutes.
- **While it runs**: prints a line a minute (time, room, HP, exp gained, kills, flees, deaths,
  narration), stops at once on a death or when the brain stops itself (and says why), and
  saves the full event and notice streams to the run's log directory.
- **At the end**: Stop, let Recover finish if it is mid-rest, walk out to the anchor's nearest
  quiet room, log off; print the summary table: minutes, exp gained, exp/hr, kills by monster,
  deaths, flees, rests, relocations (the brain's "no exit" looks), unclassified lines and
  rate, longest gap without a prompt, brain restarts; and the run id in the store.
- **Safety**: never a pool character outside the arena for long runs (the pool is for the
  short scenarios); a death ends the run; the check never trains or spends anything.

## Done when

- `--scenario soak --char Waychecka --minutes 3 --anchor 1:2140` runs the Newhaven loop for
  three minutes and prints the table (a smoke run in the card's own session; summary in the
  PR).
- John's run: `--char Thorgrim --minutes 30 --anchor 1:1839 --radius 10` with and without
  `--skip 1:1865` is documented in `docs/dev-rig.md` as the M3 acceptance command; the card
  does not run it (Thorgrim is John's).
- `tests/live/README.md` documents the scenario and the table.

## Tests

- The summary arithmetic as unit tests in `tests/live`; the run itself is live only.

## Notes for the agent

- Thorgrim exists on John's silvermere (seeded 2026-10-19; sysop; plate and the shimmering
  greatsword); his password is in John's `credentials.toml`, not in the repository.
- The Wooden Passage is 43 steps from Silvermere's Town Square via the Docks skiff (a Text
  exit), the beach, the Limestone Tunnel, the Labyrinth entrance and one door; the walker
  handles all of it, and the anchor can be the character's room once he is there.
