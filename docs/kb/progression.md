# Progression

## The loop
Choose a hunting ground, then grind it, then gear up (shop, or loot a boss), then train (level,
then CP), and choose again. Quests are occasional big boosts. [F, John]

## Experience and levels
- **The formula** (`new_calc_exp_needed(steps, race% + class%)`, LJNPRMPT.C:839-958): start at
  `(mod + 100) × 10`; for each of `steps` steps multiply by `numer/denom` and floor. The factors by
  step number (from 0): 1/1, then 40/20, 44/24, 44/24, 48/28, 48/28, 52/32, 52/32, 56/36, 56/36,
  60/40, 60/40, 65/45, 65/45, 70/50, 70/50, 75/55 (steps 1-16: ×2, 1.83, 1.83, 1.71, 1.71, 1.625,
  1.625, 1.56, 1.56, 1.5, 1.5, 1.44, 1.44, 1.4, 1.4, 1.36), then 50/40 for steps 17-24 (×1.25),
  23/20 for step 25 (×1.15), and from step 26 on 115/100 (to step 53), 109/100 (steps 54-56) and
  108/100 (from 57) (LJNPRMPT.C:32-38, :854-885). The exp to *be* level L is `steps = L-1`.
  [C] Re-checked with a port using 32-bit unsigned arithmetic; every value below agrees.
- Other modifiers (race% + class%), exp to be level 2..11, from the same port: +0%: 1,000;
  2,000; 3,666; 6,721; 11,521; 19,750; 32,093; 52,151; 81,123; 126,191. +50%: 1,500; 3,000; 5,500;
  10,083; 17,285; 29,631; 48,150; 78,243; 121,711; 189,328. +100%: 2,000; 4,000; 7,333; 13,443;
  23,045; 39,505; 64,195; 104,316; 162,269; 252,418. +150%: 2,500; 5,000; 9,166; 16,804;
  28,806; 49,381; 80,244; 130,396; 202,838; 315,525. [C]
- Kang Witchunter: level 2 at 1,650 exp, so the race and class modifier is +65%. **Level 3 is
  3,300**, then 6,050, 11,091, 19,013, 32,593, 52,963, 86,064 (L9), 133,877 (L10), 4,082,703
  (L20), 27,239,493 (L30), 110,198,100 (L40). [C] `LJNPRMPT.C:839 new_calc_exp_needed`
- The full table to level 21 for +65% (exp to *be* that level): L2 1,650; L3 3,300; L4 6,050;
  L5 11,091; L6 19,013; L7 32,593; L8 52,963; L9 86,064; L10 133,877; L11 208,253; L12 312,379;
  L13 468,568; L14 676,820; L15 977,628; L16 1,368,679; L17 1,916,150; L18 2,612,931;
  L19 3,266,163; L20 4,082,703; L21 5,103,378. [C] `LJNPRMPT.C:839-958`, re-checked
  against a port of the function with 32-bit unsigned arithmetic.
- Where the 32-bit arithmetic first bites: the function equals the exact stepwise product until
  the first step whose `rem*numer` passes 2^32; the rescale by 100 then drops the low two digits
  (+65%: L34 is 47,641,900, exactly it would be 47,642,041; +0 from L37, +200 from L29; +100
  from L32, +30 and +35 from L35). [C]
- Two more limits of the same function: the billions are kept in an `int` of which only the low
  16 bits count (sic), and `nbil × numer × 1,000,000` wraps in 32 bits from about 37 billion,
  which at +65% first changes a result at level 95. Neither is reachable in play. [C] (comments at
  LJNPRMPT.C:929-934 and :955; the level from the port)
- The exp to *be* level L is `new_calc_exp_needed(L-1)`; `train` from L checks
  `new_calc_exp_needed(L)`. Growth per step: the table to step 25, then ×1.15 to step 53, ×1.09
  for 54-56, ×1.08 from 57. (sic) Past a billion the billions half grows by numer/1000 (11.5% at
  1.15, 10.8% at 1.08) instead of the step's factor. Kang Witchunter first passes a billion at
  L57 (1,065,353,700). The older `calc_exp_needed` (`LJNUTIL.C:1547`, used by help and the
  prompt) agrees through step 52 (L54), then uses ×1.10 for steps 53-60 and ×1.05 from 61. [C]
