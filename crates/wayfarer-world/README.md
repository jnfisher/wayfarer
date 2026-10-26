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
- `--megamud` (optional): MegaMud's zip, a directory holding exactly one zip, or an unpacked
  copy (a directory with `Default/` in it). `Default/Rooms.md` and `Default/*.mp` are read
  straight from the zip or the directory; an unpacked copy may leave `Rooms.md` out (no
  waypoints: the test fixture `tests/fixtures/megamud` is nine paths alone). Without it the
  MegaMud tables are empty.
- `--out`: the pack. It is written to `PACK.sqlite.partial` and renamed at the end, so a failed
  build leaves no pack. Re-running gives a byte-identical file.

Real output (release build, about 1.5 s; a debug build about 2 s):

```
$ wayfarer import-world --export tests/fixtures/world \
    --megamud data/megamud --out world.sqlite
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
megamud (megamud):
  waypoints 783: 726 resolve to a game room (584 to exactly one)
  paths 1957 (195 loops): 1781 walk to the end, 62 break, 114 have no start room
  loops walked to the end 183: 183 close in their start room
  steps checked 44384: title matches 44264, of which full id 44076 (99.6%); no match 120; wildcards 5
  distinct rooms visited 14969
wrote world.sqlite
```

The MegaMud check runs as an ignored test, outside CI, against `data/megamud` (or
`$WAYFARER_MEGAMUD`): `cargo nextest run -p wayfarer-world --run-ignored only`.

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

## Rooms in memory: graph, routes, belief, layout, monsters, go to, loops

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
- **`route`** (`route.rs`): Dijkstra, 1 per step, plus `Costs::spawn` for entering a room that
  spawns (the destination exempt; 0 by default, so the shortest route wins), `Costs::door`
  (1, the `open`) per door, times `ceil(100 / odds)` for a locked door (`Costs::door_cost`),
  and `Costs::hidden` (2, the `search`) per Hidden exit not showing. A `Traveller` carries the
  level, class, race and evil points for gated exits (`None`: not checked), strength and
  picklocks (a locked Door or Gate opens on `Str + exitpar2` or `picklocks + exitpar2` out of
  100: `lock_odds`, `door_odds`; nil odds leave it out), the door states the game showed
  (`doors`, over the pack's), the exits refused this session (`blocked`), the Hidden exits
  showing (`found`) and known (`searchable`), and the `Category`s allowed beyond the
  default (Key, Spell, Item, Toll, Cast, Ability, every Hidden), and `no_doors` to take none.
  Remote Action exits are never walked. `distances` is a breadth-first search to a radius;
  `route_within` keeps a route inside a set of rooms.
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
  with a huntable monster (`kb/progression.md`, `kb/combat.md`). `is_boss` is a monster that
  spawns one at a time and comes back hours after its kill (`maxalive` 1, `regenhrs` set), not
  an NPC and worth exp; `boss_lair(room)` the bosses of a room's pool, if any.
- **`Finder`** (`find.rs`, "go to"): built once from the graph, the monsters, the pack's
  MegaMud waypoints and shop types. `find(graph, monsters, query, from, traveller)` matches
  two characters or more anywhere in a room name (one result per exact name), a place (a
  shop, bank or trainer room by its name, a waypoint) or a monster (with the rooms it spawns
  in; "thin giant rat" finds the giant rats); at most 30, those `from` has no way to last,
  then the better match, then the nearer (one breadth-first pass; `Reach::Steps` within 200,
  `Far` beyond, `NoRoute`), then places, monsters, room names. Each result lists its nearest
  20 rooms and its `count`. `boss_lair(key)` adds the waypoints flagged boss to
  `Monsters::boss_lair`: on the stock pack the Chieftain's Large Wooden Room (1:1865) and the
  champion's cavern (1:2124) are lairs, the Newhaven Arena and the Dungeon Entrance not.
- **`loops`** (`loops.rs`): an `Area` is the rooms within a radius of an anchor by exits the
  traveller may take; `hunt_rooms` are those whose pool has a huntable monster (with the
  monsters, arena, lair and `regendelay`). `generate` makes a closed circuit through them
  (nearest neighbour, then 2-opt, legs routed inside the area): one over all of them, and one
  per part when they don't all reach each other inside it (`Loop {name, rooms, steps, length,
  spawns}`). Newhaven, Village Entrance at radius 8: the Arena, the Dungeon Entrance and the
  Small Cavern, `n, n, s, s`. `megamud_loops` reads MegaMud's verified loops (walked to the end,
  closed) with the rooms each walk passed and the hunt rooms among them; `MmLoop::stops` is
  those in path order, each once (`Mwodloop.mp`: 41 Wooden Passages, never the Chieftain's
  room), `not_runnable` says "nothing to hunt on it" for a loop with none, `name` is its
  description or its file. `megamud_through` keeps those passing the area.
- **`PathKnowledge`** (`knowledge.rs`): for every step of a path whose walk reached its end,
  the room it was typed in and the exit it took (`PathExit`: direction, exit type, the step's
  actions, the phrase for a Text exit). `hidden_exits(file)` are the Hidden exits a path goes
  through (`Ftrehkep.mp`: north of the Hidden Keep, 9:724); `phrases(room, dir)` what paths
  type for a Text exit (`climb tree` at 9:735); `RoomGraph::learn_phrases` gives a Text exit
  whose message has no text the phrase a path types (none on the stock pack).

The graph has no state: doors relock at a cold start, found Hidden exits hide again after about
five minutes (`kb/navigation.md`); the brain's `Traveller` holds what it learns.

## Gear: the catalogue and monster sets

The data side of the gear planner (`gear.rs`), as `spellshop.rs` is for spells. Read once per
class and race and kept; the per-character part is cheap.

```rust
let cat = Catalogue::build(&pack, 2, 11)?;          // Witchunter, Kang: ~120 ms
let who = Wearer { level: 10, charm: Some(75), strength: Some(80) };
let items = cat.for_level(who, |rooms| nearest(rooms)); // Vec<GearItem>
```

- **Who may use it** (`user_can_use`, `kb/economy.md` "Carrying and wearing items"): each
  weapon and armour piece is `Now`, `Later` (only a Minimum Level or an alignment stands in
  the way: alignment depends on the legal level, which wayfarer does not track) or `Never`
  (the class or race lists, the weapon kind, the armour type, Magical against Anti-Magic, an
  off-hand item for a one-handed class, a Maximum Level passed), with every `Reason`. Being
  named in the class or race list overrides the weapon and armour types. A weapon above the
  character's Str is usable; `str_short` says by how much.
- **Where it comes from** (`Source`): the shops whose slot has `maxqty` above 0, with the price
  for the character's Charm (`buy_price`: cost in copper, the markup, then Charm) and the
  nearest shop room; the monsters that carry it, with the chance, HP, largest normal hit,
  boss and respawn hours, the rooms they are in (`genmonster` or `specmon` rooms, else those
  whose spawn pool holds them), and `first_kill` for a limited monster that has never died;
  the text blocks with `giveitem` (not a `random` table's `NN:` line) and the rooms or
  monsters they hang from (a room's command text, a monster's talk text, through links and
  talk answers); the rooms it is placed in. `sales` are the shops that buy it (any slot, not a
  gang shop) and what they pay (`sell_price`: cost × (Cha/2 + 25)%). Also `limited`,
  `deleted_at_cleanup` and `magical`.
- **Monster sets**: `loop_monsters` (a loop's stop rooms' spawn pools, each monster once with
  how many stops spawn it), `area_monsters` (the rooms of a name, or the name and a comma:
  "Slum Street" takes "Slum Street, Bend") and `named_monsters` (names, and those that match
  none).

A build reads the 26,720 rooms once, on a second read-only connection beside the monsters and
text blocks, and only the end of each room's record (where `cmdtext` and `placed` are).

## The combat model (`combat.rs`)

One character, one weapon and one monster: what each blow costs, how often it lands, crits or
is dodged, how much it does, how many blows land in each round, and the chance the monster is
dead after round 1, 2, ... for an attack, a bash and a smash. Solo play: no party damage, no
monster turns. It is the gear planner's engine; nothing in the brain uses it yet.

```rust
let fighter = Fighter::new(&Character { class: &witchunter, race: &human, level: 15,
    stats: Stats { str: 95, int: 50, agi: 56, cha: 50 }, weapon: Some(&magebane),
    worn: vec![], encumbrance: 55, max_energy: 1000, buffs: vec![] });
