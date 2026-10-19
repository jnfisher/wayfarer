# Ranger (WCCCLASS 14): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats `hp/exp/maxhit`. Prices include markup, not Charm. `1p` = 100 gold.
Steps from Town Square 1:224. Shared: `test/play/notes/combat.md`, `test/play/corpus/leveling.md`,
`ninja.md` §1 (stealth/backstab formulas), `warrior.md` §1 (DR/DV).

**One-line path:** Dwarf (or Half-Orc for speed). A Warrior-grade basher in studded leather with
druid utility: **mend** (L2) for self-healing, **barkskin** (L7, DR+DV), and class stealth to sneak
past what you don't want to fight. BASH a 2H weapon. Arena → Graveyard → Slimy Sewers/Crypt.
L10 crypt quest (witchwood spear). Kobolds → wolves → Ancestral Tomb. Heaviest exp penalty here (+150%).

## 1. Class facts [C: WCCCLASS rec 14]
| Field | Value | Meaning |
|---|---|---|
| exp | **+150%** | Joint-worst with Mystic. L2 = 10×(250+race) exp (Dwarf 2,800). |
| basehp / hplevel | 5 / 4 | +5 HP/level + ~2.5 random |
| combat | 6 | Warrior-grade melee AV |
| magery | **Druid (3), level 1** | All druid spells in the data are magelvl 0 except *camouflage* (1): a Ranger can learn every druid spell [C WCCSPELS] |
| mana / SC | mana `2×(L+3)`; SC = `2L + (Int+Wil)/3 + 5` | Int and Wil count equally (LJNUTIL.C ~4925) |
| weapons / armour | 8 = All / **6 = Studded Rigid Leather** | Shields only up to armour type 6 (buckler) unless class-listed (ironwood/darkwood shields, cls 13/14) |
| abilities | 31 Bash, **38 Tracking**, **103 Class Stealth** | Sneak, hide, backstab (1H weapon with ability 116) |
| Smash | keepable from **L22** | LJNUTIL.C:692 |
| Perfect Stealth | keepable from L20 | sneak always succeeds (LJNCMDA.C:2900) |
| Alignment-quest reward | backstab min/max +10, stealth +2 | LJNUTIL.C:504-510 |
| L11 trainer | Super Ranger Trainer, shop 52 at **1:2241** | crypt quest (§7) |

### Key druid spells [C WCCSPELS/WCCSHOPS; Druid Spell Shop = Hollow Tree 1:1314, 18 steps]
| Spell (cmd) | Lvl | Mana | Diff | Effect | Price |
|---|---|---|---|---|---|
| vine strike (`vine`) | 1 | 1 | +15 | 4-9 | free 1:1314 / Newhaven 1:2144 |
| starlight (`star`) | 1 | 4 | | room light 175, 80 rounds: **replaces a lantern** | free |
| mend (`mend`) | 2 | 4 | +8 | heal 3-12 | 16g (8g Newhaven) |
| elemental chaos | 4 | 3 | +5 | 7-13 | 40g |
| **barkskin** (`skin`) | 7 | 15 | −5 | DR +10 raw (−1/hit… +10/10 lvls) and DV +5, 40 rounds | 70g |
| camouflage (`camo`) | 7 | 10 | −5 | stealth +3, backstab AV +10 | 70g Jorah Druid 1:2334 |
| stonestrike (`strk`) | 8 | 5 | 0 | 14-11(+2/lvl) | 1.5p |
| rapid healing (`rhel`) | 11 | 8 | −20 | regen ×2, 60 rounds | 3.6p |
| swarm (`swar`) | 12 | 28 | −10 | 24-36 room attack | 4p |
| acid jet (`acid`) | 13 | 8 | −14 | 24-18(+5/2 lvls) | 3p Aldreth 1:2306 |
| entangle | 14 | 12 | | hold, 4 rounds | 2.8p |
| natural prowess | 25 | 20 | | AV/dmg/backstab/crit buff | 25p 10:273 |

