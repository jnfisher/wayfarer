# Thief (WCCCLASS 8): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats `hp/exp/maxhit`. Prices include markup, not Charm. `1p` = 100 gold.
Steps from Town Square 1:224. **Stealth/backstab/picklock formulas: `ninja.md` §1** (identical code).

**One-line path:** Goblin. The cheapest class to level (−20% exp) and the hardest-hitting opener
per level (backstab), on top of the weakest melee (combat 4, 1H only, no shield). Every fight is
sneak → backstab → short finish. Arena → Graveyard → Slimy Sewers → L10 crypt quest (crystal
shortsword, the best backstab blade until L40) → kobolds/Smuggler Den → wolves/Rhudaur alleys →
Ancestral Tomb.

## 1. Class facts [C: WCCCLASS rec 8]
| Field | Value | Meaning |
|---|---|---|
| exp | **−20%** | Lowest in the game. L2 = 10×(80+race): Goblin 1,150, Human 800 |
| basehp / hplevel | 4 / **3** | +4 HP/level + `genrdn(0,4)` ≈ 2 random: lowest melee HP |
| combat | **4** | Worst melee AV of the seven (AV = Warrior −16 at L5, −20 at L10, −24 at L20) [C LJNCOMBT.C:700] |
| magery | none | |
| weapons | **4 = Any 1-handed** | No 2H (except class-listed twin shortswords). **No off-hand items at all** unless class-listed (LJNUTIL.C:9697-9712) |
| armour | 6 = Studded Rigid Leather | Leather kit; no chain/plate |
| abilities | 103 Class Stealth, **39 Thievery +10**, 40 Find Traps, 37 Picklocks, 31 Bash | Thievery = (Agi+Int+Cha+24·L')/6 + 10 (`steal`; not needed for progression) |
| Perfect Stealth | keepable from **L20** | LJNUTIL.C:705 |
| Alignment-quest reward | backstab min/max +10, stealth +2 | LJNUTIL.C:504-510 |
| L11 trainer | Super Thief Trainer, shop 58 at **1:2275** | §7 |
| L1-10 class trainer | Thief Training Room 1:201 (mk300); use the Universal Trainer 1:1376 (mk0) | corpus strategy.md |

## 2. Race choice [C WCCRACE / I ranking]
| Rank | Race | exp (total) | Agi | Str | Hea | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Goblin** | +35 (115%) | 55-125 | 30-70 | 40-100 | Race stealth (+10 stealth, +5 BS AV with class stealth), NV 65 | No HP penalty, sees in −215 rooms, Agi cap 125. |
| 2 | Halfling | +25 (**105%**) | **60-150** | 20-60 | 40-100 | Race stealth, Dodge +10, **hpbonus −1** | Best pure backstabber and cheapest race-stealth exp, but 3+~2 HP/level (≈ 75 HP at L10). No night vision. |
| 3 | Dark-Elf | +45 (125%) | 50-120 | 40-90 | 30-90 | Race stealth, NV 80, crit +1, 3rd-AV +3 | Better Str for the non-backstab rounds; most exp. |
Human (80% total exp, fastest levelling) loses 10 stealth/5 BS AV and needs a light; a valid "speed" pick.

## 3. Stats / CP plan [I, breakpoints C]
Start (100 CP): **Goblin Str 55, Int 45, Wil 40, Agi 78, Hea 50, Cha 46 (100)**; Halfling Str 40, Int 30, Wil 40, Agi 85, Hea 50, Cha 46 (91; +9 → Agi 88).
- Agi first (stealth /4, BS AV /2, dodge /3, picklocks ×2/7). Cha 46-50 (dodge penalty, stealth /6, thievery). Str to 60/70 (+1 max = +2 backstab). Hea 55-60. Int to 50-60 (stealth /8, picklocks).
- Max HP Goblin Hea 50: ~85 at L10, ~150 at L20; Halfling ~75 at L10.

## 4. Attack style [C ninja.md §1]
- `sneak`, enter, `bs <mob>` → damage `(2×wmax-ish + 2L + stealth/10 + BS bonuses) × (100+L)%`, hit = BS AV − monster BS DV. Example L10 Goblin (stealth ≈ 83, Agi 80) with **crystal shortsword** (5-16, BS-AV+15, BSmin+5, BSmax+10), Str 55: AV ≈ 93; damage (10+20+8+5 .. 32+20+8+10) ×1.10 = **47-77**.
- Finish with **BASH** if the weapon is slow enough that bash cost is capped, otherwise ATTACK. With combat 4 and no class crit, bash's 1.65×-per-energy usually wins below L15 vs AC ≤ 30 [I from combat.md §2]; vs AC 40+ use ATTACK.
- Never start a fight without the opener before L15: the follow-up melee is weak and you have the least HP of the seven.
- Picklocks/traps: required for the crypt quest route (§7); also opens the black serpent/black star key doors late (ninja.md §1).

## 5. Gear by level band [C]
| Band | Weapon (1H) | Armour (type ≤ 6) | Thief extras |
|---|---|---|---|
| L1-3 | club (free); dagger 1-5 BS+10 2g, **shortsword 2-9 BS+5 12g** (1:355) | padded set (free) | |
| L3-10 | broadsword 3-13 26g / longsword 3-12 24g (1:355); **ancient sword 4-19 BS+10 wt0** (spectral mage 1:382, 25%) | **rigid leather tunic ac13 dr1.3 16g**, rigid leather pants 4g, hard leather gauntlets 2g (1:310); hard leather helm 4g, hardened leather boots 4g (1:2154); travelers cloak 4g (1:312) | Avoid stealth-penalty pieces (greatcloak −10, any chain). |
| L10 quest | **crystal shortsword 5-16**, spd 1000, s25, acc 10, BS-AV+15, BS min+5/max+10, picklocks+10, find traps+10, Magical 2, Thief-only, limit 1 | | Keep it to ~L40. |
| L10-20 | (backup) serrated battle axe 5-16 4p (1:307), katana 7-17 (drop 2%) | **gianthair vest ac14 dr1.4 5p**, **ogre-skin shirt ac15 dr1.5 L10 14p** (6:1343, 213 steps); golden armbands 2.4p (1:290); adamantite bracers L15 70p (6:1337); iron crown (kobold king) | Rhudaur Thieves' Guild 2:2561 (hidden; black serpent key area): **lockpicks** (+5 picklocks) 75g, thief's kit 3p (+5/+5) — both are *weapons*: wield only to pick, then re-wield. Shadow cloak st+10 BS+15 L15 150p (2:2561). Golden snake earrings BS+5 6p (1:290). Fingerbone bracelet BS+5 11p (Slum 1:2324). |
| L20-30 | **twin shortswords 5-19** 2H, BS+10, L20, Magical 2, Thief-only, 8p (Lost City Weapons 16:331, mk300) — opener only (2H can't repeat backstab) | wyvernhide set L20 (Jorah 1:2328); thief's pouch (wrist, st+2 thievery/picklocks+5, L20) 600p (16:317) | |
| L40-50 | Thief-only: raven claw 11-24 BS+30 (L40, no source traced), blacknight cloak (BS+35, L40), soulblighter 14-35 (Banished One 17:632-634, evil) | | |

## 6. Grinding by level band [C corpus leveling.md / I fit]
Same stealth-fit rules as `ninja.md` §6 (single spawns, bsdef 0, no See Hidden), but with ~10% less HP and no class dodge: keep monster maxhit ≤ 20% of HP.
| Levels | Area | Monsters | Access | Thief notes |
|---|---|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | ≤ 25 hp | maxlevel 3 | |
| 3-6 | Graveyard 1:817; Slum soft streets 1:1073 | skeleton/zombie; thug 28/20/11, orc rogue 30/22/11 (cash) | none | Gravediggers (Neutral, 130/1500/30): one backstab ≈ 40%; only with ≥ 70 HP |
| 5-10 | **Slimy Sewers 9:1** | slimes 30-55 hp, hit ≤ 14 | `search w` 1:619; light −200 | |
| 6-10 | Crypt 1:835 levels 1-3 | ghoul/wight (bsdef 0) | hidden exits | quest route |
| 10-15 | **Kobold caves 6:752** / **Smuggler Den 9:161** (thief 55/125/12, smuggler 70/150/12, Neutral Evil) | one-shot backstabs | Den: hidden `d` at Docks, 27 steps | Den is close and pays cash |
| 15-20 | **Rhudaur Dark Alleys 2:1808** (cutpurse 110/275/15, depraved murderer 130/375/15) | hits 15 | black serpent key (dark monk 20%, Dragon's Teeth 2:177); light −200 | Also where the Thieves' Guild is |
| 15-20 | Frozen Caverns 10:35 | grey wolf 125/375/21 | `search n` 10:3 | from ~L17 |
| 20-30 | **Ancestral Tomb 6:872** | tortured spirit 150/525/15, barrow wight 180/725/15 (+1690-exp variant) | hidden `w`, light −200 | Backstab ignores their ac 50-60 |
| 30+ | see corpus "30+" | | | |
Skip: anything with hit ≥ 35 before L20 (half-ogre bodyguards in Slum HO streets, gnolls, Huge Cave orcs, Orc Mansion, Labyrinth).

## 7. Class quest: L10 crypt → crystal shortsword + L11 [C rooms/exits]
From 1:1309 (mummy; bone key, or `picklock n` at bonus 0 ≈ 60% at L10): → 1:1377 → `n` ×5 → 1:1423 → **`w`** (Class 8) → 1:1429 → `w`,`w`,`sw`,`w` → 1:2266 → locked door `w` (`picklock w`, bonus −50) → `n` → 1:2268 → trap `n` (`disarm trap n`) → `w` → 1:2270 → trap `w` (disarm) → 1:2271 owlbear **93/225/28** (copper key; or picklock −20) → 1:2272 → **`e`** (Class 8) → 1:2274 → `e` → **Large Tomb 1:2275**: thief spirit, crystal shortsword placed (limit 1). `train` (shop 58, mk 500), `go portal` → 1:834.
- Details of each step: ninja.md §7 (same corridor up to 1:2272).

## Group role
Scout, picklocks and traps for the whole party, backstab opener. Exp −20% means it keeps pace (or leads) on equal shares, making it the best boxed utility alt. See `kb/grouping.md`.
