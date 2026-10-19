# Mystic (WCCCLASS 15): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats `hp/exp/maxhit`. Prices include markup, not Charm. `1p` = 100 gold.
Steps from Town Square 1:224. Stealth/backstab formulas: `ninja.md` §1.

**One-line path:** Dark-Elf, Agi-first. Fight bare-handed: KICK/JUMPKICK to L9, then **PUNCH** with the
L10 clawed gloves (fast blows → up to +20% crit from ~L15), Kai buffs (tiger/boar/mantis) before pulls,
and an unarmed sneak-backstab opener. Arena → Graveyard → Slimy Sewers → L10 crypt quest (clawed
gloves) → kobolds/bandits → wolves → Ancestral Tomb. Heaviest exp penalty (+150%), robes only.

## 1. Class facts [C: WCCCLASS rec 15]
| Field | Value | Meaning |
|---|---|---|
| exp | **+150%** | L2 = 10×(250+race) (Dark-Elf 2,950; Human 2,500) |
| basehp / hplevel | 4 / 4 | +4 HP/level + ~2.5 |
| combat | 5 | |
| magery | **Kai (5), level 1** | Kai = `level−1`, regenerates **1 per tick** regardless of stats (LJNUTIL.C ~4935; LJNBKGND.C:1689-1702). Kai powers are **learned automatically** on training (LJNUTIL.C:8149 `update_kai_powers`); no scrolls. Spellcasting base 500 → never fails. |
| weapons | **9 = Limited** | Only exempt items 100 (quarterstaff) and 68 (dagger) (LJNGLOBL.C:172), plus items naming class 15 or with "Class Item Inclusion 15" (stiletto, knife, staves, wavy/serpentine daggers…). Unimportant: Mystics fight unarmed. |
| armour | **1 = Robes** | Plus class-15 items (clawed gloves, demonhide bracers/sandals…) |
| abilities | 29 Punch 1, 30 Kick 1, 35 Jumpkick 1, **34 Dodge 25**, **58 Crit +10**, **142 Hit Magical 5**, 103 Class Stealth, 31 Bash | |
| Perfect Stealth | keepable from **L27** | LJNUTIL.C:705 |
| Alignment-quest reward | +1 max damage (fighter group) | LJNUTIL.C:479-487 |
| L11 trainer | Super Mystic Trainer, shop 51 at **1:2240** | §7 |
| L1-10 class trainer | Back Room, Mystic Trainer 1:333 (5 steps, mk300); prefer 1:1376 (mk0) | |

### Martial arts [C LJNCOMBT.C:430-500, 1255-1270, 887-901, 2558-2580; LJNCMDA.C:864-940]
| Move | Command | Speed figure | Damage (n = ability value 1) | Multiplier |
|---|---|---|---|---|
| punch | `punch <mob>` (or `a <mob>` unarmed) | 1150 | min `L/8+2`, max `(L+3)/4+6`; frozen at L20 values (4-11) | ×1 + Punch DMG items/spells |
| kick | `kick <mob>` | 1400 | min `L/8+2`, max `L/6+7` (L20+: 4-10) | ×1.33 |
| jumpkick | `jumpkick <mob>` | 1900 | min `L/8+2`, max `L/6+8` (L20+: 4-11) | ×1.66; keeps AV in group ranks |
- All add `(Str−50)/10` to max and use the normal AV formula + "Alter Punch/Kick/Jumpkick AV" abilities. Energy = `speed×1000/((5L+45)(Agi+150)/6)` × load factor (0.75-1.25), floor 200 (max 5 blows/round).
- **Crit** = `mabase + crit abilities` where mabase = `L/10 + (Int−50)/10 + (Agi−50)/20 + (Cha−50)/30` (1..75) and class crit +10. When a move's raw cost < 200, load ≤ 66% → **+min(20, 200−cost+(Agi−50)/10)** crit (halved at 33-66% load). Crit damage = 2×max..4×max.
- **Hit Magical 5**: fists/feet hurt "Magical" monsters only up to magic level 5 (the player value is subtracted first; fists can never add more). Early-mid "magical" monsters in the data are level 1-2 (moaning spirit 1000-exp, tortured spirit 1250-exp, banshee 3000-exp) → fine [C WCCKNMSR ability 28].
- Unarmed **backstab**: `punch`/`a`/`bs` while sneaking with no weapon = backstab (LJNCMDA.C:458, 930): damage `(2×0+Str bonus×2 + 2L + stealth/10 + BS abilities)×(100+L)%` → ~30 at L10, ~40 with *way of the cat* [I].

Modelled per-energy output (light load, Str 60-75, before gear/kai; dpe = avg dmg per 1000 energy) [I]:
| L / Agi | punch | kick | jumpkick |
|---|---|---|---|
| 5 / 80 | 2-9, cost 342, dpe 18 | 2-10, 416, 18 | 3-14, 566, 18 |
| 10 / 90 | 3-10, 241, 29 (**+3/+3 with clawed gloves → dpe ~41**) | 3-11, 294, 29 | 4-16, 400, 29 |
| 15 / 100 | 3-11, **<200 → fast crit**, 38+ | 3-13, 224, 42 | 4-18, 304, 41 |
| 20 / 110 | 4-13 @200 + crit +20 | 5-15 @200 + crit | 6-21, 241, 62 |
| 30 / 120 | | | 6-21 @200 + crit, 75 |
→ **kick/jumpkick L1-9; punch L10-~27 (gloves + fast crits); jumpkick again once its raw cost < 200 (~L28, Agi 120) or with kick/jk DMG gear.** Way of the mantis (energy ×0.85) moves every threshold ~3 levels earlier.

