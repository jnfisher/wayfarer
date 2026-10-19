# Paladin (WCCCLASS 3): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats `hp/exp/maxhit`. Prices include markup, not Charm. `1p` = 100 gold.
Steps from Town Square 1:224. Shared: `test/play/notes/combat.md`, `test/play/corpus/leveling.md`,
and `warrior.md` §1 (platemail DR/DV mechanics apply unchanged).

**One-line path:** Dwarf. Warrior-style BASH in platemail, plus **minor healing** (free at L1) so you
rest far less, **protection from evil** (L5, +10 defence vs evil) and **turn undead** for the crypt.
Arena → Graveyard/Crypt (undead) → Slimy Sewers. L10 crypt quest (shimmering greatsword). Kobolds,
wolves, Ancient Crypt undead, then Orc Mansion/Ancestral Tomb in plate.

## 1. Class facts [C: WCCCLASS rec 3]
| Field | Value | Meaning |
|---|---|---|
| exp | **+120%** | L2 = 10×(220+race) exp (Dwarf 2,500). Paid for spells + plate. |
| basehp / hplevel | 5 / 4 | +5 HP/level + ~2.5 random |
| combat | 6 | Same melee AV as a Warrior |
| magery | **Priest (2), level 1** | Can learn priest spells with magelvl ≤ 1: in this data that is every priest spell up to *zeal* (all magelvl 0) (LJNUTIL.C:9774-9784). Clerics/Priests add the magelvl-2/3 ones. |
| mana | `2×(L+3)` (+Mana abilities) | L1 8, L5 16, L10 26, L20 46 (LJNUTIL.C ~4915) |
| spellcasting | `2L + (3·Wil+Int)/6 + 5` | Cast chance = SC + spell diff, cap 98 (LJNSPELL.C:82). Wil drives it. |
| weapons / armour | 8 = All / **9 = Platemail** | Same as Warrior |
| abilities | 31 Bash | |
| Smash | keepable from **L25** | LJNUTIL.C:692 (source of the ability not found) |
| Meditate | keepable from L27 | LJNUTIL.C:715 |
| Alignment-quest reward | +1 max damage | LJNUTIL.C:479-487 |
| L11 trainer | Super Paladin Trainer, shop 55 at **1:2260** | crypt quest (§7) |
- No code forces a Paladin to be Good [C: no class check in LJNEVIL.C/LJNUTIL.C]. But *protection from evil*, *holy force*, the copper cross and most Paladin endgame items are **Good-aligned** only, so stay good [C item/spell abilities 97].

### Key spells (scroll = item type 9 with Link to Spell; read it to learn) [C WCCSPELS/WCCITEMS/WCCSHOPS]
| Spell (cmd) | Lvl | Mana | Diff | Effect | Scroll price / shop |
|---|---|---|---|---|---|
| minor healing (`mihe`) | 1 | 2 | +10 | heal 2-8 (+2 per 3 lvls) | **free** Newhaven 1:2144 / Holy Spell Shop 1:522 |
| bless (`bles`) | 2 | 4 | +5 | +3 AV, 40 rounds | 4g 1:2144, 8g 1:522 |
| turn undead (`turn`) | 3 | 4 | 0 | 12-15 (+3/lvl) ignoring armour, **undead only** | 16g 1:522 |
| spiritual hammer (`hamm`) | 4 | 3 | +5 | 4-14 damage | 40g 1:522 |
| **protection from evil** (`prev`) | 5 | 6 | 0 | +10 defence vs evil (defalign), 50 rounds; Good only | 30g 1:522 |
| chant (`chan`) | 6 | 10 | −5 | +3 AV, group | 40g 1:522 |
| cure poison (`cure`) | 7 | 8 | | | 60g 1:522 |
| major healing (`mahe`) | 8 | 6 | −5 | heal 6-10 (+1/lvl) | 1p 1:522 |
| greater bless (`gbls`) | 10 | 8 | −15 | +6 AV | 3p 1:522 |
| holy force (`hfor`) | 13 | 8 | −10 | 18-24 (+2/lvl), not evil | 3p Aldreth 1:2306 / Jorah 1:3308 |
| disrupt | 16 | 8 | −25 | 30-52 vs undead | 7p 1:2306 |
| godstrike | 20 | 12 | −40 | 30+ | 10p 1:2306 |
| zeal | 25 | 20 | | dmg/MR/crit/AV buff | 25p Very Powerful Spells 10:273 |
Defence-vs-evil adds straight to the DV term in the hit formula (LJNCOMBT.C:393, 1316), so *prev* is worth ~10 DV against every evil monster: e.g. DV 48 → 58 cuts an AV-75 monster's hit chance from 43% to 16% [C formula / I numbers].

