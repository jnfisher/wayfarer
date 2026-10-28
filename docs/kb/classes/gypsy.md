# Gypsy (WCCCLASS 10): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data,
**[I]** inferred/modelled, **[F]** forum/web. Monster stats are `hp/exp/maxhit`. Prices include
shop markup, not Charm. Steps are from Town Square 1:224. Shared formulas:
`test/play/notes/combat.md`; areas: `test/play/corpus/leveling.md`; bosses: `corpus/bosses.md`.
Formula details shared with the other hybrids are in `bard.md` §1.

**One-line path:** Half-Elf, Int-first. Magic missile from L1, frost jet at L4, lightning bolt at L8.
Graveyard, then Crypt, then the L10 crypt quest (silver rapier). Sunbolt at L13, then kobolds/bandits
and wolves. Eldritch bolt at L20 and meteor swarm at L25 carry you to 30+. A rogue-flavoured mage,
light on HP.

## 1. Class facts [C: WCCCLASS rec 10]
| Field | Value | Meaning |
|---|---|---|
| exp | **+120%** | L2 = 10×(100+race+120). Half-Elf: 2,350 |
| basehp / hplevel | 4 / 3 | +4 HP/level + `genrdn(0,4)` |
| combat | **4** | AV ~30 below a Witchunter at L10. Melee is a fallback only. |
| magery | **Mage (1), level 2** | Learns every mage spell with magelvl ≤ 2. Misses only the magelvl-3 spells (Mage class has those) [C LJNUTIL.C:9779] |
| weapons | 4 = Any 1H | No 2H, no off-hand items unless the item lists class 10 (LJNUTIL.C:9697) |
| armour | 6 = Studded Rigid Leather | |
| abilities | 103 Class Stealth, 37 Picklocks, 39 Thievery (+0), 40 Find Traps, 31 Bash | Same thief kit as Bard; Meditate from **L23** via Dhelvanen 7:1008 (50 nanati black diamonds, `giveability 187` at minlevel 23, WCCTEXT 9046) [C] |

Casting [C LJNUTIL.C:4919-4952; LJNBKGND.C:1677, 2143]:
- Max mana = 2×(2×level+3) (+10 Half-Elf/Elf). Mana regen/tick and MEDITATE = **Int** × (level+20) × 4 / 1650.
- SC = 2×level + (3×Int + Wil)/6 + 10. Cast chance = SC + diff (cap 98). A fail costs the energy and half the mana.
- Combat spells cost 1000 energy (one cast per round) except meteor swarm (250: four casts per round). Resist mode 0 damage spells are never MR-resisted (LJNSPELL.C:7520) [C].

## 2. Race choice [C WCCRACE; I ranking]
| Rank | Race | exp | Int | Hea | Agi | Cha | Extras | Why |
|---|---|---|---|---|---|---|---|---|
| **1** | **Half-Elf** | +15 (235%) | 45-110 | 30-100 | 45-110 | 50-110 | Mana +10, NV 25 | Cheapest exp of the Int races. +10 mana is +50% at L1. Cha 50 base helps thievery/stealth/dodge. |
| 2 | Gnome | +20 (240%) | 45-110 | 40-100 | 45-110 | 35-90 | MR +5, **NV 65**, race Picklocks | NV 65 means no lantern anywhere down to −215. Hea 40 base gives more HP. No mana bonus. |
| 3 | Dark-Elf | +45 (265%) | 50-120 | 30-90 | 50-120 | 40-110 | **NV 80**, race stealth (+10 stealth, +5 BS AV), crit +1, AV +3 | Highest Int cap and the best backstabber. Costs 13% more exp than Half-Elf. |
Elf (Int 50-120, mana +10, Hea max 80, exp 45) is a close 4th. Avoid Halfling (−1 HP/level), Gaunt One (+50 exp), Half-Ogre (Int max 60).

## 3. Stat / CP plan (Half-Elf) [C costs; I ladder]
Ladder: Int 60 → Hea 50 → Int 70 → Agi 53 → Cha 55 → Str 55 → Int 80 → Hea 60 → Wil 50 → Int 90 → Agi 62 → Hea 70 → Cha 60 → Int 100 → Wil 60 → Hea 80 → Str 65 → Int 110.

