# Warlock (WCCCLASS 11): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data,
**[I]** inferred/modelled, **[F]** forum/web. Monsters `hp/exp/maxhit/MR`. Prices include shop
markup (before Charm). Steps from Town Square 1:224. AoE mechanics/spots: `kb/aoe-grinding.md`.
Melee formulas: `test/play/notes/combat.md`. Mage spell details: `kb/classes/mage.md` §4.

**One-line path:** Half-Elf, Int + Hea/Str. Fight in chainmail with a 1H weapon and nuke
(mmis/fjet/lbol) through Graveyard → Crypt → L10 quest (flametongue); dancing blades L11 and
**fireball L15** on lairs; mana storm L23; meteor swarm L25 and dragonfire L30 as the efficient
single-target finishers. More HP than a Mage, so the Warlock is the first caster that can stand in
the 10-goblin Clearing (1:2707). No spirit horde (Mage-only).

## 1. Class facts [C WCCCLASS rec 11; LJNUTIL.C]
| Field | Value | Meaning |
|---|---|---|
| exp | +85% | |
| basehp / hplevel | 4 / 4 | +4 HP/level + 0-4 random |
| combat | 5 | mid melee (Warrior 6, Witchunter 7) |
| magery | **Mage, level 2** | all Mage spells with magelvl ≤ 2. **Missing**: spirit horde, eldritch storm, magic armour, mageshield, mute, sleep, solid fog, wind of haste, mystic mist (magelvl 3) |
| weapons | 4 = Any 1H | 1H blunt and 1H sharp; off-hand slot free (shield / wrist guard) |
| armour | 7 = Chainmail | |
| abilities | 31 Bash | Meditate from **L23**, Smash allowed from **L27** if obtained (LJNUTIL.C:694-728) |

Casting [C]: max mana = 2×(2×level+3) + Mana abilities (Half-Elf +10) → 46 at L10, 86 at L20,
126 at L30 (2/3 of a Mage). SC = 2×level + (3×Int+Wil)/6 + 10. Regen /30 s = Int × (level+20) × 4 / 1650.

## 2. Race choice [C WCCRACE; ranking I]
| Rank | Race | Int | Str | Hea | exp% | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Half-Elf** | 45-110 | 40-90 | 30-100 | +15 (200%) | Mana +10, light 25 | Mana +10 is +20% pool at L10; good Int ceiling; cheap exp. |
| 2 | Human | 40-100 | 40-100 | 40-100 | **0 (185%)** | — | Fastest leveling, better Str/Hea start; no mana bonus. |
| 3 | Dark-Elf | 50-120 | 40-90 | 30-90 | +45 (230%) | light 80, crit +1, AV +3 (3rd AV) | Melee+Int both strong; sees in most dark rooms. Exp cost high. |

## 3. Stat / CP plan [C formulas; I ladder]
Ladder: Int 70 → Hea 55 → Str 55 → Int 80 → Hea 65 → Agi 50 → Str 60 → Int 95 → Hea 75 → Wil 50 → Str 70 → Int 110 → Hea 85.
Str matters for 1H weapon energy (strreq) and carrying chainmail (Str×48 capacity; stay < 33%
encumbrance for +8 dodge/+12 AV, `notes/combat.md`).

| Lvl | CP | Int/Str/Hea/Wil | SC | Max mana | Regen /30 s | HP ≈ | MR |
|---|---|---|---|---|---|---|---|
| 1 | 100 | 70/50/55/40 | 53 | 20 | 3 | 33 | 47 |
| 10 | 190 | 80/57/65/40 | 76 | 56 | 5 | 101 | 50 |
| 15 | 265 | 95/60/65/40 | 94 | 76 | 8 | 136 | 53 |
| 20 | 340 | 95/66/75/50 | 105 | 96 | 9 | 188 | 61 |
| 25 | 440 | 109/70/75/50 | 122 | 116 | 11 | 226 | 64 |
| 30 | 540 | 110/70/85/60 | 135 | 136 | 13 | 287 | 72 |

