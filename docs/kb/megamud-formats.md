# MegaMud data and path formats

From the MegaMud v2.0 Alpha data set (`reference/megamud/`), decoded and checked against the 1.11p world data on 2026-10-17. Provenance in this file: **[C]** confirmed against the game data (numbers given), **[I]** inferred from strong patterns, **[G]** guess. Overview and client implications: `kb/megamud.md`. Parsers: `reference/megamud/parsers/`.

## 0. The room id ("room checksum"). This is the key to everything

Every room reference in every file (`.mp` headers and steps, `Rooms.md`, `Paths.md`) is a 32-bit id
written as 8 hex digits:

```
bits 31..20  title hash   = (sum over i of (i+1) * ord(title[i])) & 0xFFF   (i from 0, room title as displayed)
bits 19..0   exit bits    = 2 bits per direction, direction d at bits 2d..2d+1
             d: 0 N, 1 S, 2 E, 3 W, 4 NE, 5 NW, 6 SE, 7 SW, 8 U, 9 D   (MajorMUD's own exit order)
             code 00 = no visible exit, 01 = plain exit, 10 = door/gate, 11 = (rare, unknown)
FFFFFFFF     wildcard: unknown room, don't check (5 steps in 4 files)
```

How it was tested ([C]):
- **Title hash.** Of the 6,132 distinct ids in all `.mp` files, 6,089 (99.3%) have a 1.11p room
  with that 12-bit title hash. Chance would give about 34%, the share of the 4,096 hash values the
  game's 1,754 distinct room names occupy. Walking 1,835 paths step by step through real exits, the
  room the walker is in matches the step's title hash in **43,685 of 43,811 steps (99.7%)**. The
  misses come from version drift (2004 data vs 1.11p) and from the walker picking the wrong one of
  several identically named rooms.
- **Exit bits and direction order.** For steps whose title matched, recorded code by game exit
  type:

  | game exit type | 00 | 01 | 10 | so MegaMud shows it as |
  |---|---|---|---|---|
  | Normal | 318 | **94,442** | 18 | plain exit (01) |
  | Door | 19 | 7 | **2,560** | door (10) |
  | Gate | 8 | 1 | **260** | door (10) |
  | Key | 1 | 0 | **73** | door (10) |
  | Hidden | **984** | 15 | 8 | not shown (00) |
  | Text | **682** | 8 | 0 | not shown (00) |
  | Remote Action | **681** | 1 | 0 | not shown (00) |
  | Action, Map Change, Item, Block Guard, Class, Trap, Cast, Level, Alignment, Toll, Ability, Spell Trap, Ticket | few | **all the rest** | 0 | plain exit (01) |

  Code 01, 10 or 11 where the game room has no exit at all: 292 cases, which is version drift.