## 2. Race choice [C WCCRACE / I ranking]
| Rank | Race | exp (total) | Str | Wil | Hea | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Dwarf** | +30 (250%) | 50-110 | **50-120** | 50-120 | Night vision 75, MR +10, **Encumbrance +20%** | Wil starts at 50 (casting), Str/Hea for plate, +20% carry for plate, no lantern down to −225. |
| 2 | Half-Orc | 0 (**220%**) | 45-110 | 30-95 | 50-120 | none | 12% less exp; weaker casting (Wil from 30). |
| 3 | Half-Ogre | +30 (250%) | 70-150 | 25-70 | 60-150 | +1 HP/lvl, DR −2/hit, MR 15 | Best tank (Str 100 at creation), but Wil cap 70 → SC ~15 lower; heals fail more. Fine if you treat spells as a bonus. |
Avoid: Elf/Dark-Elf/Gnome (low Str/Hea), Halfling (−1 HP/level).

## 3. Stats / CP plan [C costs / I priorities]
Starting 100 CP: **Dwarf Str 78, Int 30, Wil 65, Agi 42, Hea 60, Cha 31 (99 CP)**; Half-Orc Str 72, Wil 48, Agi 45, Hea 58, Cha 31.
- Spellcasting Dwarf L1 = 2 + (195+30)/6 + 5 = 44 → minor healing 54%, protection from evil 44% at L5 (+8), ~70% by L10 with Wil 70 [C formula, I numbers].
- Ladder: the 2H `basher` core (`statplan`, analysis in `kb/stat-builds.md`) with the casting
  stat inserted:
  1. Cha 31
  2. Str +25 / +31 (Dwarf 75/81)
  3. Hea 64
  4. **Wil 70** (SC +2.5 per 5 Wil, MR)
  5. Hea 72
  6. Agi 45 / 48
  7. Str +35
  8. Cha 36
  9. Hea 80 / 86 / 90
  10. Wil 80
  11. Str +40, Agi 53-62, Hea 96-100, then alternate Str / Agi / Hea.

  Int only counts as the 1-in-6 casting term, so skip it.
- **Don't max Str** [I, model]. For a Kang Paladin, Str-max is −19% kills/h on the mixed
  metric (−34% in hard-hitting areas), because Hea and regen starve. The `basher` core is +3%
  vs the old ladder.
- A weapon above your Str is still fully usable (+1.5% energy per missing point, free while
  the bash is capped). Str for **plate weight** is the real reason to go past +35: stay under
  33% load.
- Max HP Dwarf Hea 60: ~111 at L10, ~215 at L20 (Hea 70).

