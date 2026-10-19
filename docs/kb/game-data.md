# Game data a client can read

The module's data files are Btrieve: `WCC*.DAT`, created from the virgin `WCC*.VIR` on
install. MBBSEmu converts each one to SQLite `WCC*.db`; table `data_t(id, data BLOB, key_0)`
holds the raw record, so live changes such as shop stock are visible while the game runs.
Records are **byte-packed little-endian** structures, laid out in
`WCCMMUD-1.11p-source/SRC/MAJORMUD.H`. The decoder is `test/play/mmud_data.py`. [C]

| File | Record | Size | Key fields (offset) |
|---|---|---|---|
| WCCMP001 | room | 1528 | map 0x000, room 0x004, name 0x105 (53), exits[10] 0x338 (u32), exittypes[10] 0x360 (i16), exitpar1[10] 0x374 (u32), exitpar2[10] 0x39C, type 0x43C, shopnum 0x43E, minlevel/maxlevel 0x460/0x462 (genindex range), genmonster 0x464, **light 0x468**, maxmonsters 0x554 (u8), mongroup 0x557, **regendelay 0x5B3** (min, 0 = 5) |
| WCCKNMSR | known monster | 732 | number 0x000, name 0x036 (30), montype 0x054, level 0x056, genindex 0x05A, dr 0x064, ac 0x066, **exp 0x06F**, maxhp 0x073, energy 0x075, grouprole 0x0A5, **alignment 0x0A9**, cash[5] 0x100 (**runic→copper**), atktypes[5] 0x11E, atkpcts 0x12D, atkmins 0x132, atkmaxs 0x13C |
| WCCITEMS | item | 1061 | number 0x000, name 0x0AD, weight 0x2F2, type 0x2F4, abilities[20] 0x2F6, charges 0x31E, **cost 0x322**, classes[10] 0x324, mindmg/maxdmg/ac 0x33E/0x340/0x342, worn 0x398, dr 0x39B, strreq 0x39F, abilvals[20] 0x3DC, **currency 0x421 (0 copper … 4 runic)** |
| WCCSHOPS | shop | 500 | number 0x000, name 0x004, type 0x0CB, min/maxlevel 0x0CD/0x0CF (training), markup 0x0D1, trainclass 0x0D5, items[20] 0x0D7, maxqty[20] 0x127, **qty[20] 0x14F (live)** |
| WCCCLASS | class | 153 | number 0x000, name 0x002, basehp 0x020, hplevel 0x022, exp% 0x024, abilities[10] 0x02C, weapon 0x044, armour 0x046, abilvals[10] 0x04A |
| WCCRACE | race | 126 | number 0x000, name 0x002, mins int/wil/str/hea/agi/cha 0x020..0x02A, hpbonus 0x02C, abilities[10] 0x032, basecp 0x046, exp% 0x062, maxs (same order) 0x066.. |
| WCCMSG | message | 253 | number 0x000, msgs[3][82] 0x004 (Text-exit phrases, arrival/death/hit texts; a text may hold `a\|b\|c` alternatives, see "Text alternatives" below) |

## Btrieve files [C]
Read from the file control record of every file in `dist/WCCMMUD_1.11p_DOS_MBBSEmu_v2.zip` (page length +0x08, key
count +0x14, record length +0x16, physical length +0x18, record count +0x1A/+0x1C, key definitions from +0x118,
30 bytes each), and compared with the `opnbtv` calls (`LJNGAME.C:945-959`, `LJNUPDT.C:1998`). The `opnbtv`
record length of every file equals the header's, except `WCCTEXT` (below).

