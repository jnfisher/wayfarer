# Warrior (WCCCLASS 1): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats are `hp/exp/maxhit`. Prices include shop markup, not Charm
(× (110 − Cha/5)%). `1p` = 100 gold. Steps are walking distance from Town Square 1:224.
Shared formulas: `test/play/notes/combat.md`; shared areas: `test/play/corpus/leveling.md`.

**One-line path:** Half-Ogre, Str 100 at creation. BASH with the heaviest 2H weapon. Get into
**platemail** as fast as Str allows: its DR cancels most early hits outright. Arena, then Graveyard,
then Slimy Sewers/Crypt. Do the L10 crypt quest (golden battleaxe, train to 11). Then kobolds,
wolves, gnolls, Orc Mansion. From ~L20 plate lets you farm hit-50/70 areas others must skip.

## 1. Class facts [C: WCCCLASS rec 1]
| Field | Value | Meaning |
|---|---|---|
| exp | **+0%** | Cheapest melee class. L2 = 10×(100+race) exp (Half-Ogre 1,300; Half-Orc 1,000). |
| basehp / hplevel | **6** / 4 | +6 HP/level, + `genrdn(0,5)` (~2.5) per level trained (LJNUTIL.C:4969) |
| combat | 6 | AV `(skill+training)*2…` uses combat−1 = 5 (Witchunter 6). 10 AV below a Witchunter at L10 (+1 combat = +2×(2+√L) AV). [C LJNCOMBT.C:700] |
| magery | none | No mana, no spells. Spellcasting base −150. |
| weapons | 8 = All | Every weapon type, plus shields |
| armour | **9 = Platemail** | Only Warrior, Paladin and Cleric wear plate (WCCCLASS armour field) |
| abilities | 31 Bash (0) | Nothing else. Every class has Bash. |
| Smash (ab 32) | may be kept from **L22** | LJNUTIL.C:692. No source of `giveability 32` found in the data (combat.md §2) [C/I]. |
| Alignment-quest reward | +1 max damage | quest2/3 ≥ 8 or quest4 ≥ 4 (LJNUTIL.C:479-487). Quest1 done = Defence +1. [C] |
| L11 trainer | Super Warrior Trainer, shop 49 at **1:2231** (crypt, markup 500) | The only practical L11 trainer. Universal Training Room 1:1376 stops at L10; 1:2305 starts at L12. [C shops] |

### Why plate matters (the Warrior's real class feature) [C]
- Player DR = sum of worn `item.dr` (raw tenths) + abilities; **each blow loses DR/10** (LJNCOMBT.C:654, 760, 1371). A blow that ends ≤ 0 does nothing.
- Player DV = sum of worn `item.ac` / 10 (+ "Alter Defence Value"). Monster hit% = 100 − 140·DV²/AV², min 10 (LJNCOMBT.C:1316).
- Example kit *half-plate corselet + light plate leggings + horned helmet + plate gauntlets + plate boots*: raw ac 190+80+75+70+65 = 480 → **DV 48**; raw dr 90+20+15+7+15 = 147 → **−14 per hit**; 2,500 wt; ~490 gold. Against an AV-75 monster: hit chance 43%, and the zombie (max 14), kobold warrior (13) and grey wolf (21) do 0-7 per hit. [C formula, I numbers]
- Full plate corselet + full plate leggings + visored greathelm + plate gauntlets + plate boots + kite shield: DV ~60 (hit chance at the 10% floor vs AV ≤ 75), **−17 per hit**, 3,175 wt.
- Cost: plate pieces carry Dodge −5…−17 and Stealth penalties, and the weight pushes you past 33% (losing +12-15 accuracy and up to +10 dodge, LJNCOMBT.C:703). Above **66%** you lose move ticks (LJNBKGND.C:448). Max weight = Str × 48 (+36/pt over 100).
- Rule [I]: **effective maxhit = monster maxhit − your DR/10.** Use that instead of raw maxhit in the danger test (corpus `min_hp_suggested` ignores DR).

## 2. Race choice [C stats WCCRACE / I ranking]
| Rank | Race | exp (total) | Str | Hea | Agi | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Half-Ogre** | +30 (130%) | **70-150** | **60-150** | 20-60 | hpbonus **+1/level**, DR +20 raw (−2/hit), MR +15 | Starts at Str 100 / Hea 75 on 100 CP. Str 100 = +16 AV, +5 max dmg, 4,800 capacity: carries full plate under 66%. 7 HP/level. Low Agi barely matters for a plate-wearing basher (energy is capped at 1000 anyway, combat.md §2). |
| 2 | Half-Orc | **0 (100%)** | 45-110 | 50-120 | 40-100 | none | Fastest levelling (23% less exp than Half-Ogre). Better Agi. But Str 80 at creation (3,840 cap: plate only from Str ~95) and no DR. Needs light below −150. |
| 3 | Dwarf | +30 (130%) | 50-110 | 50-120 | 30-90 | **Night vision 75**, MR +10, **Encumbrance +20%** | No lantern down to −225 rooms. +20% capacity offsets the Str gap for plate. Better Wil (50-120). |
| alt | Kang | +35 | 55-120 | 50-110 | 30-90 | Poison immune, DR +10, **Defence +5 (DV)** | Defence 5 is +5 DV on top of plate. Worse than Half-Ogre on Str/HP. |
Avoid: Halfling/Goblin/Elf/Nekojin (Str caps 60-100, low Hea). Human is a strictly worse Half-Orc for melee (same exp, lower Str/Hea caps).

