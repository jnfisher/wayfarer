# Mage (WCCCLASS 12): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data,
**[I]** inferred/modelled, **[F]** forum/web. Monsters `hp/exp/maxhit/MR`. Prices = list ×
(100+markup)/100 (shown with markup, before Charm ×(110−Cha/5)%). Steps from Town Square 1:224.
Shared AoE mechanics and spot lists: `kb/aoe-grinding.md`. Areas: `test/play/corpus/leveling.md`.

**One-line path:** Half-Elf, Int-first with Hea second. Magic missile → frost jet (L4) → lightning
bolt (L8) single-target through the Graveyard, Crypt and Strange Mansion; L10 crypt quest (obsidian
runestaff); **dancing blades L11, fireball L15** on lairs (Graveyard edges → Smuggler Den → nomad
caverns); mana storm L23, **spirit horde L28** (Mage-only) on map-3 caves and dragon nurseries;
Meditate at L20 via Dhelvanen.

## 1. Class facts [C WCCCLASS rec 12; LJNUTIL.C]
| Field | Value | Meaning |
|---|---|---|
| exp | +40% | lowest of the casters with Priest |
| basehp / hplevel | 3 / 3 | +3 HP/level + `genrdn(0,4)` → 0-3/level. Weakest HP in the game with Priest |
| combat | 3 | melee is a joke; never plan on it |
| magery | **Mage, level 3** | every Mage spell incl. magelvl 3 (spirit horde, eldritch storm, magic armour, mageshield, sleep, mystic mist, solid fog, wind of haste) |
| weapons | 9 = **Limited** | only exempt items 100 quarterstaff and 68 dagger (LJNGLOBL.C:172), plus items listing class 12 or "Class Item Inclusion 12" (LJNUTIL.C:9490-9730) |
| armour | 1 = Robes | robes/cloth/silk only |
| abilities | 31 Bash | Meditate allowed from **L20** (LJNUTIL.C:718), Smash never |

Casting formulas [C LJNUTIL.C:4904-4952; LJNBKGND.C:1671, 2143; LJNSPELL.C:81]:
- Max mana = 2×(3×level+3) + Mana abilities (Half-Elf/Elf +10, items). **Int does not raise max mana.**
- Spellcast SC = 2×level + (3×Int + Wil)/6 + 15 + "Spellcasting" items. Cast% = min(98, SC + diff). Fail = full energy + half mana.
- Regen per 30-s tick = Int × (level+20) × 5 / 1650 × (100+mana-regen items)%. MEDITATE adds the same every 15 s (3×).
- MR = (Int + 3×Wil)/4 + race/items.

## 2. Race choice [C WCCRACE; ranking I]
| Rank | Race | Int | Wil | Hea | exp% | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Half-Elf** | 45-110 | 40-100 | 30-100 | **+15** (total 155%) | **Mana +10**, light 25 | Cheapest exp of the good-Int races; +10 mana is +45% pool at L1-3, +10% at L15; Hea to 100 for AoE survival. Int cap 110 is enough with Spellcasting gear. |
| 2 | Gaunt One | **60-140** | 50-100 | 30-**70** | +50 (190%) | light 200 (sees in the dark), Perception 10 | Best spell success/regen without gear (+17 SC vs Half-Elf at L30); pays 23% more exp per level and caps Hea at 70 (−50 HP at L30). Late-game pick. |
| 3 | Elf | 50-120 | 40-100 | 30-80 | +45 (185%) | Mana +10, light 50, Race Stealth | Half-Elf's mana with Int 120, but exp like a Gaunt One and Hea cap 80. |
| alt | Human | 40-100 | 40-100 | 40-100 | 0 (140%) | — | Fastest leveling; no mana bonus, Int 100. |

## 3. Stat / CP plan [C formulas; I ladder]
What Int buys: +0.5 SC per point (all spells), +1% regen per ~1% Int. Wil: +0.17 SC, +0.75 MR.
Hea: +0.5 + level/16 max HP per point (base Hea). Str: carrying only. Agi/Cha: dodge; skip.
CP: 100 at creation, +10/level to L10, +15 to L20, +20 to L30, +25 to L40 (LJNUTIL.C:8346) [C].
Cost bands: points 1-10 above race min 1 CP, 11-20 2 CP, … (LJNGAME.C:1645) [C].

