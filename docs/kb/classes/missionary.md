# Missionary (WCCCLASS 6): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data,
**[I]** inferred/modelled, **[F]** forum/web. Monster stats are `hp/exp/maxhit`. Prices include
shop markup, not Charm. Steps are from Town Square 1:224. Shared formulas:
`test/play/notes/combat.md`, `bard.md` §1; areas: `test/play/corpus/leveling.md`.

**One-line path:** Dwarf, Wil-first, good alignment. Harm on the living and **turn undead on the
dead** from L3, so live in undead areas: Graveyard, Crypt, Mansion. At L10 do the crypt quest
(jeweled scimitar, no fight). Agony/holy force on kobolds and wolves. Disrupt from L16 makes the
Ancient Crypt and **Ancestral Tomb** the core grind to L30. Self-heals mean the least downtime of
the three hybrids.

## 1. Class facts [C: WCCCLASS rec 6]
| Field | Value | Meaning |
|---|---|---|
| exp | **+120%** | L2 = 10×(100+race+120). Dwarf: 2,500. Kyau lists 220%, which matches [F]. |
| basehp / hplevel | 4 / 3 | |
| combat | **4** | Weak melee. Spells carry. |
| magery | **Priest (2), level 2** | All priest spells with magelvl ≤ 2. Same list as Cleric (4/2); Priest class (5/3) gets magelvl-3 extras. |
| weapons | 4 = Any 1H | No 2H, no off-hand items unless the item names class 6 (copper cross, red skull and golden chalice do) |
| armour | 6 = Studded Rigid Leather | |
| abilities | 103 Class Stealth, 37 Picklocks, 40 Find Traps, 31 Bash | **No Thievery** (unlike Bard/Gypsy) |

Casting [C LJNUTIL.C:4924, 4952; LJNBKGND.C:1680]: mana = 2×(2×level+3) (+ items). Regen and
MEDITATE = **Wil** × (level+20) × 4/1650. SC = 2×level + (3×Wil + Int)/6 + 10. Cast chance =
SC + diff (cap 98).
Spell targeting rules [C LJNSPELL.C:7140-7160]:
- Spells with **Affects Living (108)**: harm, agony, minor/major/greater healing, healing rain, blind, black wind. They **do nothing to NonLiving monsters** (every undead, slime, golem and elemental checked in the data).
- **Kill Dead (23)**: turn undead, disrupt. They work **only on undead** (`kmon->undead`).
- Holy force needs a Not-Evil caster, protection from evil needs a Good one, and unholy force needs Not-Good (spell abilities 111/97/110, LJNUTIL.C:9790).

## 2. Race choice [C WCCRACE; I ranking]
| Rank | Race | exp | Wil | Hea | Str | Agi/Cha/Int | Extras | Why |
|---|---|---|---|---|---|---|---|---|
| **1** | **Dwarf** | +30 (250%) | **50-120** | **50-120** | 50-110 | 30-90 / 30-85 / 30-90 | MR +10, **NV 75**, encumbrance +20% | Wil starts at 50, so Wil 50→80 costs 60 CP (Human: 100). Best Hea, no lantern needed anywhere (−200+75), the highest MR (resists the casters in the Crypt/Shadow Fist). |
| 2 | Half-Orc | **0 (220%)** | 30-95 | 50-120 | 45-110 | 40-100 / 30-80 / 30-90 | none | 12% less exp than Dwarf and the same HP. Wil 30 base makes the caster stat expensive, and NV 0 means lanterns. |
| 3 | Human | 0 (220%) | 40-100 | 40-100 | 40-100 | 40-100 | none | Balanced. Better Agi/Cha (stealth, dodge) than Dwarf. |
Kang (Wil 45-110, Hea 50-110, poison immune, DR +1, exp +35) is a reasonable 4th. Avoid Half-Ogre (Wil max 70).
Dwarf caveats: Agi 30 and Cha 30 start at −6 dodge and −3 AV (−3 and −4 respectively, combat.md §1), so the plan buys them back cheaply.

