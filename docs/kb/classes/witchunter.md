# Witchunter (WCCCLASS 2): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats `hp/exp/maxhit`. Prices include markup, not Charm. `1p` = 100 gold.
Steps from Town Square 1:224. **The detailed Witchunter material already exists** and is not repeated:
`test/play/notes/combat.md` (formulas, Kang build, bash model), `test/play/corpus/gear.md`
(non-magical gear by slot), `corpus/leveling.md` (areas + "Recommended Kang Witchunter path"),
`corpus/bosses.md`, `notes/quests.md` §1-2. This file adds race choice and the class summary.

**One-line path:** Half-Ogre (Kang if already rolled). BASH with the heaviest non-magical 2H weapon
in scalemail; no healing of any kind, so pick low-maxhit, rest-friendly areas. Arena → Graveyard →
Slimy Sewers → L10 crypt (mithril cutlass) → kobolds → Frozen Caverns wolves → Ancestral Tomb;
Magebane quest at L15 (Rhudaur).

## 1. Class facts [C: WCCCLASS rec 2]
| Field | Value | Meaning |
|---|---|---|
| exp | +30% | L2 = 10×(130+race) (Kang 1,650 — observed, kb/progression.md) |
| basehp / hplevel | **6** / 4 | Warrior HP |
| combat | **7** | Best melee AV in the game: +1 combat = +2×(2+√L) AV over Warrior/Paladin/Ranger (+10 at L10, +12 at L20) [C LJNCOMBT.C:700] |
| magery | none | |
| weapons / armour | 8 = All / **8 = Scalemail** | No platemail |
| abilities | 31 Bash, **51 Anti-Magic**, **142 Hit Magical 10** | |
| Anti-Magic | damaging spells cut by **MR/2 %** (cap 75%) and resisted at MR/2 %; **friendly heals are resisted the same way**; **no item with ability 28 (Magical)** can be used — this includes every shop healing potion | LJNSPELL.C:1130, 3095; LJNUTIL.C:9528 |
| Hit Magical 10 | plain weapons hurt monsters up to magic level 10 | LJNCOMBT.C:2558 |
| Smash | keepable from **L20** | LJNUTIL.C:692 |
| Alignment-quest reward | +1 max damage | LJNUTIL.C:479-487 |
| L11 trainer | Super Witchunter Trainer, shop 50 at **1:2236** | |
MR = `(Int + 3·Wil)/4 + race MR` (LJNUTIL.C:4956) → **Wil is a real defensive stat for this class only**.

