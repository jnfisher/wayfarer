# Druid (WCCCLASS 13): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data,
**[I]** inferred/modelled, **[F]** forum/web. Monsters `hp/exp/maxhit/MR`. Prices include shop
markup (before Charm). Steps from Town Square 1:224. AoE mechanics/spots: `kb/aoe-grinding.md`.

**One-line path:** Half-Elf, Int = Wil, Hea third. Vine strike → elemental chaos (L4) →
stonestrike (L8) → poison bolt (L10), with mend for self-healing; L10 crypt quest (darkwood staff);
**swarm L12, chaos storm L15** on lairs; maelstrom L24, **earthquake L28**; plasma bolt (L23) and
earthfist (L30) single-target. The Druid is the full-caster alternative to the Mage: same mana
(magery level 3), more HP, self-heals, leather armour, but no L40+ shop AoE.

## 1. Class facts [C WCCCLASS rec 13; LJNUTIL.C]
| Field | Value | Meaning |
|---|---|---|
| exp | +90% | |
| basehp / hplevel | 4 / 3 | +4 HP/level + 0-3 random |
| combat | 4 | weak melee |
| magery | **Druid, level 3** | every Druid spell |
| weapons | 7 = Any Blunt | 1H/2H blunt (staves, maces). Class ability "Class Item Inclusion 74" (74 = sickle) has no effect: the code only reads that ability on items [C LJNUTIL.C:9727; I] |
| armour | 6 = Studded Rigid Leather | leather up to type 6 |
| abilities | 31 Bash | Meditate from **L20** |

Casting [C]: max mana = 2×(3×level+3) + Mana abilities (= Mage). SC = 2×level + **(Int+Wil)/3** + 15.
Regen /30 s = **(Int+Wil)/2** × (level+20) × 5 / 1650. Int and Wil count equally for everything;
Wil also gives MR (0.75/pt).

## 2. Race choice [C WCCRACE; ranking I]
| Rank | Race | Int | Wil | Hea | exp% | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Half-Elf** | 45-110 | 40-100 | 30-100 | +15 (205%) | Mana +10, light 25 | Mana +10, cheap exp, Int+Wil to 210. |
| 2 | Gnome | 45-110 | 40-100 | 40-100 | +20 (210%) | **light 65**, MR +5, Picklocks | Same caps, better Hea floor, sees in −150 rooms; no mana bonus. |
| 3 | Dwarf | 30-90 | **50-120** | 50-**120** | +30 (220%) | light 75, MR +10, encumbrance +20% | Tankiest caster: HP and MR; Int cap 90 costs ~5 SC late. |
| alt | Gaunt One | 60-140 | 50-100 | 30-70 | +50 (240%) | light 200 | highest Int+Wil (240) but slow and fragile |

## 3. Stat / CP plan [C formulas; I ladder]
Ladder: Wil 60 → Int 60 → Hea 50 → Wil 70 → Int 70 → Hea 60 → Wil 85 → Int 85 → Hea 70 → Wil 95 → Int 95 → Hea 80 → Wil 100 → Int 105.
Buy Int and Wil in parallel (each point = +0.33 SC, +0.5 regen-stat); Wil first because it also adds MR.

| Lvl | CP | Int/Wil/Hea | SC | Max mana | Regen /30 s | HP ≈ | MR |
|---|---|---|---|---|---|---|---|
| 1 | 100 | 60/66/50 | 59 | 22 | 4 | 31 | 64 |
| 10 | 190 | 70/76/60 | 83 | 76 | 6 | 91 | 74 |
| 15 | 265 | 80/85/60 | 100 | 106 | 8 | 121 | 83 |
| 20 | 340 | 85/88/70 | 112 | 136 | 10 (30 meditating) | 170 | 87 |
| 25 | 440 | 95/95/72 | 128 | 166 | 12 | 208 | 95 |
| 30 | 540 | 100/100/80 | 141 | 196 | 15 | 261 | 100 |