## 3. Stat / CP plan (Dwarf) [C costs; I ladder]
Ladder: Wil 70 → Hea 65 → Str 60 → Agi 41 → Cha 31 → Wil 80 → Agi 44 → Hea 75 → Agi 47 → Cha 36 → Wil 90 → Agi 50 → Hea 85 → Str 70 → Cha 41 → Wil 100 → Int 40 → Hea 95 → Cha 46 → Wil 110 …

| L | CP | Str | Int | Wil | Agi | Hea | Cha | bank | HP~ | mana | regen | SC | MR | stealth | dodge* |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 100 | 60 | 30 | 79 | 41 | 65 | 31 | 0 | 38 | 10 | 4 | 56 | 66 (+10) | 40 | −6 |
| 6 | 150 | 60 | 30 | 81 | 47 | 75 | 36 | 1 | 82 | 30 | 5 | 67 | 68 | 52 | −2 |
| 10 | 190 | 60 | 30 | 90 | 49 | 75 | 36 | 1 | 112 | 46 | 6 | 80 | 75 | 61 | 0 |
| 15 | 265 | 70 | 30 | 92 | 50 | 85 | 41 | 3 | 164 | 66 | 7 | 91 | 76 | 71 | 2 |
| 20 | 340 | 70 | 40 | 100 | 50 | 91 | 41 | 3 | 216 | 86 | 9 | 106 | 85 | 78 | 3 |
| 30 | 540 | 70 | 40 | 115 | 56 | 105 | 46 | 5 | 335 | 126 | 13 | 134 | 96 | 91 | 8 |
*Unencumbered adds ~+8 dodge. MR shown without the Dwarf +10.

- **Wil**: +6 Wil = +3 SC, plus mana regen and +0.75 MR per point. Primary to ~100.
- **Hea**: the missionary should out-HP the Bard and Gypsy. Hea 75 at L10 gives ~112 HP.
- **Str 60**: 1H weapons at speed, capacity 2,880 ×1.2 (Dwarf) = 3,456, so 33% = 1,140. A full leather kit fits.
- Agi/Cha: only up to the dodge-penalty breakpoints (Agi 50, Cha 46). Int: ignore until late.

## 4. Attack style, healing and support
Damage spells [C data, computed; avg per mana in brackets]:
| Spell (cmd) | Lvl | Mana | diff | Works on | L5 | L10 | L15 | L20 | L30 | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| **harm** (`harm`) | 1 | **1** | +15 | living only; MR-resisted (MR/2 %) | 4-18 (11/m) | 4-19 | 4-19 | 4-19 | | Best mana/damage early on living targets |
| **turn undead** (`turn`) | 3 | 4 | 0 | **undead only**; never resisted | 17-30 | 22-45 | 27-60 | 32-75 | 32-75 | **The class-defining spell to L20** (8-13/mana) |
| spiritual hammer (`hamm`) | 4 | 3 | +5 | anything (slimes, golems) | 4-19 | 4-24 | 4-25 | | | Fallback for NonLiving non-undead |
| agony (`agon`) | 8 | 5 | 0 | living; resisted | – | 18-41 | 18-56 | 18-56 | | 7/mana |
| holy force (`hfor`) | 13 | 8 | −10 | anything; caster not Evil | – | – | 18-54 | 18-64 | 18-68 | Never resisted |
| **disrupt** (`disr`) | 16 | 8 | −25 | **undead only** | – | – | – | 50-92 | 60-112 | 9-11/mana. One- or two-shots Ancestral Tomb undead. |
| godstrike (`gstk`) | 20 | 12 | −40 | anything | | | | 30-80 | 30-104 | |
| god's wrath (`gwra`) | 25 | 14 | −50 | anything | | | | | 57-150 | |
| divine fury (`fury`) | 30 | 16 | −60 | anything | | | | | 50-200 | |
AoE: **blight** (L6, AoE −6 AV curse, Unholy shop), **black wind** (L21, AoE blind, living). There is no AoE damage spell at magelvl ≤ 2.