| File | Record (header) | Page | Key 0 (other keys) | Records in the shipped file |
|---|---|---|---|---|
| WCCMP001 | 1528 | 1536 | unsigned binary 8 at 0 (map and room as one segment) | 26,720 |
| NEWMP001 | 1544 (physical 1546) | 4096 | unsigned binary 8 | **0** |
| WCCKNMSR | 732 | 1536 | AutoInc 4 | 1,101 |
| WCCACMSR | 386 | 1536 | AutoInc 4 | 0 |
| WCCITEMS | 1061 | 1536 | AutoInc 4 | 1,950 |
| WCCITOWN | 65 (physical 73) | 1536 | AutoInc 4 at 0; key 1 unsigned binary 4 at 4 + Zstring 1 at 8 (segmented, duplicates) | 0 |
| WCCSHOPS | 500 | 512 | AutoInc 4 | 178 |
| WCCSPELS | 253 | 512 | AutoInc 2 | 1,379 |
| WCCCLASS / WCCRACE | 153 / 126 | 1536 / 512 | AutoInc 2 | 15 / 13 |
| WCCMSG | 253 | 512 | AutoInc 4 | 3,867 |
| WCCACTS | 1010 | 1024 | Zstring 30 at 0 (modifiable) | 64 |
| WCCTEXT | **22 fixed + variable tail** (buffer 2022) | 2048 | unsigned binary 18 at 0 | 3,467 |
| WCCGANGS | 253 (physical 261) | 512 | Zstring 20 at 0; key 1 unsigned binary 4 at 40, duplicates, descending | 0 |
| WCCBANKS | 72 (physical 80) | 512 | Zstring 30 + Integer 4 (one segmented key, duplicates) | 0 |
| WCCUSERS | 1998 (physical 2006) | 2048 | Zstring 30 at 0; Zstring at 30 (modifiable); unsigned binary 4 at 60, duplicates, descending | 0 |
| WCCUPDAT.DAT | **2015** (the game's struct is 2027) | 2048 | AutoInc 4 (`seqnum`) | 38,754 |

- **Sixteen `.VIR` files** ship (the fifteen data files and `NEWMP001.VIR`); `WCCUPDAT.DAT` ships as a `.DAT`, and
  `WCCUSERS.DAT` ships next to `WCCUSERS.VIR`, both empty. `WCCVACN` is not there: `WCCVACN.DAT` (record 1740) is an
  optional vacation file the DLL opens only if it exists (`LJNGAME.C:960-966`). The `.MDF` says "17 files, page
  size 2048"; the page length is not 2048 everywhere (table).
- **`WCCUSERS` (1998 bytes) holds two texts a decoder must not treat as bytes:** `gang` (`char[20]`, +0x6B9,
  `MAJORMUD.H:1228`) and `suicpw` (`char[9]`, +0x6D5, `:1236`). Other header offsets used elsewhere: room `lastkill`
  +0x559 (`:1375`), `foundmask` +0x55F (`:1377`), `areacount` +0x5B7 (`:1386`), `searchers` +0x5F6 (`:1396`),
  `nummonsters` +0x5F7 (`:1397`); item `active` +0x2F0 (`:1408`); shop `qty[]` +0x14F (`:1469`); monster `curalive`
  +0x0A3 (`:1521`), `killdate`/`killtime` +0x0AE/+0x0B0 (`:1528-1529`). [C]
- **`WCCUSERS` key 1 differs between the two shipped copies:** Zstring **11** at 30 in `WCCUSERS.VIR`, Zstring **30**
  at 30 in `WCCUSERS.DAT`.
- **No file has an alternate collating sequence** (no key carries the ACS flag, and the header names none), so no
  Btrieve key is case-insensitive at the file level; every case-insensitive lookup comes from the C code (see the
  offline name lookup under "Text blocks and lookups").
- **`WCCTEXT` is a variable-length file.** The header's record length is 22 and the physical length 26 (the 22-byte
  fixed part plus a 4-byte pointer to the variable tail); the DLL opens it with 2022 (`LJNGAME.C:957`), the size of
  `struct textblk` (22 + `text[2000]`, `MAJORMUD.H:1738-1744`), and every record reads back 2,022 bytes. The user-flag
  bit that says "variable" was not decoded; the lengths show it [C/I].
- **`WCCUPDAT.DAT` records are 2015 bytes, but `struct updrec` is 2027** (`seqnum` 4, `type` 1, `data[2022]`,
  `MAJORMUD.H:1747-1751`; the buffer is `sizeof(struct updrec)`, `LJNUPDT.C:1998`). An update record therefore carries
  at most 2,010 bytes of record image against 2,022 in the structure: only the text blocks are longer than that.
  `seqnum` runs 1 to 38,754 once each. By type: 13 races, 15 classes, 26,720 rooms, 178 shops, 1,950 items, 1,379
  spells, 1,101 monsters, 3,867 messages, 3,467 text blocks and 64 actions, which are the table sizes above.

## Enumerations (LJNTABLE.C, MAJORMUD.H)
- Room types: Normal, Shop, Arena, Lair, Hotel, Colliseum, Jail, Library.
- Exit types: Normal, Spell, Key, Item, Toll, Action, Hidden, Door, Map Change, Trap, Text,
  Gate, Remote Action, Class, Race, Level, Timed, Ticket, User Count, Block Guard, Alignment,
  Delay, Cast, Ability, Spell Trap.