## 3. Stats / CP plan [C costs LJNGAME.C:1645 via statplan.stat_cost / I priorities]

**Use `statplan.allocate_cp(..., ladder="basher")` (the default). The analysis is in
`kb/stat-builds.md`.**

In short: Str first, but only through its cheap CP bands. Then Hea to the HP floor, then
alternate Str, Agi and Hea. **Don't max Str.**
- Str-max is −18% kills/h on the mixed metric for a Kang Warrior (−32% in hard-hitting areas;
  Half-Ogre −4%). Its bash DPR is +24-32% higher, but the HP and regen loss costs more.
- `basher` is +2% against the old ladder.
- A weapon above your Str is still fully usable: +1.5% energy per missing point, which is free
  while the bash is capped at 1000.
- What does need Str is **weight**. Plate plus a 300-weight maul goes over 33% load (−12 AV,
  −8 dodge) unless Str × 48 × 0.33 covers it.

Starting 100 CP on `basher` (all races basecp 100):
| Race | Str | Int | Wil | Agi | Hea | Cha | banked |
|---|---|---|---|---|---|---|---|
| Half-Ogre | **101** | 20 | 25 | 33 | 72 | 31 | 0 |
| Half-Orc | 76 | 30 | 30 | 40 | 71 | 31 | 2 |
| Dwarf | 81 | 30 | 50 | 30 | 71 | 31 | 2 |

The ladder (`"+N"` = N above the race minimum):
1. Cha 31
2. Str +25
3. Hea 64
4. Str +31
5. Hea 72
6. Agi 45
7. Str +35
8. Agi 48
9. Cha 36
10. Hea 80, 86
11. Agi 53
12. Hea 90
13. Str +40
14. Cha 41
15. Agi 56
16. Hea 96 / 100
17. Str +45 / +50 / +55, interleaved with Agi 62-80 and Hea 105-110

Targets by L20 (340 CP):
| Race | Str | Agi | Hea |
|---|---|---|---|
| Half-Ogre | 113 | 60 | 100 |
| Half-Orc | 90 | 67 | 100 |
| Dwarf | 92 | 62 | 100 |

Notes:
- **Hea rule:** keep max HP ≥ 4 × the biggest hit where you hunt.
- **Half-Ogre:** Str 101 → about 110 is still cheap (4 CP/point). The optimiser leaves Agi at
  20-33 until Hea 90 (Agi 45 costs a Half-Ogre 45 CP), which is ~4% better than following
  `basher` literally.
- **Wil 55-65** later, for monster casters (MR = (Int+3·Wil)/4). Int does nothing for a basher
  (bashes never crit).
- CP per level: +10 (L2-10), +15 (11-20), +20 (21-30), +25 (31-40) (LJNUTIL.C:8346).

Max HP (LJNUTIL.C:4969): `Hea/2 + L×(6+hpbonus) + (Hea−50)×L/16 + ~2.5×L`. Half-Ogre Hea 75: **~147 at L10**, ~275 at L20 (Hea 85). Half-Orc Hea 60: ~121 at L10.

## 4. Attack style [C mechanics combat.md §2, LJNCOMBT.C:907-936, 1255-1380]
| Mode | Use |
|---|---|
| **BASH** (`bash <mob>` once; autocombat repeats it, LJNACMB.C:736) | Default L1-30. ×1.1 range, then ×3 after monster DR, −15 AV, no crits, 2× energy capped at 1000. With any weapon whose normal cost ≥ 500 the extra cost is free. Heavier 2H = strictly better. |
| ATTACK | Against high-AC monsters (AC ≳ 0.5×AV early, 0.6×AV later) and once a fast weapon's cost drops under 200 (+up to 20% crit). |
| SMASH | L22+ only if the ability can be obtained (unknown in this data). ×1.2 then ×5, full round, knockdown. |
| Backstab | Not available (no stealth). |
- Warrior AV at L10 ≈ 73 with Str 100/Agi 32 in plate (no light-load bonus; ≈ 89 unencumbered) [I]. Bash hits AC-30 monsters ~60%.