## 4. Spell progression [C WCCSPELS + scroll items; cast% for the Half-Elf build]
| role | spell `cmd` | lvl | mana | target | dmg@learn | dmg@cap | dmg/mana | cast%@learn | casts | source (list price ×2 for mk 100) |
|---|---|---|---|---|---|---|---|---|---|---|
| nuke | vine strike `vine` | 1 | 1 | single | 4-9 | 4-12 @L6 | 8.5 | 74% | 1/rd | Newhaven 1:2144 free; **Druid Spell Shop 1:1314** (Hollow Tree, 18 steps) |
| nuke | elemental chaos `chao` | 4 | 3 | single | 7-17 | 7-24 @L11 | 5.3 | 74% | 1/rd | 1:1314, 20g |
| nuke | stonestrike `strk` | 8 | 5 | single (stone) | 14-27 | 14-39 @L14 | 5.4 | 78% | 1/rd | 1:1314, 75g |
| nuke | poison bolt `pbol` | 10 | 6 | single (poison, **resist mode always**) | 21-35 | 22-49 @L17 | 6.0 | 83% | 1/rd | 1:1314, 60g. Poison-immune monsters take nothing extra; resisted MR/2 % |
| nuke | acid jet `acid` | 13 | 8 | single (water) | 24-50 | 24-68 @L20 | 5.8 | 79% | 1/rd | Aldreth 1:2306 / Jorah's Druid Spells 1:2334, 150g |
| nuke | blaze `blaz` | 20 | 12 | single (hot) | 38-80 | 38-112 @L28 | 6.3 | 72% | 1/rd | 1:2334, 725g |
| nuke | **plasma bolt** `plas` | 23 | 14 | single | 54-142 | 54-190 @L35 | 8.8 | 72% | 1/rd | 1:2334 / Very Powerful Spells 10:273, 1750g |
| nuke | **earthfist** `fist` | 30 | 16 | single (stone) | 72-200 | 72-290 @L40 | 11.3 | 81% | 1/rd | 10:273, 3000g |
| nuke | elemental fury `fury` | 35 | 5 | single, chains lightning→fire→ice | 30-85 | 30-115 @L50 | 14.6 | 83% | 4/rd | Nekojin 12:49 3500g; manscorpion king 100% |
| nuke | summon fire/lightning/stone/ice elemental | 37-40 | 25 | single | 117-410 | up to 135-575 | 13-14 | ~77% | 1/rd | boss drops 30% (Choira, Lallim, Argak, Sharh'Kur) |
| AoE | swarm `swar` | 12 | 28 | Full Attack Area | 24-60 | 24-76 @L20 | 1.8/target | 80% | 1/rd | 1:1314, 200g |
| AoE | chaos storm `csto` | 15 | 34 | Full Attack Area | 29-80 | 29-104 @L23 | 2.0/target | 80% | 1/rd | 1:1314, 350g |
| AoE | maelstrom `mael` | 24 | 50 | Full Attack Area | 62-155 | 62-195 @L32 | 2.6/target | 80% | 1/rd | 1:2334, 1250g |
| AoE | **earthquake** `quak` | 28 | 60 | Full Attack Area | 96-230 | 99-302 @L40 | 3.4/target | 80% | 1/rd | 10:273, 2500g |
| AoE | acid tempest `atem` | 40 | 65 | Full Attack Area (+acid DoT) | 96-300 | 100-405 | 3.9/target | 83% | 1/rd | no shop/drop found |
| AoE | thunderclap `thdr` | 45 | 75 | Full Attack Area | 40-280 | 40-294 | 2.2/target | 48% | 1/rd | storm giant commander 7% |

Element note [C]: single-target elemental spells are cut by the monster's matching resistance
ability (LJNSPELL.C:7470); area spells ignore element resistance against monsters.

Heals, buffs, control [C]:
| Spell `cmd` | Lvl | Mana | Effect | Use |
|---|---|---|---|---|
| mend `mend` | 2 | 4 | heal 6-17 @L10, 13-27 @L30 | main self-heal (≈3 HP/mana) |
| barkskin `skin` | 7 | 15 | DR 10→30 (L20) [units likely tenths, I], DV +5, **fire defence −25** | keep up; drop vs fire monsters |
| rapid healing `rhel` | 11 | 8 | healing rate +100% for 60 rounds (5 min) | **cast before every rest**: halves rest time |
| regeneration `rgen` | 15 | 12 | heal 2-7 per round for 10 rounds (D2) | |
| stoneskin `ston` | 19 | 20 | DR 60→90, DV +10, MR +5 (replaces barkskin) | always on from 19 (Jorah's 1:2334, 600g) |
| charm animal `char` | 5 | 10 | enslave an Animal | pet tank early [I] |
| entangle `enta` / mass entangle `msen` | 14 / 25 | 12 / 30 | hold 4 rounds; mass = AoE | holds movement (LJNMONST.C:1233); whether it stops attacks is unverified [I] |
| acid rain `rain` / plague `plag` | 15 / 26 | 12 / 40 | room DoT 3-6/round, −DV/dodge | debuff only |
| rejuvinating field `reju` | 32 | 40 | party HoT | groups |

## 5. AoE grinding plan [I; mechanics in `kb/aoe-grinding.md`]
Druid AoE numbers track the Mage's one band later (swarm ≈ blades, chaos storm ≈ 90% of fireball,
maelstrom ≈ mana storm, earthquake ≈ 95% of spirit horde) with ~20% more HP, so the Druid can take
the second round of hits more often.

| Band | AoE (avg) | Best lairs (`map:room`, N, pool) | Fallback |
|---|---|---|---|
| 12-14 | swarm (42) | Graveyard Western Edge 1:2617 (N5, 35-62 hp, 34 steps), 1:2637 (N4); kobold Cramped Tunnel 6:754 (N3, 30-50 hp) | stonestrike/poison bolt in Crypt, kobolds |
| 15-23 | chaos storm (55-66) | **Smuggler Den 9:218/219/222** (N5 ×3, 55-70 hp; 2 casts until L20); bandit lairs 9:809/822/848/859; orc Torchlit 9:439/475; nomad Torchlit Cavern 10:43/80 (110-130 hp, 2 casts) | acid jet on wolves 10:35 |
| 24-27 | maelstrom (110) | Darkwood goblin lairs 1:424… (N5, one-shot); orc Torchlit 9:475 (N4 one-shot); map-3 caves 3:6, 1:2362, 3:593 (2 casts) | plasma bolt |
| 28-39 | earthquake (163-185) | Darkwood Clearing 1:2707 (N10 goblins, one-shot) at HP ≥ 230; Foul/Witch's Cave 3:35/37 (vampire bats 235/1100); Misty Bog 6:398/471 | earthfist |
| 40+ | acid tempest (if found) / earthquake | Lava Cavern 15:1019 (N5), Limestone 12:1282 | elemental fury, summon elementals |

## 6. Gear by level band [C shop data; armour type ≤6, blunt weapons]
| Band | Slot | Item | Stats | Price | Where |
|---|---|---|---|---|---|
| 1-10 | weapon | quarterstaff / mace / morning-star | 2-12 free / 3-12 8g / 3-15 12g | | Newhaven 1:2141 / Blunt Weapons 1:354 |
| 1-10 | torso | rigid leather tunic | ac 13 dr 1.3 | 16g | Leatherworks 1:310 |
| 1-10 | legs / hands / head / feet | rigid leather pants / hard leather gauntlets / hard leather helm / hardened leather boots | ac 3 each | 4g/2g/4g/4g | 1:310, 1:2154 |
| 1-10 | off-hand (1H weapon) | buckler / copper cross is Priest-only | ac 1 | 2g | Shield Shop 1:2153 |
| 8+ | weapon | **astral staff** (Mage/Druid/Priest) | 5-16, Mana +10, SC +10, regen +5% | drop | astral shade 5%, Strange Mansion 1:63 |
| 10 | weapon | **darkwood staff** (Druid) | 5-17 2H blunt, SC +5, regen +15%, hold-person charges | quest | Large Tomb 1:2303 (§8) |
| 10 | torso | ogre-skin shirt / gianthair vest | ac 15 dr 1.5 L10 / ac 14 | 14p / 5p | Dwarven Leatherworks 6:1343 |
| any | head | silvery skullcap | ac 3, SC +15, MR +10 | 70p | Curio 1:332 |
| 15 | head (alt) | **antlered helm** (Druid) | ac 4, **SC +15, regen +10%**, L15 | drop | high druid 2% (also "Trap Shop", location not found) |
| 15-20 | hands / arms | silver gloves / silver bracers | mana +5 regen +5% / regen +10% | 9.4p / 11.25p | Jorah's Robes 1:3303 |
| 20 | torso | **woodland robes** (Druid) | ac 17 dr 1, **Mana +20, regen +10%**, L20 | placed | Inside The Broken Willow 3:214 [C room data] |
| 20 | torso (alt) | witchwood breastplate | ac 18 dr 5 (−9 dodge), L20 | 12.5p | Elven Shop 3:541 (151 steps) |
| 20 | off-hand | runed wrist guard is not Druid-listed; **ironwood shield** (Druid) ac 3 dr .7 L20 | | 25p | Elven Shop 3:541 |
| 20+ | ring | **turquoise ring** (Druid) | ac 2, SC +5, MR +5, Mana +5 | 244p | Jorah's Misc 1:2330 |
| 20+ | weapon drop | silverwood staff (wraithlord 10%): mana +5 regen +20%; copperwood staff L37 (Lallim 20%): mana +30 regen +25% | | | |
| 30+ | waist | fur-lined pouch | SC +20, MR +10, Mana +10, L30 | 5p | Lost City 16:329 (no walk path found [I]) |

## 7. Grinding spots by level band
| Band | Single-target | AoE |
|---|---|---|
| 1-4 | Newhaven Arena 1:2150 | — |
| 4-8 | Graveyard 1:817, Slum streets 1:1073 | — |
| 8-12 | Crypt 1:835-1:1276; Strange Mansion 1:63 (astral staff); Slimy Sewers 9:1 | — |
| 10 | L10 crypt quest (darkwood staff, train to 11) | |
| 12-15 | Kobolds 6:752, Darkwood bandits 9:720 | swarm: Graveyard edges, kobold lair 6:754 |
| 15-20 | Wolves 10:35 | chaos storm: Smuggler Den 9:218-222, bandit/orc/nomad lairs |
| 20-30 | Ancestral Tomb 6:872; Dragon's Teeth Hills 2:177 | maelstrom → earthquake: goblin lairs, map-3 caves, Clearing 1:2707 |
| 30+ | earthfist, plasma bolt | earthquake: Misty Bog, Foul Cave, Lava Cavern |

## 8. Class quests and class-only items [C data; F route]
| Lvl | Quest / item | Details |
|---|---|---|
| 10 | **Crypt class quest** | Large Tomb **1:2303** (`e` from Marble Passage 1:2299): placed **darkwood staff** (limit 1) + Super Druid Trainer (shop 63, L11 only, markup 500). Route: `test/play/notes/quests.md` §1 [C rooms/items; F t=28]. |
| 20 | Meditate | Dhelvanen 7:1008 (50 nanati black diamonds; return the sigil at L20) [C WCCTEXT 9046]. |
| 20 | woodland robes | placed in Inside The Broken Willow 3:214 [C]; how to reach/claim it untested [I]. |
| — | Druid-only items | darkwood staff, antlered helm, woodland robes, ironwood shield, turquoise ring, silverwood/copperwood/burning/ice crystal/runed petrified staves, leafwoven cloak (L40, Mana +35, regen +15%, no source found) [C]. |

## Group role
Second AoE line (swarm → earthquake) plus secondary healing (mend, regeneration on others; rejuvinating field party HoT L32). Can be both caster and backup healer in a 3-party. See `kb/grouping.md`.
