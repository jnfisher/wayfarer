---
id: C-022
title: The executor and the walker: protocol M2 (map view, walk to, stop), confirmed moves, doors, relocation
tier: B
size: L
wave: 3
depends: [C-020, C-021]
crate: wayfarer-brain, wayfarer-proto
touches:
  - crates/wayfarer-proto/src/ (M2 messages; `CharacterState.position`, `.walking`)
  - crates/wayfarer-brain/src/{executor.rs,walker.rs,position.rs} (new), session.rs, brain.rs
  - tests/live/ (scenario "walk"), docs/brain.md, docs/protocol.md
---

## Goal

The brain can take a character somewhere. A UI says "walk to this room"; the brain plans a
route over the world graph, sends one move at a time at a prompt boundary, confirms each
room against the position belief, opens doors on the way, relocates when the game says there
is no exit, and stops the moment a room disagrees or combat starts. The executor it is built
on (an action is a command plus the events that confirm it and a timeout) is the one every
later mode uses. The brain also serves the map panel: the rooms laid out around the
character, the belief and the route.

## Inputs

- `docs/plan/03-engine.md` sections 2 (belief), 3 (executor: one action in flight; hard
  guards live here), 6 (typed lines), `02-architecture.md` "Input arbitration" (raw keys and
  typed lines pause the bot: in this card a typed line stops a walk).
- C-021 (`Graph`, `route`, `Belief`, `layout`, `Traveller`), C-020 (`sent` ordering),
  `crates/wayfarer-brain/src/session.rs` (the re-sync state machine is the pattern: `Want`,
  `Waiting {deadline}`, sends only at a prompt).
- `kb/navigation.md`, `kb/screen-patterns.md` "Movement results" and `kb/pitfalls.md` 34, 36
  for the exact texts: `There is no exit in that direction!`; `The door is Closed!` (shut,
  unlocked) and `The door is closed!` (locked); `The door is now open.`; `You bashed the
  gate open.` / `Your attempts to bash through fail!`; the Level, Alignment, Class and Race
  refusals; `You are not permitted in that room!`; a direction command queues with a one-tick
  delay and the game drops the 14th queued line (keep at most one in flight here).
- The prototype for mechanics: `progress.py` `step` (l.418, what it reads after a move),
  `open_way` (l.488, open then bash up to six times), `walk_to` (l.506, five attempts with
  relocation between).
- `crates/wayfarer-parse/src/typed.rs`: `RoomTitle`, `RoomExits`, `RoomDark`,
  `CombatEngaged`, and the refusal patterns (`no_exit`, `exit_level_high`, `exit_level_low`,
  `not_permitted`, `exit_forbidden`, `exit_alignment`, `need_spell`, `need_item`,
  `toll_short`, `exit_timed`, `door_closed` (a move into a shut door: `The door is Closed!`
  unlocked, `The door is closed!` locked; the regex has no group for the case, so the typed
  event reads the line), `door_locked` (the reply to `open` on a locked door: `The door is
  locked.`), `door_opened`, `bash_ok` (`You bashed the (?P<what>.+) open.`; `smashed_down` is
  a combat line, not this), `bash_fail`); add typed variants for those this card needs
  (`MoveRefused { reason }`, `DoorClosed { locked }`, `DoorOpened`, `OpenRefused`,
  `BashResult { ok }`).

## Behaviour

**Protocol (adding, no bump):**

| Message | Direction | Meaning |
|---|---|---|
| `walk_to {session, map, room}` | UI → brain | Plan and walk; replaces a walk in progress |
| `walk_stop {session}` | UI → brain | Stop after the move in flight |
| `map_request {session, radius?}` | UI → brain | Ask for a `map_view` now |
| `map_view {session, center, level, rooms, exits, route?}` | brain → UIs | The layout around the believed position: `rooms [{map, room, name, x, y, level, spawns, kind, visited, candidate, collides}]`, `exits [{from, to, dir, kind}]`, the current route as a list of room keys. `spawns` is C-021's hunt-room flag; `kind` is `shop`, `trainer`, `bank`, `arena`, `lair` or none, from the room's type and `shop_num` and the MegaMud waypoint flags where a waypoint resolved to the room; the brain computes both, the panel only draws |