## 5. Gear by level band [C shop/item data; prices with markup]
Silvermere shops are 2-6 steps from 1:224. All markup 100%.

| Band | Weapon (2H, bash) | Torso | Other armour | Notes |
|---|---|---|---|---|
| L1-3 | club 3-8 / quarterstaff 2-12 (free, Newhaven 1:2141) | padded vest (free 1:2142) | padded set (free) | Arena only |
| L2-8 | **greataxe** 6-22 s80 10g (1:307), battle-hammer 6-22 s80 18g (1:354), **maul** 8-25 s90 20g (1:354), halberd 4-25 16g | rigid leather tunic ac13 dr1.3 16g (1:310) → **chainmail hauberk** ac18 dr4 40g / scalemail tunic ac20 dr6 wt800 50g (1:306) | chain coif 10g, chain gauntlets 6g, chainmail leggings 12g, chainmail boots 10g (1:305/306) | Half-Ogre can wear scale at L2. |
| L6-12 | maul; ancient sword 4-19 (1H, spectral mage 1:382, 25%) only with a shield | **half-plate corselet ac19 dr9 wt1100 180g** or **steel brassiere ac22 dr6 wt1100 180g (no dodge penalty)** (1:306) | **horned helmet** ac7.5 dr1.5 50g, **plate gauntlets** ac7 60g, **plate boots** ac6.5 dr1.5 1p (1:305); **light plate leggings** ac8 dr2 1p (1:306); kite shield ac5 dr1 30g / tower ac6 dr1 30g (1:2153) if 1H | The big jump: −14/hit. Buy the corselet first. |
| L10 quest | **golden battleaxe** 8-21, 1H sharp, s60, acc 5, Magical 2, light +100, Warrior-only, limit 1 (placed 1:2231) | | | 1H: pair it with a kite/tower shield. A 2H maul bash still out-damages it [I]; take it for the light and the shield slot. |
| L15-20 | **three-headed flail** 9-23 s75 4p (L15, 1:354); massive wooden club 10-30 s100 37.5g (L15, Jorah 1:2329, mk275); two-headed pike 8-30 s90 14p (L15, Nekojin 12:49, 309 steps); bone-handled headcutter 7-30 (minotaur champion 1:2124, 25%) | **full plate corselet ac23 dr10 wt1500 5p** (1:306); fine breastplate ac24 dr9 L10 30p | full plate leggings ac9.5 dr2 1.5p, visored greathelm ac8.5 dr2 wt500 1.3p | Needs Str ~95+ to stay under 66% |
| L20-30 | etched adamant greatsword 10-34 s90 L20 206p (Jorah 1:3305); heavy spear 10-40 s130 L20 15p (Floating Platform 16:2663); runic warhammer 10-18 1H L20 40p (6:1344) | adamantite platemail tunic ac26.5 dr11 L30 375p (1:3302) | long battle shield ac7 dr1.5 L20 80p (6:1337); adamantite boots/gauntlets L30 (6:1337) | |
| L50 | Warrior-only drops: hero's tabard (back, +4 dmg, L40), ruby-encrusted bracers (L50, Lord Feyr) ; platinum knight set (cls 1/3/4, L50) | | | endgame, not traced |

Utility: lantern 2g (1:334) for −175/−200 areas unless Dwarf; rope and grapple 5g.

## 6. Grinding by level band [C areas/stats from corpus leveling.md, re-checked; I class fit]
Danger column uses **effective maxhit = maxhit − DR/10** for the armour you should own by then.

