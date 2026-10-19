---
id: C-024
title: Generated loops: circuits over an area's spawn rooms from the world data, listed and walkable once
tier: B
size: M
wave: 3
depends: [C-021, C-023, C-025]
crate: wayfarer-world, wayfarer-brain, ui/
touches:
  - crates/wayfarer-world/src/loops.rs (new)
  - crates/wayfarer-proto/src/ (`loops_request`, `loops`, `walk_loop`)
  - crates/wayfarer-brain/src/ (serving loops; walking one through the walker)
  - ui/src/MapPanel.svelte (a loops list), docs/
---

## Goal

The first of the three loop sources in `03-engine.md`: loops generated from the world data.
Given an area (an anchor room and a radius), the world crate finds the rooms that spawn
huntable monsters and builds a short closed circuit through them. The map panel lists the
generated loops for the focused character's surroundings, beside the verified MegaMud loops
the pack already holds, and "walk once" walks a loop through C-022's walker so John can
watch a route before any bot runs it.

## Inputs

- `docs/plan/03-engine.md` section 4 "Areas and loops" (three sources; MegaMud loops only
  when verified), `02-architecture.md` "World data" (the MegaMud rules).
- `kb/progression.md` (spawn pool: `genmonster` plus the room's `mongroup` within the
  `minlevel`..`maxlevel` genindex range when `maxmonsters` > 0; respawn timing: nothing for
  `regendelay` minutes after a kill, arenas exempt; lairs), `kb/game-data.md` (the room
  fields; `Room` in `pack.rs` already has `gen_monster`, `max_monsters`, `mon_group`,
  `regen_delay`, `room_type`).
- The prototype for the mechanics of a circuit: `mmud_data.py` `World.patrol` (l.902:
  nearest-neighbour order then 2-opt), `progress.py` `plan_ground` (an area is a BFS ball
  around an anchor).
- C-011 (`WorldPack::loops()`: MegaMud loops with their `Verification`), C-021 (`Graph`,
  `distances`, `route`), C-022 (the walker), C-023 (the panel).

## Behaviour

- **Hunt rooms.** `hunt_rooms(graph, pack, area) -> Vec<HuntRoom { key, monsters, is_arena,
  is_lair, regen }>`: the rooms in the BFS ball whose spawn pool (C-021's `spawn_pool`) has a
  huntable monster (C-021's `huntable`), with the monster numbers and names. A shop whose
  pool is an NPC is not a hunt room.
- **Loops.** `generate(graph, pack, area, traveller) -> Vec<Loop>`: a closed circuit through
  the hunt rooms (nearest neighbour then 2-opt, routed over the graph, doors allowed, no
  gated exits the traveller can't pass), as `Loop { name, rooms: Vec<key>, steps: Vec<Step>,
  length, spawns: n }`. One loop over all the area's spawn rooms, and one per connected
  cluster when the area splits (a river, a level exit). Names from the anchor's room name.
- **MegaMud loops** through the area: those of `pack.loops()` whose verified walk visits a
  room in the area, listed with their file name and verification; never an unverified one
  (the rule in `03-engine.md`).
- **Protocol**: `loops_request {session, anchor?, radius}` (anchor defaults to the believed
  room) → `loops {session, generated: [...], megamud: [...]}`; `walk_loop {session, id}` walks
  the loop once from the nearest room on it, through the walker (C-022), narrating each stop
  (`notice "Loop stop 3/7: Newhaven Arena"`), and ends where it started; `walk_stop` stops it.
- **Panel**: a "loops" section in the map panel with the two lists, the loop drawn on the map
  when hovered or selected, and `walk once`.

## Design notes

- `loops.rs` is pure over `&Graph` and `&WorldPack`; the brain calls it on request and
  caches by (anchor, radius, level).
- A loop's `steps` are route steps between consecutive rooms, so the walker needs nothing
  new: `walk_loop` is a sequence of `walk_to`s with a stop at each.

## Done when

- From the stock pack: the Newhaven area (anchor Newhaven, Village Entrance 1:2140, radius
  8; Newhaven is 14 rooms) yields a loop that visits every hunt room in it and closes, under
  40 steps; the arena (1:2150) is in it and flagged `is_arena`; the shops are not stops; the
  Lair north of the arena (Dungeon, Entrance 1:2152, behind a locked door) is a stop only if
  doors are allowed; a gated exit outside the character's level is not used.
- The MegaMud loops listed for that area are all verified ones.
- Live: `walk_loop` on a small generated loop (three or four stops) walks it and returns to
  the start (scenario added to the live check or run by hand with the summary in the PR).
- Panel: the lists show, hovering draws the loop, `walk once` walks it.

## Tests

- Loop generation and the MegaMud filter on the fixture pack; panel list rendering.

## Notes for the agent

- No bot runs loops yet; this is listing and a one-off walk. The Task mode (M3) will run
  them.
- Player-recorded loops are a later card.
