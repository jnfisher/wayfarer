# wayfarer-world

The **world pack**: one SQLite file with the stock world (rooms and exits, monsters, items,
shops, spells, classes, races, messages, text blocks, actions), optionally with MegaMud's
waypoints and path library, each path checked against the rooms (`docs/plan/02-architecture.md`
"World data"). MegaMud's data is not part of this repository: it comes from a player's own
MegaMud copy, or from `jnfisher/mmud-exploration`'s `reference/megamud/` when developing.
Every later part of the bot reads it. The crate also holds everything about *where* a
character is and how to get somewhere: the room graph, routes, the position belief, the map
layout and spawn pools ("Rooms in memory" below). Nothing here sends a command.

## Building a pack: `wayfarer import-world`

```
wayfarer import-world --export DIR [--megamud ZIP|DIR] --out PACK.sqlite
```

- `--export`: the stock-world export, one `<TABLE>.jsonl.gz` per table and `MANIFEST.json`
  (`tests/fixtures/world/`). Each file's SHA-256 must match the manifest.
- `--megamud` (optional): MegaMud's zip, or a directory holding exactly one zip.
  `Default/Rooms.md` and `Default/*.mp` are read straight from the zip. Without it the MegaMud
  tables are empty.
- `--out`: the pack. It is written to `PACK.sqlite.partial` and renamed at the end, so a failed
  build leaves no pack. Re-running gives a byte-identical file.

Real output (release build, about 1.5 s; a debug build about 2 s):

```
$ wayfarer import-world --export tests/fixtures/world \
    --megamud ../mmud-exploration/reference/megamud --out world.sqlite
tables (rows, MANIFEST.json rows):
  rooms       26720   26720  WCCMP001
  items        1950    1950  WCCITEMS
  monsters     1101    1101  WCCKNMSR
  spells       1379    1379  WCCSPELS
  classes        15      15  WCCCLASS
  races          13      13  WCCRACE
  shops         178     178  WCCSHOPS
  messages     3867    3867  WCCMSG
  texts        3467    3467  WCCTEXT
  actions        64      64  WCCACTS
  exits       62352          from rooms
  shop_stock   1203          from shops
run-time tables, not imported: WCCACMSR 795, WCCITOWN 37, WCCUSERS 0, WCCGANGS 0, WCCBANKS 0
megamud (MegaMUD_v2.0_Alpha_-_With_Paths.zip):
  waypoints 783: 726 resolve to a game room (584 to exactly one)
  paths 1957 (195 loops): 1781 walk to the end, 62 break, 114 have no start room
  loops walked to the end 183: 183 close in their start room
  steps checked 44384: title matches 44264, of which full id 44076 (99.6%); no match 120; wildcards 5
  distinct rooms visited 14969
wrote world.sqlite
```

The MegaMud check runs as an ignored test, outside CI:
`WAYFARER_MEGAMUD=../mmud-exploration/reference/megamud cargo nextest run -p wayfarer-world
--run-ignored only`.

The pack is about 80 MB, most of it the export's records kept as JSON. It is built, never
committed (`*.sqlite` is ignored).

## Schema (version 1)

The SQL is in `src/schema.rs`. `pack_meta` holds `schema_version`, `builder`, the export's date
and hashes (`source.<TABLE>.file_sha256`) and the MegaMud zip's SHA-256.

| Table | Rows | Keyed by |
|---|---|---|
| `rooms` | WCCMP001: name, light, type, shop, spawn fields, `mm_id` | map, room |
| `exits` | one per exit: direction 0-9, destination, type by name and code, `par1`-`par4` raw | from map, room, dir |
| `monsters`, `items`, `spells`, `classes`, `races` | a few typed columns each | number |
| `shops`, `shop_stock` | shop fields; stocked slots with their maximum quantity | number; shop, slot |
| `messages` | WCCMSG, its three texts | number |
| `texts` | WCCTEXT, one row per part, decoded | number, part |
| `actions` | WCCACTS | export `_id` |
| `mm_waypoints`, `mm_waypoint_rooms` | `Rooms.md`: flags as shop, bank, trainer, boss; level window, class, code, group, name; the rooms the id resolves to | code |
| `mm_paths`, `mm_path_steps` | every `.mp` file: header fields and its verification; steps with flags, `dir[actions]`, the room the walk was in and how the id matched | file; file, step |
| `mm_loops`, `mm_loop_steps` | views: the paths with FROMID = TOID | |

Every game table has a `raw` column with the export's record as written, so a later card can
reach any field without a schema change. Run-time fields (`dirty`, `lastkill`, monsters in the
room, shop quantities) are only there, never in typed columns. The run-time tables (active
monsters, item owners, users, gangs, banks) are not imported.

Notes on the typed columns:

- `exits.to_map` is the room's own map, except a Map Change exit with a non-zero parameter 1,
  which names the map (inferred, as in `mmud_data.py`).
- `texts.text` is the stored bytes up to the first stored 0x00 or 0x20, at most 1,999, each
  minus 0x20, as CP437 (`kb/game-data.md` "Text blocks and lookups").
  `WorldPack::text_block` joins a block's parts in order.

## MegaMud room ids and the walk

`mm_id = (title hash & 0xFFF) << 20 | exit bits`, two bits per exit in the order N, S, E, W, NE,
NW, SE, SW, U, D: Door, Gate and Key 10; Hidden, Text and Remote Action 00; every other type 01
(`kb/megamud-formats.md` section 0). The title hash is over the title's CP437 bytes.