- Item types: 0 armour, 1 weapon, 2 projectile, 3 special, 4 food, 5 drink, 6 light, 7 key,
  8 container, 9 scroll, 10 general.
- Currency codes (`CUR_*`): **0 copper, 1 silver, 2 gold, 3 platinum, 4 runic**. Every `cash[5]` array is
  stored the other way round, **runic first** (index 0 runic, 1 platinum, 2 gold, 3 silver, 4 copper): the monster
  record (`MAJORMUD.H:1535`, +0x100, "max runic .. copper"), the item record (`:1434`, +0x3AB), and the active monster
  (`:1586`, +0x0F0). [C] `MAJORMUD.H:404`
  - **Items:** `cash[]` is read only when a container is opened: `OPEN` adds `lngrnd(0, cash[i])` of each
    currency to the player's purse, index for index (`cash[0]` to runic, ... `cash[4]` to copper,
    `LJNCMDA.C:3515-3539`; whether the upper bound is exclusive is [I], `lngrnd` is not in the source).
    The stock `wooden box` (906) holds `[0,0,30,300,0]`, i.e. gold and silver only, about 30 and 300 at most. 275 non-container items
    (167 armour, 60 weapons, 46 specials, one scroll, one general item) hold large non-zero `cash` values such as
    `[1235881622, 475533122, 264441644, 1235881794, 244711445]` (grey robes 9, rigid leather tunic 12 to brigandine
    tunic 16, ...): leftover garbage that nothing reads. [C] `test/play/tests/fixtures/world` WCCITEMS
  - **Monsters:** a generated monster gets `lngrnd(0, kmon->cash[i] + 1)` of each currency, index for index
    (`LJNMONST.C:696-713`), and its death drops `cash[0]` into the room's runic pile first, then platinum, gold,
    silver, copper (`LJNMONST.C:1049-1062`; the runic line uses the configurable currency name `usrcurnm`).
    The giant rat (1, and its copies 109, 871, 908, 1113-1115) carries **no cash at all**. A cash example: the orc
    rogue (6, level 1) has raw `cash` `[0,0,0,10,20]`, read runic-first that is up to 10 silver and 20 copper. [C]
    WCCKNMSR fixture