### Kai powers (auto-learned) [C WCCSPELS magery 5]
| Lvl | Power (cmd) | Kai | Effect |
|---|---|---|---|
| 2 | way of the swan (`swan`) | 1 | heal 3 (+1/2 lvls) |
| 3 | way of the owl (`owl`) | 2 | MR +10, 60 rounds |
| 4 | way of the cat (`cat`) | 3 | Stealth + BS min/max (+1 per 2 lvls), 60 rounds |
| 5 | **way of the tiger** (`tige`) | 3 | +2 dmg (+1/10 lvls) and punch/kick/jk AV, 60 rounds |
| 6 | way of the rat (`rat`) | 4 | poison immunity |
| 7 | **way of the boar** (`boar`) | 4 | +15 AV (3rd AV), −5 DV, 20 rounds |
| 8 | way of the dragon (`drgn`) | 3 | 13-14 (+1/lvl) ignoring armour, non-magical, 1000 energy |
| 9 | way of the monkey (`monk`) | 3 | Dodge +1 per 4 lvls, DV +1, 40 rounds |
| 10 | **way of the mantis** (`mant`) | 6 | energy use ×85% (more blows), 20 rounds |
| 12 | way of the tortoise (`tort`) | 3 | DR +10 raw (−1/hit), 45 rounds |
| 15 | way of the bear (`bear`) | 4 | Str +20, Agi −10, 30 rounds |
| 16 | way of the troll (`trol`) | 5 | heal |
| 18/21/24/27/30 | cobra (poison) / hypnotic hands (−15 AV −8 DV confuse) / exploding fist (0-30) / **pressure points** (punch AV+10 DMG+3, kick +2, jk +1) / hand of death | 6/7/5/8/12 | |
Kai pool is tiny (L10 = 9): pre-fight `tige` + `boar` (7) and rest; `mant` (6) for boss fights.

## 2. Race choice [C WCCRACE / I ranking]
| Rank | Race | exp (total) | Agi | Int | Str | Hea | Extras | Why |
|---|---|---|---|---|---|---|---|---|
| **1** | **Dark-Elf** | +45 (295%) | 50-120 | **50-120** | 40-90 | 30-90 | Race stealth (+10 stealth, +5 BS AV), **NV 80**, **Crit +1**, 3rd-AV +3 | Int feeds mabase (crit) and stealth; extra crit and AV stack with the class's. Sees in −230 rooms. |
| 2 | Goblin | +35 (285%) | 55-125 | 45-115 | 30-70 | 40-100 | Race stealth, NV 65 | Cheaper, more Hea; Str cap 70 = −2 max dmg vs Dark-Elf 90. |
| 3 | Nekojin | +50 (300%) | **60-130** | 40-100 | 40-100 | 30-80 | Race stealth, NV 50, tracking | Highest Agi/Cha starts (dodge, energy) and Str 100 cap; Hea cap 80. |
Human (250%) is 15% cheaper but loses stealth synergy, NV and crit.

## 3. Stats / CP plan [I, breakpoints C]
Start (100 CP): **Dark-Elf Str 58, Int 50, Wil 30, Agi 75, Hea 46, Cha 46 (99)**.
- Agi first (energy → blows, crit via mabase /20 and fast-crit /10, dodge /3, AV /6, stealth /4). Then Str to 60/70/80 (+1 max per 10 on every blow: worth more to a 5-blow class than to a basher). Cha 46/50 (dodge, mabase /30). Int 60/70 (mabase /10 = +1% crit each). Hea 55 by L10.
- Robes give almost no AC: dodge is the defence. Dodge at L10 (Agi 90, Cha 50, unencumbered) = 25+0+2+13+~9 ≈ 49 → ~50% of landed blows avoided vs AV 75. Never exceed 33% load.
- Max HP Dark-Elf Hea 50: ~90 at L10, ~160 at L20.

## 4. Attack style
1. `sneak` → `cat` (L4+) → `punch <mob>` unarmed = backstab opener (~30-40 at L10).
2. Then **kick** (L1-9) / **punch** (L10+) / **jumpkick** (late), per the table in §1. Re-type the move once; autocombat repeats it (LJNACMB.C:736).
3. Pre-buff `tige`, `boar`; `mant` for bosses; `swan`/`trol` to top up.
4. Do **not** wield a weapon (a weapon disables punch unless you type `punch`, and Limited weapons are weak) [C LJNCMDA.C:458].

