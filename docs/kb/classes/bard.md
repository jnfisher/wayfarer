# Bard (WCCCLASS 9): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats are `hp/exp/maxhit`. Prices include shop markup, not Charm
(× (110 − Cha/5)%). Steps are from Town Square 1:224 (any exit type, so tolls/keys included).
Shared formulas: `test/play/notes/combat.md`; shared areas: `test/play/corpus/leveling.md`.

**One-line path:** Half-Elf, Charm-first. Melee plus discord from L3. Grind arena, Graveyard, Crypt.
At L10 do the crypt quest (jeweled main-gauche, trains you to L11). Then kobolds/bandits and
wolves, and from L19 use **song of shockwave** (AoE) as the main attack. Keep valour and life
up for any party.

## 1. Class facts [C: WCCCLASS rec 9, MAJORMUD.H:1658]
| Field | Value | Meaning |
|---|---|---|
| exp | **+110%** | L2 needs 10×(100+race+110) exp (Half-Elf: 2,250). Kyau lists Bard at 210%, which matches [F]. |
| basehp / hplevel | 4 / 3 | max HP += 4/level, + `genrdn(0,4)` (~2) per level trained |
| combat | 5 | AV ≈ 10 below a Warrior and 20 below a Witchunter at L10 (combat.md AV formula) [I] |
| magery | **Bard (4), level 2** | Every bard song in the data is magelvl 0, so a Bard can learn **all** of them [C LJNUTIL.C:9779] |
| weapons | 4 = Any 1-handed | **No 2H. No off-hand items** unless the item lists class 9 (LJNUTIL.C:9697-9712). Instruments and the jeweled main-gauche are class-9 items, so they are allowed. |
| armour | 6 = Studded Rigid Leather | No chain, scale or plate |
| abilities | 103 Class Stealth, 40 Find Traps, 39 Thievery (+0), 37 Picklocks, 31 Bash | Thief has Thievery +10, Bard +0 |