| Levels | Area (anchor) | Monsters hp/exp/maxhit | Kit DR | Access | Fit |
|---|---|---|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | giant rat 12/9/10 … carrion beast 18/17/10 | 0-1 | arena maxlevel 3 | Bank exp, then `borrow skiff` |
| 3-6 | Graveyard 1:817 (90 rooms) | skeleton 43/55/11, zombie 62/70/14, giant bat 35/40/5 | 4-6 (chain/scale) | none, 23 steps | Gravediggers 130/1500/30 (Neutral: you must start it) at 1:824/830/2618/2635/2640, respawn 1 h — with DR 6 they hit for ≤24. **Best early exp.** |
| 6-10 | **Slimy Sewer Tunnels 9:1** | slimes 30-55 hp, 60-125 exp, hit 12-14 | 6-14 | `search w` at 1:619; light −200 | With half-plate the slimes do ~0. exp/hp 2.1. Boss white jelly 325/2250/18 at 9:146 (3 h). |
| 6-10 | Crypt L1-3 1:835/1070/1276 | ghoul 73/100/9, shade 75/140/15, wight 80/140/10 | | hidden exits 1:1056 n, 1:1070 s, 1:1250 s; light | Also the L10 quest route |
| 7-10 | Strange Mansion 1:63 | vision 40/120/14 (damage shield), shadow spectre 70/160/20, astral shade 80/185/12 | | 31 steps | Spectral mage 200/1200/20 at 1:382 |
| 10 | **Crypt quest** (§7) | troll 93/225/35 or ogre 100/225/36 or fire lizard 93/225/16 | | level 10 only | golden battleaxe, L11 |
| 10-15 | Kobold caves 6:752 + 6:760 | kobold 30/80/9, kobold warrior 50/150/13 | ≥14 | 186 steps; light −200 | Kobolds do nothing through plate. King 250/2250/16 at 6:834 (iron crown). |
| 10-15 | Darkwood bandits 9:720 / Smuggler Den 9:161 | bandit 55/125/12, leader 100/275/18; smuggler 70/150/12 | | Den: hidden `d` at Docks | Cash drops (plate is expensive). |
| 12-18 | Huge Cave 1:1752 | black orc 100/160/21, orc warrior 100/275/40-45, orc fanatic 150/400/50 | ≥14 → eff. 26-36 | 99 steps, light −175 | Plate turns this from "no until L18" into ~L13. |
| 15-20 | Frozen Caverns 10:35 | grey wolf 125/375/21, guardian 145/500/22 | | `search n` at 10:3 | Wolves ~7 eff. Large yeti 380/4500/45 at 10:218 → glass key → Ice Sorceress (+1 AC quest). |
| 15-20 | Gnoll Encampment 2:1107 | scout 100/325/30, axeman 130/375/48 | 17 → eff. 31 | 104 steps | Chieftain 220/3500/60 at 2:1122 |
| 16-22 | **Orc Mansion 2:31-39** | elite orc guard 100/275/70 | 17 → eff. 53 | 43 steps | Only for plate classes. Orc warleader 175/2500/27 at 2:40 (serrated battle axe 25%, rusty iron key 25%). |
| 20-30 | Ancestral Tomb 6:872 | tortured spirit 150/525/15, barrow wight 180/725/15 | | hidden `w`, light −200 | 15-hit monsters: free exp in plate. |
| 20-30 | Dragon's Teeth Hills 2:177 (930 rooms) | brigand 115/325/50, gorgon 165/450/30, ankheg 140/450/30 (dr10), dark monk 125/525/30 | | 49 steps | Dark monks drop black serpent key (20%). |
| 20-30 | Darkwood bugbears/ogres 1:276; Labyrinth minotaurs 1:1839 | bugbear 155/575/45, ogre 140/625/66; minotaur 155/375/70 | 17+ | | Plate fit. Minotaur champion 200/6000/70 at 1:2124 (headcutter 25%). |
| 30+ | Quaggoths 6:1811 (mine pass 100g at 6:1567), Redstone 6:1407 (stone elemental dr15: ATTACK, not bash), Lava Tube 6:1210 | see corpus leveling.md "30+" | | | |

## 7. Class quest: L10 crypt → golden battleaxe + L11 [C rooms/exits; F order of steps quests.md §1]
Route from the mummy room 1:1309 (bone key, 100% drop) with the shortest data path:
1. `use bone key n` (Key 177) → Shadowed Hall 1:1377. Warrior exit is **`w` from 1:1377** (Class 1).
2. `w` ×7 more to the Marble Room 1:2219 (via 1:1425, 1:843 … 1:531, 1:2218).
3. Pick one of three button rooms (all lead to Marble Chamber 1:2226):
   - `w`,`nw` → 1:2233 (flame glyph [F]), `push button` → **fire lizard 93/225/16** (easiest), obsidian key 341 → `e`,`e`.
   - `s`,`sw` → 1:2223 (claw glyph), `push button` → troll 93/225/35, iron key 338 → `n`,`n`.
   - `n`,`ne` → 1:2237, `push button` → ogre 100/225/36, stone key 344 → `s`,`s`.
4. From 1:2226 **`n`** (Class 1) → 1:2227 → `n` → **Large Tomb 1:2231**: warrior spirit (1000 hp, do not attack), golden battleaxe placed (limit 1). `train` (Super Warrior Trainer, mk 500). `go portal` → Graveyard Tomb Entrance 1:834.
- Room 1:1309 needs level 5+ (data minlevel; forum says 10-only). The tomb route closes after L11 [F].

Other quests [F t=6/t=10/t=13, C items]: alignment quest 1 at L10 (ring + Defence +1), Ice Sorceress at L15 (+1 AC, large yeti 10:218 → glass key → 10:245), alignment quests 2-4 (Warrior +1 max dmg). Paired endgame class drops: see §5 L50.

## Group role
Plate front-liner and DR soak. No taunt exists (monsters target at random), so the Warrior is simply the member who survives its share of hits best. Pair it with a Cleric (2), add a Mage (3), add a Bard or Thief (4). Exp% 0: levels fastest of the fighters. See `kb/grouping.md`.
