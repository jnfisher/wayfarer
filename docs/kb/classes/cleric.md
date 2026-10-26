# Cleric (WCCCLASS 4): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data,
**[I]** inferred/modelled, **[F]** forum/web. Monsters `hp/exp/maxhit/MR`. Prices include shop
markup (before Charm). Steps from Town Square 1:224. Melee formulas: `test/play/notes/combat.md`;
AoE mechanics: `kb/aoe-grinding.md`; full Priest spell list: `kb/classes/priest.md` §4.

**One-line path:** Dwarf, Wil + Str + Hea. A platemail blunt-weapon fighter who heals himself:
BASH with a 2H flail/maul (or mace + tower shield), major healing between fights, turn undead/
disrupt on undead. Graveyard → Crypt → L10 crypt quest (black flail) → kobolds/Slimy Sewers →
wolves/Ancient Crypt → Ancestral Tomb/Dragon's Teeth Hills. **No AoE damage spell**; plate + heals
let it tank lairs one monster at a time. +120% exp makes it the slowest-leveling caster.

## 1. Class facts [C WCCCLASS rec 4; LJNUTIL.C]
| Field | Value | Meaning |
|---|---|---|
| exp | **+120%** | slowest of the five |
| basehp / hplevel | 4 / 4 | +4 HP/level + 0-4 random |
| combat | 5 | Warlock-level melee (Warrior 6) |
| magery | **Priest, level 2** | Priest spells with magelvl ≤2: heals to greater healing/major healing rain, turn undead, disrupt, god's wrath, divine fury, divine favour, black wind. **Missing (magelvl 3)**: holy armour, iron faith, holy aura, godheal, great healing rain, divine mercy, paralyze, soulstrike |
| weapons | 7 = Any Blunt | 1H/2H blunt |
| armour | 9 = **Platemail** | everything |
| abilities | 31 Bash | Meditate from **L23**, Smash allowed from **L27** if obtained (LJNUTIL.C:694-728) |

Casting [C]: max mana = 2×(2×level+3) (46 at L10, 86 at L20). SC = 2×level + (3×Wil+Int)/6 + 10.
Regen /30 s = Wil × (level+20) × 4 / 1650. HP ≈ 40 + 7.9×level at Hea 80 [I].

## 2. Race choice [C WCCRACE; ranking I]
| Rank | Race | Wil | Str | Hea | exp% | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Dwarf** | **50-120** | 50-110 | 50-**120** | +30 (250%) | light 75, MR +10, **encumbrance +20%** (plate weight) | Every stat a Cleric uses starts high; carries plate under 33% load longer. |
| 2 | Half-Orc | 30-95 | 45-110 | 50-120 | **0 (220%)** | — | 12% less exp per level; Wil cap 95 is enough for heals. No dark vision. |
| 3 | Kang | 45-110 | 55-120 | 50-110 | +35 (255%) | poison immune, DR 10, DV 5 | Toughest melee body; slow exp. |

## 3. Stat / CP plan [C formulas; I ladder]
Ladder: Wil 65 → Str 60 → Hea 60 → Wil 80 → Str 70 → Hea 70 → Agi 45 → Wil 95 → Str 80 → Hea 80 → Wil 110 → Str 90 → Hea 90.

**Cleric as a 2H basher** (blunt only, combat 5; model in `kb/stat-builds.md`) [I]:
- Keep Wil 65 first for heals. Then follow the `basher` core: Str +25/+31/+35, Hea 64-72,
  Agi 45/48, Cha 31/36, Hea 80-90, then alternate.
- Put the Wil 80/95 rungs after Hea 80 and Hea 90.
- **Don't max Str:** −17% kills/h on the mixed metric (−32% in hard-hitting areas). The
  `basher` core is +4% vs the old melee ladder.
- Weapon Str requirements don't gate anything. Maul (Str 90) and three-headed flail (Str 75)
  are the model's picks, and are fine below their Str while a bash is capped at 1000 energy.