| L | CP | Str | Int | Wil | Agi | Hea | Cha | bank | HP~ | mana | regen | SC | MR | stealth | thievery | dodge* |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 100 | 51 | 70 | 40 | 53 | 50 | 55 | 0 | 31 | 20 | 3 | 53 | 47 | 52 | 33 | 2 |
| 6 | 150 | 55 | 80 | 40 | 53 | 52 | 55 | 1 | 62 | 40 | 5 | 68 | 50 | 64 | 55 | 3 |
| 10 | 190 | 55 | 81 | 50 | 53 | 60 | 55 | 3 | 96 | 56 | 5 | 78 | 57 | 72 | 71 | 4 |
| 15 | 265 | 55 | 90 | 50 | 62 | 65 | 55 | 1 | 136 | 76 | 7 | 93 | 60 | 85 | 94 | 8 |
| 20 | 340 | 55 | 99 | 50 | 62 | 70 | 60 | 2 | 180 | 96 | 9 | 107 | 62 | 92 | 104 | 10 |
| 30 | 540 | 65 | 110 | 60 | 70 | 82 | 60 | 3 | 281 | 136 | 13 | 135 | 72 | 105 | 128 | 14 |
*Unencumbered dodge adds ~+8.

- **Int**: +6 Int = +3 SC, plus mana regen, crits (`(Int−50)/10`), perception and thievery. It is the primary stat to ~90.
- **Wil 50** for MR (spell defence). Mages die to enemy casters.
- **Str 55**: capacity 2,640 (33% = 870). That is enough for a leather kit without the helm, or with a skullcap (50 wt).
- Hea: HP is the gypsy's weak point. Raise it in step with Int after L6.

## 4. Attack style and spells
**Spells are the main attack. Melee is for finishing blows and saving mana.** Damage at level (avg per mana) [C data, computed]:
| Spell (cmd) | Lvl | Mana | diff | Target | L5 | L10 | L15 | L20 | L25 | L30 |
|---|---|---|---|---|---|---|---|---|---|---|
| **magic missile** (`mmis`) | 1 | **1** | +15 | single | 4-12 (8/m) | 4-12 | 4-12 | 4-12 | | |
| frost jet (`fjet`) | 4 | 3 | +5 | single | 8-19 | 8-24 | 8-25 | 8-25 | | |
| lightning bolt (`lbol`) | 8 | 5 | 0 | single | – | 16-32 | 16-42 | 16-42 | | |
| vampiric touch (`vamp`) | 11 | 8 | −7 | single, living only | – | – | 8-20 | | | |
| dancing blades (`blad`) | 11 | 26 | −10 | **AoE** | – | – | 21-68 | 21-78 | | |
| **sunbolt** (`sbol`) | 13 | 8 | −14 | single | – | – | 30-55 | 30-68 | 30-73 | |
| fireball (`fbal`) | 15 | 34 | −20 | **AoE** | – | – | 33-90 | 33-105 | 33-114 | |
| deathtouch (`dtch`) | 16 | 25 | −5 | single, living only | – | – | – | 25-58 | | |
| **eldritch bolt** (`ebol`) | 20 | 12 | −30 | single | – | – | – | 40-88 | 40-108 | 40-120 |
| mana storm (`msto`) | 23 | 50 | −45 | AoE | | | | | 59-180 | 59-205 |
| **meteor swarm** (`mete`) | 25 | 4 | −50 | single, **250 energy (×4/round)** | | | | | 20-35 ×4 | 20-40 ×4 |
| dragonfire (`dfir`) | 30 | 16 | −60 | single | | | | | | 107-250 |
Takeaways [I]:
- L1-7: magic missile. It costs 1 mana, and Half-Elf L1 mana is 20, so 20 casts. Melee in between.
- L8-12: lightning bolt. L13-19: sunbolt (6/mana). AoE (blades, fireball) costs ~3× the mana per damage, so save it for multi-monster rooms.
- L20+: eldritch bolt. **L25+: meteor swarm** at 16 mana per full round (~110 damage) is the best sustained damage. Its diff −50 needs SC ~140 for 90%, so expect ~75% at L25.
- **SC check:** at L20, SC 107 means eldritch bolt (−30) lands 77%. Push Int to keep the top nuke ≥ 75%.
- Spells ignore monster AC and "Magical" protection. Salamanders and margoyles (Magical 1) need no magic weapon.
Buffs/utility: **blur** (L1, dodge +5, 70 rounds, 4 mana); **ethereal shield** (L5, AC +3…, DR +1, self); shadowform (L7, stealth); resist cold/fire; flash (L7, AoE blind, resisted); stinking cloud (L8, AoE confuse/−5 AV); shockshield (L9); slow (L10); **speed** (L12, energy ×85% on self or a fighter); confusion (L14); vulnerability (L14, −8 AC/−10 MR); enslave (L18, pets); corrosion (L24, AoE −8 AC/−5 DR).
Thievery: `steal`/`pick`/`search`/`disarm` from L1. Backstab opener (`sneak`, then `bs`) with a dagger or the silver rapier (BS AV +20) [C LJNCOMBT.C:797].

