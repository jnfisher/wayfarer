---
id: C-011
title: wayfarer-world: the world pack (SQLite) built from the stock-world export, with MegaMud waypoints and verified loops
tier: B
size: M
wave: 2 (early: runs beside wave 1)
depends: [C-001]
crate: wayfarer-world, bin/wayfarer
touches:
  - crates/wayfarer-world/ (new: schema, builder, reader)
  - bin/wayfarer/src/ (the `import-world` subcommand)
  - assets/megamud/ (new: the MegaMud path set, with PROVENANCE.md)
  - tests/fixtures/world/ (read only)
---

## Goal

`wayfarer import-world` turns the stock-world export into one SQLite file, the **world pack**,
that every later part of the bot reads: rooms and exits, monsters, items, shops, spells,
classes, races, messages and text blocks, plus the MegaMud room id of every room, MegaMud's
named waypoints and its community loop library, each loop checked against the rooms. A small
reader API opens the pack. No routing or position tracking yet (M2).

## Inputs

- `docs/plan/02-architecture.md` "World data" (the pack, its sources, built at release time).
- `tests/fixtures/world/` (C-001): the export, one `.jsonl.gz` per table, with its README
  (line format, field names from `MAJORMUD.H`, `_id`, run-time fields to ignore) and
  `MANIFEST.json` (rows, record lengths, hashes).
- `mmud-exploration/test/play/mmud_data.py`: what the fields mean in practice: `EXITTYPES`
  (:32, the 25 exit types), `Room.links` (:78-92), `World.exit_ok` and `exit_command`
  (:797-816, Level and Text exits), `Monster.maxhit`, `coin_value`, `Spell.attack/area/heal/
  damage` (:93-210), `spawn_pool` (:940-959), `areas` (:960-985).
- `mmud-exploration/kb/game-data.md` (record layouts, enumerations, text blocks and how they
  are stored: each byte plus 0x20), `kb/navigation.md` "Exit types and how to take them",
  "Published paths and loops".
- `mmud-exploration/kb/megamud-formats.md`: the room id (sections 0), `.mp` files (1),
  `Rooms.md` (2), with the measured results to reproduce.
- `mmud-exploration/reference/megamud/MegaMUD_v2.0_Alpha_-_With_Paths.zip` and its parsers
  `reference/megamud/parsers/mp_parse.py`, `mp_walk.py` (the walk that confirmed the room id).
- `mmud-exploration/test/play/routecheck.py` (the room id in Python, already used for six
  loops).

## Behaviour

- `wayfarer import-world --export DIR [--megamud ZIP] --out PACK.sqlite`: reads the export (and
  the MegaMud zip, default `assets/megamud/`), writes a new pack, prints counts and the loop
  verification summary. Re-running gives the same content.
- **Schema** (version 1, in `pack_meta` with the source hashes from `MANIFEST.json` and the
  builder version):
  - `rooms` (map, room, name, light, the fields routing and danger will need: exits as rows
    below, spawn group fields, shop number, `mm_id`); `exits` (from map/room, direction 0-9,
    to map/room, exit type by name, its parameters as raw integers); `monsters`, `items`,
    `shops` (with stock), `spells`, `classes`, `races`, `messages` (WCCMSG), `texts` (WCCTEXT,
    decoded), `actions` (WCCACTS).
  - Every table also keeps the export's full record as a `raw` JSON column, so later cards can
    reach a field without a schema change. Run-time fields (`dirty`, `lastkill`, monsters in
    the room, …) are left out of the typed columns.
  - `mm_waypoints` (the 783 `Rooms.md` entries: id, flags as shop, bank, trainer, boss, level
    window, class, code, group, name, and the game rooms the id resolves to) and `mm_loops` /
    `mm_loop_steps` / `mm_paths` (the `.mp` files: header fields, steps with flags and
    `dir[actions]`), with a `verification` per path: walked to the end or not, steps whose id
    matches exactly, title-only matches, where it broke.
- **Room id**: `mm_id = ((Σ (i+1)·ord(title[i])) & 0xFFF) << 20 | exit bits`, two bits per
  exit in the order N, S, E, W, NE, NW, SE, SW, U, D: plain 01; Door, Gate and Key 10; Hidden,
  Text and Remote Action 00 (not shown); every other type 01 (`kb/megamud-formats.md` section
  0 has the measured table).
- **Loop walk**: start at the rooms whose `mm_id` equals the loop's start id, follow each step
  through the exits (a direction moves; a non-direction step resolves through the next step's
  id), and record the result. Ambiguous starts are tried each; the best walk is kept.
- **Reader**: `WorldPack::open(path)`; room by (map, room); exits of a room; rooms by name;
  rooms by `mm_id`; monster, item, spell, shop by number; loops with their verification. Reads
  only; opening is fast (no full load).

## Design notes

- `rusqlite` with the `bundled` feature (no system SQLite on users' machines). One
  transaction for the build.
- Keep the typed columns few and obvious now; anything uncertain stays in `raw` until a card
  needs it.
- `assets/megamud/`: copy the zip from mmud-exploration (1.6 MB) with `PROVENANCE.md`; the
  builder reads `Default/Rooms.md` and `Default/*.mp` from it directly (no unpacking to disk).

## Done when

- Building the pack from the fixtures: row counts equal `MANIFEST.json` for every content
  table (26,720 rooms, 1,950 items, 1,101 monsters, 1,379 spells, 178 shops, 3,867 messages,
  3,467 text blocks; the run-time tables noted, not checked).
- Spot checks as tests: room 1/1 is "Town Gates" with the exits and types the export holds;
  a known monster, item, shop and spell by number; a known text block decodes to readable
  text (`kb/game-data.md` "Text blocks and lookups").
- MegaMud results within a hair of the KB's: at least 99% of title-matched steps match the
  full `mm_id` exactly (KB: 99.2% of 43,685); at least 170 of the loops that walk to the end
  close in their start room (KB: 175 of 177); 783 waypoints, and at least 720 resolve to a
  game room (KB: 726).
- `wayfarer import-world` documented in the crate README with real output.
- The CI tests for this crate run in under ten seconds (building the whole pack is the slow
  one; say so in its comment), and the pack is not committed.

## Tests

- The checks above; `mm_id` unit tests against `routecheck.py`'s known ids; an `.mp` parser
  test per line shape in `kb/megamud-formats.md` section 1 (header, steps,
  `dir[actions]`, empty `[]`, `FFFFFFFF` wildcard).

## Notes for the agent

- The export is post-update content (`WCCUPDAT.DAT` applied): that is the stock world players
  meet, which is what we want.
- Building from a server's own `.DAT` files (custom worlds) is a later card, probably reusing
  silvermere's importer as a build-time tool (`02-architecture.md`); don't start it here.
- Write "KB notes" for anything the walk finds that `kb/megamud-formats.md` lacks.
