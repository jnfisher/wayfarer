# AoE grinding (cross-class): mechanics and spots by level

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** read in the source
(`WCCMMUD-1.11p-source/SRC`) or the game data (`test/run/built/WCCMMUD/*.db`), **[I]** inferred or
modelled, **[F]** forum/web. Monsters are `hp/exp/maxhit/MR`. Exp is `exp × monster.level`, the amount
the kill code pays (LJNSPELL.C:3361, 7731) [C]. Steps are walking steps from Town Square 1:224
(`walk+search` = needs `search <dir>` on a hidden exit). Per-class detail: `kb/classes/{mage,warlock,druid,priest,cleric}.md`.

## 1. Who has player AoE damage [C WCCSPELS + scroll items]

All player AoE damage spells are **Full Attack Area** (target 12): every valid monster gets the full
roll. No learnable player spell uses a "Divided" target.

| Spell `cmd` | Magery/lvl | Classes | Lvl | Mana | Dmg @learn → @cap | Diff | Where |
|---|---|---|---|---|---|---|---|
| dancing blades `blad` | Mage 1 | Mage, Warlock, Gypsy | 11 | 26 | 21-62 → 21-78 @L20 | -10 | Mage Spell Shop 1:398, 250g list |
| fireball `fbal` | Mage 1 | Mage, Warlock, Gypsy | 15 | 34 | 33-90 → 33-114 @L23 | -20 | 1:398, 350g |
| mana storm `msto` | Mage 2 | Mage, Warlock, Gypsy | 23 | 50 | 59-170 → 59-215 @L32 | -45 | Jorah's Mage Spells 1:3309, 1250g |
| spirit horde `spir` | Mage 3 | **Mage only** | 28 | 60 | 91-251 → 102-405 @L50 | -60 | Very Powerful Spells 10:273, 2500g |
| blizzard `bliz` | Mage 2 | Mage, Warlock, Gypsy | 40 | 45 | 80-250 → 80-310 | -75 | drop: storm giant king 7% |
| eldritch storm `esto` | Mage 3 | Mage only | 40 | 70 | 130-400 → 190-550 | -90 | drop: Gnaj the Creator 33% |
| swarm `swar` | Druid 0 | Druid, Ranger | 12 | 28 | 24-60 → 24-76 @L20 | -10 | Druid Spell Shop 1:1314, 200g |
| chaos storm `csto` | Druid 2 | Druid | 15 | 34 | 29-80 → 29-104 @L23 | -20 | 1:1314, 350g |
| maelstrom `mael` | Druid 2 | Druid | 24 | 50 | 62-160 → 62-195 @L32 | -45 | Jorah's Druid Spells 1:2334, 1250g |
| earthquake `quak` | Druid 3 | Druid | 28 | 60 | 96-230 → 99-302 @L40 | -55 | 10:273, 2500g |
| acid tempest `atem` | Druid 3 | Druid | 40 | 65 | 96-300 → 100-405 | -80 | no shop or drop found |
| thunderclap `thdr` | Druid 3 | Druid | 45 | 75 | 40-290 | -125 | drop: storm giant commander 7% |
| song of shockwave | Bard | Bard | 19 | 10 | see `bard.md` | | Bard Training Room 1:852 |

**Priest, Cleric, Paladin, Missionary have no AoE damage spell** (Priest magery has only party
heals and AoE debuffs: black wind `wind` blind/-10 AV, L21) [C]. Room damage-over-time spells exist
(ice storm `isto` M L17, 3/round for 15 rounds; acid rain `rain` D L15; plague `plag` D L26): 3-6
damage per round is too little to matter except to stop resting monsters [C/I].
Prices: `list × (100+markup)/100 × (110 − Cha/5)%` (LJNUTIL.C:6091) [C]. All shops above are
markup 100, so a 350g scroll costs ~700g at Cha 50.

## 2. How an AoE cast resolves [C LJNSPELL.C `cast_no_target` 1864-4930]

