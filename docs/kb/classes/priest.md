# Priest (WCCCLASS 5): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data,
**[I]** inferred/modelled, **[F]** forum/web. Monsters `hp/exp/maxhit/MR`. Prices include shop
markup (before Charm). Steps from Town Square 1:224. AoE mechanics/spots: `kb/aoe-grinding.md`.

**One-line path:** Dwarf, Wil-first, Hea second. Harm (1 mana) the living and **turn undead
(4 mana, never resisted, ignores MR)** the dead: Graveyard → Crypt → Strange Mansion (all undead)
→ L10 crypt quest (platinum mace) → kobolds/Smuggler Den with agony/holy force → **Ancestral Tomb
6:872 undead from ~L18** (30-45 exp/mana with turn undead) → god's wrath/divine fury/soulstrike late.
**The Priest has no AoE damage spell**: it out-grinds AoE casters on undead instead, and is the
best party healer (healing rains, divine mercy).

## 1. Class facts [C WCCCLASS rec 5; LJNUTIL.C]
| Field | Value | Meaning |
|---|---|---|
| exp | +40% | |
| basehp / hplevel | 3 / 3 | +3 HP/level + 0-3 random (Mage-level fragility) |
| combat | 3 | no melee |
| magery | **Priest, level 3** | every Priest spell incl. magelvl 3 (holy aura, holy armour, iron faith, godheal, great healing rain, divine mercy, paralyze, soulstrike, serenity); Cleric (level 2) gets none of those |
| weapons | 7 = Any Blunt | |
| armour | 1 = Robes | |
| abilities | 31 Bash | Meditate from **L20** |

Casting [C]: max mana = 2×(3×level+3) (+10 Half-Elf). SC = 2×level + **(3×Wil+Int)/6** + 15.
Regen /30 s = **Wil** × (level+20) × 5 / 1650. MR = (Int+3×Wil)/4 — a Wil caster is also the most
spell-resistant caster.

## 2. Race choice [C WCCRACE; ranking I]
| Rank | Race | Wil | Int | Hea | exp% | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Dwarf** | **50-120** | 30-90 | **50-120** | +30 (170%) | light 75 (dark rooms), MR +10, encumbrance +20% | Highest Wil cap and floor (cheap Wil 83 at creation), best HP for a robe caster, sees in the −175 crypts and tombs where Priests farm. |
| 2 | Half-Elf | 40-100 | 45-110 | 30-100 | +15 (155%) | Mana +10 | Cheaper exp and +10 mana; Wil cap 100. |
| 3 | Human | 40-100 | 40-100 | 40-100 | 0 (140%) | — | Fastest leveling. |
| alt | Kang | 45-110 | 30-90 | 50-110 | +35 | poison immune, DR 10 | durable |

## 3. Stat / CP plan [C formulas; I ladder]
Ladder: Wil 75 (Dwarf starts 50 → 83 at creation) → Hea 55 → Int 45 → Wil 90 → Hea 65 → Int 55 → Wil 105 → Hea 75 → Wil 120 → Hea 90 → Int 65.
Wil = SC (0.5/pt), regen, MR. Int: 0.17 SC/pt only. Hea: survival.

| Lvl | CP | Wil/Int/Hea (Dwarf) | SC | Max mana | Regen /30 s | HP ≈ | MR |
|---|---|---|---|---|---|---|---|
| 1 | 100 | 83/45/55 | 66 | 12 | 5 | 32 | 83 |
| 10 | 190 | 95/55/65 | 91 | 66 | 8 | 86 | 95 |
| 15 | 265 | 105/55/73 | 106 | 96 | 11 | 124 | 102 |
| 20 | 340 | 115/55/75 | 121 | 126 | 13 (39 meditating) | 158 | 110 |
| 25 | 440 | 120/60/90 | 135 | 156 | 16 | 220 | 115 |
| 30 | 540 | 120/65/90 | 145 | 186 | 18 | 255 | 116 |