## 2. Race choice [C WCCRACE / I ranking]
| Rank | Race | exp (total) | Str | Int/Wil | Hea | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Dwarf** | +30 (280%) | 50-110 | 30-90 / **50-120** | 50-120 | NV 75, MR 10, Encumbrance +20% | Wil starts 50 (half of SC), Str/Hea melee, sees in −200 rooms (no lantern/starlight). |
| 2 | Half-Orc | **0 (250%)** | 45-110 | 30-90 / 30-95 | 50-120 | none | 11% less exp. Casting weaker; needs a light (or *starlight*). |
| 3 | Half-Elf | +15 (265%) | 40-90 | 45-110 / 40-100 | 30-100 | Mana +10, NV 25 | Best SC/mana; weaker melee. |
Race stealth races (Elf, Dark-Elf, Goblin, Halfling, Nekojin) add +10 stealth/+5 BS AV with class stealth (LJNUTIL.C:4790), but their Str/Hea caps make a poor basher; only worth it for a backstab-first Ranger.

## 3. Stats / CP plan [I]
Start (100 CP): **Dwarf Str 80, Int 38, Wil 55, Agi 42, Hea 60, Cha 31** (~100); Half-Orc Str 75, Int 38, Wil 40, Agi 45, Hea 58, Cha 31 (92 + 8 spare → Str 77).
- SC Dwarf L7 = 14 + (38+55)/3 + 5 = 50 → barkskin 45%; push Int+Wil to ~150 combined by L15 for ~75% [C formula / I].
- Ladder: the 2H `basher` core (`statplan`, `kb/stat-builds.md`) with the casting stats
  inserted:
  1. Cha 31
  2. Str +25 / +31 / +35 (Dwarf 75/81/85)
  3. Hea 64 / 72
  4. Wil and Int alternately (each +1 SC per 3 points summed)
  5. Agi 45 / 48 / 53
  6. Hea 80-90
  7. Str +40, Agi 56-62, Hea 96-100.
- **Don't max Str** [I, model]. For a Ranger in leather, Str-max is −26% kills/h on the mixed
  metric (−40% in hard-hitting areas). The `basher` core is +5% vs the old ladder, the largest
  gain of any class, because leather makes Hea and dodge matter more.
- Leather armour (no dodge penalties) and lighter loads mean the **<33% light-load bonus is reachable** (+12-15 accuracy, up to +10 dodge, LJNCOMBT.C:703): keep it. Str × 48 × 33% ≥ gear weight.
- Max HP Dwarf Hea 60: ~111 at L10.

## 4. Attack style [C combat.md §2]
- **BASH** with a heavy 2H weapon is the main attack (same rules as Witchunter/Warrior).
- Optional opener: **sneak + backstab** with a 1H weapon (any weapon with ability 116; the class quest spear is 2H). Damage `(2×wpn + 2L + stealth/10) × (100+L)%` at full-round energy, hit% = AV − DV directly (ninja.md §1). Only worth it with Perfect Stealth (L20) or a race-stealth race [I].
- *mend* between fights, *barkskin* before tough pulls, *starlight* in dark areas, *entangle* (L14) to stop a dangerous hitter.