Ladder: Int 75 → Hea 50 → Wil 50 → Int 90 → Hea 60 → Wil 55 → Int 105 → Hea 70 → Wil 60 → Int max → Hea 80-90 → Wil 75+.

Half-Elf build and derived numbers [I from the formulas]:
| Lvl | CP | Int/Wil/Hea | SC | Max mana | Regen /30 s | HP ≈ | MR |
|---|---|---|---|---|---|---|---|
| 1 | 100 | 75/50/50 | 62 | 22 | 4 | 30 | 56 |
| 5 | 140 | 85/50/50 | 75 | 46 | 6 | 48 | 58 |
| 10 | 190 | 90/50/58 | 88 | 76 | 8 | 79 | 60 |
| 15 | 265 | 100/55/60 | 104 | 106 | 10 | 106 | 66 |
| 20 | 340 | 105/60/70 | 117 | 136 | 12 (36 meditating) | 150 | 71 |
| 25 | 440 | 110/65/80 | 130 | 166 | 15 | 199 | 76 |
| 30 | 540 | 110/76/90 | 142 | 196 | 16 | 255 | 84 |
Gaunt One at L30: Int 140, SC 159, regen 21, HP ~207.
Gear Spellcasting (+5..+45) is the cheapest way past 98% on diff −60..−90 spells (§6).

## 4. Spell progression [C WCCSPELS + scroll items; cast% for the Half-Elf build at learn level]
| role | spell `cmd` | lvl | mana | target | dmg@learn | dmg@cap | dmg/mana@cap | cast%@learn | casts | source (markup) |
|---|---|---|---|---|---|---|---|---|---|---|
| nuke | magic missile `mmis` | 1 | 1 | single | 4-12 | 4-12 @L6 | 8.5 | 77% | 1/rd | Newhaven Spell Shop 1:2144 (mk 0, free), Mage Spell Shop 1:398 |
| nuke | frost jet `fjet` | 4 | 3 | single | 8-18 | 8-25 @L11 | 5.7 | 77% | 1/rd | 1:398, 20g ×2 |
| nuke | lightning bolt `lbol` | 8 | 5 | single | 16-28 | 16-42 @L15 | 5.9 | 84% | 1/rd | 1:398, 75g ×2 |
| nuke | sunbolt `sbol` | 13 | 8 | single | 30-50 | 30-73 @L22 | 6.5 | 83% | 1/rd | Aldreth's Spells 1:2306 / Jorah's Mage Spells 1:3309 (85 steps), 150g ×2 |
| nuke | eldritch bolt `ebol` | 20 | 12 | single | 40-88 | 40-120 @L28 | 6.7 | 87% | 1/rd | 1:2306 / 1:3309, 600g ×2 |
| nuke | meteor swarm `mete` | 25 | 4 | single | 20-35 | 20-45 @L35 | 8.2 | 80% | **4/rd** (energy 250) | Very Powerful Spells 10:273 (86 steps, search), 1750g ×2 |
| nuke | dragonfire `dfir` | 30 | 16 | single | 107-250 | 110-350 @L40 | **14.4** | 82% | 1/rd | 10:273, 3000g ×2 |
| nuke | colour spray `colu` | 32 | 20 | single | 104-256 | 120-320 @L40 | 11.0 | 98% | 1/rd | drop: dark wizard 25% |
| nuke | glacial blades `glac` | 33 | 20 | single | 76-320 | 80-490 @L50 | 14.3 | 81% | 1/rd | "icy scroll": no shop/drop found |
| nuke | forked lightning `fork` | 35 | 10 | single | 47-175 | 50-250 @L50 | 15.1 | 80% | 2/rd | Nekojin Village Store 12:49 (309 steps) 3500g ×2; rakshasha rajah 100% |
| AoE | dancing blades `blad` | 11 | 26 | Full Attack Area | 21-60 | 21-78 @L20 | 1.9/target | 81% | 1/rd | 1:398, 250g ×2 |
| AoE | fireball `fbal` | 15 | 34 | Full Attack Area | 33-90 | 33-114 @L23 | 2.2/target | 84% | 1/rd | 1:398, 350g ×2 |
| AoE | mana storm `msto` | 23 | 50 | Full Attack Area | 59-170 | 59-215 @L32 | 2.8/target | 81% | 1/rd | 1:3309, 1250g ×2 |
| AoE | spirit horde `spir` | 28 | 60 | Full Attack Area | 91-251 | 102-405 @L50 | 4.2/target | 78% | 1/rd | 10:273 2500g ×2; dark-elf archmage 10% |
| AoE | blizzard `bliz` | 40 | 45 | Full Attack Area | 80-250 | 80-310 | 4.3/target | 90% | 1/rd (+freezing room DoT) | storm giant king 7% |
| AoE | eldritch storm `esto` | 40 | 70 | Full Attack Area | 130-400 | 190-550 @L55 | 5.3/target | 75% | 1/rd | Gnaj the Creator 33% |