Healing [C]:
| Spell | Lvl | Mana | diff | Amount (L10 / L20 / L30) | Target |
|---|---|---|---|---|---|
| minor healing (`mihe`) | 1 | 2 | +10 | 5-14 (cap L10) | self/one |
| major healing (`mahe`) | 8 | 6 | −5 | 9-20 / 12-30 / 16-40 | self/one |
| healing rain (`rain`) | 10 | 5 | −15 | 7-15 / 10-20 / 12-22 each | **whole party** |
| greater healing (`grhe`) | 15 | 12 | −25 | – / 25-50 / 35-70 | self/one |
| major healing rain (`mrai`) | 17 | 15 | −40 | – / 14-40 / 18-50 each | party |
| annointed hands (`anno`) | 10 | 1 | −5 | 2-10 → 2-15 | one other (750g scroll, Nekojin store 12:49, 257 steps) |
Cures: cure poison (L7), cure blindness (L12), cure disease (L13), cure paralysis (L23).
Buffs: bless (L2, +3 AV), **chant** (L6, party +3 AV), protection from evil (L5, good only, defence vs evil +10), greater bless (L10, +6 AV), divine favour (L19, +10 AV), zeal (L25, self dmg/MR/crit/AV). Debuffs: curse/greater curse (−6/−10 AV), blind (L12), hold person (L14), divine disfavour (L19).

Pattern [I]:
- **Living targets**: harm (L1-7), then agony (L8-12), then holy force (L13+), then godstrike/god's wrath.
- **Undead targets**: turn undead (L3-15), then disrupt (L16+). Undead areas are where the missionary out-earns everyone.
- **Slimes/golems/elementals (NonLiving, not undead)**: only hammer, holy force or melee work. Skip the Slimy Sewers.
- Heal mid-fight with major/greater healing (no energy cost: energy 0). Cast before resting.
- At L10 mana is 46: ~11 turn undeads or ~9 agonies per bar. MEDITATE between pulls.

### Scroll sources [C shop data]
| Shop | Room / steps | Markup | Scrolls (price) |
|---|---|---|---|
| Newhaven Spell Shop | 1:2144 | 0 | harm, minor healing (free), bless 4g |
| **Holy Spell Shop** (Temple Spell Store) | **1:522**, 8 steps | 100 | harm/minor healing free, bless 8g, **turn undead 16g**, spiritual hammer 40g, prot. from evil 30g, **chant 40g**, cure poison 60g, **major healing 100g**, **agony 150g**, **healing rain 300g**, greater bless 300g, glitterdust 360g, cure blindness 70g, cure disease 500g, cure paralysis 18p |
| Jorah's Priest Spells (Blue Tower) | 1:3308, 37 steps (5g toll at 1:1381) | 100 | greater curse 120g, blind 200g, hold person 280g, **holy force 300g**, **greater healing 7p**, **disrupt 7p**, major healing rain 8.5p, godstrike 10p, divine favour/disfavour 12p, black wind 17p, **god's wrath 25p** |
| Unholy Spells (Slums) | 1:2307 | 150 | curse 10g, turn undead 20g, blight 75g, prot. from good 37.5g, control undead 12.5p |
| Very Powerful Spells | 10:273 (level-25 exit at 10:272) | 100 | zeal 25p, **divine fury 50p** |
| Aldreth's Spells | 1:2306 | – | No inbound exit in the data [C]. Ignore it. |