Derived formulas [C LJNUTIL.C:4740-4960, LJNBKGND.C:1690-1710, 2143-2200]:
- Max mana = 2×(2×level+3) + "Mana" abilities (Half-Elf/Elf +10). Example: L10 = 46 (+10).
- Spellcasting (SC) = 2×level + (3×Cha + Wil)/6 + 10. **Cast chance = SC + spell diff, cap 98.** A failed cast still costs the energy and half the mana (LJNSPELL.C:2225).
- Mana regen per tick = **Cha** × (level+20) × 4 / 1650. MEDITATE adds the same amount every 15 fast cycles until mana is full.
- Stealth = Agi/4 + 2/level (to 15, then 1/level) + Cha/6 + Int/8 + 20. With race stealth too: +10, and backstab AV +5.
- Thievery = (Agi + Int + Cha + 24×lvl')/6. Traps = (Int + Agi + 2·Cha + 28×lvl')/7. Picklocks = (Agi + Int + 10×lvl')×2/7.
- Backstab (any weapon with ability 116, which is most 1H; LJNCMDA.C:484): AV = (stealth+Agi)/2 + BS-AV/2. Damage = (2×weapon + 2×level + stealth/10) × (100+level)%, at a whole round's energy (LJNCOMBT.C:797).
- **Charm drives** casting, mana regen, thievery, stealth, traps, dodge `(Cha−50)/5`, buy and sell prices (combat.md §1). It is the bard's primary stat.

## 2. Race choice (data: WCCRACE) [C stats / I ranking]
| Rank | Race | exp | Cha min-max | Str | Hea | Agi | Extras | Why |
|---|---|---|---|---|---|---|---|---|
| **1** | **Half-Elf** | +15 (total 225%) | **50-110** | 40-90 | 30-100 | 45-110 | Mana +10, night vision 25 | Cheapest exp of the high-Charm races. Cha starts at 50, so 50→70 costs 30 CP. With NV 25 a −175 room reads −150, which is still visible (LJNUTIL.C:8820), so no lantern is needed down to −175. |
| 2 | Human | 0 (210%) | 40-100 | 40-100 | 40-100 | 40-100 | none | Cheapest exp and best Str/Hea caps, but Cha 40→70 costs 60 CP (vs 30). Needs a light in every −175 room. |
| 3 | Elf | +45 (255%) | 50-120 | 35-90 | **30-80** | 50-120 | Mana +10, race stealth (+10 stealth, +5 BS AV), NV 50 | Best caster/backstab ceiling, but 13% more exp than Half-Elf and a Hea cap of 80 (fragile). |
Avoid: Halfling (hpbonus −1/level, Str max 60), Nekojin/Gaunt One (+50 exp), Half-Ogre (Cha max 60).

## 3. Stat / CP plan (Half-Elf) [C costs: statplan.stat_cost; I ladder]
Ladder: Cha 60 → Hea 50 → Str 55 → Cha 70 → Agi 53 → Str 60 → Hea 60 → Cha 80 → Str 70 → Agi 56 → Hea 70 → Cha 90 → Wil 50 → Agi 62 → Hea 80 → Str 80 → Cha 100 …
(I ran it with `statplan.allocate_cp`, swapping in this ladder and banking leftovers.)

| L | CP | Str | Int | Wil | Agi | Hea | Cha | bank | HP~ | mana | regen/tick | SC | MR | stealth | thievery | dodge* |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 100 | 60 | 45 | 40 | 53 | 50 | 70 | 2 | 31 | 20 | 3 | 53 | 41 | 51 | 32 | 5 |
| 6 | 150 | 60 | 45 | 40 | 53 | 60 | 77 | 1 | 69 | 40 | 4 | 67 | 41 | 62 | 53 | 7 |
| 10 | 190 | 70 | 45 | 40 | 55 | 60 | 80 | 0 | 96 | 56 | 5 | 76 | 41 | 71 | 70 | 9 |
| 15 | 265 | 70 | 45 | 40 | 56 | 70 | 88 | 1 | 143 | 76 | 7 | 90 | 41 | 83 | 91 | 12 |
| 20 | 340 | 70 | 45 | 50 | 62 | 79 | 90 | 1 | 195 | 96 | 8 | 103 | 48 | 90 | 100 | 16 |
| 30 | 540 | 80 | 55 | 50 | 62 | 90 | 106 | 0 | 300 | 136 | 12 | 131 | 51 | 103 | 125 | 21 |
*Dodge = (Cha−50)/5 + (Agi−50)/3 + level/5. Add ~+8 when you carry under 33% of capacity (combat.md). [C formula, I numbers]

Breakpoints and reasons:
- Each **+6 Cha = +3 SC** (≈ +3% on every song), plus +1 dodge per 5 and more mana regen. Charm is the best CP per point until ~90.
- Str 60 lets you wield every Str-60 1H weapon at full speed and gives capacity 2,880 (33% = 950: enough for a full leather kit). Str 70 meets the runic warhammer and starsteel weapons.
- Hea: max HP uses base Hea (+0.5 + level/16 HP per point). Bards have the lowest HP of the hybrids, so don't skip it.
- Wil: magic resistance only ((Int+3·Wil)/4). Take it late, or early if you hunt casters.
- Int: thievery/traps/stealth only. Leave it at the minimum.

## 4. Attack style and spells
Every song is learned from a songsheet (item type 9, ability 42 → spell). Use it to learn the song.
The magery and level checks are in LJNUTIL.C:9779 [C]. Songs have no level cap below their `cap`
field; damage scaling is `max += maxinc×level/maxlvls` (LJNSPELL.C:2239). Damage spells with
resist mode 0 are **never resisted** and only elemental resistances cut them (LJNSPELL.C:7520-7597) [C].

Damage songs (avg damage per mana) [C data, computed]:
| Song (cmd) | Lvl | Mana | diff | Target | L5 | L10 | L15 | L20 | L30 | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| discord (`disc`) | 3 | 2 | +8 | single | 4-16 | 6-19 | 7-21 | 7-21 | 7-21 | Damage that ignores armour. Cheap. L3-12 main nuke. |
| blasting (`blas`) | 10 | 4 | −15 | single | – | 9-27 | 11-31 | 12-34 | 12-34 | Cap 20 |
| draining (`dran`) | 11 | 8 | −10 | single | – | – | 7-18 | 9-18 | 11-20 | Steals HP. Resisted by MR. Weak per mana. |
| pain (`pain`) | 16 | 8 | −25 | single | – | – | – | 22-46 | 22-54 | |
| **shockwave** (`shok`) | 19 | 10 | −35 | **Full Attack Area** | – | – | – | 24-90 | 29-120 | Every monster in the fight takes the full roll. **Main attack from L19.** |
| sonic blast (`sbla`) | 22 | 12 | −45 | single | – | – | – | – | 46-72 | Shockwave usually beats it, even on one target |
| note of destruction | 0 | 0 | 200 | AoE 20-100 | — | | | | | Songsheet #1374 is not sold anywhere (drop/quest; source not traced) |

Melee: combat 5 with a 1H weapon. At L10 with a mace (3-12, speed 1350, Agi 55, light load), energy is
~350/blow, so ~2.9 blows/round [I from the combat.md energy formula]. Expect ~20-25 DPR before
crits. That beats discord (~12/round) early, so the **L1-9 pattern is: melee, and open with discord
or misfortune on tough targets**. From L19, shockwave out-damages melee on any multi-monster room.
BASH (×3 damage after DR, 2× energy, −15 AV) only pays against low-AC targets with this AV [I combat.md §2].
Backstab: sneak, then `bs` for the opener. With a dagger it adds 2×level + stealth/10 before
the (100+level)% multiplier. Worth using on the first hit only.

Support/utility songs (the bard's group value):
| Song | Lvl | Mana | Effect [C abilities] | Use |
|---|---|---|---|---|
| valour (`valr`) | 2 | 4 | **party** AV +5 (Bless), 35+5/level rounds | Always up in groups. Solo it's +5 AV for 4 mana. |
| misfortune (`mfor`) | 3 | 4 | all attackers −10 AV, 20+5/level rounds (resisted) | Cuts incoming hits. Use on tough fights. |
| wisdom / brilliance / might / agility / beauty | 4-9 | 4-10 | +5 Wil/Int/Str/Agi/Cha (self or one target) | Might = +5 Str for a melee ally. Beauty = +5 Cha = +2.5 SC for yourself. |
| foolishness / stupidity / weakness / clumsiness / ugliness | 5-10 | 6-10 | −5 stat on one monster | Low value |
| charming (`char`) | 4 | 4 | Enslave (living only) | Pets. Niche. |
| **life** (`life`) | 8 | 10 | **party healrate +50%**, 30 rounds | Faster resting for everyone. The bard's "healer" substitute. |
| traveling (`trav`) | 9 | 15 | party encumbrance +20%+ | Haul loot, stay under 33% load |
| silence / lethargy / dazzling / stunning | 12-18 | 12-24 | mute casters / slow / confuse / stun (resisted) | Lethargy (energy use 120%) and stunning control bosses |
| quickness (`quik`) | 15 | 12 | energy use 85% (−15%) | Self or a fighter ally ≈ +18% blows |
| heroism (`hero`) | 21 | 14 | +10 Str, +10 Agi, +2 damage | Self or a melee ally |
| enlightenment / sageness / knowledge / force / swiftness | 23-30 | 15-20 | party Int/Wil, MR+mana regen, crit+mana regen, +1 dmg/encumbrance, crit/dodge | Orfeo's songsheets, 60p each |
| evasion (`evas`) | 24 | 20 | self DR/MR/defence/dodge | Boss fights |
| soothing / elements / battle | 35-40 | 16-20 | party healrate +75 / elemental resist / dodge+crit+DR | Late |

Mana efficiency: discord 5-7 dmg/mana, shockwave 5.7-7.5/mana **per target**. At L20 a full bar
(96 mana) is ~9 shockwaves. Regen is ~8/tick, so **MEDITATE between fights** (it adds the same
amount again every 15 fast cycles) and rest with song of life up.

### Where to get songsheets [C shop data]
| Shop | Room | Markup | Sells | Access |
|---|---|---|---|---|
| Newhaven Spell Shop | 1:2144 | 0 | valour 4g, misfortune 8g, discord 8g | Newhaven start area |
| **Bard Song Store** | **1:851** | 100 | lore 0g, valour 8g, misfortune/discord 16g, wisdom/charm 30g, brilliance/foolishness 40g, might/stupidity 50g, agility/weakness 70g, clumsiness 90g, traveling/beauty 120g, blasting 150g, ugliness 120g | **Needs a room ticket** (item 924, 7g at the Tavern 1:311 Homely Hearth). Route: 1:311 s → 1:313 `u` (Item exit, ticket) → 1:847 n → 1:848 `e` (door) → Bard's Room. 7 steps. |
| Bard Training Room | 1:852 | **300** (4× cost) | life 240g, draining 7.2p, silence/lethargy 8p, dazzling 10p, quickness 12p, pain 14p, stunning 20p, **shockwave 24p**, heroism 34p, sonic blast 40p, evasion 60p | Same inn floor. Also class trainer L1-10 (4× fee: use 1:1376 instead). Markup on its songsheets **[I]**: shop markup applies to all items. |
| Orfeo's Stuff | 6:126-6:132 | 0 | enlightenment, sageness, knowledge, force, swiftness 60p each; soothing 80p; elements 70p; battle 100p; silver flute 1p, golden harp 5p, bardic lute 20p | 82 steps: 5g toll at 1:1381 e, then Item exits needing **rope and grapple** (#191, 5g, General Store 1:334) |

Budget: **save ~24 platinum for shockwave by L19.** It is the bard's power spike.

## 5. Gear by level band [C item/shop data; respect classes and armour ≤6]
Weapons (1H only; Str req; speed; price; room):
| Band | Weapon | Dmg | Str | Speed | Price | Where | Notes |
|---|---|---|---|---|---|---|---|
| 1 | club | 3-8 | 40 | 1000 | free | Newhaven 1:2141 | deleted at cleanup |
| 1-3 | dagger | 1-5 | 0 | 900 | 2g | 1:355 | BS AV +10: backstab opener |
| 2-8 | **mace** | 3-12 | 45 | 1350 | 8g | Colin's Maces 1:354 | best value early |
| 4-10 | heavy-mace / warhammer / battle axe | 4-13 / 4-14 / 4-15 | 50/60/60 | 1550-1725 | 10g/13g/8g | 1:354, 1:307 | Similar damage per energy. Pick by your Str. |
| 8-15 | serrated battle axe | 5-16 | 60 | 1775 | 4p | Axes 1:307 | |
| 10 | **jeweled main-gauche** (quest, off-hand) | ac3, acc +3, crit +3, BS min +5, Quality 50 | — | — | quest | placed in 1:2278 Large Tomb | Class 9 only, Magical 3, L10, **limit 1**. Worn **with** your weapon. |
| 15-20 | two-headed morningstar | 4-18 | 65 | 1725 | 15p | Jorah 1h Blunt 1:2329 (L15) | |
| 20+ | runic warhammer | 10-18 | 60 | 1675 | 40p | Dwarven Weapon Store 6:1344 (L20) | acc +9, Magical |
| 30+ | starsteel mace / etched adamant warhammer | 11-26 / 10-25 | 60/70 | 2300/2000 | 188p / 169p | 1:2329 (L30) | |
| 50 | Solarblade 16-35, Windblade 14-36, moonstone rod 15-35, Demontongue 11-40, … | | 60-75 | | not sold | drop/quest only (classes 6/8/9/10/11) | |

Off-hand (class-9 items only; these replace the main-gauche when you want casting):
| Item | Effect | Price | Where |
|---|---|---|---|
| silver flute | SC +1 | 1p | Orfeo's 6:126 |
| golden harp | SC +2, mana regen +5% | 5p | 6:126 |
| bardic lute | SC +3 | 20p | 6:126 |
| silverwood lute | SC +5, mana +10, BS AV +3 | 20p (limit 5) | Lost City 16:318 (no walk route found [C path search]) |
| main-gauche (#634) | ac2 | 10.5p | Rhudaur Exotic 2:2566 (black serpent key) |
| adamantite wrist guard | ac3.5 dr.4, L30 | 20p | Dwarven Armoury 6:1337 |

Armour (non-magical, type ≤6). Kit ≈ 850 weight, under 33% at Str 60:
| Slot | L1-10 | L10-20 | L20+ |
|---|---|---|---|
| Torso | rigid leather tunic ac13 dr1.3 wt400, 16g (1:310) | gianthair vest ac14 5p (Dwarven Leatherworks 6:1343); ogre-skin shirt ac15 dr1.5 L10 14p (6:1343) | wyvernhide tunic ac16 dr1.6 L20 105p (1:2328); lacquered battle armour ac17 L30 262p |
| Legs | rigid leather pants ac3 4g (1:310) | — | wyvernhide pants ac3.5 dr.5 L20 94p |
| Head | hard leather helm ac3 4g (1:2154) | — | wyvernhide helm ac4 L20 56p |
| Hands | hard leather gauntlets ac3 2g (1:310) | — | wyvernhide gauntlets ac4 L20 75p |
| Feet | hardened leather boots ac3 4g (1:2154) | padded black boots ac1 stealth +3 1.5p (Rhudaur Thieves 2:2561) | wyvernhide boots ac4 L20 75p |
| Back | travelers cloak ac1 4g (1:312). **Not greatcloak** (stealth −10). | crimson cloak ac2 45p (1:3307) | lyrist's companion ac3, BS +15, mana regen +20%, L40 (class 9; Recycler 6 1:516, only if a player sold one) |
| Arms | — | golden armbands ac.7 2.4p (1:290) | adamantite bracers ac2 L15 70p (6:1337) |
| Waist | leather belt 2s (1:310) | — | ogre-skin baldric ac1.5 L20 50p (3:541) / 60p (2:2566); shimmering sash (class 9, SC +3, dodge +10, mana +10, stealth −2, no level req) 60p at Lost City 16:318 |
| Neck | golden amulet ac.2 acc +1 1p (1:290) | | |
| Rings | amethyst ring ac.5 20g (1:290) | 1st alignment-quest ring at L10 (bosses.md) | |
Light: Half-Elf NV 25 covers −175 rooms. Carry a **lantern (2g, 1:334)** for −200 areas (Slimy Sewers, Smelly Cave, Ancestral Tomb, Rhudaur alleys).

## 6. Grinding progression [C monster/room data; F t51 ranking; I class fit]
Rule: max hit ≤ 25% of max HP, using the HP column of §3 (96 at L10, 143 at L15, 195 at L20).
Bard songs ignore armour (discord) or monster AC, so high-AC monsters are less of a problem than for melee.
| Band | Area (anchor) | Monsters hp/exp/maxhit | Access | Fit |
|---|---|---|---|---|
| 1-3 | **Newhaven Arena 1:2150** | giant rat 12/9/10, kobold thief 20/13/8, acid slime 25/16/9 … | arena max level 3; instant respawn | Melee. `borrow skiff` to Silvermere (notes/quests.md). |
| 3-6 | **Graveyard 1:817** | skeleton 43/55/11, zombie 62/70/14, giant bat 35/40/5 | none | Melee + discord. Undead are NonLiving, so charming/draining fail, but discord works. |
| 5-10 | **Crypt lvl 1-3** 1:835 / 1:1070 / 1:1276 | + ghoul 73/100/9, shade 75/140/15, wight 80/140/10 | −175 light (NV25 OK); hidden exits 1:1056 n, 1:1070 s, 1:1250 s | Best fit: low hits, close to the L10 quest |
| 6-10 | Strange Mansion 1:63 | vision 40/120/14, shadow spectre 70/160/20, astral shade 80/185/12 (all ac25-35) | none, ~31 steps | exp/hp 2.45. Spectral mage 200/1200/20 at 1:382. |
| 6-10 | Slimy Sewers 9:1 | slimes 30-55 hp / 60-125 exp / 12-14 | search w at 1:619; −200 (lantern) | Good exp/hp, but slimes are ac25-30. Use discord. |
| **10** | **Crypt class quest** (§7) | owlbear 93/225/28 | bone key from the mummy (1:1309) | jeweled main-gauche + train to L11 |
| 10-15 | Kobold caves 6:752 / 6:760 | kobold 30/80/9, kobold warrior 50/150/13 (ac35) | 134 steps; 6:760 −200 | Low hits. Blasting at L10+. Kobold king 250/2250/16 at 6:834. |
| 10-15 | Darkwood bandits 9:720 | bandit 55/125/12, bandit leader 100/275/18 | −100 | exp/hp 2.4, cash |
| 12-15 | Dark Caverns, Pool's Edge 1:1531 | tentacled abomination 100/250/12 | black star key (dark cultist 7%, Slums 1:1127) | |
| 15-20 | **Frozen Caverns 10:35** | grey wolf 125/375/21, guardian wolf 145/500/22 | search n at 10:3; −175 | hit 22 < 25% of 143 |
| 19-25 | **Dragon's Teeth Hills 2:177** (930 rooms) | harpy 140/450/12, gorgon 165/450/30, ankheg 140/450/30, dark monk 125/525/30 | none, 49 steps | Wide multi-monster area: **shockwave country**. Dark monks drop the black serpent key (20%). |
| 20-30 | **Ancestral Tomb 6:872** | tortured spirit 150/525/15, barrow wight 180/725/15 (ac50-60) | hidden w; −200 (lantern) | Hit 15, exp/hp 3.8. High AC makes melee poor, but songs don't care. |
| 20-30 | Elder Darkwood Grove 7:21 | vampire rosebush 110/200/14 … earth elemental 185/450/35 | hidden exit at Darkwood Small Clearing | Mixed packs suit shockwave |
| 30+ | Quaggoths 6:1811 | quaggoth 180/725/16, thonot/jald 200/1500/20-30 | mine pass 100g (6:1567) | Best 30+ solo fit (low hits) |
| 30+ | Lava Tube 6:1210 | salamander 225/1500/25 (Magical 1) | black star key + hidden exits | Songs hit Magical monsters, no magic weapon needed |
| 30+ | Black Cave 8:1 | cave lizard 115/325/15 … mind flayer 185/1025/25 | via DTH hidden exits | |
Avoid: Slum HO streets before L9 (half-ogre bodyguard hit 36); Ancient Crypt (skeletal warrior hit 60) until ~L25; Huge Cave orcs (hit 40-50); ogres/bugbears (hit 45-66) until ~L28.

## 7. Class quest and class items
**L10 crypt quest (Bard path)** [C room/exit/monster data; F general procedure, notes/quests.md §1]:
1. Crypt levels 1-3 as for every class (levers, `search s`, `push button`, mummy 100/200/20 at 1:1309 drops the **bone key** 100%). `use bone key n` → Shadowed Hall 1:1377.
2. Go `n` ×6 (1:1377 → 1:1424). At 1:1424 go `w` (class exit, **Bard only**) → 1:1429 `w` 1:2263 `w` 1:2264 `sw` 1:2265 `w` 1:2266, then `w` through a door → 1:2267 `n` 1:2268 `n`.
3. 1:2268 n and 1:2270 w are **Trap** exits (params 50/70). Bards have Find/Disarm Traps: `search`/`disarm trap <dir>` first [C exit types; I effect].
4. 1:2271 Marble Chamber: **owlbear 93/225/28** (ac20) drops the **copper key** 100%. `use copper key w` → 1:2272, then `se` (class exit) → 1:2277 `se` → **1:2278 Large Tomb**: bard spirit (Lawful Good, **don't attack**), the **jeweled main-gauche** (limit 1) and the Super Bard Trainer (shop 59, markup 500, trains L11 only). `go portal` (blue portal) to leave.
5. Bring ~30g for the L11 training fee (5 silver × level × 6) [C markup 500].

Class-only items [C item.classes]: jeweled main-gauche (L10 quest), the instruments (flute/harp/lute, Orfeo's), silverwood lute and shimmering sash (Lost City), lyrist's companion (L40 cloak), elven harp (SC +10, mana regen +10, L20), blackwood harp (evil-only, SC +15, dodge +10, L45), battlehorn (L25, +2 dmg). The last three have no shop source (drop/quest) [C].
Also open to Bard: soulblighter (14-35 1H, evil 210, L50, old lady shawl shop 17:776), the L50 class-group weapons above, and thief's pouch.
Shared quests: alignment quests L10/L20, Ice Sorceress (+1 AC), Phoenix Feather, Sunstone Wristband. See `test/play/corpus/bosses.md`.

## 8. Group role (brief)
Party buffer and secondary AoE: **valour** (+5 AV for the whole party, 4 mana, long duration), **life** (+50% party regen), traveling (encumbrance), quickness/might/heroism on the tank, misfortune/lethargy/stunning on bosses, **shockwave** for packs from L19. Picklocks/traps let the party through locked or trapped exits. Not a healer. A separate agent covers grouping in depth.

Group role: full mechanics, compositions and spots in `kb/grouping.md`.