let monk = Defender::new(&dark_monk, &[]);           // + its weapon and worn item, if any
let fight = Fight::new(&fighter, &monk, Move::Bash);  // cost 632, blows 1, 2, 1, 2, 1, 2, 2, 1
fight.p_kill_by_round[1];                             // dead after round 2
```

- The formulas are the recovered source's, in C integer arithmetic: `move_player_to_fighter`,
  `move_monster_to_fighter`, `calculate_attack` and the auto-combat blow loop (LJNCOMBT.C),
  with the bonus totals of `update_dynamic_stats` (LJNABIL.C: the best Alter AV, the averaged
  Alter Energy Usage). `kb/combat.md` has them in words.
- Every roll includes its top value (`ROLL_TOP_INCLUDED`, John, 2026-10-26): a hit chance of
  34 lands 33 times in 100, the damage roll runs `min..=max+1`.
- The kill chances are exact: the damage dealt so far is a distribution over `0..maxhp`, and
  each blow (a miss, glance or dodge, a normal range and a crit range, ×3 or ×5) is folded
  in with a sliding window. A smash that does damage knocks the monster down (if
  `Str + level > its level + magicres + 30`) and the later smashes use its DV less 35.
- `expected_rounds` counts at most 20 rounds; `rounds_capped` says the fight may well run
  longer. A planner request (150 weapons × 3 moves × 10 monsters of 300 HP) takes about 0.2 s.
- Records come from the pack's `raw` JSON: `ItemRecord`, `MonsterRecord`, `ClassRecord`,
  `RaceRecord` (`from_raw`). Left out: casting, backstab, martial arts, Protective Shield,
  light and blindness, Quickness and Slowness, rank, the alignment defences.