## 4. Spell progression [C WCCSPELS + scroll items; cast% for the Dwarf build]
Damage:
| role | spell `cmd` | lvl | mana | target | dmg@learn | dmg@cap | dmg/mana | cast%@learn | notes | source (list ×2 for mk 100) |
|---|---|---|---|---|---|---|---|---|---|---|
| nuke | harm `harm` | 1 | 1 | single, living only | 4-14 | 4-19 @L6 | 12.0 | 81% | **resist mode always**: MR/2 % resisted (~22-25% vs MR 45-50) | Newhaven 1:2144 free; Holy Spell Shop 1:522 |
| nuke | spiritual hammer `hamm` | 4 | 3 | single | 4-18 | 4-25 @L11 | 5.0 | 80% | never resisted | 1:522, 20g |
| nuke | agony `agon` | 8 | 5 | single, living | 18-35 | 18-56 @L15 | 7.5 | 85% | resisted MR/2 % | 1:522, 75g |
| nuke | holy force `hfor` / unholy force `ufor` | 13 | 8 | single | 18-50 | 18-68 @L22 | 5.4 | 91% | Not-Evil / Not-Good casters | Aldreth 1:2306 / Jorah's Priest 1:3308 150g; unholy: Rhudaur 2:2574 |
| **undead** | **turn undead** `turn` | 3 | 4 | monster, **undead only** | 15-24 | 32-75 @L20 | **13.5** | 72% | damage **ignores armour and MR**, never resisted (LJNSPELL.C:7696, 7140) | 1:522, 8g |
| **undead** | **disrupt** `disr` | 16 | 8 | monster, undead only | 46-84 | 70-132 @L40 | 12.7 | 84% | MR multiplier applies | 1:2306 / 1:3308, 350g |
| nuke | godstrike `gstk` | 20 | 12 | single | 30-80 | 30-104 @L28 | 5.6 | 81% | | 1:2306 / 1:3308, 500g |
| nuke | god's wrath `gwra` | 25 | 14 | single | 57-130 | 57-170 @L35 | 8.1 | 85% | P2 | 1:3308, 1250g |
| nuke | divine fury `fury` | 30 | 16 | single | 50-200 | 50-290 @L40 | 10.7 | 85% | P2 | Very Powerful Spells 10:273, 2500g |
| nuke | soulstrike `soul` | 35 | 20 | single | 90-250 | 90-370 @L50 | 11.5 | 85% | P3 (Priest only) | Nekojin 12:49 3500g; saracen high priest 100% |
| nuke | soul rip `srip` | 45 | 30 | single | 100-440 | 120-540 | 11.0 | 75% | P3 | fallen angel 40% |
| undead | exorcism `exor` | 47 | 35 | monster, undead only, Good casters | 335-470 | 400-600 @L60 | 14.3 | ~80% | P3 | "white scroll": no source found |

Heals, buffs, control:
| Spell `cmd` | Lvl | Mana | Effect | Magelvl | Source |
|---|---|---|---|---|---|
| minor healing `mihe` | 1 | 2 | heal 5-14 (cap L10) | 0 | free, 1:2144/1:522 |
| major healing `mahe` | 8 | 6 | 9-20 @L10 → 16-40 @L30 | 0 | 1:522, 50g |
| greater healing `grhe` | 15 | 12 | 25-50 @L20 → 35-70 @L30 | 2 | 1:2306/1:3308, 350g |
| **godheal** `gdhe` | 22 | 24 | 45-70 @L20 → 50-90 @L30 | 3 | 1:3308, 1000g |
| healing rain `rain` / major / greater | 10 / 17 / 24 | 5 / 15 / 30 | party heal 7-22 / 11-50 / 20-65 | 0/2/3 | 1:522 / 1:2306 / 1:3308 |
| divine mercy `dmer` | 30 | 50 | party heal 31-85 | 3 | 10:273, 3000g |
| bless / greater bless | 2 / 10 | 4 / 8 | +3 / +6 AV | 0 | 1:522 |
| holy armour `armr` | 11 | 10 | DV +7 → +17 (L30), not evil | 3 | 1:522, 180g |
| iron faith `irfa` | 7 | 6 | **MR +7 → +15** (Self) | 3 | 1:522, 75g |
| holy aura / unholy aura | 9 | 15 | party +10 defence vs evil (good) | 3 | 1:522 / 1:2307 |
| protection from evil | 5 | 6 | +10 defence vs evil | 0 | 1:522 |
| black wind `wind` | 21 | 30 | **AoE**: blind, −10 AV, −5 DV, 10 rounds | 2 | 1:3308 / Rhudaur, 850g |
| paralyze `para` | 25 | 30 | single hold | 3 | 10:273 |

## 5. AoE grinding [C: no Priest AoE damage; I strategy]
- No learnable Priest spell deals area damage (`kb/aoe-grinding.md` §1). The AoE-equivalent is
  **mana efficiency on undead**: turn undead never misses on resist, ignores MR, costs 4 mana.
  Example: Ancestral Tomb 6:872 tortured spirit 150/525/15 at L20 = 3 turn undead (12 mana) =
  **44 exp/mana**, about 3× a Mage's fireball on a 5-monster lair [I].
- Lairs of undead let you chain kills without walking: Graveyard edges 1:2617 (N5)/1:2637 (N4)
  (skeleton, zombie are undead; giant bat is not), Strange Mansion 1:63 hallway pool (vision 40,
  shadow spectre 70, astral shade 80, all undead).
- In a group, **black wind** (L21) is the Priest's room-wide contribution: −10 AV on every monster
  for 10 rounds while the Mage/Druid AoEs. Healing rains keep an AoE party alive.