## 2. Race choice [C WCCRACE / I ranking]
| Rank | Race | exp (total) | Str | Wil | Hea | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Half-Ogre** | +30 (160%) | **70-150** | 25-70 | **60-150** | +1 HP/lvl, DR +20 (−2/hit), **MR +15** | Str 100 at creation (scalemail under 33% at 4,800 cap), 7 HP/level for a class that can't be healed, and MR 15 feeds Anti-Magic directly (+7.5% spell cut). |
| 2 | Dwarf | +30 (160%) | 50-110 | **50-120** | 50-120 | MR +10, NV 75, Encumbrance +20% | Highest Anti-Magic ceiling (Wil 120: MR ≈ 100 → 50% cut/resist). No lantern down to −225 rooms (Slimy Sewers, Smelly Cave, Rhudaur, Ancestral Tomb). |
| 3 | Kang (current bot char) | +35 (165%) | 55-120 | 45-110 | 50-110 | **Poison immune**, DR +10, **Defence +5 (DV)** | Poison spots need no cure (a WH can't be cured by spell). Weaker start than Half-Ogre. |
| speed | Half-Orc | 0 (130%) | 45-110 | 30-95 | 50-120 | none | 19% less exp than the three above. |

## 3. Stats / CP plan [C costs LJNGAME.C:1645 / I priorities, model in kb/stat-builds.md]

**Use `statplan.allocate_cp(..., ladder="basher_am")`.** That is the 2H basher ladder plus Wil 55
after Hea 90. `python3 test/play/statplan.py` prints Kasimir's plan.

**Str first only through the cheap CP bands, never to max.**
- Kang Str 90→120 costs 165 CP, which is all the CP from L11-20.
- Str-max (Kang) kills **−23% / +3% / +1% / +7% / +2%** per hour at L5/10/15/20/30 in low-hit
  areas, and **−55% / −41% / −47% / −6% / +2%** in hard-hitting ones (mixed −19%). Its HP is
  68/110/152 at L5/10/15, against 84/148/210 on `basher`.
- `basher` is +2% mixed vs the old ladder and ties the greedy optimum.
- Str doesn't gate weapons: the Str requirement only adds +1.5% energy per missing point, and
  that is free while a bash is capped at 1000 (to about L15).
- Weight does need Str: stay under 33% load.

Kasimir (current S79 I30 W45 A42 H74 C30):
| CP (≈ level) | Str | Int | Wil | Agi | Hea | Cha |
|---|---|---|---|---|---|---|
| 110 (L2, now) | 82 | 30 | 45 | 42 | 74 | **31** (bank 2) |
| 150 (L6) | **90** | 30 | 45 | 48 | 74 | 32 |
| 200 (L11) | 90 | 30 | 45 | 50 | 86 | 36 |
| 250 (L14) | 93 | 30 | **55** | 53 | 90 | 36 |
| 300 (L18) | 95 | 30 | 55 | 56 | 96 | 41 |
| 400 (L23) | 102 | 30 | 55 | 68 | 100 | 41 |

Other races (fresh, 100 CP on `basher`):
- Half-Ogre: Str 101, Int 20, Wil 25, Agi 33, Hea 72, Cha 31.
- Dwarf: Str 81, Wil 50, Agi 30, Hea 71, Cha 31.
- Half-Ogre can take Str to about 110 early and leave Agi at 20-33 until Hea 90.

Witchunter-specific:
- **Wil 55-65 earlier when hunting casters.** Anti-Magic cuts spell damage by MR/2 % and the
  model leaves spells out, so it understates Wil.
- Int does nothing: bashes never crit.
- **Hea matters more than for other melee.** A Witchunter can't be healed (it resists heals), so
  keep max HP ≥ 4 × the biggest hit where you hunt.

Race, exp-adjusted kills/h against Kang (kb/stat-builds.md §5): **Half-Orc +11%, Half-Ogre +9%**,
Dwarf −9% (its +20% encumbrance isn't modelled). Half-Ogre also brings MR 15.

## 4. Attack style [C combat.md §2]
- **BASH** every round with the heaviest-damage 2H weapon; Smash at L20 if obtainable. ATTACK vs AC ≳ 0.5-0.6×AV.
- **Worked at L20** [C formulas from `move_player_to_fighter`/`calculate_attack`, LJNCOMBT.C:338-943,
  1226-1427; I: the basher-ladder stats Str 97 Agi 62 Int 30, the damage roll taken as min..max,
  rank and spells left out]. In the scalemail kit (about 48% load, AV 109-115, crit 1) BASH beats
  ATTACK on every L15-20 target tried, AC 20 to AC 60 (barrow wight) and DR 0 to 12, by 25-45% a
  round: the ×1.1 then ×3 outweighs half the blows and −15 AV, because the crit chance is 1%.
  Damage a round averaged over kobold warrior, bandit leader, guardian wolf, skeletal warrior,
  stone golem, slaver, barrow wight, guardian golem and minotaur champion (attack / bash):
  Magebane sword 48/68, etched adamant greatsword 46/67, katana 43/62, bone-handled headcutter
  42/61, mithril cutlass 41/59, three-headed flail 41/57.
- **ATTACK wins only with a fast blow.** Under 200 energy a blow gets up to +20 crit (halved
  from 33% load). In the leather kit (about 22% load) the katana costs 177 and gets crit 21:
  attack 88 against bash 76 a round. Magebane in leather costs 209, just over the line, and
  bashes (85 against 58).
- **SMASH** (one blow a round, −25 AV, ×1.2 then ×5, and the monster down with −35 AV and DV
  while it is smashed every round) averages, on the same targets once the monster is down:
  etched adamant greatsword 119, headcutter 98, three-headed flail 83, Magebane 80, cutlass 66.
  The slow heavy weapons gain most. **How to get it** [C text blocks 2940-2942, 9034]: hand
  **50 orc-heads** (item 1101: 3% from orc captains and orc lieutenants, the 180 HP kind; 1%
  from black orcs and other orcs) to Aldreth (1:2305, Darkwood Tree, Small House) for a quest
  token, then at level 20 or more Aldreth gives a Witchunter Smash (Warrior and Ranger at 22,
  Paladin 25, Cleric and Warlock 27).
- Rest to ~80% between pulls (regen `max(1,(L+20)·Hea/750)`/tick). No potions, no heals: area choice is the defence.

## 5. Gear by level band → `corpus/gear.md` [C]
Model pick (kb/stat-builds.md): greataxe/halberd/maul at L5-10 (Str req irrelevant under the bash cap; weight decides), three-headed flail L15-30, etched adamant greatsword bashes hardest at L30. Summary: greataxe 6-22 10g (1:307) / maul 8-25 20g (1:354) → **mithril cutlass** 8-18 (L10 quest, 1H, MR+5, WH-only; pair with curved heater ac4 24g) or keep bashing a maul → three-headed flail 9-23 4p (L15) / **Magebane** 9-22 (L15 quest) → etched adamant greatsword 10-34 (L20). Armour: chain → **scalemail tunic ac20 dr6 50g**, scalemail leggings 16g, pot-helm 16g, scale gauntlets 8g, chainmail boots 10g (1:305/306) → fine/dwarven chainmail (6:1337), adamantite scalemail L30. Every item with Magical (28) is unusable: check before buying.
Armour progression in full [C item, shop and text-block data; prices at Charm 75]: the Silvermere
scalemail set (about 100 gold) is the best value to L30. Optional at L10: the **black chitin**
set from a crafter (text blocks 405-414, 4336; where it is run was not traced): armour AC 22/7,
leggings 9/1, helm 7.5/1.5, gauntlets 6/1, shield 5.5/1, for 8,200 gold, 17 pieces of black
chitin (10% from mermex) and 16 spider silks (4% from bola spiders) in all. L30, Dwarven
Armoury 6:1337: adamantite scalemail tunic 24/7 (22,800 g), leggings 9/1.5 (7,600), scale
gauntlets 7.5/0.8 (950), enforced boots 7/1 (6,650). L40: crimson helm 10/1 (2,850 at Skali's
1:305), crimson scalemail leggings 10/1.5 (10,688) and tunic 28/9.5 (32,063, Jorah 1:3302);
L45: stormmetal scalemail tunic 30/10, cheaper (14,250, Lost City 16:317). Quest pieces: medal
of flame (worn, AC 1, MR 10, Witchunter quest), inquisitor's cloak (L40, AC 5/1, MR 15,
Witchunter quest, Good only).

## 6. Grinding → `corpus/leveling.md` "Recommended Kang Witchunter path" [C/F]
| Levels | Area | Anchor |
|---|---|---|
| 1-3 | Newhaven Arena | 1:2150 |
| 3-6 | Graveyard (gravediggers 130/1500/30 if neutral targets allowed) | 1:817 |
| 6-10 | Slimy Sewer Tunnels (`search w` at 1:619, lantern) / Crypt 1-3 / Strange Mansion | 9:1 / 1:835 / 1:63 |
| 10 | Crypt quest | §7 |
| 11-15 | Kobold caves (+ king 6:834) / Darkwood bandits | 6:752 / 9:720 |
| 15-20 | Frozen Caverns wolves (`search n` 10:3), Gnoll Encampment from ~L16 | 10:35 / 2:1107 |
| 20-30 | Ancestral Tomb (hit 15), Dragon's Teeth Hills (black serpent key 20% from dark monks) → Shadow Fist/Rhudaur | 6:872 / 2:177 |
| 30+ | Salamanders (magic level 1: Hit Magical covers it), Quaggoths, Redstone | 6:1210 / 6:1811 / 6:1407 |
Witchunter-specific: casters hurt you less than anyone else (Anti-Magic), so dark priest/cleric pools (Dark Caverns pool's edge 1:1531) are relatively better for you than for other melee [I].

## 7. Class quests [C rooms; F quests.md]
- **L10 crypt → mithril cutlass + L11:** 1:1309 (bone key) → 1:1377 → `n` → 1:1419 → **`w`** (Class 2) → `w` ×7 → Marble Room 1:2219 → a button room (`w`,`nw` fire lizard 93/225/16 is the softest; `s`,`sw` troll 93/225/35 is the captured route) → Marble Chamber 1:2226 → **`w`** (Class 2) → 1:2228 → `w` → Large Tomb **1:2236** (cutlass placed, limit 1; train; `go portal`). Full walkthrough: notes/quests.md §1.
- **L15 Magebane:** `greet`/`ask huge dwarven witchunter witchunter`, then `… sword` at Rhudaur Exotic Weapons 2:2566 (black serpent key) → Magebane sword 9-22 WH-only [F t=40; NPC/item C].
- L50: Obsidian Trident / Demonslayer / Judicator (good 5th quest), Draka's Blade (Magebane follow-up) [F t=6, t=41; items C]. Inquisitor's cloak (L40) and crimson ring (L35) have no traced source.

## Group role
Top melee damage and the anti-caster. No heals, so it needs a Cleric or Priest. **Anti-Magic also resists friendly heals and buffs at MR/2 %** (LJNSPELL.C:1130, 3095) [C], so healer-based groups get little out of it. It's best solo (John: "ironically not a great grouper"). See `kb/grouping.md`.