| Step | Rule | Source |
|---|---|---|
| Targets | Every monster in your room except: your charmed/friendly followers, **Guard and Angel monster types unless you have ≥40 evil points**, and **Lawful Good** monsters. Neutral and Good monsters **are hit** (Good = evil act, EP charged once). Players only under PvP rules. | `is_valid_monster_target` :979-1040 |
| Out of combat | `cast fbal` outside combat does not fire: "*Combat Engaged*… moves to attack everyone in the room", your energy goes to 0, and **auto-combat recasts the spell every round** until targets or mana run out. Every monster in the room is now in the fight. | :2050-2075 |
| Success roll | `roll 0-99 < min(98, spellcast + diff)`. Fail = full energy + **half mana**. AoE (energy 1000) = one cast per 5-s round. | :2133, macro :81 |
| Amount | `minval+mininc×L/minlvls .. maxval+maxinc×L/maxlvls`, L capped at the spell's level cap. Full Area: each monster gets the whole roll. | :2230-2290 |
| Monster resist roll | Coded `if (roll > 999)`: **monsters never resist an area spell** (sic). | :3009, :3340 |
| Element resistance | Not applied to monsters by area spells (only to players). Single-target spells do apply it. | :1217, :7470 |
| MR multiplier | `mr = monster.magicres + "Magic Resistance" ability (min 1)`. mr ≤ 50: **damage × (1 + (50−mr)/100)** (up to +49%). mr > 50: × (1 − (mr−50)/2 %), max −50%. Anti-Magic monster: × (1 − mr/2 %), max −75%. Same formula single-target. The monster's resistance is a signed `int` (`mres` :3289, `magres` :6881), so the extra damage below 50 is real for monsters; a *player's* `mr` is unsigned and does not get it (§2a, `spells.md`). | :3285-3330, :7799-7860 |
| Exp | Each kill pays `exp × level` to the caster (party share rules apply). | :3361 |
| Who is reached | The count marks each valid player who does not resist (`ST2_AREATARGET`), clears the mark of invalid ones, and leaves a resisting valid player's mark as it was; the slots then hit every valid *marked* player. So a player you hit with an earlier area spell who resists this one ("X resists your spell!") is still hit (sic); the division uses only this cast's non-resisters plus the monsters. Each player reached counts as an evil target unless he has an evil timer on you or is an outlaw, your own party included; the charge itself comes only for attack spells and Evil in Combat. | :1086-1202, :2894-2897, :813-818 |

Single-target comparison: combat spells loop **while energy remains** (max 20 tries): energy-250
spells (meteor swarm, elemental fury) cast 4×/round, 500 (forked lightning) 2×/round (:7276-8443) [C].
Single-target spells roll MR/2 % resist only for resist-mode "always" (harm, agony, poison bolt) or
"anti-magic" vs Anti-Magic monsters (:7520) [C].

Typical MR [C WCCKNMSR]: L1-15 monsters 30-48 (AoE does **+2..+20%**), L15-30 monsters 48-60
(±5%), orc fanatic 95 (−22%), crimson priest 110 (−30%), animated books 150 (−50%).

## 2a. Monster area spells [C LJNMONST.C `monster_cast_area` 2125-3191]

