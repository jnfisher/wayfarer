---
id: C-034
title: Go to: find a room, a place or a monster by typing part of its name, walk there, and stop one room short of a boss lair
tier: B
size: M
wave: 4
depends: [C-030]
crate: wayfarer-world, wayfarer-brain, wayfarer-proto, ui/
touches:
  - crates/wayfarer-world/src/{find.rs (new),monsters.rs} (search; boss rooms)
  - crates/wayfarer-brain/src/{session.rs,walker.rs} (`find`, `walk_to` with `stop_short`)
  - crates/wayfarer-proto/src/ (`find`, `found`, `walk_to.stop_short`), ui/src/MapPanel.svelte, ui/src/lib/keys.ts
  - docs/brain.md, docs/protocol.md, docs/web-ui.md
---

## Goal

John (2026-10-20): walking a character by hand across the world is not easy, and the map only
shows rooms within its radius, so Silvermere to the island (43 steps through the Docks skiff)
has no obvious click. A go-to box fixes it: type part of a name (`woo`, `chief`, `skali`) and
pick from what matches: rooms, well-known places, monsters and their lairs; Enter walks there.
And the MegaMud habit he liked: a walk to a **boss lair** stops one room short, so he can look
before going in. Only boss lairs, not every lair (John's decision).

## Inputs

- `docs/kb/megamud-manual.md` "Stop Before Entering Room" (stops outside when this room is the
  destination, e.g. a boss room), `docs/kb/progression.md` (spawn pools, unique monsters:
  `maxalive`, `regenhrs`), `docs/kb/pitfalls.md` 51 (looking into a dead boss's room brings it
  back).
- C-021 (`RoomGraph`, `route`, `monsters_by_name`, `spawn_pool`), C-011 (`WorldPack::waypoints`:
  MegaMud waypoints with `is_shop`, `is_bank`, `is_trainer`, `is_boss` flags and the rooms
  they resolve to; `rooms_by_name`; `shop`), C-022 (`walk_to`, the walker, `map_view`),
  C-023 (the map panel's drawer and walk bar), C-031 (`Alt+` keys in `keys.ts`).

## Behaviour

- **Boss lair** (`wayfarer-world`): a room is a boss lair when its spawn pool, or its `spec_mon`,
  holds a monster that spawns one at a time (`maxalive` 1, with `regenhrs` set: the Minotaur
  Chieftain, the champion), or when a MegaMud waypoint flagged boss resolves to it. The
  Chieftain's Large Wooden Room (1:1865) is one; a Wooden Passage room spawning plain
  minotaurs is not. `boss_lair(room) -> Option<Vec<Monster>>`.
- **Search** (`find.rs`, pure over the graph, the pack and the monsters): `find(query, from,
  traveller) -> Vec<Found>` for a query of two or more characters, case-insensitive, matching
  anywhere in the name; at most 30 results, best first:
  - rooms by name, grouped by exact name with the count and the nearest room by route length
    from `from` ("Wooden Passage, 42 rooms, nearest 43 steps"), each group expandable to its
    rooms;
  - well-known places: the MegaMud waypoints (name, kind: shop, bank, trainer, boss, other) and
    the pack's shops and training rooms by their room name;
  - monsters by name (`monsters_by_name` with adjective stripping), each with the rooms it
    spawns in, boss lairs marked ("minotaur chieftain, boss, lairs at Large Wooden Room
    1:1865, 46 steps").
  Unreachable rooms show "no route" and sort last.
- **Protocol**: `find {session, query}` → `found {session, query, results}` (`[{kind, name,
  rooms: [{map, room, name, steps?, boss}], monsters?}]`); `walk_to` gains `stop_short:
  Option<bool>` (absent: the default, which is on for a boss lair and off otherwise).
- **Walker**: with stop-short in effect and the destination a boss lair, the route is cut
  one room before it; arrival narrates "Stopped one room short of Large Wooden Room (boss:
  minotaur chieftain)"; `walking` carries `short_of` so the UI can say it. A destination
  that is not a boss lair is walked into as before.
- **Map panel**: a `go` box (`Alt+G`, and a `go` button) above the map: typing fills the
  list below it as you type (debounced `find`), arrow keys and Enter walk to the chosen
  row (`walk_to`), a boss lair row shows a ticked "stop short" box you can untick for that
  walk, Esc clears. A chosen row also centres the map on the room when it is in view.
- Keys table, help overlay and docs updated.

## Design notes

- The search runs in the brain (it has the graph and the monsters); results are small.
- Route lengths for ranking use `distances` from the character's room with the traveller's
  gates, capped at a radius (200 steps) so a query never walks the whole graph; beyond it
  "far" rather than a number.
- The walker's stop-short is a route cut, not a new mode: the destination becomes the
  previous room on the route, with the narration naming the real one.

## Done when

- World tests on the stock pack: `boss_lair` is some for 1:1865 (Chieftain) and 1:2124 (the
  champion's cavern) and none for 1:1839 (plain minotaurs) and the Newhaven Arena; `find`
  for `woo` lists Wooden Passage with 42 rooms, for `chief` the chieftain with his lair
  flagged boss, for `skali` the three Skali rooms as places, for `arena` the Arena with its
  steps; a two-letter query works and a one-letter one returns nothing.
- Brain tests (fake game): `find` answers; `walk_to` 1:1865 from the Wooden Passage stops in
  the neighbour with the narration and `short_of`; with `stop_short: false` it walks in;
  `walk_to` an ordinary lair walks in.
- **Live** (pool character, Newhaven): `find` for `arena` from the Village Entrance and a
  `walk_to` on the result arrives; `find` for `dungeon` and a stop-short walk to the Dungeon
  Entrance only if the pack marks it boss (it does not; say so in the PR) else skip the
  stop-short live step. Summary in the PR.
- Panel: the box, the list, Enter walks, the stop-short tick; `npm test`, `docs/web-ui.md`.

## Tests

- As above; `find` ranking as unit tests.

## Notes for the agent

- John's own characters (Waydeva, Waydevb, Thorgrim) are not yours to log in; the live
  check uses the pool character only.
- Keep the search cheap: build the name index once per brain.