### Spell scroll sources [C shop data]
| Shop | Room / steps | Markup | Scrolls (price) |
|---|---|---|---|
| Newhaven Spell Shop | 1:2144 | 0 | magic missile, blur (free), illuminate 4g, smite 8g |
| **Mage Spell Shop** (Magic Shoppe) | **1:398**, 16 steps | 100 | mmis/blur free, illuminate 8g, smite 16g, frost jet 40g, resist cold 30g, ethereal shield free, detect magic free, resist fire 60g, shadowform 200g, flash 90g, stinking cloud 250g, **lightning bolt 150g**, shockshield 120g, dancing blades 500g, vulnerability 600g, **fireball 7p** |
| Jorah's Mage Spells (Blue Tower) | 1:3309, 37 steps (5g toll at 1:1381) | 100 | slow 180g, vampiric touch 200g, speed 200g, **sunbolt 300g**, confusion 300g, deathtouch 8p, ice storm 9p, enslave 12p, **eldritch bolt 12p**, mana storm 25p, corrosion 30p |
| Very Powerful Spells | 10:273, ~77 steps (Frozen Caverns hidden exits; **Level exit ≥25** at 10:272) | 100 | **meteor swarm 35p**, dragonfire 60p |
| Aldreth's Spells | 1:2306 | 100 | Duplicates Jorah. **No inbound exit in the data** [C], so it is unusable. |
Budget: ~1.5g by L4, ~1.5p by L8, 3p by L13, 12p by L20, 35p by L25.

## 5. Gear by level band [C]
Weapons (1H; Int-built Str ~55 → prefer Str ≤ 55):
| Band | Weapon | Dmg | Str | Price | Where | Notes |
|---|---|---|---|---|---|---|
| 1-5 | dagger / knife | 1-5 / 2-5 | 0 | 2g / 3g | 1:355 | Backstab opener (BS AV +10) |
| 2-10 | mace / longsword | 3-12 | 45 | 8g / 24g | 1:354 / 1:355 | |
| 4-10 | heavy-mace | 4-13 | 50 | 10g | 1:354 | |
| **10** | **silver rapier** (quest) | **10-17**, speed 1300, acc +10, ac1, BS AV +20, crit +4, SC +2, mana regen +10% | 40 | quest | placed in 1:2300 Large Tomb | Class 10 only, Magical 2, L10, **limit 1**, procs spell 776 |
| 15+ | fine throwing knife | 3-10, acc +10 | 0 | 7.5p | Gypsy Man 6:124 | throwing |
| 30+ | starsteel shortsword | 6-23 | 60 | 150p | Jorah 1H Sharp 1:3304 (L30) | |
| 40 | midnight glare | 9-28, BS +20/+10/+20 | 0 | not sold | class 10, L40 | drop/quest |
| 50 | soulblighter 14-35 (evil 210), Solarblade, Windblade, acid slasher … | | | 110p / not sold | 17:776 / drops | |

Gypsy Man shop 6:124 (82 steps: 5g toll at 1:1381, then **rope and grapple** Item exits): class-10 gear [C]:
| Item | Slot | Stats | Min L | Price |
|---|---|---|---|---|
| red paisley bandanna | head | ac3, perception +5, crit +1, dodge +5, mana regen +5% | 15 | 9p |
| layered sun skirt | legs | ac2, **night vision +25**, dodge +5 | 10 | 4.5p |
| blue pantaloons | legs | ac3, dodge +5, mana regen +10% | 20 | 6p |
| **mirrored choli** | torso | **ac16 dr1.6, wt 200** (half a leather tunic), MR +10, mana regen +10% | 20 | 90p |
| gold-coin sash | waist | ac.5, **SC +10**, dodge +5, stealth −10 | – | 30p |
| brass zills | off-hand | acc +5, +2 dmg, BS min +10 | 30 | 3p |
| swamp boots | feet | ac1, stealth −10 | 30 | 2.2p |

Armour otherwise (type ≤6): rigid leather tunic ac13 16g (1:310) → ogre-skin shirt ac15 L10 14p (6:1343) → mirrored choli L20. Rigid leather pants 4g, hard leather helm 4g (or skullcap 6g, wt50), hard leather gauntlets 2g, hardened leather boots 4g, travelers cloak 4g (avoid greatcloak, stealth −10), golden armbands 2.4p, golden amulet 1p, amethyst ring 20g. L20+: wyvernhide head/hands/feet 56-75p (1:2328). L40: shawl of fortune (class 10 cloak, ac3, BS +10/+15, mana regen +10%; only from Recycler 6 1:516 if a player sold one).
Other class-10 items: arcane tome (off-hand, SC +10, mana +20, L20, 5100p at the sleazy dark-elf shop 8:486); thief's pouch (wrist, thievery/picklocks +5, L20, 600p at Lost City 16:317); deck of cards (L35, no source).
Light: NV 25 covers −175. A lantern (2g, 1:334) covers −200.