Resist: all Mage damage spells above are resist mode 0 → never resisted; damage × MR multiplier
(mr<50 bonus, mr>50 penalty; `kb/aoe-grinding.md` §2). Vampiric touch/deathtouch (steal HP,
1.8 dmg/mana) are not worth buying [C/I].

Buffs and control (all Self unless noted) [C]:
| Spell `cmd` | Lvl | Mana | Effect | Use |
|---|---|---|---|---|
| blur `blur` | 1 | 4 | Protective Shield 5, 70 rounds | early AC |
| ethereal shield `shld` | 5 | 6 | Protective Shield 3→12 (L18) + DR 1, 60+ rounds | keep up L5-9 |
| magic armour `armr` | 6 | 6 | DR 5→10 + DV 5, 60 rounds (Mage-only) | stack with mageshield |
| mageshield `mshi` | 9 | 10 | DV 7→18 (L22) + DR 1, 60 rounds (Mage-only); replaces ethereal shield | **always on** from L9 |
| shockshield `shok` | 9 | 12 | damaging shield 1-5, resist lightning 10 | optional |
| flash `flas` | 7 | 10 | **AoE**: blind, −8 AV, −5 DV, 5 rounds | cuts incoming during AoE pulls |
| stinking cloud `stnk` | 8 | 10 | **AoE** confusion 5-15: each monster loses its turn 5-15% of rounds (LJNMONST.C:4743); −5 AV | minor |
| vulnerability `vuln` | 14 | 6 | target −8 DV, **−10 MR** | before long single-target fights (+5% dmg) |
| slow / speed | 10 / 12 | 12 | energy use 125% / 85% | speed = more melee, not more casts (AoE is 1/rd) |
| sleep `slep` | 15 | 30 | confuse 100, single (Mage-only, 1250g ×2) | emergency |
| solid fog `sfog` | 19 | 30 | **AoE** slow 125% (Mage-only) | |
| mystic mist `mist` | 26 | 30 | **AoE** confusion 25 = each monster skips 25% of rounds (Mage-only) | big-lair safety |
| mana flux `flux` | 16 | 20 | mana regen −100..+150% for 80 rounds (gamble) | beholder 10% drop only |

Buy order (cost incl. markup, ≈): L1 mmis/blur free (Newhaven) → L4 fjet 40g → L5 shld free →
L8 lbol 150g → L9 mshi 320g → L11 blad 500g → L13 sbol 300g → **L15 fbal 700g** → L20 ebol 12p
→ L23 msto 25p → L25 mete 35p → L28 spir 50p → L30 dfir 60p (100g = 1p). Bank for each before the level.

## 5. AoE grinding plan [C mechanics, I numbers]
Mechanics, safety rules and the full spot tables are in `kb/aoe-grinding.md`. Mage specifics:
- Pool is the limit, not regen: L15 = 106 mana = 3 fireballs; L28 = 178 = 2-3 spirit hordes.
  Enter a lair with ≥2 casts; one-shot lairs whose pool hp ≤ AoE avg × MR mult.
- HP ≈ 30+5L: N × monster hits must stay < 40% HP. A Mage cannot stand in 1:2707 (10 goblins,
  ~130/round) until spirit horde one-shots them (L28+, HP ~240).