## 5. Gear by level band [C; armour type ≤ 1 or class 15]
| Band | Hands/arms (attack gear) | Body | Neck/waist/other |
|---|---|---|---|
| L1-9 | padded gloves (free); silk gloves 8g (Arlysia) | padded set (free 1:2142); **robes ac6 1g** (Robes Shop 1:309); skullcap ac2 6g (1:2154); travelers cloak 4g (1:312) | golden amulet ac0.2 1p (1:290) |
| L10 | **clawed gloves** (quest: hands ac3, Punch DMG+3, Punch AV+3, Magical 1, Mystic-only, limit 1); or brass knuckles (Punch DMG+1, L10, 50g 1:305; also dread mystic 9:930 35%) | iron crown ac2 (kobold king 100% / 5p 6:1337) | bone charm (crit+1, dodge+1) 1p (Curio 1:332); golden armbands ac0.7 2.4p (1:290) |
| L15-20 | adamantite bracers ac2 dr0.5 L15 70p (6:1337) | | cat's-eye pendant (crit+2, dmg+1) 100p (1:332) |
| L20-30 | **demonhide bracers** (Punch AV+5 DMG+2, L20) 18.75p (Jorah 1:3306) or chimera drop 3:583 (10%); **demonhide sandals** (Kick/Jk DMG+3, Kick AV+4, L20, chimera 10%) | **black silk robes ac9 dodge+5 L20** 7.5p, black silk pants/slippers L20 (Jorah 1:3303) | **tiger-claw necklace dodge+15 L25** 187.5p (1:3307); belt of might dmg+1 112.5p (1:3307) |
| L30-40 | sapphire bracers (Punch AV/DMG+5, L40, Kai Master 1%); curved fang (Punch DMG+2, L35) | prismatic robes ac10 dodge+5 L30 20p / astral robes ac12 L35 60p (Curio 1:332); silversilk tunic ac15 dodge+10 L40 625p (Elven 3:541) | peasant cloak (crit+2 dmg+2, L40) |
| L50 | enigma gloves/sandals (Enigma Lord 5%), Kai battle-staff (no source traced) | | |

## 6. Grinding by level band [C corpus leveling.md / I fit]
Mystic fit = many-blow DPS with dodge, low AC: prefer monsters with low AC (your AV is modest) and hits ≤ 20% of HP; magic level ≤ 5.
| Levels | Area | Monsters (ac) | Access | Notes |
|---|---|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | rats/worms/slimes (ac 0-10) | maxlevel 3 | |
| 3-6 | **Graveyard 1:817** | skeleton 43/55/11 (ac8), zombie 62/70/14 (ac5), giant bat (dodge 20) | none | Low AC = punch heaven |
| 5-10 | Slimy Sewers 9:1 (`search w` 1:619) | slimes (ac 25-30) | light −200 (NV) | |
| 6-10 | Crypt 1:835 (ghoul ac10, wight ac20) | | hidden exits | quest route |
| 10-15 | **Kobold caves 6:752** (kobold ac30, warrior ac35, hit ≤ 13) / Darkwood bandits 9:720 (ac20-40) | | 186 / ~98 steps | Bandits ac 20 suit punching better |
| 15-20 | **Frozen Caverns 10:35** grey wolf 125/375/21 (ac20), guardian 145/500/22 (ac30) | | `search n` 10:3 | Best Mystic fit: low AC, Animal |
| 15-20 | Rhudaur Dark Alleys 2:1808 (cutpurse ac50 — skip; depraved murderer 130/375/15 ac30-50) | | black serpent key | |
| 20-30 | Shadow Fist Temple 9:913: dark apprentice 110/375/24, dark monk 125/525/30, dark mystic 150/775/36 | | black serpent key; t51: monks non-hostile to evil [F] | Thematic and the dread mystic 9:930 (360/9250/50) drops **brass knuckles** 35% |
| 20-30 | Ancestral Tomb 6:872 (ac 50-60: open with backstab, use boar) ; Dragon's Teeth 2:177 harpies 140/450/12 | | | |
| 30+ | Quaggoths 6:1811 (ac45, hit 16-30) | | mine pass 100g 6:1567 | |
Avoid: stone elementals (dr 15 eats small punches), skeletal warriors/minotaurs/elite orcs (hit 60-70 vs robes).

## 7. Class quest: L10 crypt → clawed gloves + L11 [C rooms/exits]
From 1:1309 (mummy; bone key): `use bone key n` → 1:1377 → `n` ×3 → 1:1421 → **`w`** (Class 15) → `w` ×7 → Marble Room 1:2219 → `w`,`nw` → 1:2233 → `push button` → **fire lizard 93/225/16** (obsidian key 100%) → `use obsidian key e` → 1:2235 → `e` → Marble Chamber 1:2226 → **`s`** (Class 15) → 1:2229 → `s` → **Large Tomb 1:2240**: mystic spirit, clawed gloves placed (limit 1). `train` (shop 51, mk 500), `go portal` → 1:834.
(Alternatives at 1:2219: `s`,`sw` troll 93/225/35 iron key; `n`,`ne` ogre 100/225/36 stone key — see warrior.md §7.)

## Group role
Melee damage with a backstab opener. Robes plus +150% exp: weak early in groups (it eats 1/N of the hits), strong late. Box it with a plate healer. See `kb/grouping.md`.
