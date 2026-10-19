---
id: C-021
title: `wayfarer-world`: the room graph, routing, the position belief, and the map layout
tier: B
size: L
wave: 3
depends: [C-011, C-015]
crate: wayfarer-world
touches:
  - crates/wayfarer-world/src/{nav.rs,route.rs,belief.rs,layout.rs,monsters.rs} (new; `walk.rs`
    already has a `Graph` for the MegaMud walk, so the room graph is `RoomGraph` in `nav.rs`)
  - crates/wayfarer-world/src/pack.rs (loading the graph), README.md
  - tests/fixtures/ (nothing new expected; the stock export is there)
---

## Goal

Everything about *where* a character is and how to get somewhere, as pure data and functions
with no I/O beyond opening the pack: the walkable room graph, shortest routes with the
commands to type, the position belief that the brain advances and collapses as rooms are
seen, and a grid layout of the rooms around a position for the map panel. The brain (C-022)
and the UI (C-023) consume it; nothing here sends a command.

## Inputs

- `docs/plan/03-engine.md` section 2 ("Position is a belief"), `02-architecture.md` "World
  data".
- `kb/navigation.md` (the exit type table: which types are walked by a direction, which need
  a phrase, which are gated by level, class, race, alignment, item, spell, toll; door lock
  states in `exitpar1`/`exitpar2`; hidden exits never listed; the cold-start relock),
  `kb/pitfalls.md` 2, 3, 34, 36, `kb/screen-parsing.md` and `kb/screen-patterns.md` section 3
  (titles are not unique: 1,754 names for 26,720 rooms; Silvermere's "Docks" is five rooms
  of one name, 1:27, 31, 32, 33 and 296; Text and Remote Action exits are never listed in
  `Obvious exits`, Hidden ones only once found, Trap and Timed ones conditionally),
  `kb/combat.md` (what makes a monster huntable: evil alignment, not an NPC, exp above 0).
- The prototype, as a reference for mechanics only: `mmud-exploration/test/play/mmud_data.py`
  `WALKABLE` (l.43), `World.path` (Dijkstra, l.827), `World.exit_command` (l.809),
  `World.patrol` (l.902); `progress.py` `fits`/`update` (l.257-310: candidates filtered by
  name and visible exits, widening when nothing fits).
- `crates/wayfarer-world/src/pack.rs` (`Room`, `Exit` with `dir`, `to_map`, `to_room`,
  `exit_type`, `params`), `src/walk.rs` (the MegaMud verification walk, which already resolves
  exits by direction; reuse its direction handling).

## Behaviour

- **Graph.** `Graph::load(&WorldPack)` reads every room and exit once (about 27k rooms, 62k
  exits, well under a second, in memory). Each edge carries the direction, the exit type, the
  command to type (`n` for a direction; the phrase for a Text exit, from the WCCMSG record's
  first text), the gates (level range, class, race, alignment, item, spell, toll) and whether
  it is a door or gate with a lock state. Hidden exits are edges flagged hidden (walkable
  only if found). Remote Action, Key, Spell, Item, Toll and Cast exits are not walkable by
  default; the caller can allow categories.
- **Routing.** `route(graph, from, to, &Traveller) -> Option<Route>`: Dijkstra, cost 1 per
  step, plus a configurable cost for entering a room that spawns monsters (default 40, the
  destination exempt), plus a cost for doors (default 2). `Traveller` carries the character's
  level, class, race and alignment for gated exits, the set of exits the game has refused this
  session (`blocked`), and the categories allowed. A `Route` is `Vec<Step { command, from,
  to, dir, exit_type, door: Option<LockState> }>`. `distances(from, radius)` is a BFS.
- **Belief.** `Belief` is a set of room keys (empty means unknown). `observe(title, exits,
  dark)` keeps the candidates whose name matches and whose listed exits equal the observed
  set, where a room's listed exits are its pack exits minus Hidden, Text and Remote Action
  (never shown), with Trap and Timed exits optional (match whether or not they appear); from an empty or unknown belief it starts from every room
  with that name. `moved(command)` advances every candidate along the matching edge (and
  keeps those with no such edge out). `refused(command)` marks the edge blocked for the
  candidates. When nothing fits, widen: rooms within 12 steps of the old candidates by name
  and exits, then by name alone, then the world (prototype `update`). `best()` is the lowest
  key among candidates, and `certain()` is true with one candidate. A title lost behind a
  prompt or replaced by "You hear movement" still advances on `Obvious exits` (KB).
- **Monsters and spawns.** `monsters_by_name(name) -> Vec<Monster>` (exact, and with
  leading adjectives stripped until a name matches), an iterator over every monster, and
  `spawn_pool(room) -> Vec<Monster>`: the room's `gen_monster` plus the monsters whose type
  equals the room's `mon_group` within its `min_level`..`max_level` when `max_monsters` > 0
  (`kb/progression.md`). `huntable(&Monster) -> bool`: evil alignment, not an NPC
  (`montype` 2), exp above 0 (`kb/combat.md`); a room with a huntable pool is a **hunt
  room**, the rest (shops with an NPC in the pool, say) are not. C-024 and C-025 use these.
- **Layout.** `layout(graph, center, radius) -> Vec<Placed { key, x, y, level }>` places the
  center at (0, 0) and each neighbour one cell in its exit's direction (N, S, E, W and the
  diagonals; up and down change `level`, which the panel shows one at a time), breadth first,
  stopping a branch where a cell is already taken (the room is still returned, flagged
  `collides`, so the panel can mark it). Exits between placed rooms come back as segments
  with their kind (plain, door, hidden, text, one way) for the panel to draw.

## Design notes

- Room keys are `(map, room)` as `pack.rs` uses them. The graph is `Vec`-indexed with a key
  map; edges in a flat `Vec` with per-node ranges.
- Keep every function pure over `&Graph` and plain values, so the brain can call them from
  its single task and tests need no clock.
- The exit type names are the pack's strings (`"Normal"`, `"Door"`, ...); map them to an enum
  once, at load, with a test that every name in the stock pack is known.

## Done when

- From the stock pack: a route from Newhaven, Village Entrance (1:2140) to the Newhaven
  Arena (1:2150) and back; a route through a Text exit (Newhaven, Docks 1:2149 to the Small
  Pier: `borrow skiff`, message 8564) comes out as that phrase; a Level-gated exit is skipped for a level outside its range and taken inside
  it; a blocked edge is avoided; the spawn-room cost steers around a Lair when a plain
  alternative is one step longer.
- Belief: starting unknown with title "River Street" and a known exit set gives the matching
  rooms; one move collapses it to one; Silvermere's "Docks" stays five until an exit set
  tells them apart; Newhaven, Docks (a Text exit south) matches on `Obvious exits: north`; a refused move marks the edge and `observe` after a look
  relocates (the pitfall 34 case, scripted from the `room-walk` capture).
- Layout: the rooms within 15 steps of Newhaven, Village Entrance place without a collision
  in the stock data (or every collision is listed in the PR), and the segments connect
  placed rooms.
- Monsters: the Newhaven Arena's pool is huntable; a Newhaven shop's pool (an NPC) is not;
  `monsters_by_name("thin giant rat")` finds the rat by stripping the adjective.
- Load time of the graph from the pack under 500 ms in release (number in the PR).

## Tests

- The scenarios above as unit tests against the pack built from `tests/fixtures/world` (the
  C-011 test already builds it; share the fixture builder).

## Notes for the agent

- Hidden exits found by `search` relock after about five minutes and doors relock at a cold
  start (KB); the graph has no state, the brain's `Traveller` does.
- Nothing here chooses where to hunt; "Where next?" is M4.
