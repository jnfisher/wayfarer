---
id: C-030
title: Task mode: run a loop: walk to the area, go stop to stop, fight at each, rest between, respect respawn delays, narrate
tier: B
size: L
wave: 4
depends: [C-024, C-028, C-029, C-031]
crate: wayfarer-brain, wayfarer-proto, ui/
touches:
  - crates/wayfarer-brain/src/{task.rs (new),modes.rs,session.rs,walker.rs,policy.rs}
  - crates/wayfarer-proto/src/ (`run_loop`, loop stop toggles in the snapshot), ui/src/MapPanel.svelte
  - tests/live/ (scenario "loop" grows: the run), docs/brain.md, docs/protocol.md, docs/web-ui.md
---

## Goal

The bot hunts. From the map panel's loop list John presses **Run** on a loop: the character
walks to the area, then goes round the loop stop by stop, fights what is at each stop (Fight
and Recover as in "fight here"), skips a stop whose respawn delay has not passed, keeps a
per-stop record of what it found, and narrates each step. The Minotaur Chieftain's room is a
stop John ticks on or off. This is M3's "a character runs the area John picks".

## Inputs

- `docs/plan/03-engine.md` section 3 ("moves along a known route may be pipelined a few steps
  ahead ... cancelled the moment a room disagrees with the belief or combat starts"), section
  4 (Task: "walks to the area the player chose and runs its loop"; narration), section 5
  (pause after the action in flight; resume re-syncs and narrates), `09-milestones.md` M3.
- `docs/kb/progression.md` (respawn: nothing for `regen_delay` minutes after a kill; arenas
  exempt; lairs fill to `max_monsters`), `docs/kb/pitfalls.md` 20, 24, 42, 51 (looking into a
  dead boss's room brings it back).
- The prototype for mechanics: `progress.py` `patrol` (l.1578: stops with a respawn timer
  `cleared + regen`, nearest ready stop first, fight on the way, rest when needed).
- C-024 (`Loop { rooms, steps }`, ids, `walk_loop`), C-025 (Fight, Recover, home), C-028
  (Flee), C-029 (`loop_id`, `stop_index`, runs), C-022 (walker, `walking`).

## Behaviour

- **Protocol**: `run_loop {session, id, skip?: [{map, room}]}` starts Task mode on a loop
  (replacing a running "fight here"); `bot_control stop|pause|resume` apply. The snapshot's
  `bot` gains `task { loop_id, loop_name, stop, stops, skip }`.
- **Task mode** (below Flee, Recover and Fight; above Idle): with a loop set, it walks to the
  nearest stop (the walker), then to each next stop in loop order, skipping the ones in
  `skip` and the ones whose respawn delay has not passed since the bot last cleared them
  (`regen_delay` minutes from the pack; 0 means the stock 5; arenas always ready). At a stop:
  home becomes the stop, so Fight and Recover work as in "fight here" until the room is
  clear (no hostile in `Also here` after a look) or quiet for 20 s; then the stop is marked
  cleared and Task moves on. When every stop is waiting on its delay, rest if needed, else
  wait at the current stop with the narration "Waiting for respawns: next in 2 m".
- **Pipelining**: along a known route the walker may have two moves in flight when the belief
  is certain and nothing is engaged; a room that disagrees or `*Combat Engaged*` cancels the
  second. (The executor's one-action rule becomes one *non-move* action; moves carry their
  own confirmation.)
- **Interrupts**: Fight takes over on the way when attacked or a hostile is in the room
  (then Task continues from where it is); Flee and Recover as before; a typed line pauses;
  resume re-syncs and continues the loop at the same stop.
- **Skipping stops**: the map panel's loop list shows each stop with a tick; unticking sends
  `skip`. Hovering shows the stop's monsters. The Chieftain's room (1:1865) is the use case.
- **Narration**: "Loop Wooden Passage: to stop 3/9 Wooden Passage (2 steps)", "Stop 3/9
  cleared: 2 minotaurs", "Skipping Large Wooden Room (unticked)", "Stop 5/9 not ready (3 m)".
- **Stats**: kills, flees, deaths, rests per stop go to the store's run (C-029).

## Design notes

- `task.rs` holds the loop, the stop cursor, the per-stop `cleared_at` map and the skip set;
  it asks the walker for legs and hands the room to Fight by setting home.
- Respawn readiness is a pure function (`stop_ready(now, cleared_at, regen, is_arena)`).
- Persist `loop_id`, the stop keys, `stop` and `skip` through C-029 on every change, so a
  restart resumes even when the character's level changed the id's meaning.
- C-031's arbitration is in: a typed talk-or-look line goes between legs without pausing;
  an act pauses the loop.

## Done when

- In-process (fake game with a three-stop loop cut from the Newhaven captures and scripted
  minotaur-like fights): Run walks to the nearest stop, fights, moves on, skips an unticked
  stop and a not-ready stop, waits when none is ready, resumes after a pause at the same
  stop, resumes after a brain restart (C-029), pipelines two moves and cancels the second on
  a wrong room, and never sends an attack while engaged; determinism holds.
- **Live** (`scripts/live-check.sh --scenario loop`): the pool character runs the Newhaven
  loop for five minutes: at least one kill at the Arena, stops narrated, the delay honoured,
  no death; then Stop, walk out, log off. Summary in the PR.
- Map panel: Run and the stop ticks; `docs/web-ui.md`, `docs/brain.md` "Running a loop".

## Tests

- As above on the paused clock; `stop_ready` unit tests.

## Notes for the agent

- "Where next?" and moving to the next area are M4; the loop is John's choice.
- Keep the executor's guards; pipelining must not let two non-move actions overlap.