## 6. Grinding progression [C data; I fit]
HP is ~10% below a Bard's at equal Hea, and AC is leather. **Pick areas with max hit ≤ 20% of HP and kill before contact** (open with the nuke, finish in melee).
| Band | Area | Monsters | Access | Fit |
|---|---|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | rat/lashworm/kobold thief/slime ≤25 hp, hit ≤10 | arena max L3 | mmis one- or two-shots everything |
| 3-6 | Graveyard 1:817 | skeleton 43/55/11, zombie 62/70/14, bat 35/40/5 | none | mmis + melee |
| 5-10 | Crypt 1:835 / 1:1070 / 1:1276 | ghoul 73/100/9, shade 75/140/15, wight 80/140/10 | −175 (NV25 OK) | Frost jet/lbol. Low hits. |
| 6-10 | Strange Mansion 1:63 | vision 40/120/14, astral shade 80/185/12 | 31 steps | exp/hp 2.45. Shadow spectre hits 20: careful before L8. |
| 6-10 | Slimy Sewers 9:1 | slimes 30-55/60-125/12-14 | search w 1:619, −200 lantern | Spells ignore their ac25-30 |
| **10** | **Crypt quest** (§7) | two-headed hydra 93/225/20 | bone key | silver rapier + L11 |
| 10-15 | Kobold caves 6:752 | kobold 30/80/9, warrior 50/150/13 | 134 steps | Safest band. Lbol one-shots kobolds. |
| 10-15 | Darkwood bandits 9:720 / Smuggler Den 9:161 | bandit 55/125/12, leader 100/275/18; thief 55/125/12, smuggler 70/150/12 | none / hidden d at Docks | Cash for scrolls. Smugglers are Neutral (you start the fight). |
| 15-20 | Frozen Caverns 10:35 | grey wolf 125/375/21, guardian wolf 145/500/22 | search n 10:3 | 2-3 sunbolts per wolf. Hits 21-22: OK at 130+ HP. |
| 20-30 | **Ancestral Tomb 6:872** | tortured spirit 150/525/15, barrow wight 180/725/15 (ac50-60) | hidden w, −200 | **Best**: hit 15, and spells ignore their AC |
| 20-30 | Dragon's Teeth Hills 2:177 | harpy 140/450/12, gorgon/ankheg 30, dark monk 125/525/30 | 49 steps | Fireball/mana storm on packs. Black serpent key. |
| 25+ | Lava Tube 6:1210 | salamander 225/1500/25 (Magical) | black star key + hidden exits | Spells bypass Magical. Fire damage: bring resist fire. |
| 30+ | Quaggoths 6:1811 | quaggoth 180/725/16, thonot/jald 200/1500/20-30 | mine pass 100g | |
| 30+ | Redstone Tunnels 6:1407 | troglodyte 120/400/15 … white scorpion 200/1100/34 | −200 | **Avoid the purple worm** 750/8000/120 |
Avoid before ~L25: Huge Cave orc packs (hit 40-50), Ancient Crypt skeletal warriors (60), Labyrinth minotaurs (70), Orc Mansion guards (70).

## 7. Class quest and class items
**L10 crypt quest (Gypsy path)** [C rooms/exits/monsters; F procedure notes/quests.md §1]:
1. Crypt levels 1-3 → mummy 100/200/20 at 1:1309 → **bone key** → `use bone key n` → Shadowed Hall 1:1377.
2. `n` ×4 to 1:1421, then `e` (class exit, **Gypsy only**) → 1:1436 `e` 1:2279 `e` 1:2280 `ne` 1:2293 `ne` 1:2292 `e` 1:2291 `se` 1:2290 `ne` 1:2289 `se` 1:2288 `se` 1:2287 `e` → **1:2294 Marble Chamber**.
3. Kill the **two-headed hydra 93/225/20** (ac20, Chaotic Evil). It drops the **brass key** 100%. `use brass key e` → 1:2295 `n` (class exit) → 1:2296 `n` → **1:2300 Large Tomb**: gypsy spirit (do not attack), the **silver rapier** (limit 1), and the Super Gypsy Trainer (shop 60, markup 500, L11 only, ~30g). `go portal` to leave.
4. Hydra: ~5 lightning bolts at L10. Fight from full mana.
Shared quests: alignment L10/L20, Ice Sorceress (+1 AC), Phoenix Feather, Sunstone Wristband (`corpus/bosses.md`).

## 8. Group role (brief)
Single-target and AoE nuker plus the party's rogue (picklocks, traps, scouting by stealth). Speed and ethereal shield can go on a fighter. Slow, confusion and vulnerability help on bosses. No heals. A separate agent covers grouping.

Group role: full mechanics, compositions and spots in `kb/grouping.md`.