- Status line modes (`player.statline`): 0 off, 1 on, 2 full are the header's `SL_OFF`/`SL_ON`/`SL_FULL`
  (`MAJORMUD.H:537-539`); **3 `SL_CUSTOM` and 4 `SL_FULLCUSTOM`** (the player's own text, up to `STATLINELEN` 60 characters)
  exist only in `LJNCMDA.C:131-133`. [C]
- Ability names: `abilnames[189]` holds 188 names (0 "No ability" to 187 "Meditate") and a NULL end marker
  (`LJNABIL.C:39-229`); every `ABL_` value is the index of its name. [C]
- Abilities used so far: 2 defence, 7 damage resistance, 13 user light, 21 poison immunity,
  28 Magical, 31 Bash, 32 Smash, 51 Anti Magic, 54 general light, 142 Hit Magical.
- Alignments: Good, Evil, Chaotic Evil, Neutral, Lawful Good, Neutral Evil, Lawful Evil.

## Spawn rule
See `progression.md`. If `maxmonsters > 0`: `genmonster` alone when set, else monsters with
`montype == mongroup` and `genindex` within `[minlevel, maxlevel]` (`maxlevel` 0: none); plus
the room's `specmon`.

## Text blocks and lookups [C]
- **A text block** is a run of WCCTEXT records with the same `number` and `part` 0, 1, 2, ... (`struct textblk`:
  `part` +0, 12 key bytes, `number` +0x0E, `link` +0x12, `text[2000]` +0x16, each byte stored +0x20,
  `MAJORMUD.H:1738-1744`). The stock file has 3,467 records for 3,267 blocks: 3,267 first parts and 200 continuations,
  the longest block having 62 parts; 645 records carry a non-zero `link`. [C] fixture
- **Where the text ends.** `decode_text_block` subtracts 0x20 from the stored bytes up to the first **stored** NUL and
  leaves the rest alone, and `get_text_block` forces `text[1999]` to NUL (`LJNMAP.C:1273`, `LJNBUFF.C:4430-4448`). The
  text is therefore the stored bytes up to the first stored 0x00 or 0x20 (a decoded NUL), at most 1,999 of them. In the
  stock file every block ends at a stored 0x00, none holds a stored 0x20 before it, and the longest text is 1,998
  bytes. [C] fixture
- **`display_long_text`** (`LJNDISP.C:5370-5460`) starts at part 0 and reads the next part with a "first record at or
  after (part, number)" lookup, stopping when the record found belongs to another number, when a part is missing or
  after 100 parts. Output is word-wrapped at 79 columns in page mode (22-line pages, prompt `Hit any key to continue...`),
  starts green, and a final `\r` is added if the last part did not end with one. A `%s` in the text takes the caller's
  argument. It returns the first non-zero `link` seen (the comment says the first part's), which callers that chain
  texts use as the next block number: `ask_monster_a_question` shows the monster's talk block's `link` with the monster's
  name, or `<name> doesn't understand you.` when the link is 0 (`LJNUTIL.C:10140-10160`). `link` is a pointer to another
  block, not a part chain.
- **Highest-number helpers** (`LJNBUFF.C:1340-1458`) read the last record by key 0: `get_last_item_number` is the only
  one with a cap (`MAXITEMNUM` 50000, so item numbers above it are never found by the name search); monsters, spells and
  active monsters have none; `get_last_room_number` returns the room number of the last (map, room) record, the highest
  room of the highest map, and nothing in the DLL calls it.
- **Offline player lookup by name** (gang commands such as PROMOTE, `LJNCMDA.C:6968-7004`): exact `acqbtv` first; on a
  miss the first letter is upper-cased and `agebtv` finds the name at or after it; if that is not `sameas` the name,
  `alebtv` takes the one at or before. `sameas` compares the **whole name ignoring case**, so this is a case repair and
  not an abbreviation match (the key itself is case-sensitive, see Btrieve files); a miss prints `Syntax: PROMOTE {user
  name}` (sic, the PROMOTE text is also used elsewhere).
- **`RF_SPECALIVE`** (room flag 0x08, `MAJORMUD.H:63`) is the one room-flag bit the game flips at run time: set when the
  room's specific monster is generated (`LJNMONST.C:821-822`), cleared when it dies (`:983`) and by the two start-up checks
  (`LJNBUFF.C:742-744, :822`); a specific monster is not generated while the flag is set (`LJNMONST.C:583`). Every other
  flag bit is design data. A room with a `specmon` whose flag is clear also regenerates it when a player moves into the
  room or LOOKs into it from next door (`user_allowed_in_room`, `LJNUTIL.C:2525-2533`), so a dead boss is back as
  soon as someone looks.
- **Text alternatives.** `get_component_string` picks one alternative out of an `a|b|c` text (used for the combat
  verbs in WCCMSG and item hit/miss texts, `LJNSUPP.C:1713-1790`): at most `MAXALTS` 5 alternatives count, so any text
  after the fifth `|` is unreachable; each alternative is cut at `COMPSIZ - 1` = 39 characters. The same pick number is
  passed on to the second and third form of the verb (`punch|punches|punches`), so the three forms must have the
  same number of alternatives. (`LJNSUPP.C:30-31`.)
- **Colours.** `NOCOLOR` is the empty string (`LJNGLOB2.C:40-49`; written `"\0"` only because the compiler pools duplicate
  literals).

## Stock data notes [C]
Checked on `test/play/tests/fixtures/world` (the exports) unless another source is named.
- **Empty room slots.** `EMPTY_ROOM_ITEM` and `EMPTY_ROOM_HIDDEN` write `uses` -1 (`LJNGAME.C:155-164`). In the stock
  rooms 332 hold -2 on empty hidden slots (room 1:1, slots 0 to 8). Nothing reads the `uses` of an empty slot.
- **Monster spell slots.** The after-round pick takes the first slot whose `spellpcts[i]` beats the roll and does not
  look at `spells[i]`, but the whole pick runs only when `spells[0]` is set (`LJNMONST.C:5093-5104`). Stock consequences:
  four slots carry a level and no spell (huge gruesome creation 191 slots 2 to 4, canimera 883 slot 1) and are never
  picked because their chance is 0; three monsters have `spells[0] == 0` and spells in later slots that therefore
  never cast after a round: vampire elder 835, darken beast 1031 and fallen angel 1035.
- **Specific monsters** (`specmon`, WCCMP001): mummy 31 in 1:1309, troll 82 in 1:2224, witchunter spirit 85 in 1:2236,
  boatman 61 in 1:1813; drops are in `test/play/notes/quests.md` §1 (mummy 31 carries the bone key 177 at 100%
  and the four rings 161 to 164 at 1% each, troll 82 the iron key 338 at 100%). The class weapon of the Large Tombs
  comes from the `placed` list (1:2236 places the mithril cutlass 342).
- **The Temple's ambient lines.** Twelve rooms of map 1 (the Temple rooms 1:520-523 and 1:525-527 with the Spell Store, Chapel and healer,
  the Stone Street rooms 1:237 and 1:240, the crossing 1:324, the Temple Foyer 1:2316 and the Halls of the Dead temple 1:2189) have room spell 925 `temple noise`, whose abilities are Trigger Text Block (148) with text block 9127 and a blank
  Descriptive Message (115, message 66). Text block 9127 is `random 9128`, and 9128 holds the lines `97:addexp 0`,
  `98:message 2676`, `99:message 2678`, `100:message 2677`. A random block rolls `genrdn(0,100)` (0 to 99 [I], the source comment says 0 to 100) and runs the first line whose
  number is above the roll (`LJNSPEC.C:1932-2010`), so each room-spell cast (every other medium pass, 6 s, for a
  player standing there) shows nothing 97% of the time [I] and otherwise one of three lines, 1% each, only to that
  player (`show_action_message` text 0, `LJNSPEC.C:74-95`; `LJNROOMC.C:241-243`): WCCMSG 2676 `The smell of incense
  wafts faintly in the air.`, 2677 `The angelic sound of a choir floats down through the air.`, 2678 `The large temple
  bell clangs loudly, echoing the time of day.` Not a game event, a client can ignore the three lines.

## Start-up passes (LJNBUFF.C) [C]
- **Placed items** (`:883-938`), on a cold start only, not on a SYSOP restart of the buffers (`if (!reinit)`, `:871`): each placed item of a room that is not lying there (visible or
  hidden) gets a new copy with its normal charges: in view if it has Visible Placed Item or is
  gettable, hidden otherwise. This keeps fixed items such as the class quest weapons.
- **Specific monsters** (`:741-840`): RF_SPECALIVE is cleared when the room has no `specmon`, or when
  no active copy of it (generated in that room) is found. A room whose specific monster is not
  alive then gets `generate_monster(room, map, 0, specmon, 0, 0x7FFF, -1, 0, 0)`. Lairs and
  shops are filled after that, until generation fails or 16 are made (`:844-862`).
- **Which rooms the pass visits.** `preload_and_generate_buffers` tries room numbers in turn from map 1 room 1, with
  no list of rooms. The miss counters are never reset when a room is found (the comment says "in a row", the code
  does not): a map ends at its **50th missing room number in total** (`MAXMISSROOMS`) and the scan ends at the
  **100th empty map** in total (`MAXMISSMAPS`) (`LJNBUFF.C:57-59, :672-689, :1122-1127`). Replaying that walk over the
  stock rooms reaches all 26,720 of them (last map tried 118). [C] `WCCMP001.VIR` decoded from the zip
- **The 16 never happens.** A lair or shop fill makes at most 15 monsters, because a room lists at most 15
  (`NROOMMONS`, `MAJORMUD.H:44`); the sixteenth `generate_monster` fails (`LJNBUFF.C:849-863`).
- **Specific-monster search quirks.** It stops at the first copy found (outside recovery, where it searches on and
  kills every duplicate), looks only at active-monster numbers up to the highest at the start of the cycle (capped at
  `MAXACTMONS` 40,000, `LJNBUFF.C:485-493`), and misses copies numbered above 32767 because the number is cut to 16
  bits (`(int)actcur`, `:782`).
- **Cold start versus SYSOP restart.** The reset of `lastkill`, `foundmask` and `searchers` (`:873-881`), the
  placed-item pass and the **exit reset** all sit inside `if (!reinit)` (`:871`, closing at `:1113`). A SYSOP
  restart of the buffers (`reinit`, `:350-389`) runs the specific-monster and the lair/shop passes but none of
  those. The exit reset (`:947-1112`) relocks every Door and Gate exit (`exitpar1` 2), every Key exit
  (`exitpar2` 2), re-arms Traps and hides Hidden exits again. The stock `WCCMP001.VIR` holds 138 Door/Gate exits
  in 112 rooms whose `exitpar1` is not 2 (mostly 0). [C] decoded from the zip. The committed export (a MBBSEmu `.db`
  after one start) differs from it in ten of those exits only, in rooms 1:1169, 1:1325, 1:1355, 1:1790, 1:2123, 1:2138,
  1:2153, 1:2442, 1:2515 and 1:2516, now locked; the other 128 exits in 102 rooms (e.g. 1:3439-3463, maps 12 and 17)
  are still unlocked, so a copy of the live files is not a reliable view of the relocked state; why is not known [I].

## Developer code and developer SYSOP commands [C]
- `twinkcode` is 78 (`LJNCMDB.C:1438-1441`). DEBUG (`:1529-1530`, which also needs `devlevel == 1`), MONSTER (`:1559`), SUMMON (`:1754`),
  EXPRESS_TRAIN (`:1783`) and TWEAK (`:3059-3060`, which also needs `devlevel == 1`) work only for a
  SYSOP holding the key while `codectwinks` equals it. `codectwinks` comes from `CODEC_OF_TWINKS=`
  in `wccmmud.ini` in the BBS folder (`LJNLOG.C:24, 393-394`; undocumented). `devmode` (eight
  developer registration numbers plus the file `DREAMUDS.WCC`, `LJNGAME.C:843-882`) is not needed.
- `SYSOP MONSTER [number [room [map [minlevel [maxlevel]]]]]` generates with force=1, ignoring the
  room's spawn rules (`LJNCMDB.C:1553-1600`; not in demo mode).
- `SYSOP SUMMON <name>` looks the name up among the items first, then the spells; it prints
  `<name> conjured.` and sets `immortal=1` (`LJNCMDB.C:1748-1776`). The `conjured` line has no
  row in `screen-patterns.md` yet.
- `SYSOP EXPRESS_TRAIN <room> [map]` moves the operator and prints nothing (`:1778-1811`).
  Bot policy is unchanged: no room-number teleports. (silvermere T-057)

## Configuration
- `WCCMMUD.MSG` holds sysop options: `ACTIVATE` code, `SYSOPKEY wccsysop`, `PLAYKEY USER`,
  `DEMOLVL`, `DATADIR`, `INACTIVE`, credit charges, and so on. [C]
- **The shipped values are what the game ran with**, because `reload__wccmmud` reads the options at start-up and
  overrides the module defaults (`LJNGLOBL.C`). The ones that differ from the defaults: `SECOTH 160` (default 60;
  `WCCMMUD.MSG:124`), `XTRALVL 2` (1; `:261`), `DEATHHP -200` (-10; `:255`), `PUNHUP NONE` (HIGH; `:272`),
  `INVGLO1-3 ;x ;- ;re` (`:278-290`), `EVILKEEP 0` (100; `:343`), `LVLDIFF 9` (10; `:407`), `MINHUPHP`/`MAXHUPHP`/`MAXHUPIT`
  0/0/0 (25/50/100; `:413-424`), `INACTIVE 900` (3600; `:431`), `KEEPEXP NO` (yes; `:452`), `GENDERGO 0` (-1; `:467`),
  `SECBUFF 1200` (60; `:131`). [C] `LJNGAME.C:520-545, :748-767`
- **Rules that depend on them.** `TOURNAMENT_PLAY=on` in `wccmmud.ini` (read once, only the first time the file is
  read, so only a BBS restart changes it) forces `enablawf` 0 and `lvldiff` 100 (`LJNGAME.C:529-535`, `LJNLOG.C:225-240`).
  `SECOTH` is rounded down to whole minutes with a floor of 60 (`LJNGAME.C:762-767`). The `deathhp > 2000` clamp can
  never fire, since the option's range is -20000 to 0 (`:748`). [C]
- **`.MSG` files.** `WCCTEXT.MSG` has 8 entries; its header skips number 3 (`WCCTEXT.H`: 1, 2, 4 to 9). In
  `WCCMMHLP.MSG` a help topic is a pair of messages read by ordinal: the topic string is message `n`, its body `n + 1`,
  so a body can carry a different name than its topic (`HL1T60` is message 144, `HL1B61` is 145). The file has 131
  topic strings (messages 2, 4, ... 262) and 263 defines, the last topic being the end marker `Help ?`; at most 1000
  pairs are searched (`LJNHELP.C:27-30, :118, :158-219`, `WCCMMHLP.H`). The files are stored with CR LF line ends; the
  captured main menu and greeting keep a line feed before a space or an escape sequence, so a reader that drops every
  LF except before a letter, digit, `%`, `(` or `"` (MBBSEmu's rule) does not reproduce the original bytes [I:
  compared against captures; the kernel's `prfmsg` is not in the recovered source].

## Full-record export
Every field of every table, not only the ones above, is decoded in `RECORDS` in `mmud_data.py`
(offsets from the `+0x` comments of `MAJORMUD.H`, checked to tile each struct exactly) and
exported as JSON lines to `test/play/tests/fixtures/world/` (format in its `README.md`; run
`python3 mmud_data.py export --data DIR --out DIR`). [C] Checked on the stock data (post
`WCCUPDAT`): 26,720 rooms, 1,754 distinct names, 11,122 darker than -150; room types Normal
23,274 / Shop 193 / Arena 3 / Lair 3,126 / Hotel 4 / Colliseum 118 / Jail 2; every `WCCTEXT`
record holds exactly 2,000 text bytes at +0x16; FCR record counts equal the `.db` row counts for
all ten stock tables. Newly decoded record fields include room `desc[7][71]` +0x13A, `mapfile`
+0x32B, `exitpar3s/4s` +0x3B0/+0x3D8, `items/hiditems/coins`, `flags` +0x55B, `deathroom/huproom`,
`cmdtext` +0x5AB, `areamax`, `ctlroom`, `specmon`, `placed`, `protmsg`, `picktrap`, `roomspell`;
every item, monster, spell, class, race, message, action, shop and text field. [C]
Walking the exported exits from Newhaven 1:2140 with `WALKABLE` reaches **7,745** rooms, not the
7,369 in `navigation.md`; the earlier figure could not be reproduced (see the changelog). [C]

Record order: the stock `WCCMSG`, `WCCKNMSR` and `WCCSPELS` files are **not** in `number` order;
each exported record's `_id` is its position in the file. Anything that rebuilds order-sensitive
data from a copy keyed by number (wayfarer's world pack rebuilding `patterns.py`'s registry, whose
tie-breaks follow record order) must sort by `_id`, not `number`, to match the export. [C]
(wayfarer C-015, 2026-10-19)

## Database update (`WCCUPDAT.DAT`)
- The DLL applies `WCCUPDAT.DAT` at every start: `check_begin_updating("wccupdat.dat")`
  (LJNBUFF.C:520-521; without the file `No database update found - Game not started` and nobody
  may play), then `automatic_update_polling_routine` (LJNUPDT.C:1896) applies one record per poll
  in ascending `seqnum` (GETGT on key 0 from `updseq` 0, LJNUPDT.C:1906-1909). Each record is
  `struct updrec` (MAJORMUD.H:1747): `seqnum`, `type` (`UPD_RACE 1 .. UPD_ACTION 10`) and a
  record image, merged into the existing record by its kind's `apply_*_update` or inserted when
  the key is new (`add_*_from_update`, LJNUPDT.C:1393-1600). Per kind (LJNUPDT.C): races and
  classes copy the whole record when a listed member differs (:197, :266); rooms copy only the
  design members, keep run-time state (monsters, floor items, coins, `lastkill`, `foundmask`,
  `areacount`), relock keys, doors and gates and add non-gettable hidden items to a free slot
  (:353-676); shops merge slot by slot keeping `qty`, and gang shops are left alone (:717-804,
  :1674-1680); items keep `number` and `active` (:838-996); spells are replaced on any byte
  (:1024-1031); monsters keep `number`, `curalive`, `killdate`, `killtime` (:1068-1259);
  messages copy the three texts (:1285-1297); actions are replaced unless `targets` has
  `AT_KEEP` (:1325-1333); text blocks are replaced when any of the 2022 bytes differs
  (:1358-1367). [C]
- **Order and edge cases.** Records are taken in ascending `seqnum` by GETGT on key 0 starting from `updseq` 0
  (`LJNUPDT.C:1906-1909`, `:2060`), so a record with `seqnum` 0 is never applied. A record whose `type` is outside 1
  to 10 is ignored without a word (`:1790-1792`), and a text block missing from WCCTEXT is inserted with
  `strlen(text) + 23` bytes (`TEXTOVERHEAD`, `:99`, `:1561`). The text lookup is a "first record at or after
  (part, number)" read on the 18-byte key 0, then a check that number and part both equal the update's
  (`:1743-1757`), through a buffer taken from `allocate_text_block`'s free list. An existing action whose `targets`
  has `AT_KEEP` (0x40, `MAJORMUD.H:78`) is not replaced (`:1330`); an existing gang shop is skipped and logged
  `AUTO UPDATE : skipping GANG SHOP.` (`:1674-1680`). [C]
- `NEWMP001.VIR` is never used: the DLL opens rooms only from `WCCMP001.DAT` (`roombb`,
  LJNGAME.C:948; the files it opens are LJNGAME.C:945-962 plus `WCCUPDAT.DAT`, the 17 of
  `WCCMMUD.MDF`), and the update looks rooms up in that cache and inserts new ones there
  (LJNUPDT.C:1656-1669, :1433-1441). It is not patched and is not a "new map 1";
  `WCCMMUD.RLN`'s install list and `WCCMISC.BAT` do not mention it, and the stock file is empty (0 records, page 4096,
  record length 1544, a version-6 file, unlike the others). [C]
- The stock stream (record count, length, `seqnum` 1 to 38,754 once each and the per-type counts checked on the file in
  the zip; the 26,720 room images checked byte for byte against `WCCMP001.VIR`; the other kinds measured in a port's tests, not read from the source): 38,754 records of 2015 bytes (the game's buffer is 2027),
  one image of every stock record (13 races, 15 classes, 26,720 rooms, 178 shops, 1,950 items, 1,379 spells, 1,101
  monsters, 3,867 messages, 3,467 text blocks, 64 actions) and nothing new; each image equals
  its virgin `.VIR` record byte for byte except the text blocks, whose 2022-byte image holds only
  2010 bytes. Applying it with the rules above (as measured in that port) changes
  almost nothing: 4 doors relocked (`exitpar1s` 2 in rooms the merge rewrites,
  LJNUPDT.C:545-552) and 21 gang shops left alone. [C]
- The 110 text blocks whose text runs past `text[1988]`: the image a 2015-byte record carries
  does not hold those bytes. Read literally, `apply_text_update` compares all 2022 bytes and
  writes the update record over the block (`memcmp` then `dupdbtv`, LJNUPDT.C:1364-1367), which
  would shorten them; the source leaves open how many bytes `dupdbtv` writes on the
  variable-length `WCCTEXT` (LJNUPDT.C:1352-1356), and the T-006 export (MBBSEmu, after one
  start) holds them unshortened. So whether they end up shortened is unknown; the MBBSEmu export suggests the stored block keeps
  its full text. [I]

## Monster type 37 ("Angel") [C]

The game's monster-type table names type 37 "Angel" (LJNTABLE.C:160; `MTYPE_ANGEL`, MAJORMUD.H:449), but no
celestial angel exists in the stock data. The 32 type-37 monsters in WCCKNMSR are mostly the ones that come
after players or serve as special encounters: guardsmen (13, 396, 426, 538, 905), the bounty hunter (32),
the warlock mercenary (70), greater hellion (181), demon imps (1033, 1052, 1088), cloaked figures
(1059-1062), Zanthus the Lich (215), the ancient sand dragon (434), Kai Master (448), Lorzina (1032), Nahr
(1084) and others [C, test/play/tests/fixtures/world WCCKNMSR]. What the type changes:
- area spells hit them only when the caster has 40+ evil points (LJNSPELL.C:1029-1034);
- they never start following a player because of a fight or a damaging spell, and charm spells can't take
  them (LJNCOMBT.C:2622-2632, LJNSPELL.C:3049-3058, :8180, :8335);
- like good-aligned monsters, they take a free swing at a fleeing player only if already following him as
  an enemy (LJNMONST.C:4636-4640);
- when a Villain or FIEND leaves the game or dies, every non-friendly type-37 monster following him by name
  is removed without a kill (`dismiss_users_angels`, LJNUTIL.C:9831-9860, called from
  `cleanup_when_user_leaves` :10475). The recovered source's comment calls these "summoned angels"; the code
  says hunters on his trail [I: the comment's wording is the reconstruction's, the behaviour is the code's].
- tournament mode 2 spawns none (LJNMONST.C:628-631).
A client should treat a type-37 monster following you as a pursuer that disappears if you log off as a
Villain or FIEND.