- **Full 32-bit id with this mapping:** **43,324 of 43,685 title-matched steps (99.2%) equal exactly**
  the id computed from the 1.11p room. 726 of 783 Rooms.md ids also resolve exactly to a game room
  (40 have no title match: renamed or added rooms such as the Black Wastelands vortexes, whose
  title hashes ED1 and ED2 don't exist in 1.11p).
- Dark rooms ([C]): the id still carries the full exit bits. Steps flagged dark (0x0001) match
  exactly as often as lit ones (3,627 of 3,652).

The id is a fingerprint, not a unique key ([C]). The 26,720 rooms produce 8,448 distinct ids, and
only 5,197 rooms have an id of their own. 288 hash buckets hold more than one distinct name. MegaMud
therefore can't locate itself from the id alone. It tracks position by following the path and uses
the id to confirm each step. A client should do the same: id plus previous position.
`routecheck.py` in the repo already implements this scheme. What's new here is the check over all
1,957 files and the exit-type mapping derived from data rather than assumed.

---

## 1. `.mp` path files (text, CRLF, `\x1a` EOF byte on about 70 files)

```
[Description][Author]                         line 1, always present (both may be empty: "[][]")
[CODE:Region:Start name]                      optional, copied from Rooms.md
[CODE:Region:End name]                        optional, absent for loops
FROMID:TOID:STEPS:-1:COST:ITEM:PREPATH:NEXTPATH
ROOMID:FLAGS:COMMAND                          one line per step
...
```

Bracket-line shapes seen ([C]): desc+start+end for point-to-point (1,622); desc+start for loops
(194); desc only (117 point-to-point files have no location lines); desc+start with no end on a
point-to-point path (23). The location brackets are cosmetic. 3,169 of 3,462 equal the Rooms.md
entry for that id; the rest are stale names. The client identifies endpoints by FROMID and TOID
[I].

### Header line (8 colon-separated fields; all 1,957 files have exactly 8) [C]
| # | field | evidence |
|---|---|---|
| 0 | FROMID: room id of the start room | equals the first step's id in 1,957 of 1,957 files [C] |
| 1 | TOID: room id of the end room; FROMID == TOID means a **loop** | the walker's end room matches TOID's title in 1,702 of 1,749 fully walked paths [C] |
| 2 | number of steps | equals the step-line count in 1,956 files (1 off by one) [C] |
| 3 | always `-1` | [C] value; meaning unknown [G: a "last position"/resume index] |
| 4 | 0 in 1,951 files; 5, 7, 10, 997, 9999 in six | [G] extra cost/weight for MegaMud's auto-router. 9999 is on `Shshdcws` (`ask seher activate`), 997 on she-dragon → lava tube, so it looks like "avoid unless needed" |
| 5 | required item, e.g. `rusty iron key`, `rope and grapple`, `black star key`, `ruby key*` (a trailing `*` seems to be a wildcard) | appears on paths that use that item [C] |
| 6 | prerequisite path (`.mp` file name). Examples: `DCTDCAS1` (needs moldy key) → `ARCHSLEZ.mp` (go to the shop that sells it); `Slmbbhdc` (black star key) → `SLMBLOOP.mp` (the key loop); `Blcodech` (bloodstone orb) → `BLCOGNCM.mp` | [I] some point at themselves (`Cry1cry2`→`CRY1CRY2.MP`) |
| 7 | next path to chain to when this one finishes: `Fng1loop`→`FNG2LOOP`→`FNG1LOOP`, `Fng5loop`→`SBNKSOVN` (to the bank), `Fng6loop`→`CPLNLOOP` | [I] only on 12 files, nearly all loops |

### Step lines `ROOMID:FLAGS:COMMAND` [C]
- ROOMID = id of the room **the command is typed in**, i.e. the room before the move. Confirmed
  by the walk.
- COMMAND: usually a direction (`n s e w ne nw se sw u d`; `up` and `down` appear too). Otherwise
  any text sent verbatim: `go path` (71), `sea` = search (62), `go hole`, `pull lever`,
  `@party go portal` (an MegaMud `@` remote command to the party), `give spider silk to dhelvanen`,
  `buy harm`/`sell harm` (cash conversion), `i`, `st`, `par`. 202 distinct non-direction commands.
  A non-direction step either stays in the room or moves through a Text exit or a teleport. The
  walker resolves these with the next step's id.
- `dir[action,action,...]` (375 steps): **actions to run if the move fails** (or before it), then
  retry the direction [I]. Seen: `[sea w]`/`[search sw]` (174, for hidden exits), `[use brass key n]`,
  `[pull lever]`, `[aim fork east]`, `[say gazmuldduhaz]`, `[n,pull lever,s]` (comma-separated
  multi-command), `[equip glowing red amulet, use glowing red amulet e]`, and `[]` (17, empty).
- FLAGS: a 16-bit hex bitmask; 0000 on 42,927 of 47,768 steps. A flag on a step isn't a fixed
  property of the room: 1,340 rooms appear with different flags in different paths.

  | bit | count | what it correlates with | reading |
  |---|---|---|---|
  | 0x0001 | 3,907 | 94% of flagged steps are in rooms with light ≤ -150 (dark), vs 28% of unflagged | room was **dark** when recorded [I, strong] |
  | 0x0008 | 46 (0009, 000B) | Labyrinth passages and Great Pyramid, light -999; **only 12 of 44 match the exit bits exactly** | dark maze: match by title only, exits unseen [I] |
  | 0x0080 | 614 | runs of consecutive steps (Blackwood Forest → Darkenlord, Quest hall); the FFFFFFFF wildcards sit inside such runs | [G] "don't stop/verify here", or a run-through section |
  | 0x0004 | 116 | runs through water: Black Moat, Shimmering Pool | [G] swimming/water (or "drop to the room below") section |
  | 0x0040 | 15 (+0x0044) | Magma River (Dragon Nest→Kotar), Abandoned Temple | [G] hazard-damage section |
  | 0x0200 | 18 (+0202/0203) | Ancient Ruin streets and gates | [G] |
  | 0x0400 | 45 (+0401/0402/0440) | jail cells, path ends and starts, key doors | [G] "wait/pause" or door-unlock step |
  | 0x0002 | 12 | Torchlit Tunnels, Great Pyramid | [G] |
  | 0x0010 | 5 | single steps in loops (BHBSLOOP, Lavmloop…) | [G] |
  | 0x0100 | 6 | every 6th step of NPPVLOOP/NPPVLOO2 | [G] (periodic rest/check point?) |

### How path files reference each other
- File name = 4-letter start code + 4-letter end code from Rooms.md (`DMGODMNP` = DMGO→DMNP), or
  code + `LOOP`/`LOO2`/`LOO3`… for loops. 1,923 of 1,957 file names resolve their first 4 letters to
  the FROMID through Rooms.md [C]. Case is mixed (`Slmwjagh.mp`, `DMGODMNP.mp`, `.MP`).
- **Paths.md is an index (cache) of all `.mp` headers**, one record per file keyed by file name
  (1,957 keys = the 1,957 files) [C]. MegaMud builds its route graph from it: nodes = Rooms.md ids,
  edges = point-to-point paths. Header fields 6 and 7 chain paths together (prerequisite, next).
- **v2.2 names carry the start room.** In the stock v2.2 set (`data/megamud/`, 2026-10-22) path
  descriptions, location brackets and Rooms.md waypoint names end in `-MAP ROOM`
  (`[Black Star Key Gather Loop-1 1224]`, `Ancient Ruin Dark Alley-3 606`); 171 of 1,957 `.mp`
  descriptions and 669 of 783 waypoints [C]. The ids are unchanged: the v2.2 set gives the same
  totals as v2.0 (783 waypoints, 726 resolved; 1,957 paths; 183 loops walked and closed) [C].
  wayfarer's importer drops the suffix from names.
- No path calls another path inline. Composition happens through the route graph and fields 6/7
  [I].

---

## 2. `Rooms.md`: named waypoints (text, 783 lines)

```
CAB00180:00004040:0:0:0:AALY:Ancient Ruin:Ancient Ruin Dark Alley
ROOMID  :FLAGS   :MINLV:MAXLV:CLASS:CODE:Group:Display name
```
| field | meaning | evidence |
|---|---|---|
| ROOMID | room id (section 0) | 726 of 783 resolve exactly to a 1.11p room, 584 of them uniquely [C]. The display name is user-written: AALY `Ancient Ruin Dark Alley` = game room 3:606 "Dark Alleyway, Dead End" |
| FLAGS | room-type bits: 0x02 **shop** (79: Armour Shop, Spell Shop, Temple Healer), 0x04 **bank** (5, all banks), 0x08 **trainer/guild** (32), 0x10 **boss/lair monster** (118: Beholder, Chimera, Prismatic Dragon…), 0x18 class "Spirit" trainers, 0x20 (12: Wastelands vortexes, NPP vortex, High Druid, Grey Lord, Horner) [G: special/teleport/NPC], 0x4040 (134: junctions like plazas, forks, obelisks, "Outside X") [G], 0xC040 sysop chamber, 0x8020 Grey Lord | shop/bank/trainer/lair [C] by name; others [G] |
| MINLV / MAXLV | level window, 0 = none. Newhaven rooms 0:3, Crypt 0:10, Wastelands and NPP 20/40/50, class trainers 0:9, Spirits 10:10, Elven trainer 20:49 | [C] by pattern (matches MajorMUD level gates such as Newhaven ≤ 3) |
| CLASS | class number for trainers (Warrior 1, Witchunter 2, Paladin 3, Cleric 4, Priest 5, Missionary 6, Ninja 7, Thief 8, Bard 9, Gypsy 10, Warlock 11, Mage 12, Druid 13, Ranger 14, Mystic 15) | [C] equals the WCCCLASS numbers and the Classes.md keys |
| CODE | unique 4-character code, used in path file names | 783 unique [C] |
| Group | area/region (36 groups: Scorching Desert 73, Darkwood Forest 51, Silvermere 43…) | [C] |
| Display name | free text | [C] |

781 of the 782 distinct path endpoint ids are Rooms.md entries [C]: Rooms.md is the node list of
the path graph.

---

## 3. `Messages.md`: status/event recognition table (text, 527 entries)

```
name:FLAGS:ACTION:command          header (name not unique; it's a label)
line 1                             "effect starts" text, or a one-off event text
line 2 (optional)                  "effect wears off" text
(blank line, usually)
```
Example: `blind:0001:0:` / `You are blind` / `You can see again`.
`acid hits:0000:0:` / `Acid burns {target} for {dmg} damage!`. Placeholders: `{target}` (179),
`{dmg}` (104), `{source}` (3) [C]. Purpose [I]: recognise spell/effect damage lines (count damage
done or taken) and track status effects on the player from their start and end lines, so the bot
knows when it is blind, held, poisoned and so on.

FLAGS (bitmask, [I] from the texts):

| bit | entries | effect class |
|---|---|---|
| 0x0001 | blind, black curse, globe of darkness | **blinded** |
| 0x0002 | confusion, hallucinations, mesmerized | **confused** |
| 0x0004 | poison, bee poison, mermex poison, teleport sickness | **poisoned/sick** |
| 0x0008 | bleeding, burning, disease, acid | **damage over time** |
| 0x0010 | entangle, net, web, constriction, fear, petrified legs | **held/can't move** |
| 0x0020 | (with 0x10) paralysis, sleep, stun | **can't act** |
| 0x0080 | regeneration, rapid healing | **regenerating** (good) |
| 0x0200 | mana flux, nature tap, arcane incantation | **mana/spell buff** |
| 0x1000 | petrification, stunned, unconscious | **incapacitated** |
| 0x0400 | (with 0x1000) shapeshifter/brain-eater transforms | monster changes |
| 0x2000 | trap damage, retching, "too afraid to do that" | **lost action/one-off negative** |
| 0x4000 | 78 `chase (...)` entries: `{target} slips into the dark alley.` with command `go alley` | **follow a fleeing monster** through a special exit; the 4th field is the command to type |
| 0x0000 | 322: plain damage messages and harmless buffs (alertness, angelic halo) | — |

ACTION (third field) [G]: 0 = just track (467); 1 = "monster/event appears", re-scan the room (39:
"A skeleton arises…", forest sounds); 2 = immobilising effect, wait (18: confusion, net, web);
5 = environmental damage, run the command (`desert damage` → `drink water`, swamp damage);
6 = alarm (`2 lives left` → `.LOW ON LIVES!^M=x^M`). In command strings a literal `^M` = Enter
[C]. `.text` is presumably a say/gossip and `=x` a MegaMud client command such as hang up or exit
[G].

---

## 4. `Macros.md`: keypad macros (text, 11 lines) [C]

```
104:0:0:0:n^M
KEY:M1:M2:M3:text
```
KEY = Windows virtual-key code: 96–105 = numpad 0–9, 110 = numpad decimal. The default set is the
keypad movement layout: 8 n, 2 s, 4 w, 6 e, 7 nw, 9 ne, 1 sw, 3 se, 0 u, . d, 5 `rest`. `^M` =
Enter. M1–M3 are always 0 [G: Shift/Ctrl/Alt modifiers].

---

## 5. MDB2 binary databases

### Container [C for record layout, I for page details]
- Header: `"MDB2"`, then small integers (record counts, page counts; not fully decoded). The rest
  of the first 1 KB is uninitialised buffer data, which is why Items.md shows class names there.
- **1024-byte pages** after the header. Page header (12 bytes): `u16 type` (0 = leaf/data,
  1 = index, 2 = one per file, probably free list), `u16 entry count`, `u16 bytes used/free`,
  `u16 0`, `i16 prev page`, `i16 next page`.
- Entries: `u8 len` (bytes that follow), `u8 0x01`, **key as an ASCII string + NUL** (the game record
  number in decimal, e.g. `"14"`; for Paths, the file name), 4 zero bytes, `u8 flag` (0x80 data
  record; Paths also has 0x81 27-byte all-zero secondary entries), then the fixed-size record.
  Leaf keys are sorted as strings ("14" < "15" < "2"). Index pages hold (key, child page) separators,
  so this is a B+tree.
- All integers are little-endian. Names are `char[30]` at body offset 2, NUL-padded with stale bytes
  after the NUL. `u16 number` at offset 0 equals the key.

`mdb2.py` reads every file: Classes 15 × 81 bytes, Races 13 × 87, Items 1,938 × 200,
Monsters 1,128 × 211, Spells 471 × 158 (player spells only; the game has 1,379), Paths 1,957 × 236.

### Field maps (offset within the record body; [C] = match rate against 1.11p records joined on number)
**Classes (81)** [C 15/15 for each]: 0 u16 number, 2 name[30], 32 u16 exp% (=100+game `exp`:
Mystic 250, Witchunter 130), 34 u8 combat, 35 u8 basehp, 36 u8 basehp+hplevel, 37 u8 weapon type,
38 u8 armour type, 39 u8 magelvl, 40 u8 magery, 41 u16 abilities[10], 61 i16 abilvals[10].

**Races (87)** [C 13/13]: 0 number, 2 name[30], 32 u8 min stats[6], 38 u8 max stats[6], **stat
order STR, AGI, INT, WIL, HEA, CHA** (differs from the game record order), 44 u8 hpbonus, 45 u16
exp%, 47 u16 abilities[10], 67 i16 abilvals[10].

**Items (200)** [C ≥ 0.94 unless noted; name equals game name 1,912/1,938]: 0 number, 2 name[30],
32 char[~59] **where to get it** (shop name, e.g. "Druid Spell Shop", "Blue Tower (Spells)"; empty for
704), 91 u8 type, 93 i16 weight, 103 u8 limit, 105 i16 charges, 107 u8 strreq, 109 u16 mindmg,
111 i16 maxdmg, 113 i8 accuracy, 117 i16 ac, 119 u8 dr, 121 u16 abilities[10], 141 i16
abilvals[10], 161 u8 wpntype, 162 u8 armtype, 163 u8 worn slot, 174 u8 classes[] (class
restrictions). Unidentified: 95–102 (cost/currency likely, e.g. grey robes: 0x64), 164–199.

**Monsters (211)** [C ≈ 0.99 per field; name 1,088/1,101]: 0 number, 2 name[30], 52 gender,
53 i16 hpregen, 57 i16 maxhp, 59 energy, 61 magicres, 63 followpct, 65 ac, 67 dr, 69 charmlvl,
71 montype, 73 grouprole, 74 alignment, 75 maxalive, 77 regenhrs, 83 i16 items[5], 93 u8
itempcts[5], 98–107 cash (5 denominations), 108 u16 abilities[5], 118 i16 abilvals[5], 128 weapon,
130 i16 spells[5], **148 u32 exp** (equals the game in 85%; the rest are multiples such as 15000 →
150000, so a different exp multiplier in that era), 152 spellpcts[5], 162 atkmins[5], 172
atkmaxs[5], 182 atkenergies[5], 192 deathspell, 194 createspell, 196 spelllvls[5], 201 undead,
202 hitspells, 208 charmres. 32–51 is mostly zero apart from a constant `40 04` [G: MegaMud flags].

**Spells (158)** [C ≈ 0.99; name 461/466]: 0 number, 2 name[30], **32 short name** (`mmis`, `trav`:
the cast mnemonic), 84 level, 85 maxinc, 86 mana, 88 energy, 90 minval, 92 maxval, 94 duration,
96 diff, 98 target, 100 element, 101 u16 abilities[10], 121 i16 abilvals[10], 141 levelcap,
142 maxlvls, 143 mininc, 144 durinc, 145 type.

**Paths (236)** [C: equals the `.mp` headers in about 98%; the rest are stale]: 0 description[61],
61 author[41], 102 file name[14], 116 required item[31], 147 prerequisite path[14],
161 next path[18], 179 u32 FROMID, 183 u32 TOID, 187 i32 cost (header field 4), 191 i32 -1
(field 3), 195 i32 step count, 199–235 zero. Mismatches against the files: from 34, to 31,
steps 40, item 15, author 11, desc 7, next 2, cost 1 (paths edited after indexing).

---

## 6. Statistics [C]

- **1,957 path files: 195 loops (FROMID == TOID), 1,762 point-to-point.**
- Loop length: 2–681 steps, median 78, mean 112 (21,743 steps). Point-to-point: 1–302 steps,
  median 8, mean 15 (26,025 steps). 47,768 step lines in all.
- Walked through 1.11p: 1,749 paths walk to the end, 94 break (the game data lacks an exit, or an
  `@party`/teleport command can't be resolved), 114 have no start room in 1.11p (Wastelands
  vortexes ED1xxxxx/ED2xxxxx, NPP vortex DD055500, gang houses). **175 of the 177 fully walked loops
  close**: they end in their start room.
- The walked paths visit **14,736 distinct game rooms** on maps 1, 2, 3, 6, 7, 8, 9, 10, 12, 15,
  16 and 17.
- **A stricter walk does better** (wayfarer's world pack, C-011, jnfisher/wayfarer#5) [C]: start
  only from rooms whose full id equals FROMID (falling back to the title hash for the 36 paths
  where no room has it), resolve a non-direction step by the next step's full id before its title
  hash, and count `FFFFFFFF` steps as wildcards. Then:
  - 1,781 paths walk to the end, 62 break, 114 have no start room (111 point-to-point, 3 loops);
  - **all 183 loops that walk to the end close** in their start room;
  - **no loop step waits or attacks** (wayfarer C-036, 2026-10-20) [C]: of the 183 loops' 19,535
    steps, 45 carry `[actions]` (32 searches, `sea x` or `search x`, for a Hidden exit; 10 `pull
    lever`; one each of `say gazmuldduhaz`, `push eye`, `show signet ring`) and 412 are not a
    direction (`go path` 29, `give ... to dhelvanen` 15, `pull lever` 10, `climb strand` 8, `go
    crack` 8, `climb tree` 4, `buy harm`/`sell harm` 3 each, `stat` 3, `rest` once, ...). No
    command waits or attacks, and the step flags have no confirmed meaning beyond darkness, so a
    loop pauses only where its rooms spawn: a task's stops are its hunt rooms. 7 loops pass no
    room that spawns anything worth hunting (Bosploop, LCDELOOP, Lcbaloop, SGFGLOOP, SGFSLOOP,
    SPWMLOOP, WFDPLOOP);
  - 44,076 of 44,264 title-matched steps (99.6%) equal the full id; 120 steps match not even the
    title hash; the walks visit 14,969 distinct rooms;
  - the 62 breaks: 41 have no exit in the direction taken (version drift); 21 have a
    non-direction command whose destination can't be pinned to one room (`@party go portal` 8,
    `get gold` 3, `@party go vortex` 3, and one each of `go portal`, `@party say tree`,
    `@party touch crystal`, `dive pool`, `dive well`, `@party go hole`,
    `@party ask dhelvanen ready`);
  - the 26,720 rooms have 62,352 exits with a non-zero destination; 8,448 distinct ids, 5,197
    rooms with an id of their own (as above).
- `routecheck.py` mapped Action exits to 00 until 2026-10-17; the measured table above says 01
  (139 rooms have Action exits). Fixed.
- Coverage by start-room group (paths / loops): Scorching Desert 184/19, Darkwood Forest 131/10,
  Silvermere 116/0, Arlysia 100/2, Blackwood Forest 93/11, Black House 73/17, Lost City 70/4,
  Black Wastelands 69/0, Dark-Elf City 61/8, Jungle 61/6, Northern Forests 60/1,
  Dragon's Teeth Hills 60/4, Khazarad 58/1, Lava Fields 58/10, Trainers 55/0, Barren Hills 54/12,
  Goblin caves 54/12, Rocky Trail 51/5, Volcano 48/9, Rhudaur 47/5, Dwarven Mines 47/12,
  Slums 44/8, Misty Valley 41/6, Black Fort 35/1, Black Caves 34/1, Ancient Ruin 33/2, Sewers 33/4,
  Ocean 33/4, Island 23/4, Graveyard 21/5, Gang Houses 21/0, Newhaven 21/3, Mossy Tunnels 20/1,
  Negative Power Plane 18/2, Gnome Village 16/0, Cracked Plains 13/6.
- Authors (from the description line): blank 839, Winterhawk 585, Fizban the Fabulous 203,
  Olorin 119, Spork ZSpoon 89, Merlin 27, Kitty & Wulfman 15, then a long tail.

## 7. What a client needs from this
1. Compute the room id from what's on screen: title hash plus visible exits (plain = 01,
   door/gate = 10, hidden/secret = 00). Compare it with the path step's id, and accept a title-only
   match in dark rooms (flag 0x0008).
2. A loader for `.mp` (header + steps, `dir[fallback actions]`) and for Rooms.md (waypoints, with
   shop/bank/trainer/lair flags and level windows). Point-to-point paths form a route graph over
   Rooms.md ids. Header fields 5, 6 and 7 supply item requirements, prerequisites and loop
   chaining.
3. Messages.md is a ready-made table of status-effect start and end texts with effect classes, plus
   "chase" texts and the commands that follow a fleeing monster.
4. MegaMud's MDB2 Items, Monsters and Spells hold nothing that isn't in the game data, apart from
   item **source shop names** and spell **short names**.