- Open with flash/stinking cloud on 4+ monster lairs below L20 (−8 AV / confusion) if the AoE needs 2 casts.
- Meditate (L20, Dhelvanen quest) triples regen → ~36 mana per 30 s; lair regen is 5 min, so 2-3
  lairs in rotation + meditate between them is sustainable.

| Band | AoE spell | Best lairs (`map:room`, N, pool) | Fallback single-target |
|---|---|---|---|
| 11-14 | dancing blades (avg 41) | Graveyard Western Edge 1:2617 (N5, skeleton/zombie/bat 35-62 hp, 34 steps); 1:2637 (N4); kobold Cramped Tunnel 6:754 (N3, 30-50 hp) | lightning bolt in Crypt / kobolds |
| 15-22 | fireball (avg 62-74) | **Smuggler Den Sleeping Quarters 9:218/219/222** (N5 ×3, 55-70 hp, 60 steps, hidden d at Docks 1:31); bandit Living Quarters 9:809/822/848/859 (N3); orc Torchlit Tunnels 9:439/475 (N3-4); nomad Torchlit Cavern 10:43/80 (N3, 110-130 hp, 2 casts) | sunbolt on wolves 10:35 |
| 23-27 | mana storm (avg 115-125) | Darkwood goblin lairs 1:424… (N5) one-shot; **map-3 caves**: Mossy Cave 3:6 (fungus lizard 205 hp), Rocky Waterfall 1:2362, Wide Passage 3:593 (dragon serpents 190-225) — 2 casts; Foul/Witch's Cave 3:35/37 (vampire bat 235/1100, hit 18) | eldritch bolt / meteor swarm |
| 28-39 | spirit horde (avg 170-200+) | **Darkwood Clearing 1:2707 (N10 goblins, one-shot: 1,150 exp/cast)**; Foul Cave 3:35 (N4 ×1100); Misty Bog 6:398/471 (dragon whelps 300-320 hp, 2 casts, HP≥300); Lava Cavern 15:1019 (N5 magma men 225) | dragonfire (14 dmg/mana) |
| 40+ | eldritch storm / blizzard | Fortress Courtyard 12:101 (rakshasha N6, 24k/clear); green dragon nursery 1:2805; Limestone Caverns 12:1282 | colour spray, glacial blades |