| Step | Rule | Source |
|---|---|---|
| Who | Every player in the monster's room, party or level notwithstanding, who does not resist: worn item, or `1d100 <= min(MR/2, 97)` (resist mode "always", or "anti-magic" with Anti-Magic). Never a monster. The mark is never cleared here, so a player marked earlier is hit even after resisting. | :1804-1871 |
| Cast | `roll 0-99 < atkmins[slot]` (the attack slot's cast %); fail = half the slot's energy (min 1) when the slot has an energy cost; a cast from no slot (100%, no energy) costs nothing either way. Too little energy: nothing. | :2169-2243 |
| Amount | `minval+mininc×level/minlvls .. maxval+maxinc×level/maxlvls` (level = the slot's cast level, no cap), divided by the players hit for every kind but the Full ones; each player's share cut by his element resistance for attack-type spells. | :2254-2298, :60-96 |
| Damage Ignoring Armour (ability 1) | An instant one hurts **only** for Divided Area Incl. You, Full Party and Divided Party; Full Attack / Full Area / Divided Attack hurt nobody (sic). Stock monster spells this makes harmless: flesh-eating gas, a cloud of poison gas, hail of stones, sand breath, tapestry room. | :2514-2518 |
| Damage on Armour (ability 17) | Every area kind, instant or lasting: each player's share, then a cut by his MR. With Anti-Magic the cut is `min(MR/2, 75)` %. Without it the C tests `plr->mr-50 > 0`, but `mr` is a 16-bit `unsigned` (MAJORMUD.H:1164), so for **every MR under 50 the subtraction wraps and the player gets the full 50 % cut** (sic); above 51 the cut is `(MR-50)/2` %, at most 50; MR 50 takes the share in full and MR 51 takes `share × 99/100` (the only reach of the "extra damage" branch `share×(50−MR)/100 + share`). The cut starts at 0 for every player here (:2626), unlike the player engine's area loop, which carries the previous target's cut (`spells.md`). The same unsigned test is in the player engines' Damage on Armour against players (LJNSPELL.C:3109, :3205, :6025) and in `monster_cast` (LJNMONST.C:3878); §2's MR row is the multiplier on *monsters*, whose `magicres` is signed. Most stock monster AoE (hellstorm, spirit horde, quake, blizzard...) is this. | :2570-2692, :2625-2667 |
| No effect | Stat alters, hunger, thirst, dispel, freedom, remove curse, text block do nothing in a monster's area cast. | :3145-3177 |
| Slot values | A slot's own value (as opposed to the rolled one) reaches only Clear Item, Summon, the caster's own Heal and Cure Poison and the lasting effects on the caster; every per-player effect uses the divided base roll. | :2422-2424 |
| Steal Hit Points | Each player loses his share and the caster gains it, capped at the caster's own `maxhp`; a single-target monster cast caps at the template's `maxhp`. | :2731-2736, :3933-3935 |
| Cure Poison | Cures every marked player **without Poison Immunity** by the share, with no floor at 0 (a player can end up with negative poison); the self form floors at 0 and reports the per-target share, not the amount cured. A single-target monster cast also has no floor, where a room's and a player's cast floor it at 0. | :3126-3127, :3085-3093, :4205-4206; LJNROOMC.C:744-751 |
| Summon | An area or self "Summon Creature" creates the monster with nothing set on it (no follow order); the single-target form sets the caster's `genindex` as both ends of the index range and orders the new monster to follow the target player. | :2838-2840, :5003-5004, :4007-4018 |
| Lasting | Put on each marked player with the record's **raw** duration (no `durinc` growth, no re-roll, no "Alter spell duration"), renewed only if stronger, in the last free slot; refused by everyone = nothing cast. A single-target monster cast on a player does grow with the slot's level (`duration + level/durlvls*durinc`, division first, :3539-3542), as does the time a lasting spell runs on the monster itself (:2305-2308). | :1727-1780, :1880-2010 |
| Messages | Each player reached by the first acting slot sees the cast (WCCMSG target line, light red); the room sees the room line each time. A death pays nobody. | :1589-1715, :2547-2556 |

## 3. Spawning: why lairs are the AoE engine [C LJNBKGND.C:2378-2560, LJNMONST.C:459-600]

| Room type | What spawns near a solo player (creation cycle every 5 s) |
|---|---|
| Normal (23k rooms) | Your room: 4 in 99 per cycle **only while monsters < players** (=1 for solo). Each neighbour: 5 in 99 per exit per cycle, again only while < players, and after the first neighbour that gets a spawn attempt no other exit of the room is tried (a full or Lair neighbour lets the next exit roll). So a solo player rarely sees >1-2 monsters, except wanderers and leader+followers groups walking in. [C LJNBKGND.C:2489-2598] |
| **Lair** (3.1k rooms) | Filled **until `maxmonsters` is reached**, while you are in it or (5 in 99 per exit) next to it. The 9-per-cycle cap stops a Lair fill one monster late (the call comes before the count test) and does not end the cycle, so every later lair in the same cycle still gains one. [C LJNBKGND.C:2517-2525] |
| Arena | 89 in 99 per cycle, still only while monsters < players; each player in the arena rolls, so two players see about two spawns a cycle. Exempt from the regen delay. [C] |
| All | After any kill in the room, nothing spawns until `regendelay` minutes (0 → 5) pass. Rooms under a controlling room also obey its `areamax`. A room with `genmonster` set spawns **only that monster** (`mmud_data.spawn_pool` also lists the group pool; that over-includes). |

Wanderers ignore these caps: lair monsters (group 6) and other wanderers walk between rooms, so a monster can walk into an arena or a full lair and push it past its limit [C LJNBKGND.C:1318-1372, LJNMONST.C:1310-1341]. John remembers this from the live game too [F]. Most hunting grounds are ordinary rooms and multi-room areas. Single-room spawn spots are the exception; the notable ones are the Newhaven Arena, the instant-spawn caveworm under Silvermere and the instant-spawn slime beast in the orc caves near Thragg [F].

So the AoE loop is: **enter lair → it fills to maxmonsters → one or two AoE casts → leave or
meditate → regen delay (5 min, 20 min in map-3 caves) → repeat**, ideally rotating 2-4 lairs.

## 4. Mana economics [C formulas, I numbers]

| Item | Formula | Source |
|---|---|---|
| Max mana | `Mana abilities + 2×(magelvl×level + 3) + manabonus`. **Int/Wil do not raise max mana.** Mana abilities: Half-Elf/Elf +10, items. | LJNUTIL.C:4904-4945 |
| Regen (every 30-s slow tick) | `stat × (level+20) × (magelvl+2) / 1650`, × (100+"Alter Mana Regeneration")%. stat: Mage magery Int, Priest Wil, Druid (Int+Wil)/2. | LJNBKGND.C:1671-1716 |
| MEDITATE | Same amount every 15 s **in addition** to the slow tick → **3× regen**. Needs ability 187: Dhelvanen (Luxurious Room 7:1008, 227 steps) quest: 50 nanati black diamonds, return his sigil at **L20** (Mage/Priest/Druid), **L23** (Warlock/Cleric/Gypsy/Missionary) or **L27** (Paladin/Bard/Ranger); Warrior, Witchunter, Ninja, Thief and Mystic never get it (text 9046 has no `giveability` for them). Poisoned = can't. | LJNBKGND.C:2143-2210, LJNCMDS.C:993, WCCTEXT 9024-9046 |
| Spellcast | `2×level + castbase + 5×magelvl + "Spellcasting" items`; castbase Mage (3Int+Wil)/6, Priest (3Wil+Int)/6, Druid (Int+Wil)/3 | LJNUTIL.C:4919-4952 |

Break-even (AoE vs best single-target nuke at the same level, by dmg/mana) [I]:

| Level | AoE (avg dmg / mana) | Nuke (avg dmg / mana) | AoE wins on mana at N ≥ | AoE wins on time at |
|---|---|---|---|---|
| 12-14 | blades/swarm 41/26-28 | lightning bolt/stonestrike 33/5 | ~4.2 | N ≥ 2 if all die in 1-2 casts |
| 15-22 | fireball 62-74/34, chaos storm 55-68/34 | sunbolt 41-52/8 | ~3.3 | N ≥ 2 |
| 23-27 | mana storm 115-125/50, maelstrom 110/50 | eldritch bolt 75/12, plasma 100/14 | ~2.7 | N ≥ 2 |
| 28-35 | spirit horde 171-197/60, earthquake 163-185/60 | dragonfire 179/16, earthfist 136/16 | ~3.7 (dragonfire is very efficient) | N ≥ 2 |
| 40+ | eldritch storm 265-370/70 | colour spray 220/20, glacial blades | ~2.5-3 | N ≥ 2 |

Rule [I]: **AoE is a time multiplier, not a mana saver.** Exp/hour is capped by mana regen ×
exp-per-mana, which is similar for AoE (N≈3-4) and nukes. AoE wins when the lair holds ≥4, when the
pool hp ≈ one cast (no overkill wasted), and when killing everything in 1 round avoids N monsters
hitting you for 2-3 rounds. Mage L15 (Int 100) regenerates ~20 mana/min without meditate
(~1 fireball per 1.7 min); meditate (L20+) → ~60-70/min.

## 5. Safety rules for AoE pulls [I from the formulas]

1. **One-shot rule**: pick lairs where `avg AoE dmg × MR mult ≥ pool avg hp`. Otherwise N monsters
   × 2-3 rounds of hits. Second cast only if `N × monster dmg/round × 2 < 40% HP`.
2. Incoming ≈ N × (energy/atk-energy) × avg hit. Kobold = 2 swings × 5.5 = 11/round; 5 smugglers
   ≈ 75/round upper bound. Keep `N × maxhit ≤ 50% HP` and `pool maxhit ≤ 25% HP`.
3. Enter with **≥ 2 casts of mana and ≥ 80% HP**; leave (flee) at < 40% HP or < 1 cast of mana.
   Escape route = the way you came; lairs are often dead ends (1 exit) [C room data].
4. Never AoE in rooms with Neutral/Good bystanders (townsfolk, gravediggers, hill centaurs) unless
   you mean to fight them; Good = evil points; Neutral joins the fight.
5. Monsters with `Spell Immunity` > spell level ignore a *hand-cast* opener (LJNSPELL.C:7240): open
   with `attack`, then cast inside autocombat [C].
6. Light: most lairs are −150..−200. Druid/Elf/Gaunt/Gnome/Dwarf light abilities or a lantern.

## 6. Best AoE spots by level band [C room/monster data; ranking I]

Score = exp per clear / casts needed, filtered by the safety rules for a Mage-like HP (≈30+5L).
N = min(room maxmonsters, area max). `rg` = regen minutes. All Lair rooms.

### L11-14 (dancing blades / swarm, avg ~42)
| Map:room | Name | N | rg | Pool `hp/exp/maxhit/MR` | Casts | Exp/clear | Steps | Notes |
|---|---|---|---|---|---|---|---|---|
| 1:2617 | Graveyard, Western Edge | 5 | 1 | skeleton 43/55/11/40, zombie 62/70/14/42, giant bat 35/40/5/40 | 1-2 | 275 | 34 walk | **Best early AoE**. Also 1:2637 NE Edge (N4), 1:2633/2647 (N3). Undead → Priest turn undead too. Gravediggers (Neutral, 130/1500) nearby: don't splash them by accident. |
| 6:754 | Cramped Tunnel, Intersection | 3 | 5 | kobold 30/80/9/42, kobold warrior 50/150/13/45 | 1 | 345 | 188 walk, light −200 | Kobold caves; exp/hp 2.9. Pair with kobold king 6:834. |
| 9:218, 219, 222 | Smuggler Den, Sleeping Quarters ×3 | 5 | 5 | thief 55/125/12/44, smuggler 70/150/12/45 (Neutral Evil) | 2 | 690 | 60 (hidden d at Docks 1:31) | 3 lairs × 5 in one area: top L13-20 rotation. NE: may not start fights. |
| 1:1470 | Dark Cavern | 3 | 5 | cultist 50/50, grey spider 66/85, dark priest 75/150, dark cleric 86/155 | 2 | 290 | black star key | Dark Caverns. |

### L15-22 (fireball / chaos storm, avg 55-74)
| Map:room | Name | N | rg | Pool | Casts | Exp/clear | Steps | Notes |
|---|---|---|---|---|---|---|---|---|
| 9:218-222 | Smuggler Den (above) | 5 | 5 | 55-70 hp | **1** at L18+ | 690 | 60 | One fireball clears; rotate the 3 rooms. |
| 9:809, 822, 848, 859 + 9:845 | Darkwood bandit Living Quarters / Recreation Room | 3 | 5 | bandit 55/125, warlock bandit 60/125, bandit leader 100/275/18/48 | 1-2 | 525 | 67-108 | 5 lairs in one area (map 9 Darkwood bandits). |
| 9:439, 9:475 | Torchlit Cave / Tall Wooden Door | 3 / 4 | 5 | half-orc sentry 65/125/18, orc warrior 80/180/21, orc shaman 80/275/13 | 1-2 | 580-773 | 71-86 walk (door w at 1:1098) | Orc tunnels. |
| 9:648, 662, 676 / 9:717 | Earthen Tunnel / Egg Breeding Chamber | 4 / 3 | 5 | worker ant 80/150/12, soldier ant 80/225/16 | 2 | 750 | 201-257 | Ant nest (queen ant 9:717 area). |
| 10:80, 10:43, 66, 67, 72 | Torchlit Cavern (+Firepit) | 3 | 5 | nomadic shaman 110/365/16/50, tribesman 130/365/19, huntsman 130/355/16 | 2 | ~1,095 | 88 (hidden e 1:600, n 10:3) | Best exp/clear in band; same entrance as the wolves (Frozen Caverns). |
| 1:2707 | Darkwood Forest, Clearing | **10** | 5 | dark goblin archer 73/80/12, dark goblin 90/150/21 | 2 | 1,150 | 54 walk | **10-monster lair**: ~130 dmg/round upper bound. Only for **HP ≥ 250** (Warlock/Cleric-level HP, or L28+ Mage with spirit horde one-shot). |
| 1:424, 425, 451, 474 … (8 rooms) | Darkwood Forest | 5 | 3 | same goblins | 2 | 575 | 42 | Safer goblin lairs, 3-min regen. |
| 1:1771-1775 | Huge Cave, Barracks | 4 | 5 | dark goblin 90/150/21 | 2 | 600 | 71 | |

### L23-29 (mana storm / maelstrom, avg 110-125)
| Map:room | Name | N | rg | Pool | Casts | Exp/clear | Steps | Notes |
|---|---|---|---|---|---|---|---|---|
| 1:2707 | Darkwood Clearing | 10 | 5 | goblins 73-90 hp | **1** | 1,150 | 54 | One mana storm kills all 10 (avg 120 ≥ 90). Take one round of hits. |
| 3:593, 3:606 | Wide Passage / Dark Alleyway, Dead End | 3 | 5 | dragon serpents 190-225 hp /650-975/18-20/54-57 | 2 | 2,565 | 123-125 walk | map 3 ruins; light −175. Avoid 3:560 Plaza: its pool includes crimson mist 800 hp and ogre-mage. |
| 3:6 Mossy Cave; 1:2362 Rocky Waterfall | fungus lizard 205/725/24/55; dragonfish 205/725/20/55 | 3 | **20** | | 2 | 2,175 | 50-56 walk | Short walk; 20-min regen → rotate. Dragonfish Neutral Evil. |
| 3:35 Foul Cave, 3:37 Witch's Cave | vampire bat 235/1100/18/58 | 4 / 3 | 20 | | 2 | 4,400 / 3,300 | 60-62 | **Best L25-35 exp/clear** with 2 casts; hit only 18. |
| 9:475 | Orc Torchlit Tunnels | 4 | 5 | 65-80 hp | 1 | 773 | 86 | Cheap mana filler. |

### L30-40 (spirit horde / earthquake avg 170-200; eldritch storm 265+ at 40)
| Map:room | Name | N | rg | Pool | Casts | Exp/clear | Steps | Notes |
|---|---|---|---|---|---|---|---|---|
| 3:35 / 3:37 | Foul / Witch's Cave | 4 / 3 | 20 | vampire bat 235 hp | 1-2 | 4,400 | 60 | One-shot at L40. |
| 6:398, 444 / 6:471-475 (8 rooms) | Misty Bog | 3 / 4 | 5 | baby black dragon 320/2800/35/65; black dragon hatchling 300/2350/38/65 | 2 | 8,400-9,400 | 133-209 walk | Dragon whelp bog: big exp/clear; MR 65 (−7%). HP ≥ 300. |
| 1:2805, 2809, 2821 … (7) | Darkwood Forest (green dragon nursery) | 3 | 2 | baby green dragon 400/3200/38/65, hatchling 315/2450/38 | 2 | 8,475 | 83-91 walk | 2-min regen. |
| 15:1019-1029 (6) | Lava Cavern, Obsidian Cliffs | 5 | 2 | magma man 225/675/30/56 | 1-2 | 3,375 | 281 | Lava area: fire protection [F t=51]. |
| 12:1282-1300 (17) | Limestone Caverns | 5 | 5 | sandworm 250/1100/36/58, dustdevil 200/725/32/55 | 1-2 | 4,560 | 305-354 | Desert; long walk. |
| 15:1053 | Crimson Fortress, Entrance | 5 | 5 | flaming zombie 300/2000/20/**25** (+25% dmg) | 1-2 | 10,000 | 297 | Low MR. Not flagged undead despite the name [C]. |
| 12:101-108 (7) | Fortress Courtyard | 6 | 5 | rakshasha 300/4000/40/70 (spell immunity 10) | 2 | 24,000 | 348 | L40+. Open with `attack`. |

Unverified oddity: 17:2029 Treasure Room (light −600, maxmon 15, stitched zombie 10/3,500) looks
like a scripted trap/quest room. Don't plan around it until seen in play [I].

## 7. Single-target grinding (all casters) by band [C data; ranking I]

Reuse `test/play/corpus/leveling.md` areas. Caster adjustments:
- **Spells ignore weapon "Magical" requirements** (only `Required to hit`, ABL 158, gates spells)
  (LJNMONST.C:5151) [C]: astral shades, spectres and salamanders are fine for casters.
- Prefer MR ≤ 45 monsters (+5..+20% dmg). Avoid orc fanatics (MR 95).
- **Undead** (`undead` flag) take turn undead/disrupt (Kill Dead, damage ignoring armour and MR):
  Graveyard skeleton/zombie, crypt ghoul/shade/wight/mummy, Strange Mansion vision/shadow spectre/
  astral shade, Ancient Crypt skeletal warrior/moaning spirit, Ancestral Tomb 6:872 tortured
  spirit 150/525/15 and barrow wight 180/725/15 [C].

| Band | Area (anchor) | Why for casters |
|---|---|---|
| 1-4 | Newhaven Arena 1:2150 (arena: no regen delay; locked out at L4) | 1-mana nukes one-shot 12-25 hp rats/slimes |
| 4-8 | Graveyard 1:817 surface + edge lairs; Slum soft streets 1:1073 | MR 40 undead; 34 steps |
| 8-12 | Crypt L1-3 1:835/1:1070/1:1276; Strange Mansion 1:63 (astral shade drops **astral staff** 5%: Mage/Druid/Priest, +10 mana, +10 SC); Slimy Sewers 9:1 | |
| 10 | L10 crypt class quest (class weapon + train to L11) | `kb/classes/<class>.md` §8 |
| 12-16 | Kobold caves 6:752 (+6:754 lair); Smuggler Den 9:161/9:218; Darkwood bandits 9:720/9:809 | |
| 16-22 | Frozen Caverns wolves 10:35 + nomad lairs 10:43/80; Torchlit orc tunnels 9:439 | |
| 22-30 | Ancestral Tomb 6:872 (undead, hit 15); map-3 caves 3:6/35/37; Dragon's Teeth Hills 2:177 | |
| 30+ | Misty Bog 6:398; green dragon nursery 1:2805; Lava 15:1019; Quaggoths 6:1811 | |
