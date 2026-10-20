---
id: C-036
title: Run a MegaMud loop as a task: the verified path's rooms become the stops, its step actions are honoured, the soak and the map panel can pick one
tier: B
size: L
wave: 4
depends: [C-030, C-037]
crate: wayfarer-world, wayfarer-brain, wayfarer-proto, ui/, tests/live
touches:
  - crates/wayfarer-world/src/{loops.rs,mm.rs} (`MmLoop` stops and the knowledge its steps carry)
  - crates/wayfarer-brain/src/{session.rs,task.rs,walker.rs,position.rs} (`run_loop` on a MegaMud id; path knowledge for the router and the walker)
  - crates/wayfarer-proto/src/ (`MegaMudLoop.stops`, `run_loop` on a MegaMud id), ui/src/MapPanel.svelte (run button on MegaMud loops)
  - tests/live/src/soak.rs (`--loop` by file name), docs/brain.md "Loops" and "Running a loop", docs/protocol.md, docs/web-ui.md
---

## Goal

John (2026-10-20): "what we ultimately want is a prescribed loop that optimizes hitting lairs
in a given area (just minotaurs in the Wooden Passages, just staying in the Hidden Keep),"
and MegaMud's well-known path set (Winterhawk's paths, 1,957 files, 195 loops, 183 verified
end to end against the stock world) is the starting point. The pack already imports it
(`wayfarer import-world --megamud`; the dev scripts pass it since PR #57), the brain lists
the verified loops through an area (C-024) and the map panel walks one once. What is
missing: **running one as a task**, with fights at each stop, respawn waits and laps, like a
generated loop (C-030). Examples the acceptance run uses: `Mwodloop.mp` "Wooden Passages"
(starts at 1:1875, 54 steps, never enters the Chieftain's room) and `HKEPLOOP.mp` "Bandit
Keep" (inside the courtyard, 62 rooms). The way into the keep is in `Ftrehkep.mp`: from the
Hidden Keep `n[search n]` (a hidden exit, searched first), along the Keep Wall, `climb tree`.

## Inputs

- `docs/kb/megamud-formats.md` (the `.mp` file: steps as `dir[actions]`, flags, the id per
  step), `docs/kb/megamud-manual.md` (paths and loops, how MegaMud runs a path), `03-engine.md`
  "Areas and loops" and "Trusting MegaMud's data" (only verified loops run),
  `crates/wayfarer-world/README.md` (`mm_paths`, `mm_path_steps`: `walk_map`/`walk_room` per
  step, `actions`, `id_match`).
- C-024 (`MmLoop { path, rooms }`, `megamud_through`, `walk_loop`), C-030 (`Task::new`,
  `run_loop`, `find_loop`, `parse_loop_id`), C-021 (`Traveller`, `found` hidden exits,
  `Category`), C-022 (the walker's `Doing` steps), C-037 (the walker searches a hidden exit
  on its route; unreachable stops do not end the loop).

## Behaviour

- **Stops of a MegaMud loop** (`wayfarer-world`): the rooms of the verified walk, in path
  order and without repeats, that spawn something huntable (`hunt_room`), plus any room the
  path pauses in (a step with a wait or an attack action, if the format has them; say in the
  PR what the 183 loops use). `MmLoop::stops() -> Vec<Key>`; the walk's full room list stays
  as `path`. A loop with no stops is listed but cannot be run ("nothing to hunt on it").
- **Path knowledge** (`wayfarer-world`, `PathKnowledge` built once from the pack): for every
  verified step, the room the walk typed it in and the exit it took: a direction, a direction
  with actions before it (`n[search n]`: search, then north), or a phrase (`climb tree`, a
  Text exit). From it: the hidden exits a path finds (room, direction), and the phrase a Text
  or Action exit takes where the pack's message is blank. The router treats a hidden exit a
  path searches as findable (`Traveller::found` is seeded from it for the loop's traveller),
  and the walker (C-037) searches before taking it.
- **`run_loop` on a MegaMud id**: ids are the file name (`Mwodloop.mp`), distinct from
  generated ids (`map:room:radius:doors:index`). `find_loop` resolves both. The task's name is
  the path's description, or its file name when blank; its stops are `MmLoop::stops()`;
  `skip` works as for generated loops; the first stop is the nearest reachable one, as today.
  Legs between stops are routed by the router (shortest, C-021), not by replaying the file's
  commands: the path's rooms are a known-good way, so legs along it cost no more than the
  router's own; a leg that leaves the path (a shortcut through a door) is allowed when the
  router finds it. The laps, respawn waits, rests and the stats are C-030's unchanged.
- **Walking to the loop**: from wherever the character stands, `run_loop` walks to the
  nearest stop as C-030 does; when no stop is reachable it says which stop is nearest and
  why ("no way to Bandit Keep: the hidden exit north of the Hidden Keep is not known"), so
  the player can walk there by hand or use go-to.
- **Protocol**: `MegaMudLoop` gains `stops: Vec<LoopStop>` (same shape as a generated loop's,
  with monsters, arena, lair, regen) and `runnable: bool` with a `reason` when not; `run_loop
  {session, id}` accepts a MegaMud id. `bot.task` reports the loop's id and name as today.
- **Map panel**: the MegaMud loops in the drawer get the same *run* button as generated ones
  (walk once stays); a loop that is not runnable shows the reason, greyed.
- **Live check**: `--scenario soak --loop Mwodloop.mp` picks the MegaMud loop by file name
  (`pick_loop` searches both lists); the summary names the loop and its kind.
- Docs: `docs/brain.md` "Loops" and "Running a loop" (both kinds, how legs are routed, the
  path knowledge), `docs/protocol.md`, `docs/web-ui.md`.

## Design notes

- Routing legs with the router rather than replaying the `.mp` commands keeps one walker and
  one belief; the file's value is its room sequence and its step actions, which become
  knowledge the router and walker use. If a verified loop's leg cannot be routed (an exit
  kind the router refuses), the PR lists which of the 183 that affects; it is acceptable to
  mark those not runnable with a reason rather than widen the router in this card.
- Stops are hunt rooms only: a MegaMud loop passes through many corridor rooms; walking them
  as stops would make the task look at empty rooms all day.
- Keep the path knowledge in `wayfarer-world` (pure, from the pack) so tests run on the
  fixture pack with the reference path set; `tests/fixtures/world` has it if the fixture
  pack is built with `--megamud` (check; if not, add a small fixture of a handful of `.mp`
  files with a PROVENANCE.md, Winterhawk's set is public).

## Done when

- World tests: `Mwodloop.mp` yields stops in path order, all Wooden Passage hunt rooms, none
  repeated, 1:1865 absent; `HKEPLOOP.mp` yields courtyard and keep stops; `PathKnowledge`
  holds the hidden exit north of 9:724 and the `climb tree` phrase at 9:735 from
  `Ftrehkep.mp`; a loop with no hunt rooms is not runnable with a reason.
- Brain tests (fake game): `run_loop "Mwodloop.mp"` builds a task with those stops, goes to
  the nearest and round once; `skip` passes a stop by; `run_loop` with an unknown file warns;
  a generated id still works; a restarted brain resumes a MegaMud task from the store
  (C-029's record holds the id).
- Live (pool character, Newhaven): the brain lists the MegaMud loops through Newhaven (name
  them in the PR), and `--scenario soak --loop <one of them> --minutes 3` runs it round at
  least once with the summary naming it; if no verified loop passes Newhaven, say so and run
  the nearest one the pool character can reach safely, else fall back to a generated loop
  and say why.
- Panel: run button on MegaMud loops; `npm test`; docs updated; checks green.

## Tests

- As above. The walker's search-before-hidden-exit is C-037's; here only that the loop's
  traveller is seeded with the path's hidden exits.

## Notes for the agent

- John's own characters (Waydeva, Waydevb, Thorgrim) are not yours to log in; the live check
  uses the pool character only. Never start, stop or reset the silvermere server.
- The acceptance run on `Mwodloop.mp` and `HKEPLOOP.mp` is John's, with Thorgrim, after the
  merge.