Each path is walked from the rooms whose `mm_id` equals its first step's id (when none does,
from the rooms with the same title hash). A direction step moves through that exit, whatever
its type. Any other command (`go path`, `@party go portal`, `search`) stays, or moves to the
neighbour, or for a teleport the one room in the world, that the next step's id names (full id
before title hash). Of several starts, the best walk is kept: walks to the end, then most steps,
most exact ids, most title matches. `mm_paths` records the start, end, whether it walked to the
end and closed, the step counts by match, and where and why it broke.

## Reading a pack

```rust
let pack = wayfarer_world::WorldPack::open(Path::new("world.sqlite"))?;
let gates = pack.room(1, 1)?;            // Option<Room>
let exits = pack.exits(1, 1)?;           // Vec<Exit>
let rooms = pack.rooms_by_mm_id(0x4BA00010)?;
let orc = pack.monster(6)?;
let loops = pack.loops()?;               // Vec<MmPath>, each with its Verification
```

`pack.game_texts()?` gives the parser's game texts (`wayfarer_parse::GameTexts`: messages,
monster and item message numbers, spell cast texts, room names), equal to
`GameTexts::from_export` on the export the pack was built from. Each table is read in the
export's record order (`_id` in `raw`), which decides between equally good patterns; about
30 ms in a release build.

Also `rooms_by_name`, `room_raw`, `item`, `spell`, `shop` (with stock), `message`,
`text_block`, `waypoints`, `paths`, `path_steps` and `meta`. The pack is opened read only and
nothing is loaded up front.

## Rooms in memory: graph, routes, belief, layout, monsters

Pure data and functions over `&RoomGraph` and plain values; no I/O beyond loading from the pack,
no clock. The brain advances the belief and asks for routes; the map panel asks for a layout.

```rust
let graph = RoomGraph::load(&pack)?;     // every room and exit, ~50 ms in release
let t = Traveller { level: Some(2), ..Traveller::default() };
let r = route(&graph, (1, 2140), (1, 2150), &t);   // Some([w, w, d])
let mut b = Belief::unknown();
b.observe(&graph, Some("Docks"), Some(DirSet::from_names(["north", "south"]).unwrap()), false);
b.moved(&graph, "n");
let map = layout(&graph, b.best().unwrap(), 15);
let monsters = Monsters::load(&pack)?;
```

- **`RoomGraph`** (`nav.rs`): rooms in key order, edges in one flat list. Each `Edge` has the
  direction, the `ExitType` (the pack's names mapped once, at load; an unknown name fails the
  load), the command (`n`, or a Text exit's phrase from its WCCMSG record, all three texts in
  `phrases`), its `Gate` (level, class, race, alignment, item, toll, spell, cast, key, ability)
  and, for doors, gates and key doors, the pack's `LockState`. `Node::spawns` says the spawn pass
  makes monsters there. `listed_exits`/`exits_fit` say what `Obvious exits` can show: Text and
  Remote Action never, Trap, Spell Trap, Timed and Hidden maybe (a found Hidden exit shows).
- **`route`** (`route.rs`): Dijkstra, 1 per step, plus `Costs::spawn` (40) for entering a room
  that spawns (the destination exempt) and `Costs::door` (2) per door. A `Traveller` carries the
  level, class, race and evil points for gated exits (`None`: not checked), the exits refused
  this session (`blocked`), the Hidden exits `found`, and the `Category`s allowed beyond the
  default (Key, Spell, Item, Toll, Cast, Ability, every Hidden). Remote Action exits are never
  walked. `distances` is a breadth-first search to a radius.
- **`Belief`** (`belief.rs`): the candidate rooms, empty for unknown. `observe(title, exits,
  dark)` keeps the candidates with that name whose exits fit; from unknown it starts from every
  room of that name; when nothing fits it widens to 12 steps around the old candidates (name and
  exits, then name), then the world. No title: the exits narrow it. `moved(command)` follows the
  exit `command` takes from each candidate; `refused(command)` returns those exits for the
  `Traveller`'s `blocked` (a "no exit" refusal means the position is wrong: look and observe,
  `kb/pitfalls.md` 34). `best()` is the lowest key, `certain()` one candidate.
- **`layout`** (`layout.rs`): the center at (0, 0, level 0), each room one cell from the room it
  was reached from in its exit's direction (x east, y south, Up raises the level), breadth first.
  A room whose cell is taken comes back with `collides` and nothing is placed from it. Segments
  join placed rooms, a two-way pair once, with a kind (plain, door, hidden, text) and `one_way`.
  Within 15 steps of Newhaven, Village Entrance two rooms collide, both where the river meets the
  island: The Silver River, North of Silvermere (1:2363) on Newhaven, Docks, and Gravelly Beach
  (1:1816) on Newhaven, Forest Path.
- **`Monsters`** (`monsters.rs`): `by_name` (any case; leading words stripped until a name
  matches, so "thin giant rat" finds the giant rats), `iter`, `generated(room)` (what the spawn
  pass makes: `genmonster` alone, else the group within `minlevel..maxlevel`, only in Normal,
  Arena and Lair rooms, only when `maxmonsters` > 0 and never with `maxlevel` 0) and `spawn_pool(room)` (that plus the room's
  `specmon`); `huntable` is evil, not an NPC (`montype` 2) and worth exp; `hunt_room` is a pool
  with a huntable monster (`kb/progression.md`, `kb/combat.md`).

The graph has no state: doors relock at a cold start, found Hidden exits hide again after about
five minutes (`kb/navigation.md`); the brain's `Traveller` holds what it learns.