## 5. Gear by level band [C]
Weapons (1H, Str ~60):
| Band | Weapon | Dmg | Str | Price | Where | Notes |
|---|---|---|---|---|---|---|
| 1 | club | 3-8 | 40 | free | Newhaven 1:2141 | |
| 2-10 | **mace** | 3-12 | 45 | 8g | 1:354 | |
| 5-10 | warhammer / morning-star | 4-14 / 3-15 | 60 | 13g / 12g | 1:354 | |
| 8-15 | serrated battle axe | 5-16 | 60 | 4p | 1:307 | |
| **10** | **jeweled scimitar** (quest) | **5-18**, speed 1200, acc +10, ac2, BS AV +15, SC +2, mana regen +10% | 40 | quest | placed in 1:2262 Large Tomb | Class 6 only, Magical 2, L10, **limit 1** |
| 20+ | runic warhammer | 10-18 | 60 | 40p | 6:1344 (L20) | |
| 30+ | starsteel mace / etched adamant warhammer | 11-26 / 10-25 | 60 / 70 | 188p / 169p | 1:2329 (L30) | |
| 40 | azure shamshir | 8-25 | 60 | not sold | class 6, L40 | |
| 50 | Darkbane 16-45 (good, classes 1/3/6) 1600p at Arlysia 17:2438; rod of might 17-42 (classes 3-6); diamond scimitar, Solarblade, moonstone rod (classes 6/8/9/10/11) | | | | drops/quests | |