## 6. Gear by level band [C shop/item data]
Mage weapon slot: quarterstaff (2H: no off-hand) or dagger (1H: allows off-hand). Melee is irrelevant; pick the stat item.
| Band | Slot | Item | Stats | Price | Where |
|---|---|---|---|---|---|
| 1-10 | weapon | quarterstaff | 2-12, del@cleanup | free | Newhaven Weapon 1:2141 (or 1:354) |
| 1-10 | torso | white/black/... robes | ac 6 | 1g | Robes Shop 1:309 (2 steps) |
| 1-10 | rest | padded helm/gloves/pants/boots | ac 1 each, del@cleanup | free | Newhaven Armour 1:2142 |
| 8+ | weapon | **astral staff** (Mage/Druid/Priest) | 5-16, **Mana +10, SC +10, regen +5%**, L8 | drop | astral shade 5%, Strange Mansion 1:63 (31 steps) |
| 10 | weapon | **obsidian runestaff** (Mage) | 5-17, SC +5, regen +15%, burning death charges (15-30) | quest | Large Tomb 1:2302 (§8) |
| any | head | **silvery skullcap** | ac 3, **SC +15, MR +10**, no min level | 70p | Curio Shop 1:332 (4 steps) |
| 10-20 | head (alt) | runed cowl / silver hood | SC +5 / SC +5 regen +5% | 18.75p / 9.4p | Jorah's Robes 1:3303 (82 steps) |
| 10-20 | torso | runed robes | ac 8, SC +5, L10 | 37.5p | 1:3303 |
| 15-20 | hands / arms | silver gloves / silver bracers | mana +5 regen +5% / regen +10% (−2 dmg) | 9.4p / 11.25p | 1:3303 |
| 20 | torso | black silk robes | ac 9, dodge 5 | 7.5p | 1:3303 |
| 20+ | off-hand (dagger users) | arcane tome (Mage/Warlock) | SC +10, Mana +20, L20 | no shop/drop found [I: quest] | — |
| 25-30 | neck / waist | bone necklace / **fur-lined pouch** | Mana +20 regen +10% / **SC +20, MR +10, Mana +10** | 75p / 5p | Lost City 16:434 / 16:329 (no walk path found from TS [I]) |
| 30+ | torso / ring | astral robes / opal ring (Mage, Evil) | ac 12 dodge 5 / regen +5%, 25-48 nuke charge | 60p / 120p | Curio 1:332 / Gnome Jeweler 6:567 |
| 20+ | weapon drops | dragonclaw staff (Goru-Nezar 5%) mana +5 regen +20%; rosewood staff (drop from "chest" monster #69, 20%) SC +5 mana +2; vortex staff L40 (mind flayer magus 5%) SC +20 mana +50 regen +20% | | | |
Light: Half-Elf light 25 is not enough for −175 rooms: carry a lantern (2g, 1:334) [C].

## 7. Grinding spots by level band (single-target + AoE)
| Band | Single-target | AoE | Notes |
|---|---|---|---|
| 1-4 | Newhaven Arena 1:2150 (arena: instant respawn; locked at L4) with magic missile | — | 1-mana mmis one-shots 12-25 hp. |
| 4-8 | Graveyard surface 1:817 (34 steps); Slum soft streets 1:1073 | — | frost jet from L4. Mana 30-46: rest between pairs. |
| 8-11 | Crypt L1-3 1:835/1070/1276; **Strange Mansion 1:63** (astral staff); Slimy Sewers 9:1 | — | lightning bolt; mageshield L9. |
| 10 | L10 crypt quest → Super Mage Trainer (train to 11) | | §8 |
| 11-15 | Kobold caves 6:752; Darkwood bandits 9:720 | blades on Graveyard edges 1:2617/2637, kobold lair 6:754 | |
| 15-20 | Wolves 10:35 (hit 22) | **fireball: Smuggler Den 9:218-222**, bandits 9:809, orc 9:439/475, nomads 10:43/80 | Most efficient band for a Mage. |
| 20-30 | Ancestral Tomb 6:872; Dragon's Teeth Hills 2:177 | mana storm → spirit horde: map-3 caves 3:6/35/37/593, goblin lairs, Clearing 1:2707 at 28+ | Meditate from 20. |
| 30+ | dragonfire on anything | spirit horde: Misty Bog 6:398/471, green dragon nursery 1:2805, Lava 15:1019; eldritch storm at 40 | |

## 8. Class quests and class-only items [C data; F where noted]
| Lvl | Quest / item | Details |
|---|---|---|
| 10 | **Crypt class quest** | Graveyard crypt levels 1-3 → mummy (bone key) → Shadowed Hall → Marble Passage → **Large Tomb 1:2302** (`s` from Marble Passage 1:2298): placed **obsidian runestaff** (limit 1) + **Super Mage Trainer** (shop 62, trains L11 only, markup 500). Route details: `test/play/notes/quests.md` §1 (Witchunter path; the Mage room is a different branch of the same hub) [C rooms/items; F route t=28]. |
| 20 | **Meditate** | Dhelvanen, Luxurious Room 7:1008 (227 steps, search): bring 50 nanati black diamonds (his portal), get a sigil (item 1351), return at L20+: `giveability 187` (WCCTEXT 9024-9046) [C]. |
| 10/20 | Alignment quests | ring at L10, more at L20 [F t=6, t=10] |
| — | Mage-only items | obsidian runestaff, rosewood staff, frost sceptre (ice sorceress 5%), dragonclaw staff, opal ring (Evil), twisted bone staff (L25, dark-elf archmage 5%), skull-capped staff (L50, Zanthus 100%), staff of power (L50, regen +99%), runed cape (L40, Mana +50, regen +15%, no source found) [C]. |

## Group role
Primary AoE (fireball L15, mana storm L23, spirit horde L28) plus party haste/frenzy. AoE in no-target autocombat shares **every** kill in the room. Party members are never hit by its attack-area spells. Robes: back rank, healed first. Best partner: Cleric. See `kb/grouping.md`.