## 4. Spell progression [C WCCSPELS + scroll items; cast% for the Half-Elf build]
| role | spell `cmd` | lvl | mana | target | dmg@learn | dmg@cap | dmg/mana | cast%@learn | casts | source |
|---|---|---|---|---|---|---|---|---|---|---|
| nuke | magic missile `mmis` | 1 | 1 | single | 4-12 | 4-12 | 8.5 | 68% | 1/rd | Newhaven 1:2144 free; Mage Spell Shop 1:398 |
| nuke | frost jet `fjet` | 4 | 3 | single | 8-18 | 8-25 @L11 | 5.7 | 67% | 1/rd | 1:398 40g |
| nuke | lightning bolt `lbol` | 8 | 5 | single | 16-28 | 16-42 @L15 | 5.9 | 72% | 1/rd | 1:398 150g |
| nuke | sunbolt `sbol` | 13 | 8 | single | 30-50 | 30-73 @L22 | 6.5 | 73% | 1/rd | Aldreth 1:2306 / Jorah's Mage Spells 1:3309, 300g |
| nuke | eldritch bolt `ebol` | 20 | 12 | single | 40-88 | 40-120 @L28 | 6.7 | 75% | 1/rd | same, 12p |
| nuke | **meteor swarm** `mete` | 25 | 4 | single | 20-35 | 20-45 @L35 | 8.2 | 72% | **4/rd** | Very Powerful Spells 10:273, 35p |
| nuke | **dragonfire** `dfir` | 30 | 16 | single | 107-250 | 110-350 @L40 | **14.4** | 75% | 1/rd | 10:273, 60p |
| nuke | colour spray `colu` | 32 | 20 | single | 104-256 | 120-320 | 11.0 | 94% | 1/rd | dark wizard 25% |
| AoE | dancing blades `blad` | 11 | 26 | Full Attack Area | 21-60 | 21-78 @L20 | 1.9/target | 69% | 1/rd | 1:398 500g |
| AoE | fireball `fbal` | 15 | 34 | Full Attack Area | 33-90 | 33-114 @L23 | 2.2/target | 74% | 1/rd | 1:398 700g |
| AoE | mana storm `msto` | 23 | 50 | Full Attack Area | 59-170 | 59-215 @L32 | 2.8/target | 70% | 1/rd | 1:3309 25p |
| AoE | blizzard `bliz` | 40 | 45 | Full Attack Area | 80-250 | 80-310 | 4.3/target | 80% | 1/rd | storm giant king 7% |

Buffs/control available to a Warlock (magelvl ≤2) [C]: blur L1 (shield 5), ethereal shield L5
(shield 3-12, DR 1), shockshield L9, flash L7 (AoE blind −8 AV), stinking cloud L8 (AoE confusion),
slow L10, speed L12 (energy 85%: **more melee swings** per round, good for a Warlock), confusion L14,
vulnerability L14 (−10 MR), ice storm L17 (room DoT 3/rd), enslave L18 (M2), corrosion L24.
Cast% is ~10 points under a Mage at equal Int (magelvl 2 → +10 instead of +15): buy Spellcasting
gear early (runed wrist guard SC +10 at L20).

## 5. AoE grinding plan [I; mechanics in `kb/aoe-grinding.md`]
- Mana pool is small (86 at L20 = 2 fireballs + change). Warlock AoE is "**one cast, then melee the
  survivors**": fireball for 62-74 leaves 70-100-hp monsters on ~10-30 hp; finish with lightning
  bolt/sunbolt or the weapon.
- HP ≈ 35 + 7.25L (188 at L20) lets a Warlock take 2 rounds from 5 monsters that a Mage cannot.

| Band | AoE | Best lairs | Fallback |
|---|---|---|---|
| 11-14 | dancing blades (avg 41) | Graveyard Western Edge 1:2617 (N5), 1:2637; kobold lair 6:754 | melee + lbol in Crypt/kobolds |
| 15-22 | fireball | **Smuggler Den 9:218/219/222** (N5 ×3); bandit lairs 9:809…; orc Torchlit 9:439/475 (N3-4); Darkwood goblin lairs 1:424… (N5, 73-90 hp: fireball + melee) | wolves 10:35 |
| 23-29 | mana storm (avg 115-125) | **Darkwood Clearing 1:2707: N10 goblins, one mana storm kills all (1,150 exp/cast)** at HP ≥ 230; map-3 caves 3:6/593/35/37 | meteor swarm (4 casts/rd) |
| 30+ | mana storm / blizzard (40) | Foul Cave 3:35, Misty Bog 6:398/471, Lava 15:1019 | **dragonfire** (14 dmg/mana) |

