---
id: C-028
title: Flee: below the flee line, leave by a confirmed exit to a quiet room, keep running if followed, rest, return
tier: B
size: L
wave: 4
depends: [C-025, C-027, C-029]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/{flee.rs (new),modes.rs,policy.rs,session.rs,walker.rs}
  - tests/live/ (scenario "flee"), docs/brain.md, docs/plan/04-settings.md (the flee row)
---

## Goal

The Flee mode of `03-engine.md`, replacing C-025's "stop below 30% HP" stand-in. When HP falls
under the flee line the character leaves the fight by a confirmed exit toward a room that
spawns nothing, keeps running if a monster follows, rests there, and comes back to where it
was. The flee line is a plain HP percentage, 30 to start; John dropped the plan's "twice the
hardest hit this fight" addition (2026-10-19), so that checkbox never exists.

## Inputs

- `docs/plan/03-engine.md` section 4 (Flee: highest priority; "leaves by a confirmed exit
  toward a room with no spawns; keeps running if followed"), section 5 (Flee still acts while
  paused when "guard while paused" is on: deferred to M4 with the settings; here paused means
  nothing), `01-goals.md` "Rules" (flee distance and direction are settings: constants here),
  `04-settings.md` section 3 (the Flee row: change it to the percentage alone, noting John's
  decision).
- `docs/kb/navigation.md` "Leaving a room" (one roll per room: the first monster whose
  `followpct` beats it strikes once; each follower rolls to come along), "Movement lines"
  (`just left to the`, arrivals, the follow text `The giant rat creeps in after you!`),
  `docs/kb/pitfalls.md` 24 (the first exit was a closed door; rested in the arena; died), 27
  (check on every HP value), 32, `docs/kb/combat.md` (the monster's free blow; `*Combat Off*`
  when the target is gone).
- The prototype for mechanics: `progress.py` `flee` (l.889: rank exits quiet-first, plain
  before doors; confirm by room; read 1.5 s for the chaser; up to three hops; then rest).
- C-025 (`modes.rs`: the stack, `Bot::hp`, `STOP_BELOW`), C-022 (the walker; the belief;
  `RoomSeen`), C-021 (`spawn_pool`, `hunt_room`, `distances`), C-027 (`in_combat` lapses).

## Behaviour

- **Flee line**: `FLEE_BELOW` = 30 (percent of max HP), a constant in `policy.rs` with the
  M4 note. Checked on every HP value (`Bot::hp` path), whatever is in flight.
- **Wants control** (above Recover and Fight): HP under the line while running, whether or
  not engaged. Flee ends only when the character is in a room with no hunt pool and nothing
  attacking, or after `FLEE_HOPS` (3) hops.
- **The exit**: from the believed room, rank the exits the character can take now: plain
  exits to rooms that are not hunt rooms first, then plain exits to rooms the belief knows
  but that spawn, then doors (`open` then the move; never bash while fleeing, so a locked
  door is no candidate); never an exit the game refused this session, never Text, Hidden or
  gated exits. With no candidate (an uncertain belief, nothing walkable, or a room like the
  Newhaven Arena whose only exit is a door that is locked) narrate it and fall back to
  C-025's stop with a warning: the player is told at once that the character is cornered.
- **Logging off** (C-027's `logging_off`): if the session is attacked or engaged, flee first
  (the same hops), then answer `ready_to_leave`; otherwise answer at once.
- **The hop**: send the direction (through the executor; one action in flight), confirm the
  room (`RoomSeen` feeding the belief as the walker does); then wait up to 1.5 s at the prompt
  for a follow text or an arrival naming a hostile monster; if one comes, hop again (at most
  `FLEE_HOPS`); if the room is quiet, Flee is done. A blocked exit (`The door is closed!`, a
  refusal) tries the next candidate at once.
- **After**: Recover takes over (HP is under its line), rests here (a quiet room), and its
  "walk back home" returns the character to where Fight was running (C-025's home), so Fight
  resumes. Narration throughout: "Fleeing: HP 24%: east to Narrow Path", "Followed by a giant
  rat: north", "Safe in Dead End".
- `STOP_BELOW` goes; the "flee is not built yet" warning with it. The bot stops itself only
  when Flee has no exit.

## Design notes

- `flee.rs` is a mode like the others (`wants`, `step`, `on_event`); it uses the executor
  directly for single hops (not the walker, which plans routes): a hop is one move with a
  `RoomSeen` expectation, so the confirmation is the same machinery as the walker's steps.
- Exit ranking is a pure function in `policy.rs` (`flee_exits(atlas, room, belief, blocked,
  locks) -> Vec<Candidate>`), unit-tested on the stock pack: from Newhaven, Narrow Path the
  first candidate is a plain exit to a non-spawning room; from the Newhaven Arena (its only
  exit is the Dungeon door, locked at a cold start) there is no candidate unless the door is
  known open.
- Update `docs/plan/04-settings.md` section 3: "Flee below HP %" stays; drop the "twice the
  hardest hit" sentence and checkbox (John, 2026-10-19).

## Done when

- In-process tests (fake game): HP under 30% mid-fight → one direction sent, the room
  confirmed, a follow text → a second hop, a quiet room → rest → walk home → Fight resumes;
  a closed door first candidate is skipped for the next exit; three hops then stop; no
  candidate → `break` and stop with the warning; the check fires on an HP value while a walk
  or action is in flight; nothing is sent while paused or stopped; determinism holds.
- `flee_exits` unit tests on the pack (the Arena with the door locked and open, Narrow Path,
  a dead end).
- **Live** (`scripts/live-check.sh --scenario flee`): the pool character in a Newhaven-area
  hunt room that has a plain exit to a quiet room (pick it from C-021's `hunt_rooms` within
  radius 8 of 1:2140; the Arena does not qualify, its only exit is the locked Dungeon door;
  say which room in the PR), with the flee line raised for the run (a `--flee-below 95` test
  knob on `wayfarer brain`, hidden, documented as test-only): Start, the first blow drops HP
  under the knob, the character leaves to the quiet room, rests, walks back, and the log
  shows no punishment and no death. Then the shutdown case: with the character attacked,
  stop the gateway: `logging_off` makes it flee first, the log-off succeeds. Summary in the PR.
- `docs/brain.md` "Flee"; `04-settings.md` updated.

## Tests

- As above on the paused clock; policy tests on the pack.

## Notes for the agent

- A monster's free blow on leaving is the game's rule; nothing to do about it.
- Keep the hop count and the line as constants in one place; the settings card gives them
  a page.