## 6. Gear by level band [C shop/item data; robes, blunt]
| Band | Slot | Item | Stats | Price | Where |
|---|---|---|---|---|---|
| 1-10 | weapon | quarterstaff (2H) or mace (1H + off-hand) | 2-12 free / 3-12 8g | | Newhaven 1:2141 / Blunt Weapons 1:354 |
| 1-10 | off-hand (1H only) | **copper cross** (Good) or red skull (Evil) | SC +5, Mana +5 / **SC +10, regen +10%** | 20g / 2.25p | Holy Item Shop 1:525 (9 steps) / Dreary Shop 7:1136 (355 steps) |
| 1-10 | neck | silver holy amulet | SC +1 | 30g | Jewellry 1:290 |
| 1-10 | torso | robes / **holy vestments** (Good, Priest) | ac 6 / ac 9 | 1g / 16p | 1:309 / 1:525 |
| 1-10 | head | holy miter (Good, Priest) | ac 2 | 4p | 1:525 |
| 8+ | weapon | **astral staff** (Mage/Druid/Priest) | 5-16, Mana +10, SC +10, regen +5% | drop | astral shade 5%, Strange Mansion 1:63 — a Priest's natural hunting ground |
| 10 | weapon | **platinum mace** (Priest) | 6-15 1H, SC +3, regen +15%, self-heal charges 1-25 | quest | Large Tomb 1:2256 (§8) |
| any | head (alt) | silvery skullcap | ac 3, SC +15, MR +10 | 70p | Curio 1:332 |
| 15-20 | hands / arms | silver gloves / silver bracers | mana +5 regen +5% / regen +10% | 9.4p / 11.25p | Jorah's Robes 1:3303 |
| 15 | off-hand drop | **golden chalice** (Priest/Cleric, Good) | +2 dmg, Mana +10, regen +10%, L10 | drop | greater hellion 100% (Champion of Blood chain, `corpus/bosses.md`) |
| 20 | neck drop | holy medallion | SC +3, Mana +10 | drop | guardian golem 5% |
| 20 | torso drop | shimmering white robes | ac 12, SC +3, Mana +5 | drop | "chest" monster 25% |
| 25+ | off-hand | large silvery cross (L25, SC +10 Mana +20 +3 dmg) | | no source found | |
| 30+ | waist | fur-lined pouch | SC +20, MR +10, Mana +10 | 5p | Lost City 16:329 (no walk path found [I]) |

## 7. Grinding spots by level band
| Band | Spot | Spell | Notes |
|---|---|---|---|
| 1-4 | Newhaven Arena 1:2150 | harm | |
| 3-8 | **Graveyard 1:817** + edge lairs 1:2617/2637 | turn undead on skeleton/zombie, harm on bats | 34 steps, light 0 |
| 8-11 | **Crypt L1-3** 1:835/1070/1276 (ghoul/shade/wight/mummy, all undead); **Strange Mansion 1:63** (vision/spectre/astral shade, undead; astral staff drop) | turn undead | Dwarf light 75 vs −175 crypt: borderline; carry a lantern [I] |
| 10 | L10 crypt quest | | §8 |
| 11-17 | Kobolds 6:752; Smuggler Den 9:161; Darkwood bandits 9:720 | agony / holy force | living targets: harm/agony get resisted ~25% |
| 16-22 | Ancient Crypt 1:1943 moaning spirit 128/375/20 (undead) — **avoid skeletal warriors (hit 60)** until HP ≥ 240 | disrupt / turn undead | |
| **18-30** | **Ancestral Tomb 6:872** (hidden w, light −200): tortured spirit 150/525/15, barrow wight 180/725/15, both undead | **turn undead** (32-75 @L20) | Best Priest spot in the game by exp/mana; hits only 15 [C] |
| 25-35 | Dragon's Teeth Hills 2:177; map-3 caves 3:6/35/37 | god's wrath / divine fury | |
| 35+ | soulstrike on anything; undead: Mausoleum 17:2091 (vampire magus/acolyte/minion 500-600 hp, all undead, hit 40-50) | disrupt / turn undead | |

## 8. Class quests and class-only items [C data; F route]
| Lvl | Quest / item | Details |
|---|---|---|
| 10 | **Crypt class quest** | Large Tomb **1:2256** (`sw` from Marble Passage 1:2255): placed **platinum mace** (limit 1) + Super Priest Trainer (shop 53, L11 only, markup 500). Route: `test/play/notes/quests.md` §1 [C rooms/items; F t=28]. |
| 20 | Meditate | Dhelvanen 7:1008, 50 nanati black diamonds, return sigil at L20 [C WCCTEXT 9046]. |
| 10/20 | Alignment quests | ring L10, L20 follow-ups [F t=6, t=10]. Good priests get holy vestments/miter/copper cross; evil get sacrificial robes/red skull [C item alignment flags]. |
| — | Priest-only items | platinum mace, holy vestments, holy miter, sacrificial robes (Evil), emerald-tipped crozier (L20, crimson mist 5%), amber sceptre (L40, SC +12, Mana +66, crimson high priest 11%), crimson blood robes (L40 Evil), golden sun robes (L50 Good, Mana +50 regen +40%) [C]. |

## Group role
Strongest healer: healing rains through greater (L24), godheal L22, divine mercy L30, holy/unholy aura party buffs, turn undead. Robes and random aggro mean it must heal itself first and stand in the back rank. Pair it with plate (Witchunter, Warrior, Cleric). See `kb/grouping.md`.