- Your own heals remove most of the HP penalty that makes Str-max lose. Even so, the cheap Str
  bands give most of the damage.
Str: AV (+1/3 pts), max damage (+1 per 10 over 50), plate capacity (Str×48, ×1.2 Dwarf). Wil:
heal success + regen + MR. Melee details (BASH vs ATTACK, encumbrance): `notes/combat.md` §1-3.

| Lvl | CP | Wil/Str/Hea (Dwarf) | SC | Max mana | Regen /30 s | HP ≈ | MR |
|---|---|---|---|---|---|---|---|
| 1 | 100 | 80/70/60 | 57 | 10 | 4 | 36 | 77 |
| 10 | 190 | 92/70/70 | 81 | 46 | 6 | 107 | 86 |
| 15 | 265 | 95/80/80 | 92 | 66 | 8 | 158 | 88 |
| 20 | 340 | 108/80/80 | 109 | 86 | 10 | 197 | 98 |
| 30 | 540 | 110/90/90 | 130 | 126 | 13 | 300 | 100 |

**Attack or bash at L20** [C formulas from LJNCOMBT.C:338-943, 1226-1427; I: the class ladder's L20 stats, the damage roll taken as min..max, targets kobold warrior, bandit leader, guardian wolf, skeletal warrior, stone golem, slaver, barrow wight, guardian golem, minotaur champion]. Dwarf on the ladder above (Str 80, Wil 108, Agi 45), black flail,
scalemail kit (47% load, AV 84): bash 29 a round against attack 23 on average, but attack wins
from about AC 50 (slaver 20 against 17, barrow wight 13 against 7). Bash at AC 40 and under,
attack above. The **huge darkwood club** (10-34 2H blunt, Str 100, limited 1, Thrag 1:1810)
bashes hardest: 34 a round, ahead of the black flail on every target but the barrow wight. Being
20 Str short costs it nothing, because its bash already takes the whole round (cost capped at
1000), and its accuracy keeps the hit chance level with the flail's. It attacks badly (16), so
keep the flail for `a` on AC 50+ and for a shield. Maul 17, three-headed flail 24. The full plate kit (corselet, leggings, greathelm, gauntlets, boots) is 74% load
at Str 80 even with the Dwarf's +20%: every blow costs more (bash 25). Holy force (18-64 at
L20, about 98% to cast, 8 mana) does about 40 a cast, more than either, but the mana is for heals.

## 4. Spell progression [C; cast% for the Dwarf build at learn level]
| Role | Spell `cmd` | Lvl | Mana | Effect | Cast% | Source (list ×2 for mk 100) |
|---|---|---|---|---|---|---|
| heal | minor healing `mihe` | 1 | 2 | 5-14 | 72% | Newhaven 1:2144 free / Holy Spell Shop 1:522 |
| buff | bless `bles` | 2 | 4 | +3 AV, 40+ rounds | 77% | 1:522, 4g |
| undead | **turn undead** `turn` | 3 | 4 | 15-24 → 32-75 @L20; undead only, ignores armour/MR, never resisted | 61% | 1:522, 8g |
| nuke | harm `harm` | 1 | 1 | 4-19, living, MR/2 % resisted | 72% | free |
| buff | protection from evil `prev` | 5 | 6 | +10 defence vs evil | ~70% | 1:522, 15g |
| heal | **major healing** `mahe` | 8 | 6 | 9-20 @L10 → 16-40 @L30 | 69% | 1:522, 50g |
| nuke | agony `agon` | 8 | 5 | 18-56, living, resisted MR/2 % | 74% | 1:522, 75g |
| buff | greater bless `gbls` | 10 | 8 | +6 AV | ~73% | 1:522, 150g |
| heal | healing rain `rain` | 10 | 5 | party 7-22 | | 1:522, 150g |
| nuke | holy force `hfor` | 13 | 8 | 18-68 (not-evil casters) | 78% | Aldreth 1:2306 / Jorah's Priest 1:3308, 150g |
| heal | greater healing `grhe` | 15 | 12 | 25-50 @L20 → 35-70 @L30 | ~68% | 1:2306 / 1:3308, 350g |
| undead | disrupt `disr` | 16 | 8 | 46-84 → 70-132; undead only | 71% | 1:2306 / 1:3308, 350g |
| heal | major healing rain `mrai` | 17 | 15 | party 11-50 | | 1:2306 / 1:3308, 425g |
| buff | **divine favour** `dfav` | 19 | 12 | **+10 AV**, 30 rounds (good) | ~72% | 1:3308, 600g |
| nuke | godstrike `gstk` | 20 | 12 | 30-104 | 69% | 1:2306 / 1:3308, 500g |
| debuff | black wind `wind` | 21 | 30 | **AoE** blind, −10 AV, −5 DV | | 1:3308 / Rhudaur 2:2574, 850g |
| nuke | god's wrath `gwra` | 25 | 14 | 57-170 | 70% | 1:3308, 1250g |
| buff | zeal `zeal` | 25 | 20 | +1..+5 dmg/AV/crit/MR scaling | | Very Powerful Spells 10:273, 1250g |
| nuke | divine fury `fury` | 30 | 16 | 50-290 | 70% | 10:273, 2500g |

