---
id: C-053
title: "Lost in a player's house: a room the map lacks is unknown, failed legs pause, `par` only when apart; rests where it stands by default"
tier: B
size: M
wave: 8
depends: [C-030, C-039]
crate: wayfarer-world, wayfarer-brain, wayfarer-settings
touches:
  - crates/wayfarer-world/src/{belief.rs,nav.rs,route.rs}
  - crates/wayfarer-brain/src/{policy.rs,recover.rs,task.rs,walker.rs,party.rs,session.rs}
  - crates/wayfarer-settings/src/lib.rs (`health.rest_quiet_room`)
  - docs/brain.md; docs/plan/01-goals.md and CLAUDE.md "Rules"
---

## Goal

A character that ends up in a room the map lacks knows it is lost and stops, instead of
flooding the game with moves; nothing walks it into a player's house; a party on a street of
many same-named rooms does not `par` every 5 s; and resting happens where the character stands
unless the player asks for a quiet room. (Added 2026-10-24 after Faye got lost on the Slums
loop; resting in lairs at John's request: "the spawn rate is slow enough that the bot doesn't
have to find a quiet place to rest".)

## Inputs

- Faye's logs, 2026-10-24 14:22-14:24: Recover at Slum Street, Bend (1:1096) chose the house
  north of it (15:874, a Map Change into a room the pack has no name for) as its quiet room;
  in "Shop White House Room" every `look` showed a title no room has, the belief kept its wrong
  room, and the walker sent `e`, `look`, `e`, `n` about ten times a second until the loop gave
  up; `par` went every 5-7 s throughout.
- `docs/plan/01-goals.md` "Rules": "never rest in a room that spawns monsters" is a setting.

## Behaviour

- **Rests** (`health.rest_quiet_room`, off by default): off, Recover rests where it stands, a
  lair or an arena too; on, as before (the nearest quiet room within 5 steps), never a room the
  pack has no name for.
- **Players' houses.** The 34 exits into rooms the pack has no name for (map 15's houses) stay
  in the graph, so `Obvious exits` still fit, behind `Gate::Unnamed`: no route, rest or flee
  takes them.
- **A room the map lacks.** A room shown whole (title and exits) whose title no room has leaves
  the position unknown; the walk stops, naming the room ("in Shop White House Room, a room not
  on the map"); the next known room relocates from the rooms around the old ones. A title
  alone changes nothing, as before.
- **Failed legs pause.** After a loop leg that ends away from its stop, the next leg (to it or
  the next stop) waits `LEG_PAUSE`, 5 s, narrated "Loop X: did not get to stop N; trying again
  in a moment".
- **`par`.** A follower is out of the leader's room only when neither belief holds a room the
  other does (each member's candidates go to the coordinator with its best room).

## Done when

- Default settings: low HP in the Arena rests there.
- The belief goes unknown on "Shop White House Room" with its exits and relocates on the Bend.
- Three refused legs take two pauses at least; overlapping beliefs plan no `par`.

## Tests

- `tests/nav.rs`: the house door is `Gate::Unnamed`, untakeable; the belief on the house.
- Brain: resting in place by default; the quiet-room walk with the setting on (the existing
  tests, through `game::test_settings`); the leg pause; `party.rs` overlapping beliefs.