`CharacterState` gains `position {map?, room?, candidates, title}` and `walking {to, steps_left, step}` (absent when not walking). A `map_view` goes out whenever the believed room changes, when a route starts, changes or ends, and on request; radius 15 by default.

**Executor.** `Action { command, expect: Expect, timeout }` where `Expect` names the typed events that confirm it (`RoomSeen`, `DoorOpened`, `BashResult`, `Prompt`) and which refusals fail it. One action in flight per session; it is sent only at a prompt boundary and never while the session is outside the realm. Outcomes: confirmed, refused (with the typed reason), timed out. Hard guards here: an attack command is refused while `in_combat`; `train stats` is refused always (nothing sends it yet, the guard exists).

**Walker.** On `walk_to`: route from `belief.best()` with the character's level, class, race and the session's blocked set; narrate the route (`notice`: "Walking to Newhaven Arena: 6 steps, one door"). Each step: send the command (direction or phrase); on `RoomSeen` feed the belief (`moved` then `observe`); if the destination of the step is in the belief, next step; if not, replan from the belief once, then stop with a `warning`. On `DoorClosed { locked: false }`: `open <dir>` then resend the move. Locked: `bash <dir>` up to 3 times, then mark the edge blocked, replan. On a refusal: mark the edge blocked for this session, replan. On `There is no exit in that direction!`: pitfall 34, send `look` (an empty line redisplays only in FULL modes), `observe`, replan from the new belief; two relocations in one walk stop it. On `CombatEngaged`, a typed line from the player (`sent` not the brain's own), the session leaving the realm, or `walk_stop`: stop after the move in flight, narrate why. Arrival: `notice "Arrived: <name>"`, `walking` cleared.

**Position without walking.** Every `RoomSeen` and every `sent` direction from the player
feed the belief too, so the map follows a character played by hand.

## Design notes

- `executor.rs`: the state machine and the guards; `walker.rs`: the route and the per-step
  policy; `position.rs`: the belief wrapper with the `Traveller` and the blocked set. All in
  the session's single task, driven by events and the clock as the re-sync is.
- `RoomSeen` is the pair `RoomTitle` then `RoomExits` (or `RoomDark`); assemble it where the
  parser's events arrive, with the title allowed to be missing when `Obvious exits` came
  (KB: titles get lost behind prompts).
- Keep one move in flight. Pipelining known routes is a later card.
- The `RoomGraph` loads once per brain, shared by sessions: `World` (brain.rs) keeps the
  `WorldPack` open beside the registry instead of dropping it after `game_texts()`, and holds
  the graph; C-024 and C-025 build on that shape.

## Done when

- In-process tests on the `room-walk` capture replayed through the fake gateway: the brain
  walks a scripted route, each step sent only after the previous room was confirmed; a
  scripted "no exit" triggers a look and a replan; a closed door produces `open n` then `n`;
  a locked door produces up to three `bash n`; a refusal marks the edge and the replan avoids
  it; `CombatEngaged` mid-walk stops after the move in flight; a typed line stops it; no
  command is ever sent outside `realm` or while another action is in flight.
- `map_view` after each room change has the believed room at (0, 0) with its neighbours
  placed; the route appears in it while walking.
- The guards: an attack while `in_combat` is refused by the executor (unit test), `train
  stats` is refused.
- **Live**: `scripts/live-check.sh --scenario walk`: from the test character's start room,
  `walk_to` the Newhaven Arena entrance (or another room a few steps away through at least one
  door), confirm arrival by the room title, walk back, stop the brain. Paste the summary.
- `docs/brain.md` "Walking" and `docs/protocol.md` updated.

## Tests

- As above, on a paused clock. The executor's guard tests are plain unit tests.

## Notes for the agent

- The walker does not fight. If combat starts, it stops and the player (or C-025's modes)
  take over.
- Never send two direction commands without a room between them; the game queues them and
  the belief would drift.