Cleric spell use [I]: buffs (bless → divine favour) before a fight, melee, heal between kills.
Nukes cost mana that heals need; use them for finishing fleeing or ranged targets, and turn
undead/disrupt against undead (they out-damage the weapon from ~L12).

## 5. AoE grinding [C: no Cleric AoE damage; I strategy]
- No learnable Priest-magery spell deals area damage (`kb/aoe-grinding.md` §1). Clerics clear
  lairs by **tanking** them: platemail (full plate corselet ac 23 dr 10 for 5p at 1:306) + DR makes
  small hitters (≤ dr+2) glance off, so 3-5 weak monsters at once are safe, killed one by one.
- Good tank lairs (low per-hit damage vs plate DR ~8-15): Graveyard edges 1:2617/2637 (hits ≤14,
  undead → turn undead), kobold lair 6:754 (hits ≤13), Smuggler Den 9:218-222 (hits 12, N5),
  bandit lairs 9:809… (≤18), goblin lairs 1:424… (≤21, N5) from ~L15.
- In a party the Cleric is the healer/tank while a Mage/Druid AoEs; black wind (L21) gives every
  monster −10 AV.

## 6. Gear by level band [C shop data; platemail allowed, blunt weapons]
| Band | Slot | Item | Stats | Price | Where |
|---|---|---|---|---|---|
| 1-10 | weapon | flail / battle-hammer / maul (2H BASH) or mace + shield | 5-20 16g / 6-22 18g / 8-25 20g / 3-12 8g | | Blunt Weapons 1:354 |
| 1-10 | torso | scalemail tunic → **full plate corselet** | ac 20 dr 6 50g → ac 23 dr 10 **5p** (wt 1500) | | Skali's 1:306 |
| 1-10 | legs / head / hands / feet | light plate leggings / horned helmet / plate gauntlets / plate boots | ac 8 dr 2 1p / ac 7.5 dr 1.5 50g / ac 7 60g / ac 6.5 dr 1.5 1p | | 1:306, 1:305 |
| 1-10 | off-hand (1H) | tower shield | ac 6 dr 1 | 30g | Shield Shop 1:2153 |
| 1-10 | off-hand (1H, alt) | copper cross (Good) / red skull (Evil) | SC +5 Mana +5 / SC +10 regen +10% | 20g / 2.25p | 1:525 / 7:1136 |
| 10 | weapon | **black flail** (Cleric) | 9-20 1H blunt, SC +2, regen +10%, MR self-buff charges | quest | Large Tomb 1:2258 (§8) |
| 10-20 | weapon | three-headed flail (2H) | 9-23, L15 | 4p | 1:354 |
| 15 | waist drop | belt of faith (Cleric) | +2 dmg, SC +5, Mana +10, L15 | drop | gigantic black ooze 1% (2500 hp: don't) |
| 15 | off-hand drop | golden chalice (Priest/Cleric, Good) | +2 dmg, Mana +10, regen +10% | drop | greater hellion 100% |
| 20 | torso | spiked plate corselet / crystal platemail | ac 25 dr 12 (damaging shield 5) 9.4p / ac 26.5 dr 9 75p, L20 | | Jorah 1:3302 |
| 20 | head | crested war helm | ac 8 dr 3, L20 | 41p | 1:3302 |
| 20-30 | off-hand | long battle shield | ac 7 dr 1.5, L20 | 80p | Dwarven Armoury 6:1337 |
| 30 | hands / feet | adamantite scale gauntlets / adamantite enforced boots | ac 7.5 / ac 7, L30 | 10p / 70p | 6:1337 |
| 50 | set | platinum plate (knight drops), Cleric L50 quest hammers (spiritual warhammer, white gold flail, hammers of balance…) | | | quest/drops |
Keep weight under 33% of Str×48 (×1.2 Dwarf) for the light-load bonus (`notes/combat.md`).

## 7. Grinding spots by level band
| Band | Spot | Method | Notes |
|---|---|---|---|
| 1-4 | Newhaven Arena 1:2150 | melee + minor healing | free healer there too |
| 3-8 | Graveyard 1:817 + edges 1:2617/2637 | BASH; turn undead finishes | undead-heavy |
| 8-11 | Crypt 1:835-1:1276; **Slimy Sewers 9:1** (melee best, hit ≤14) | BASH + major healing | |
| 10 | L10 crypt quest | | §8 |
| 11-16 | Kobold caves 6:752 + lair 6:754; Smuggler Den 9:161/9:218 | tank lairs | |
| 16-22 | Frozen Caverns wolves 10:35 (hit ≤22, plate DR shrugs) ; Ancient Crypt 1:1943 (undead, skeletal warrior hit 60 needs HP ≥ 240) | divine favour + BASH | |
| 20-30 | **Ancestral Tomb 6:872** (undead, hit 15); Dragon's Teeth Hills 2:177; Darkwood bugbears 1:276 | turn undead/disrupt in the Tomb | |
| 30+ | Orc Barracks 15:5 (avoid ogre gladiator 150 hit); Quaggoths 6:1811; Misty Bog | divine fury | |

## 8. Class quests and class-only items [C data; F route]
| Lvl | Quest / item | Details |
|---|---|---|
| 10 | **Crypt class quest** | Large Tomb **1:2258** (`nw` from Marble Passage 1:2257): placed **black flail** (limit 1) + Super Cleric Trainer (shop 54, L11 only, markup 500). Route: `test/play/notes/quests.md` §1 [C rooms/items; F t=28]. |
| 23 | Meditate | Dhelvanen 7:1008 (50 nanati black diamonds; return the sigil at **L23**) [C WCCTEXT 9046]. |
| 27 | Smash allowed | how to obtain is unknown in this data set [I]. |
| 50 | Alignment weapon quest | Cleric L50 hammers: spiritual warhammer / white gold flail / great mithril battle-hammer (Good), Frosthand / stone warhammer / hammers of balance (Neutral), AngelCrusher / netherworld hammer / flail of oblivion (Evil) [C items; F t=6 for the quest chain]. |
| — | Cleric-only items | black flail, belt of faith, cloak of faith (L40, no source), platinum plate set (Good) [C]. |

## Group role
**The party's main healer that survives being targeted** (plate): healing rain L10, major healing rain L17, greater healing, divine favour; black wind debuff. The core of Cleric + Mage and Warrior + Cleric + Mage. Re-`attack` after each heal (casting ends autocombat and with it the exp share). See `kb/grouping.md`.