- Exp stops (`You have progressed too far without training to acquire anymore experience!`)
  once you hold more than the exp for level + XTRALVL (module default 1; the stock `WCCMMUD.MSG` sets 2, range 0-16, read at
  LJNGAME.C:552). [C]
  `LJNUTIL.C:1667-1700`
  - In levels: `new_calc_exp_needed(level + XTRALVL)` is the exp to *be* level + XTRALVL + 1,
    so with XTRALVL 1 a level-1 character earns up to the exp of level 3 (3,300 for +65%);
    holding exactly the cap still earns, the gain that crosses it is kept whole, and the next
    one is refused. The module default is `LJNGLOBL.C:158`. [C]
  - `You gain N experience.` (`LJNUTIL.C:1777`) prints the amount after whole billions were
    taken off it. (sic) The amount is compared with a billion unsigned, so a negative amount
    (-5) adds four billions and 294,967,291. A gang member's gain (above 1) is added to the gang
    total too. [C] `LJNUTIL.C:1711-1779`
  - The top-ten list is updated only when a player's auto-combat stops
    (`display_autocombat_broken` → `update_top_list`, `LJNACMB.C:152`), not by
    `add_experience`; a quest reward shows up after the next fight. The list is built from the
    whole user file when the module starts (LJNTOP.C:1-30) and a character is taken off it only
    by a suicide (`remove_reroll`, whose one caller is `commit_suicide`, LJNMISC5.C:532): one
    who dies on his last life is deleted (LJNUTIL.C:4586-4604) but keeps his TOPTEN line until
    the next restart. [C]
- Training gives CP 10 (to L10), 15 (to L20), then ((L−1)/10−2)×5+20; HP 0..hplevel at random;
  +5 lives, capped at 9. [C] `LJNUTIL.C:8352-8380`
  - The expression alone gives the same value at every band edge (L2 and L10: 10; L11 and L20:
    15; L21 and L30: 20; L31 and L40: 25; L41: 30): `(L-1)/10-2` is -2 for L2-10 in `int`. [C]
  - `N additional lives` is printed only when the lives changed (nothing at 9). `trainlives` is
    set to 5 at start-up (`LJNGAME.C:1021`), not an option. [C]
- CP to raise a stat: 1 per point for the first 10 above the race minimum, 2 for the next 10,
  ... 9, then 10 per point beyond 90 (cumulative 10/30/60/100/150/210/280/360/450). [C]
  `LJNGAME.C:1645 stat_amount_used`
- Monster exp is per kill (the `exp` field). Kills split exp in a party
  (`distribute_experience`). [C]

## Training
- Trainers are shops of type Training (8) with `minlevel`/`maxlevel` and `trainclass`
  (0 = any). [C]
  - Universal, levels 1–10: Newhaven Adventurer's Guild 1:2147, Adventurer's Guild 1:1376,
    Small Dark Room 1:2325.
  - Class trainers for levels 1–10 are in Silvermere (Witchunter Training Room 1:498).
    **Prefer the Universal Trainer 1:1376 (no markup)**: the class trainer charges 4×
    and the HP gain is the same. [F, prices C]
  - Higher levels: "Super <class>" trainers in the Large Tomb at level 11; High Level Trainer
    12–20 (1:2305); Rhudaur, Dwarven, Titan and others beyond.