**Off-hand caster items (class 6 is named, so the off-hand ban doesn't apply):**
| Item | Effect | Min L | Price | Where |
|---|---|---|---|---|
| **copper cross** | ac.5, **SC +5, mana +5**; good-aligned | – | **20g** | Holy Item Shop 1:525 (9 steps). **Buy at L1.** |
| red skull | ac.5 dr.5, SC +10, mana regen +10%; evil-aligned | – | 2.2p | Dreary Shop 7:1136 |
| golden chalice | +2 dmg, def vs evil +5, mana +10, mana regen +10%; good | 10 | not sold | drop/quest |
| large silvery cross | acc +10, SC +10, mana +20, +3 dmg | 25 | 180p (Recycler 4 1:39) / 1530p | |
Neck: silver holy amulet SC +1 30g (1:290, Magical); holy medallion ac3 SC +3 mana +10 (Recycler 1:168, only if a player sold one). Back (L40): faith keeper (class 6, BS +10/+20, mana regen +10%, Recycler 6 1:516).
Armour (type ≤6, same kit as Bard): rigid leather tunic 16g → ogre-skin shirt L10 14p (6:1343) → wyvernhide tunic L20 105p (1:2328). Leather pants 4g, hard leather helm 4g, gauntlets 2g, boots 4g, travelers cloak 4g, golden armbands 2.4p, adamantite bracers L15 70p (6:1337), ogre-skin baldric L20 50p, golden amulet 1p, amethyst ring 20g. Dwarf NV 75: no light source needed.

## 6. Grinding progression [C data; F t51; I fit]
| Band | Area | Monsters hp/exp/maxhit | Access | Fit |
|---|---|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | rat/kobold thief/carrion ≤25 hp, hit ≤10 | arena max L3 | Harm (1 mana) + melee. Acid slime is NonLiving: melee it. |
| **3-6** | **Graveyard 1:817** | **skeleton 43/55/11, zombie 62/70/14 (undead)**, giant bat 35/40/5 | none, 23 steps | Turn undead 17-30 two-shots skeletons at 4 mana. Harm the bats. Gravediggers 130/1500/30 (Neutral, living): harm them at L6+ (bosses.md). |
| **5-10** | **Crypt 1:835 / 1:1070 / 1:1276** | ghoul 73/100/9, shade 75/140/15, wight 80/140/10, zombie, skeleton (all undead) | NV 75 covers −175; hidden exits 1:1056 n, 1:1070 s, 1:1250 s | **Ideal**: all undead, low hits, leads into the L10 quest |
| 6-10 | **Strange Mansion 1:63** | vision 40/120/14, shadow spectre 70/160/20, astral shade 80/185/12 (all undead) | none, 31 steps | exp/hp 2.45, all turnable. Spectral mage 200/1200/20 at 1:382 (living: agony at L8+). |
| **10** | **Crypt quest** (§7) | none to fight | bone key | jeweled scimitar + L11 |
| 10-15 | Kobold caves 6:752 | kobold 30/80/9, warrior 50/150/13 (living) | 134 steps | Agony one-shots kobolds |
| 10-15 | Darkwood bandits 9:720 | bandit 55/125/12, leader 100/275/18 | none | Holy force from L13 |
| 15-20 | Frozen Caverns 10:35 | grey wolf 125/375/21, guardian wolf 145/500/22 | search n 10:3 | Agony/holy force + heals |
| 16-25 | **Ancient Crypt 1:1943** | skeletal warrior 130/300/60, moaning spirit 128/375/20 (undead) | 41 steps, −175 | Disrupt 50-92 at L20. The warrior hits 60, so engage at ≥200 HP with greater healing. Banshee 250/4000/36 at 1:1952 (24h). |
| **20-30** | **Ancestral Tomb 6:872** | tortured spirit 150/525/15, barrow wight 180/725/15 (undead, ac50-60) | hidden w; −200 (NV 75 OK) | **Best in the game for this class**: hit 15, 2 disrupts per kill, exp/hp 3.8 |
| 20-30 | Dragon's Teeth Hills 2:177 | harpy 140/450/12, gorgon 165/450/30, dark monk 125/525/30 … | 49 steps | Living mix: godstrike. Black serpent key. |
| 30+ | Quaggoths 6:1811 | quaggoth 180/725/16, thonot/jald 200/1500/20-30 (Neutral, living) | mine pass 100g | God's wrath / divine fury |
| 30+ | Black Cave 8:1 | cave lizard … mind flayer 185/1025/25 | DTH hidden exits | |
Skip: Slimy Sewers (NonLiving and not undead: only hammer works), Elder Grove golems/elementals/trees, Redstone purple worm.

## 7. Class quest and class items
**L10 crypt quest (Missionary path)** [C rooms/exits/monsters; F procedure notes/quests.md §1]:
1. Crypt levels 1-3 → mummy at 1:1309 (undead: turn undead it) → **bone key** → `use bone key n` → 1:1377.
2. `n` ×5 to 1:1422, then `e` (class exit, **Missionary only**) → 1:1435 `n` 1:1432 `nw` 1:2242 `n` 1:2243 `ne` 1:2244 `n` 1:2245 `n` 1:2246 `n` 1:2247 `w` 1:2251 `nw` → **1:2252 Marble Chamber** with a **young girl** (12 hp, 0 exp, **Lawful Good: do not attack**, +10 evil points).
3. Type **`turn right candle left`** (Text exit north; it also accepts "twist right candle left" and "turn right candlestick left", WCCMSG 8586) → 1:2254 → `ne` (class exit) → 1:2261 `ne` → **1:2262 Large Tomb**: missionary spirit (don't attack), the **jeweled scimitar** (limit 1), the Super Missionary Trainer (shop 56, markup 500, L11 only, ~30g). `go portal` to leave.
4. No guardian and no key past the bone key. This is the easiest class path of the three.
Shared quests (`corpus/bosses.md`): **good** alignment line recommended. Holy force, protection from evil, the copper cross and the golden chalice all need non-evil or good alignment. Ice Sorceress (+1 AC), Phoenix Feather, Sunstone Wristband.

## 8. Group role (brief)
Party healer (healing rain at L10, major healing rain at L17, greater healing), party AV buffer (chant, greater bless, divine favour), cleanser (poison, blindness, disease, paralysis) and the party's undead killer (turn undead, disrupt). Also has picklocks/traps for doors. Weak melee: keep it in the back rank (+15 defence, combat.md). A separate agent covers grouping.

Group role: full mechanics, compositions and spots in `kb/grouping.md`.