## 4. Attack style
- **BASH with the heaviest 2H weapon**, exactly as the Warrior (warrior.md §4). Paladin combat = Warrior combat.
- Spells per fight: pre-buff *bless*/*prev* (out of combat; casting in a round costs +10% energy, LJNCOMBT.C:907); in fights only heal. Casting uses a whole round's spell energy for damage spells (energy 1000) [C spell.energy]. Mana is tiny before L10 (26), so: *prev* (6) + 3× *mihe* (2) per rest cycle.
- Undead (data `undead` flag): *turn undead* 12-15+3/lvl ignoring armour is a 1000-energy spell; a bash outdamages it by L5 [I]. Use it on high-AC undead (shade ac30-50, barrow wight ac60) where bash misses.
- Self-heal replaces most rest time: regen is `(L+20)·Hea/750` per tick (≈ 2/tick at L10) vs *mihe* ~8 for 2 mana.

## 5. Gear by level band [C; same armour as warrior.md §5]
| Band | Weapon | Armour | Paladin-specific |
|---|---|---|---|
| L1-5 | quarterstaff (free) → greataxe 10g (1:307) / maul 20g (1:354) | padded → chain/scale (1:305/306) | scroll of minor healing (free 1:2144), bless 4g |
| L5-10 | maul / battle-hammer | **half-plate corselet ac19 dr9 180g** or steel brassiere ac22 dr6 180g (1:306); horned helmet, plate gauntlets, plate boots, light plate leggings | *prev* 30g, *turn* 16g, *mahe* 1p (1:522); silver holy amulet (neck, SC+1, Magical) 30g at 1:290, cls 3/4/5/6 |
| L10 quest | **shimmering greatsword** 9-28, 2H sharp, s45, acc 8, Magical 2, 40% chance to cast spell 170 (1-3 dmg ignoring armour) on hit, Paladin-only, limit 1 | | The best weapon a Paladin can get before L15: max 28 at Str 45 (greataxe is 22 at Str 80). |
| L15-20 | three-headed flail 9-23 (4p, 1:354) or keep the greatsword; massive wooden club 10-30 s100 | full plate corselet ac23 dr10 5p, full plate leggings, visored greathelm | copper cross off-hand (SC+5, Good) 20g 1:525 if 1H; *holy force* 3p |
| L20-30 | etched adamant greatsword 10-34 L20 206p (1:3305) | adamantite platemail L30 375p (1:3302) | red skull (evil only) – skip |
| L40-50 | Paladin-only: Caladbolg 15-60 1H (crit+10, no source traced), firebrand cape (back, +4 dmg, L40), star helm (L50, Dreadlord of Blood 17:979), platinum knight set (L50, cls 1/3/4) | | |

## 6. Grinding by level band [C areas from corpus leveling.md / I fit]
Paladin = Warrior table (warrior.md §6) plus these adjustments:
| Levels | Area | Why it suits a Paladin |
|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | Heal between pulls instead of resting |
| 3-8 | **Graveyard 1:817 + Crypt L1-3 1:835/1070/1276** | All undead (skeleton 43/55/11, zombie 62/70/14, ghoul 73/100/9, shade 75/140/15, wight 80/140/10): *turn undead* and *prev* both apply. Crypt needs light (Dwarf: none). Hidden exits `search` 1:1056 n, 1:1070 s, 1:1250 s. |
| 6-10 | Slimy Sewers 9:1 (`search w` at 1:619) / Strange Mansion 1:63 | Mansion = undead (vision, shadow spectre, astral shade); spectral mage 200/1200/20 at 1:382 |
| 10-15 | Kobold caves 6:752 (186 steps) or Darkwood bandits 9:720 | Kobold king 250/2250/16 at 6:834 |
| 15-20 | **Ancient Crypt 1:1943** (skeletal warrior 130/300/60, moaning spirit 128/375/20 bsdef60) | Undead; −17 DR in plate cuts the 60-hit to 43 and *prev* makes them miss. Banshee 250/4000/36 at 1:1952 (24 h). |
| 15-20 | Frozen Caverns 10:35 (`search n` 10:3), Gnoll Encampment 2:1107 | as Warrior |
| 20-30 | **Ancestral Tomb 6:872** (tortured spirit 150/525/15, barrow wight 180/725/15 + 1690-exp variant) | Undead, hit 15, ac 50-60: alternate bash with *turn undead*/*disrupt*. |
| 20-30 | Orc Mansion 2:31-39 (elite orc guard hit 70), Dragon's Teeth 2:177, Darkwood ogres 1:276 | Plate + *prev* |

## 7. Class quest: L10 crypt → shimmering greatsword + L11 [C rooms/exits]
From mummy room 1:1309 (bone key 100%): `use bone key n` → 1:1377 Shadowed Hall. `n` ×5 to 1:1423, **`e`** (Class 3) → 1:1434 → `n` → 1:1432 → `nw` → 1:2242 → `n`,`ne`,`n`,`n`,`n` → Marble Chamber 1:2247 → `w` → 1:2251 → `nw` → **Marble Chamber 1:2252** (young girl 12/0/0, room minlevel 5) → type **`turn right candle left`** (Text exit, WCCMSG) → 1:2254 → **`se`** (Class 3) → 1:2259 → `se` → **Large Tomb 1:2260**: paladin spirit (1090 hp, don't attack), shimmering greatsword placed (limit 1). `train` (shop 55, mk 500), `go portal` → 1:834.
- **Do not** go `e` from 1:2247 into 1:2248: `pull lever` there drops you into 1:2250 with a **horned demon 9999/1000/50** [C room data].
- No key fight on this route (unlike Warrior/WH/Ranger/Mystic) [C].
Later: alignment quests (good line: Annora 1:1333) for the +1 max-damage reward; Ice Sorceress +1 AC at L15 (see corpus bosses.md).

## Group role
Off-tank and backup healer in plate: minor/major healing on others, healing rain (L10), chant, turn undead. Lets a party run one dedicated healer instead of two. Re-`attack` after every heal to keep its exp share. See `kb/grouping.md`.