## 6. Gear by level band [C shop data; chainmail = armour type ≤7]
| Band | Slot | Item | Stats | Price | Where |
|---|---|---|---|---|---|
| 1-10 | weapon | club / mace / morning-star | 3-8 free / 3-12 8g / 3-15 12g | | Newhaven 1:2141 / Blunt Weapons 1:354 |
| 1-10 | torso | chainmail hauberk / black chainmail hauberk | ac 18 dr 4 / ac 19 dr 4.5 | 40g / 69g | Skali's 1:306 / Slum Store 1:2324 |
| 1-10 | legs, head, hands, feet | chainmail leggings / chain coif / chain gauntlets / chainmail boots | ac 5 / 4.5 / 5 / 5 | 12g / 10g / 6g / 10g | 1:306, 1:305 |
| 1-10 | off-hand | curved heater | ac 4 dr .4 | 24g | Shield Shop 1:2153 |
| 10 | weapon | **flametongue** (Warlock) | 4-18 1H sharp, SC +2, regen +10%, light 100, auto flame hits | quest | Large Tomb 1:2301 (§8) |
| 15 | torso | fine chainmail hauberk | ac 20 dr 4, L15 | 14p | Dwarven Armoury 6:1337 (205 steps) |
| 15 | weapon (alt) | dwarven hammer / dwarven axe | 6-17 / 5-16 | 20p | 6:1344 / 6:1336 |
| 20 | off-hand | **runed wrist guard** | ac 2.8, **SC +10, regen +5%**, L20 | 11.25p | Jorah's Leather/Chain 1:2328 (81 steps) |
| 20 | head | silvery skullcap (robes, type 1: allowed) | ac 3, SC +15, MR +10 | 70p | Curio 1:332 |
| 20 | torso (alt) | dwarven chainmail hauberk | ac 21 dr 4, L20 | 80p | 6:1337 |
| 30 | legs/head/hands | adamantite chainmail leggings / coif / chain gauntlets | ac 8.5 / 7 / 6, L30 | 80p / 50p / 60p | 6:1337 |
| 30 | off-hand | adamantite wrist guard | ac 3.5 dr .4, L30 | 20p | 6:1337 |
| 45 | set | lunar chainmail hauberk/leggings/coif | ac 27/10/8, Mana +10/+5/+2, L45 | 206p/169p/56p | 1:2328 |
| 20 | off-hand drop | arcane tome (Mage/Warlock) SC +10 Mana +20 | | no source found | |

## 7. Grinding spots by level band
| Band | Single-target / melee | AoE |
|---|---|---|
| 1-4 | Newhaven Arena 1:2150 | — |
| 4-8 | Graveyard 1:817, Slum streets 1:1073 | — |
| 8-11 | Crypt 1:835-1:1276; Slimy Sewers 9:1 (melee-friendly, hit 12-14); Strange Mansion 1:63 | — |
| 11-15 | Kobolds 6:752; Darkwood bandits 9:720 | blades: Graveyard edges, kobold lair 6:754 |
| 15-20 | Frozen Caverns wolves 10:35 (hit 22) | fireball: Smuggler Den 9:218-222, nomad caverns 10:43/80 |
| 20-30 | Ancestral Tomb 6:872; Dragon's Teeth Hills 2:177 | mana storm: Clearing 1:2707, map-3 caves |
| 30+ | dragonfire anywhere | Misty Bog, Lava Cavern, green dragon nursery 1:2805 |

## 8. Class quests and class-only items [C data; F route]
| Lvl | Quest / item | Details |
|---|---|---|
| 10 | **Crypt class quest** | Large Tomb **1:2301** (`w` from Marble Passage 1:2297): placed **flametongue** (limit 1) + Super Warlock Trainer (shop 61, L11 only, markup 500). Shared crypt route: `test/play/notes/quests.md` §1 [C rooms/items; F t=28]. |
| 23 | Meditate | Dhelvanen 7:1008, 50 nanati black diamonds, return sigil at **L23** for a Warlock (WCCTEXT 9046) [C]. |
| 27 | Smash allowed | how to obtain is unknown in this data set (`notes/combat.md`) [I]. |
| — | Warlock-only items | flametongue, tails of battle (L40 back, +50 HP, +2 dmg, no source), L50 good/neutral/evil quest blades (Solarblade, Windblade, Demontongue, Soulwaste…) [C]. |

## Group role
AoE plus melee in chainmail. Survives its share of hits better than a Mage, so it fits Warrior + Cleric + Warlock lair clears. See `kb/grouping.md`.