- `train` refusals, in order: `You must be in an appropriate training room to train!` (not a
  shop, or not a Training shop), `This shop is not suitable for your training.` (another
  class's trainer), `You have not progressed far enough to use the training provided here.` /
  `You have progressed too far to use the training provided here.` (the *next* level outside
  `minlevel..maxlevel`), the DEMOLVL cap, `You do not have the required experience to train
  yet!`, then `You do not have the money required for your training.` (experience is checked
  before money). Not refused in combat; refused in tournament mode (`LJNCMDA.C:4403`). [C]
  `LJNUTIL.C:8236-8405`
- Fee: `level×5` silver × `(markup+100)/100`, worked in whole silver (Charm plays no part): 5
  silver at L1 at the universal trainer (markup 0), exactly 4× (20 silver) at a class trainer
  (markup 300). The receipt: `You hand over 5 silver nobles and you receive training to attain
  level 2.`, `You receive the following:`, `10 additional character points`, then the lives
  line. `train` does not save the character; the periodic save does. [C] `LJNUTIL.C:8327-8394`
- `train` levels you up if you have the exp, and charges a fee (see `economy.md`). `train
  stats` at a trainer for your class opens the same full-screen editor as creation, but without
  the first name. [C] `LJNCMDA.C:4369`
  - **`train stats` refusal order** (LJNCMDA.C:4403-4468): tournament mode (`You may not do any
    training while in tournament mode!`, which `train` alone gives too); a room that is not a
    Shop, or a Shop that is not a Training shop, or a trainer of another class (`trainclass` 0 =
    any): `You may not train your stats here!`; and, checked before the shop type, a stat under
    an Alter Int/Wis/Str/Hea/Agi/Cha ability: `Your stats are unnaturally altered!  You may not
    train stats now.` So that last message appears in any Shop room, whatever the trainer. [C]
  - **It saves first.** The record is saved, then everyone in the game (`tell_game`, not just the
    room) sees `<Name> just left the Realm.` unless you are invisible, and the editor opens
    (LJNCMDA.C:4441-4446). After SAVE the game shows `show_status` and the prompt (no room
    display), and HP is refilled only if you have 0 experience (recalculation mode 0), otherwise
    kept (mode 2) (LJNGAME.C:2304, LJNUTIL.C:4987-4990). QUIT re-enters unchanged, unless the
    stats had been reset, in which case it goes back to the main menu (LJNGAME.C:2351-2386). The
    editor never lets a stat go below its current value (LJNGAME.C:1936). [C]
- **Race and class change (CHANGERACE)** [C]: every character made by 1.11p's character creation
  has `PF_RACECHANGED` set and `racechg` 0 (LJNUTIL.C:1520-1521), so the menu's `CHANGERACE` is
  offered only to older records, or to ones a sysop reset (`GOD ... RACE 0`, LJNSUPP.C:1583-1592,
  LJNCMDB.C:3252-3253); typed when closed it falls through to the menu's `C` (Game Counts)
  (LJNGAME.C:3739-3740). A first change stores `racechg` (minutes played) and an undo is allowed
  while `sesstime/2 + lifemins - racechg` (unsigned) is within `racechgdur` minutes of play
  (default 300, range 1-720; LJNLOG.C:31-33, :114). The class change needs the sysop's
  ENABLE_CLASS_CHANGE (off by default, LJNLOG.C:481), never sets `racechg`, and clears the random
  HP. Both set `cp = -1`: on the next entry (LJNUTIL.C:928-985, LJNGAME.C:3914-3915) the
  character is sent to the reincarnation room (the evil one at 40+ points), every spell on him
  ends, the stats are rolled again, the CP of his levels are given back (10 / 15 / 20 ... per
  level as in `train`), and he goes straight into the stat editor. A record from before version
  22 is reset the same way, and so is any with a stat more than 50 above its race maximum or
  more than 30 below its race minimum (never below 5) (LJNUTIL.C:870-912). The notice is `Your stats have been reset due to an earlier bug.` [C]
- **Automation policy:** a bot may `train` (level up), but **never `train stats`**; a human
  spends the CP, with `statplan.py` as the advice. (John, 2026-10-13)
- **Stat priority (melee, Kang Witchunter):** Str to 86–90 first, then Agi, Hea and cheap Cha.
  Charm below 46 costs dodge. Wil matters for Anti-Magic spell defence. Int is useless before
  about level 20. Ladder in `test/play/statplan.py`, reasoning in `notes/combat.md §3`. [C/I]

## Progression plans (how the tool should work; John)
- Like MegaMud: **a fixed area per level range, then run that area's loop** until you outlevel
  the stage, then move to the next planned area. No dynamic roaming. Plans live in
  `test/play/plans/<class>.json`: stages `{levels:[lo,hi], area, anchor "map:room", radius}`,
  plus `{levels, quest}` stages for a human. Dynamic ground choice is only a logged fallback
  when a stage's area is unreachable.
- Why: dynamic choice sent Kasimir (level 5) 100 steps through the Western Road's dark monks to a
  map-9 area, and he died. Plans are predictable, reviewable and safe by construction.

## Choosing where to grind
- A room's spawn pool: nothing unless `maxmonsters > 0`; then **its `genmonster` alone** when it
  has one, else every known monster with `montype == room.mongroup` and `genindex` within
  `[room.minlevel, room.maxlevel]` (`minlevel` 0 = no floor; **`maxlevel` 0 = no group spawn at
  all**). Plus the room's specific monster `specmon`, which is made at start-up and again whenever
  a player enters while it is dead: that is how shopkeepers, healers and the ferryman are there
  (Newhaven's shops have `maxmonsters` 1-5 but `maxlevel` 0, and their keeper as `specmon`). The
  spawn pass only visits Normal, Arena and Lair rooms. [C] `LJNMONST.C:493-496, 510-515,
  594-603` (`generate_monster`: `monnum` given = that monster, no random pick),
  `LJNBKGND.C:2466-2620`, `LJNUTIL.C:2525-2535`, `LJNBUFF.C:825-845`. (Earlier this said
  `genmonster` *plus* the group, as `mmud_data.spawn_pool` computes; that over-includes, see
  `aoe-grinding.md`.) Stock rooms with `maxmonsters > 0` and a `genmonster` or a `maxlevel`:
  20,355 Normal, 3,102 Lair, 62 Colliseum, 57 Shop, 3 Arena. [C, stock export, wayfarer C-021]
- **Respawn**: after a kill, a room spawns nothing until its `regendelay` minutes have passed
  (default `regenmins` = 5). **Exempt, so they respawn immediately: arenas (room type 2),
  forced spawns, and a room's own specific monster (`genmonster`/`specmon`).** [C]
  `LJNMONST.C generate_monster`, `LJNGLOBL.C:161`
- **The pick within the pool is not even:** the first matching monster in number order is
  taken, and each later one replaces it on 29 rolls of 99 (`genrdn(1,100) < 30`); a roll of
  99 ends the search. [C] `LJNMONST.C:600-621`
- **Unique monsters** (`maxalive` 1) come back `regenhrs` hours after their kill, plus
  `lngrnd(0, regen/4) - regen/8` minutes (4 h: 210-269 min). Any limited monster
  (`maxalive` > 0) that has never been killed carries all its items, unique or not. [C]
  `LJNMONST.C:644-657, :741-742`
- **The respawn clock** is a minute-of-the-day stamp (`lastkill`), plus 1440 once midnight has
  passed since start-up, cleared for every room at a cold start. Early in the start-up day a
  delay longer than the minutes so far never blocks (the unsigned subtraction wraps). [C]
  `LJNMONST.C:526-554, :991-998`, `LJNBUFF.C:868-877`
- **The spawn pass** (every 5 s) [C] `LJNBKGND.C:2411-2620`:
  - each player's room rolls `genrdn(1,100)`: an Arena spawns on 89 rolls of 99 (`90 >
    roll`), a Normal room on 4 of 99 (`5 > roll`), only while monsters < players. A second
    chance (`roll > 99`) can never hold.
  - **every player in the room rolls**, with the first player's counts (the table entry is
    not updated after a spawn), so an empty arena gains about one monster per player per pass.
  - then each exit of each occupied room: on 5 rolls of 99 the room beyond (Arena/Normal)
    spawns one while it has fewer monsters than the occupied room has players; after the
    first neighbour that gets a spawn attempt no other exit of the room is tried (a full
    neighbour, or a Lair neighbour, lets the next exit roll). The neighbour is loaded with the map of the
    last player seen (harmless on one map), and a neighbour that is itself occupied is not
    skipped while the table has free entries (sic).
  - the pass stops at the 9th monster (`made > MAXGENERATE` 8) and resumes with that player
    next pass. **A Lair fill stops at the cap one monster late** (`generate_monster` runs
    before the `made <= MAXGENERATE` test), so a large lair entered empty gets about 10 in the
    first pass and the rest on later passes. The Lair branch does **not end the pass**: every
    later lair in the same pass still gains one.
- **A new monster's arrival direction**: of the room's eight compass exits of type Normal the
  first is taken and each later one replaces it on 4 rolls of 9; none: "from nowhere". With a WCCMSG arrival text the
  arguments are the name then the direction when the monster has a random-name list, and the
  direction then the name when it has none (a missing text gives `X moves into the room from the
  north.`). [C] `LJNMONST.C:851-889`
- **Wandering** [C] `LJNBKGND.C:1258-1370`: every 3 s (medium pass, `secmed`) a monster that is
  not following a player and has not fought since the last pass may step through a random usable exit
  (the first usable one, each later one replacing it on 40%, `pick_valid_random_direction`,
  LJNMISC5.C:189-245; the step is skipped if the pick is the direction of its last move), at
  most three monsters per pass; the "has fought" mark is cleared at the end of each pass. Wanderers
  (types Wanderer, Living, Random and the rest) move on `genrdn(0,100) < (100 - followpct)/2`;
  Lair and NPC types never move; Guards need no dice roll (they are limited by the three-mover count only
  while the monster is not marked changed). A monster keeps to rooms whose
  `mongroup` equals its own type, except that an Angel may enter rooms with no flags (the
  precedence slip `(!torm->flags) & 1`, LJNMONST.C:1325-1330), a Quest monster goes anywhere,
  and a Guard also enters rooms flagged Patrol but never through a Block Guard exit.
- **Arena level limit:** an arena refuses players whose level is above the room's `maxlevel`
  (Newhaven Arena 1:2150 allows up to level 3; 1:2155 and 1:2313 allow level 1 only). [C]
  `LJNUTIL.C:2519`
- **"Instant spawn" spots** (forum lore). The source settles what can be instant: the spawn pass
  runs every `seccreate` seconds (5 by default) and only for rooms with a player in them (or next
  to one). An Arena has no regen delay after a kill and spawns on 89 of 99 rolls, so it refills
  within a pass or two (while monsters < players). A Normal room still needs its 4-in-99 roll
  each pass (about two minutes on average) even where the regen delay does not apply (a room's own
  `genmonster`/`specmon`, forced spawns), and respects the delay otherwise. A Lair fills to
  `maxmonsters` within one pass once the delay is over. Nothing else spawns on the next tick, so
  "instant" spots are arenas, lairs, or dense clusters of spawn rooms. [C] `LJNBKGND.C:880-890,
  :2486-2520`, `LJNMONST.C:509-554`. Cave worms
  (Stone Tunnel 1:1541, 42 rooms; 70 HP / 100 exp / max hit 16, AC 14) are ordinary 5-minute
  group spawns in the data (no room has them as `genmonster`), so the cluster explains the lore.
  Access needs the black star key (7% drop from slum dark cultists). [C data, F lore]
- Score candidate areas on paper by the exp on offer from safe monsters, a weighted count of
  spawn rooms, and distance. Then **measure exp/hr and prefer measured results**. [C]
- Measured for a level-1 Kang Witchunter: Newhaven Arena about 2,830 exp/hr, Silvermere slums
  about 920 exp/hr (the slums' thugs hit for up to 10, so time goes to resting). [C]
- The forum corpus (`test/play/corpus/`) has community recommendations by level range. For a
  Kang Witchunter, the rooms, monsters and prices were checked against the data [F→C]:
  - **L1–3: Newhaven Arena 1:2150.** It locks you out once you train past level 3.
  - **L3–6: Graveyard 1:817**: 90 rooms, max hit 14. Its gravediggers (1,500 exp) are
    Neutral, so the evil-only filter skips them.
  - **L6–10: Slimy Sewer Tunnels 9:1**, which needs a lantern and `search` west at 1:619.
    Alternative: Strange Mansion 1:63.
  - **L10: Witchunter crypt quest** for the mithril cutlass, which also trains you to 11.
  - **L11–15: kobold caves 6:752**, with the kobold king at 6:834.
  - **L15–20: Frozen Caverns 10:35**: wolves, hits up to 22; needs `search` north at 10:3.
  - Weapons by stage: greataxe (6–22, 10 gold, 1:307), battle-hammer, then the three-headed
    flail (9–23, 4 platinum, 1:354).

## Patrolling an area
- Compute a loop through the safe spawn rooms (nearest-neighbour plus 2-opt). A 46-room slum
  area gives a 90-step loop. Better: **go to the nearest spawn room whose respawn timer has run
  out**, using cleared timestamps, and wait or rest if none is due. [C] `progress.py
  Bot.patrol`

## Quests (see `test/play/notes/quests.md`)
- **Level 10, Witchunter: mithril cutlass** (item 342, 8–18 damage, Str 45, Witchunter-only).
  The route is through the Graveyard crypt (Sealed Tomb 1:1309 → Shadowed Hall → Marble Room
  1:2219 → Large Chamber → Large Tomb 1:2236, which has the Super Witchunter Trainer). It
  needs a bone key and an iron key from the guardians. [F, rooms and item C]
- Alignment quests at level 10, Ice Sorceress at 15, Magebane (Witchunter) at 15. [F]