## 5. Gear by level band [C]
| Band | Weapon | Armour (type ≤ 6) | Notes |
|---|---|---|---|
| L1-3 | quarterstaff (free) | padded set (free) | |
| L2-10 | **greataxe** 6-22 10g (1:307), maul 8-25 20g, battle-hammer 6-22 18g (1:354) | **rigid leather tunic ac13 dr1.3 wt400 16g**, rigid leather pants ac3 4g, hard leather gauntlets ac3 2g (1:310); hard leather helm ac3 4g, hardened leather boots ac3 4g (1:2154); travelers cloak 4g (1:312); buckler ac1 2g (1:2153, only with 1H) | Leather kit raw dr ~2 → small; Ranger relies on HP + mend, not DR. |
| L10 quest | **witchwood spear** 6-22, 2H sharp, s45, acc 6, MR+5, SC+1, 5% chance to cast *hold* (spell 779) on hit, Magical 2, Ranger-only, limit 1 | | Same damage as a greataxe at Str 45 and lighter (100). |
| L10-20 | three-headed flail 9-23 4p (L15, 1:354), massive wooden club 10-30 (L15, 1:2329), bone-handled headcutter 7-30 (drop) | **gianthair vest ac14 dr1.4 5p** (Dwarven Leatherworks 6:1343), **ogre-skin shirt ac15 dr1.5 L10 14p** (6:1343), adamantite bracers ac2 dr0.5 L15 70p (6:1337) | |
| L20-30 | etched adamant greatsword 10-34 L20 206p (1:3305) | wyvernhide tunic/helm/gauntlets/boots/pants L20 (Jorah 1:2328, mk275); ironwood shield ac3 dr0.7 L20 25p (Elven Shop 3:541, Ranger/Druid only, Magical) | |
| L30+ | Ranger-only: silverwood helm (L30, woodelf lord 3:543 10%), mantle of the woodsman (L40), Dusk 29-74 2H (L60, Darken Beast Lord) | lacquered battle armour L30 262.5p | |

## 6. Grinding by level band [C areas/stats corpus leveling.md / I fit]
| Levels | Area (anchor) | Monsters | Access | Ranger fit |
|---|---|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | rats/worms/bugs ≤ 25 hp, hit ≤ 10 | arena maxlevel 3 | |
| 3-6 | Graveyard 1:817 | skeleton 43/55/11, zombie 62/70/14 | none | *mend* between pulls |
| 6-10 | **Slimy Sewers 9:1** | slimes 30-55 hp, 60-125 exp, hit ≤ 14 | `search w` at 1:619; light −200 (*starlight* or Dwarf NV) | best exp/hp in band |
| 6-10 | Crypt L1-3 1:835 | ghoul/shade/wight, hit ≤ 15 | hidden exits; light | quest route |
| 10 | Crypt quest §7 | fire lizard 93/225/16 | L10 | |
| 10-15 | Kobold caves 6:752; Darkwood bandits 9:720 (light −100) | kobold warrior 50/150/13; bandit leader 100/275/18 | 186 / ~98 steps | Bandits are outdoors-style: Ranger's natural turf, cash drops |
| 15-20 | **Frozen Caverns 10:35** | grey wolf 125/375/21 (Animal) | `search n` at 10:3 | *charm animal* (L5) works on Animals [C ability 80] — untested in practice [I] |
| 15-20 | Gnoll Encampment 2:1107 (axeman hit 48) | | 104 steps | from ~L17 (≥ 190 HP) |
| 20-30 | Ancestral Tomb 6:872 (hit 15); Elder Darkwood Grove 7:21 (plants/golems, hit 35-50; hidden exit at Darkwood Small Clearing); Dragon's Teeth 2:177 | | | Grove is "forest" content (t51 Mage list) |
| 30+ | see corpus leveling.md 30+ | | | |
Avoid early: Orc Mansion / Labyrinth (hit 70) — no plate DR.

## 7. Class quest: L10 crypt → witchwood spear + L11 [C rooms/exits]
From 1:1309 (mummy, bone key): `use bone key n` → 1:1377 → `n`,`n` → 1:1420 → **`w`** (Class 14) → `w` ×7 → Marble Room 1:2219 → `w`,`nw` → 1:2233 `push button` → fire lizard 93/225/16 (obsidian key) → `e` → 1:2235 → `e` → Marble Chamber 1:2226 → **`e`** (Class 14) → 1:2230 → `e` → **Large Tomb 1:2241**: ranger spirit (don't attack), witchwood spear placed (limit 1). `train` (shop 52, mk 500), `go portal`.
(Alternatives at 1:2219: `s`,`sw` troll 93/225/35 iron key; `n`,`ne` ogre 100/225/36 stone key — see warrior.md §7.)

## Group role
Melee damage with self-sustain (mend can also target others) and stealth for scouting. Shares the Druid's swarm. Exp +150%: it lags when boxed with low-exp% classes. See `kb/grouping.md`.
