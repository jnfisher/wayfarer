# Stat builds for two-handed melee: Str-first vs balanced

Question (John, 2026-10-13): *for two-handed melee, especially bashers, should you max Str first
to wield the best weapons, rather than split CP between Health, Str and Agi?*

Scope: MajorMUD 1.11p, stock data, MBBSEmu. Tags: **[C]** recovered source (file:line under
`WCCMMUD-1.11p-source/SRC/`) or game data. **[I]** modelled or inferred. The model is
`test/play/statmodel.py` (read-only; it uses `mmud_data` and `statplan`), and every table below
can be regenerated with it. The ladders are in `test/play/statplan.py` (`LADDERS`).

## Verdict

**Partly right.** Str is the best stat for a 2H basher for its first ~30-35 points above the race
minimum. **Maxing it is wrong**, and so is the premise that Str unlocks weapons.

1. **Str doesn't gate weapons [C].** No rule stops you wielding a weapon above your Str. You
   also keep full accuracy, damage and hit chance. The only effects are +1.5% energy per missing
   point and no fast-weapon crit bonus. A blow costs at most 1000 energy, and a BASH doubles the
   cost before that cap. So until about L15-20 the penalty usually costs a basher **nothing**
   (§1.1, §3.1). Each build in the model picked its best affordable weapon. Str changed that pick
   only through **weight** (the 33% encumbrance cliff), never through the Str requirement.
2. **Str is the best damage stat [C formula / I size].** Each point gives AV/3, max damage/10
   (×3 on a bash), 48 carrying capacity, and helps keep you under 33% load. Str-max bash DPR is
   **+24-32%** over the balanced ladder at L10-20.
3. **Maxing it starves HP, and the CP cost curve punishes it [I].** Kang Str 90→120 costs 165 CP:
   every CP from L11 to L20. Str-max Kang has **68 / 110 / 152 HP at L5 / L10 / L15** with
   regen 1-2, against 88 / 143 / 204 for the balanced ladder. On kills per hour:
   - **Safe, low-hit areas:** −23% at L5 (rest-bound), +3 to +7% at L10-20, +2% at L30.
   - **Hard-hitting areas,** where the 4×-max-hit HP floor binds: −55%, −41%, −47%, −6%, +2%.
   - **Mixed:** **−19%** (geometric mean, Kang Witchunter). Warrior, Paladin, Ranger and Cleric
     come out −17% to −26%.
4. **Best plan (`statplan` ladder `"basher"`, now the default) [I]:**
   - Str through its cheap CP bands first: race minimum +25/+31/+35, which is Kang 80/86/90.
   - Then Hea to the HP floor of your hunting area, plus Agi 45/48 and Cha 31/36 to clear their
     penalties.
   - Then Hea 86-100, alternating with Str +40..+65 and Agi 53-80.

   It beats or ties the old balanced ladder for every class and race modelled: **+2 to +5%
   mixed, +1 to +3% safe**. It lands within about 1-2% of a CP-by-CP greedy optimiser, except on
   Half-Ogre (see Races).
5. **Race matters more than the ladder [I].** Exp-adjusted, against a Kang Witchunter:
   **Half-Orc +11%** (0% exp), **Half-Ogre +9%** (Str 70-150, +1 HP/level), Human +3%, Dwarf −9%
   (understated: its +20% encumbrance isn't modelled), Elf/Nekojin about −30%.

## 1. The rules that decide it [C]

### 1.1 What Str does, and what it doesn't
| Effect | Formula | Source |
|---|---|---|
| Wield above your Str | allowed. Message "This weapon feels heavy in your hands." | LJNSUPP.C:2417 |
| Energy per blow if too weak | `cost × (3·(strreq−str) + 200)/200` (+1.5%/point) | LJNCOMBT.C:513-517 |
| Fast-weapon crit bonus | needs `str ≥ strreq` (plus cost < 200 and encumbrance ≤ 66%) | LJNCOMBT.C:892 |
| Attack value | `+(str−50)/3` (integer: 80, 83, 86, 89, 92 …) | LJNCOMBT.C:715 |
| Damage | max `+(str−50)/10`; min `+2·((str−100)/10)` above 100 | LJNCOMBT.C:629-633 |
| Carrying capacity | `str×48`, +36 per point above 100 | LJNUTIL.C:4743 |
| Load < 33% | accuracy `+15−enc/10` (≈ **+12 AV**), dodge `+10−enc/10` | LJNCOMBT.C:703-705 |
| Energy from load | `cost × (enc%/2 + 75)/100` (75% empty … 125% full) | LJNCOMBT.C:883 |
| Smash knockdown | `str + level > …` | LJNCOMBT.C:3066 |
| **Not** affected | accuracy, hit chance, damage roll, blows-per-round cap | the whole of `move_player_to_fighter` / `calculate_attack` |

The only checks that make a weapon unusable are these:
- class weapon type, e.g. a Cleric may use only blunt (LJNUTIL.C:9561-9680)
- the item's class or race lists
- Anti-Magic versus Magical (28) items (LJNUTIL.C:9528)
- minimum level, ability 135 (LJNUTIL.C:9541)
- **two-handed only:** nothing worn in the off hand (LJNSUPP.C:2370, LJNUTIL.C:6615), and no
  backstab (LJNACMB.C:493).

None of them looks at Str. [C]

### 1.2 Blows per round
- The cost per blow is `speed×1000 / ((level·combat+45)(agi+150)/6)` (LJNCOMBT.C:236). Then come
  the Str penalty, the load factor, ×2 for BASH (:913), and a clamp to 200..1000 (:936-940).
- A round strikes up to 6 blows while energy lasts (LJNCOMBT.C:2717). With carry-over, that
  averages **1000/cost** blows per round.
- **Str changes blows per round only through the too-weak penalty and through load.** Agi does
  it through `agi+150`: +10 Agi is about 5% cheaper.
- While the doubled cost is ≥ 1000, a bash is one blow per round whatever the Str or Agi. That
  holds for heavy 2H weapons to about L10-15 for a combat-7 Witchunter, and somewhat later for
  combat-6/5 classes.

### 1.3 BASH, damage resistance and armour
- BASH: −15 hit modifier (LJNCOMBT.C:1269), ×1.1 range, no crits, and **damage = (roll − DR) × 3**:
  the monster's DR comes off *before* the ×3 (LJNCOMBT.C:1371, 1394). Monster DR is its raw
  `dr` (LJNCOMBT.C:1136).
- Per energy point, bash/attack = 3×(roll−DR)/(2·cost) against (roll−DR)/cost. That is **1.5×
  for any DR** (more under the cap), so DR doesn't move the bash-vs-attack choice. DR does hurt
  low-damage weapons more in proportion, which is one more reason to want max damage (Str /10,
  ×3).
- Player side:
  - DV = armour AC/10, plus the weapon's AC (LJNCOMBT.C:741).
  - Damage taken = roll − (armour DR + race DR)/10 per hit (LJNCOMBT.C:760, 1371).
  - Dodge chance = `dodge·10/(monsterAV/8)` (LJNCOMBT.C:1380).
  - **A negative dodge makes −dodge% of incoming blows land at 99%** (LJNCOMBT.C:1316). Chain
    and scale pieces carry Dodge −2 to −6 (item ability 34) [C data].

### 1.4 Hea and the CP curve
- Max HP = `hea/2 + L·(basehp+hpbonus) + (hea−50)·L/16 + rolled` (LJNUTIL.C:4969).
- Regen = `max(1, (L+20)·hea/750)`. You get that every 30 s, plus 3× it every 21 s while
  resting (LJNBKGND.C:1656, 2114). The integer breakpoints matter: Hea 50 at L5 regenerates
  1/tick, Hea 75 regenerates 2.
- CP cost per point above the race minimum (LJNGAME.C:1645): 1 for points 1-10, 2 for 11-20,
  3 for 21-30, 4 for 31-40, 5 for 41-50 … 10 beyond 90. CP per level: +10 (L2-10), +15 (L11-20),
  +20 (L21-30) (LJNUTIL.C:8346).

  **This is the core of the answer.** Stats of roughly equal value per point, on an escalating
  cost curve, should be spread. The first ~30-35 Str points cost 1-3 CP each. Points 61-65
  above the minimum cost 7 each.

## 2. Model [I]

`statmodel.py` builds a fresh character of the race at each level, from the race minimums plus
`100 + per-level CP`.

**For each build it picks the best weapon and kit:**
- **Weapon:** the best usable, affordable 2H weapon from reachable shops, and **non-magical for
  a Witchunter**. Budgets in copper: L5 2,000; L10 5,000; L15 40,000; L20 150,000; L30 600,000.
  Missile weapons and the Lost City / Floating Platform shops are excluded.
- **Kit:** leather (AC 260, DR 23, weight 860), chain (385/61/1330, Dodge −8),
  scale (450/87/1850, Dodge −3), or scale plus adamantite bracers. Rangers are limited to
  leather.
- **Attack:** the better of ATTACK and BASH against each monster.

**Monster sets** (stats from WCCKNMSR) cover two scenarios:
- **safe:** graveyard L5, slimes and crypt L10, kobolds and bandits L15, wolves and gnolls L20,
  bugbear, ogre and Ancestral Tomb ghosts L30.
- **hard:** the hardest common hitters in each band: half-ogre bodyguard, dark goblin and black
  orc, dark monk and skeletal warrior, minotaur, bugbear captain and elite orc guard.

**Metric:** kills/hour = 3600 / (fight time + 20 s walking + rest time).
- A round is 5 s.
- Incoming damage counts monster AV against your DV, dodge (including the negative-dodge rule),
  and armour DR.
- Rest runs at the regen rates above.
- The score is multiplied by `min(1, HP / (4 × biggest monster hit))²`. That is the bot's "no hit
  above 25% of max HP" rule (corpus/leveling.md), as a soft penalty.
- "Mixed" is the geometric mean of safe and hard.

**Not modelled:**
- spells, both theirs and yours. This understates Wil for Witchunters and casters.
- group play
- Dwarf's +20% encumbrance and races' magic resistance
- Int crits (worth nothing before about L20)
- drops and quest weapons

**Builds compared:**

| build | what it does |
|---|---|
| `balanced` | the old ladder (notes/combat.md §3) |
| `str_max` | Str to the race max, then `balanced` |
| `1:1:.5` | Str/Hea/Agi points over the minimum in that ratio |
| `basher` | the new default |
| `greedy` | CP-by-CP optimiser on the mixed metric, a few levels of lookahead |

### What changed from the earlier model (notes/combat.md §3)
The earlier model averaged flail, battle-hammer and maul and ignored rest time. It found all-Str
within 1-3% of the ladder. Three things change that here:
- Each build now picks its best weapon and kit. That raises Str-max's damage edge (+24-32% bash
  DPR at L10-20).
- Rest time is charged.
- There is an HP floor.

Str-max still kills **fastest** in many fights (its seconds per kill are equal or better at
L10-30). It loses on **rest at low level**, where Hea 50 regenerates 1-2 per tick, and on **which
areas it can safely farm**.

## 3. Results

### 3.1 What wielding above your Str costs (Kang Witchunter, `basher` ladder) [C formulas / I numbers]
`python3 statmodel.py strpenalty` (bash against AC 30, DR 2):

| L | Str | weapon (Str req) | kit | attack cost | bash cost | bashes/rd | bash DPR | if Str = req | if Str +10 |
|---|---|---|---|---|---|---|---|---|---|
| 5 | 90 | greataxe (80) | leather | 774 | 1000 | 1.00 | 29.3 | 29.3 | 33.1 |
| 5 | 90 | maul (90) | leather | 905 | 1000 | 1.00 | 21.1 | 21.1 | **37.1** |
| 10 | 90 | maul (90) | chain | 664 | 1000 | 1.00 | 36.7 | 36.7 | 40.5 |
| 15 | 95 | massive wooden club (100) | scale+ | 650 | 1000 | 1.00 | 50.0 | 51.8 | 52.4 |
| 20 | 97 | massive wooden club (100) | scale+ | 492 | 984 | 1.02 | 57.6 | 62.1 | 64.7 |
| 30 | 110 | etched adamant greatsword (90) | scale+ | 240 | 480 | 2.08 | 155.3 | 155.3 | 169.5 |

- Raising Str to the requirement gains 0% while the bash is capped (L5-10). It gains 3-8% at
  L15-20, and most of that comes from the Str damage and AV, not the energy penalty.
- The L5 maul row is the **33% load cliff**. With leather, a 300-weight maul and 275 of misc,
  Str 90 is 33.2% loaded and loses the light-load bonus (≈ −12 AV, −8 dodge). Str 100 is under
  33%. **That is the real "heavy weapon needs Str" effect: weight, not the Str requirement.**

### 3.2 Kang Witchunter, per band
`python3 statmodel.py table Witchunter Kang safe|hard`. Stats are S/I/W/A/H/C. DPR is the
monster-set average. `in/rd` is damage taken per round. `s/kill` is fight plus rest seconds.

**Safe areas**

| L | build | stats | weapon / kit | AV | enc% | DPR atk/bash | in/rd | HP | regen | s/kill | kills/h | vs bal |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 5 | balanced | 86/30/45/48/76/31 | greataxe / leather | 74 | 32 | 19/45 | 2.6 | 88 | 2 | 18 | 95 | +0% |
| 5 | str_max | 103/30/45/30/50/30 | maul / leather | 74 | 28 | 19/56 | 2.9 | 68 | 1 | 30 | 74 | **−23%** |
| 5 | basher | 90/30/45/46/72/31 | halberd / leather | 71 | 32 | 20/47 | 2.7 | 84 | 2 | 18 | 96 | +1% |
| 10 | balanced | 88/30/55/53/80/36 | long-spear / leather | 96 | 29 | 31/43 | 0.8 | 143 | 3 | 9 | 122 | +0% |
| 10 | str_max | 111/30/45/30/50/30 | maul / leather | 95 | 25 | 28/55 | 1.1 | 110 | 2 | 8 | 127 | +3% |
| 10 | basher | 90/30/45/48/84/36 | long-spear / leather | 97 | 29 | 31/45 | 0.8 | 148 | 3 | 9 | 123 | +1% |
| 15 | balanced | 92/31/55/62/86/41 | three-headed flail / scale+ | 90 | 57 | 38/53 | 0.7 | 204 | 4 | 10 | 123 | +0% |
| 15 | str_max | 120/30/45/44/50/31 | three-headed flail / scale+ | 96 | 39 | 48/70 | 0.8 | 152 | 2 | 9 | 124 | +1% |
| 15 | basher | 95/30/45/56/91/41 | three-headed flail / scale+ | 90 | 55 | 38/52 | 0.7 | 210 | 4 | 10 | 122 | −1% |
| 20 | balanced | 98/31/55/68/90/46 | three-headed flail / scale+ | 111 | 53 | 54/78 | 3.1 | 265 | 4 | 15 | 103 | +0% |
| 20 | str_max | 120/30/45/49/80/36 | three-headed flail / scale+ | 115 | 39 | 64/97 | 3.4 | 247 | 4 | 13 | 109 | +7% |
| 20 | basher | 97/30/45/62/100/41 | three-headed flail / scale+ | 109 | 54 | 51/75 | 3.2 | 282 | 5 | 14 | 105 | +2% |
| 30 | balanced | 110/41/65/79/100/46 | three-headed flail / scale+ | 138 | 44 | 105/126 | 1.7 | 398 | 6 | 9 | 124 | +0% |
| 30 | str_max | 120/41/65/68/97/46 | three-headed flail / scale+ | 140 | 39 | 104/137 | 1.7 | 391 | 6 | 9 | 126 | +2% |
| 30 | basher | 110/30/45/80/108/41 | three-headed flail / scale+ | 139 | 44 | 103/127 | 1.7 | 417 | 7 | 9 | 125 | +1% |

**Hard areas**

| L | build | stats | HP | bash DPR | s/kill | kills/h | vs bal |
|---|---|---|---|---|---|---|---|
| 5 | balanced / str_max / basher | 86-48-76 / 103-30-50 / 90-46-72 (S-A-H) | 88 / 68 / 84 | 34 / 43 / 35 | 23 / 38 / 23 | 32 / 14 / 29 | 0 / **−55%** / −8% |
| 10 | same | 88-53-80 / 111-30-50 / 90-48-84 | 143 / 110 / 148 | 35 / 43 / 36 | 17 / 17 / 16 | 98 / 57 / 101 | 0 / **−41%** / +3% |
| 15 | same | 92-62-86 / 120-44-50 / 95-56-91 | 204 / 152 / 210 | 57 / 74 / 56 | 15 / 18 / 16 | 73 / 39 / 78 | 0 / **−47%** / +6% |
| 20 | same | 98-68-90 / 120-49-80 / 97-62-100 | 265 / 247 / 282 | 66 / 85 / 63 | 14 / 11 / 14 | 96 / 90 / 105 | 0 / −6% / +10% |
| 30 | same | 110-79-100 / 120-68-97 / 110-80-108 | 398 / 391 / 417 | 115 / 127 / 116 | 10 / 10 / 10 | 119 / 122 / 122 | 0 / +2% / +3% |

- At L5, no build reaches the 144 HP that the half-ogre bodyguard's 36-point hit asks for. That
  row only measures HP.
- L30 kills end in 1-2 rounds, so walking dominates and every build ties.

### 3.3 All 2H melee classes, Kang (% kills/h vs `balanced` at L5/10/15/20/30, then the geometric mean)
`python3 statmodel.py compare Warrior,Witchunter,Paladin,Ranger,Cleric Kang`

| Class | scenario | str_max | 1:1:.5 | **basher** | greedy optimum |
|---|---|---|---|---|---|
| Witchunter | safe | −23/+3/+1/+7/+2 (**−3**) | +1 | +1/+1/−1/+2/+1 (**+1**) | +2 |
| | hard | −55/−41/−47/−6/+2 (**−33**) | +6 | −8/+3/+6/+10/+3 (**+3**) | +3 |
| | mixed | **−19** | +3 | **+2** | +3 |
| Warrior | safe / hard / mixed | −1 / −32 / **−18** | +1 / +5 / +1 | +1 / +3 / **+2** | +2 / +4 / +3 |
| Paladin | safe / hard / mixed | −1 / −34 / **−19** | +1 / +7 / +2 | +1 / +5 / **+3** | −1 / +9 / +4 |
| Ranger (leather) | safe / hard / mixed | −8 / −40 / **−26** | +1 / +8 / +5 | +3 / +7 / **+5** | +3 / +12 / +7 |
| Cleric (blunt only) | safe / hard / mixed | 0 / −32 / **−17** | 0 / +7 / +4 | +2 / +7 / **+4** | +1 / +8 / +5 |

**Greedy optimum (mixed), Kang Witchunter:** L1 S90 A30 H64 C31 → L5 S91 A45 H70 C36 → L10 S92
A48 H81 C41 → L15 S93 A60 H90 → L20 S95 A64 H100 → L30 S110 A85 H103.
- Every class follows the same shape: **Str to about 86-93 by L1-5, then almost nothing into
  Str until L20.** Hea goes to 80-100, Agi to 45-65, Cha to 36-41.
- Warrior / Paladin / Cleric land within ±3 Str and ±10 Hea of that.
- Str-max's DPR edge is real: bash +28% at L10, +32% at L15, +24% at L20. It mostly turns into
  fewer seconds of fighting, which walking and resting dilute.

## 4. Recommended CP ladders [I]

### 2H melee core (`statplan.allocate_cp(..., ladder="basher")`, the default)
`"+N"` means N above the race minimum.

1. Cha 31
2. Str +25
3. Hea 64
4. Str +31
5. Hea 72
6. Agi 45
7. Str +35
8. Agi 48
9. Cha 36
10. Hea 80, 86
11. Agi 53
12. Hea 90
13. Str +40
14. Cha 41
15. Agi 56
16. Hea 96
17. Agi 62
18. Hea 100
19. Str +45
20. Agi 68
21. Str +50
22. Agi 74
23. Str +55
24. Hea 105
25. Agi 80
26. Hea 110
27. Str +65 …

Rules of thumb behind it:
- **Str: spend the 1-3 CP bands (min+30..35) before anything but the 1-CP Cha point and a
  minimal HP floor.**
- **Hea: keep max HP ≥ 4 × the biggest hit where you hunt** (corpus/leveling.md "MinHP"). For
  Kang Witchunter / Warrior that means Hea ≈ 64-72 for L1-5, 80-86 for L10, 90 for L15, ~100 for
  L20.
- **Agi/Cha:** get out of the penalties (Agi 45/48, Cha 31/36/41). Past that, Agi is worth about
  as much as Str per CP from L10 on: blows per round once uncapped, AV/6, dodge/3.
- **Load:** stay under 33% (Str×48×0.33 ≥ gear + weapon + coins). It's worth about +12 AV and +8
  dodge, more than any 20 CP.
- Bank CP rather than spend lower on the ladder. Points between Str breakpoints only buy
  capacity.

### Per class
| Class | Ladder | Notes |
|---|---|---|
| Warrior | `basher` | Platemail is allowed, so the 33% load needs more Str. Watch enc% when going to plate. |
| **Witchunter** | **`basher_am`** (`basher` + **Wil 55 after Hea 90**, about L14) | Anti-Magic cuts spell damage by MR/2 %, and spells aren't in the model. Move Wil 55-65 earlier when hunting casters. No Magical weapons: three-headed flail (L15, 4p) is the model's pick L15-30; etched adamant greatsword (L20, ≈206p) bashes harder at L30. |
| Paladin | `basher` with **Wil 70 right after Str +35** | Spellcasting needs Wil (paladin.md). The melee core is otherwise identical. |
| Ranger | `basher` with Int/Wil rungs after Hea 72 | Leather only (armour 6), so the 33% bonus is easy to keep and Hea matters more (≈ +5% mixed vs balanced). |
| Cleric as a basher | caster stats first (Wil 65), then `basher` from Str +25 | Combat 5 and blunt only. The `basher` core is +4% mixed vs balanced for the melee part. Heals are the class's real defence (cleric.md). |

### Kasimir (Kang Witchunter, current S79 I30 W45 A42 H74 C30, 98 CP spent; `python3 statplan.py`)
| total CP (≈ level) | Str | Int | Wil | Agi | Hea | Cha | banked |
|---|---|---|---|---|---|---|---|
| 110 (L2, now: 12 unspent) | 82 | 30 | 45 | 42 | 74 | **31** | 2 |
| 150 (L6) | **90** | 30 | 45 | 48 | 74 | 32 | 0 |
| 200 (L11) | 90 | 30 | 45 | 50 | 86 | 36 | 0 |
| 250 (L14) | 93 | 30 | 55 | 53 | 90 | 36 | 3 |
| 300 (L18) | 95 | 30 | 55 | 56 | 96 | 41 | 0 |
| 400 (L23) | 102 | 30 | 55 | 68 | 100 | 41 | 1 |

Kasimir's Hea 74 already passes the early HP rungs, so his CP goes to Str 90 first.

## 5. Races for a Str-heavy 2H build [C race data / I ranking]
`python3 statmodel.py races Witchunter mixed`. Exp-adjusted: kills/h divided by (100 + class
exp + race exp)% (LJNUTIL.C:8291), relative to Kang.

| Race | Str | Hea | Agi | Extras | exp | L20 on `basher` | adj. vs Kang (safe / hard / mixed) |
|---|---|---|---|---|---|---|---|
| Kang | 55-120 | 50-110 | 30-90 | DR +1, DV +5, poison immune | +35 | S97 A62 H100, 282 HP | 0 / 0 / 0 |
| **Half-Ogre** | **70-150** | 60-150 | 20-60 | **+1 HP/level**, DR +2, MR 15 | +30 | S113 A60 H100, 302 HP | **+7 / +11 / +9%** |
| **Half-Orc** | 45-110 | 50-120 | 40-100 | none | **0** | S90 A67 H100 | **+14 / +7 / +11%** |
| Human | 40-100 | 40-100 | 40-100 | none | 0 | S80 A62 H98 | +11 / −4 / +3% |
| Dwarf | 50-110 | 50-120 | 30-90 | MR 10, **Encumbrance +20%** (not modelled) | +30 | S92 A62 H100 | −6 / −11 / −9% |
| Gnome / Elf / Nekojin | ≤ 90-100 | ≤ 80-100 | | | +20 / +45 / +50 | | mixed: −14 / −32 / −29% |

- **Half-Ogre is the only race where something close to "Str first" is right.** Its cheap Str
  band runs from 70 to about 105. The greedy optimum is Str 107 at L1 (min +37) and 112 by L10,
  with Hea 82-90 and **Agi left at 20-33 until L10** (Agi 45 costs Half-Ogre 45 CP).
- Half-Ogre greedy beats `basher` by about 4% on the mixed metric. With a Half-Ogre, skip the
  Agi 45/48 rungs until Hea 90.
- Pure Str-max on a Half-Ogre (Str 147, Agi 20, Hea 60 at L20) is +6% in safe areas and −20%
  in hard ones, against Half-Ogre `balanced`.
- Half-Orc wins on exp/hour only because of its 0% exp penalty.

## 6. Caveats [I]
- The model is flat near the optimum. `balanced`, `1:1:.5`, `basher` and `greedy` all land within
  ±5%. Only `str_max` stands out (it's worse). Treat the breakpoints and the HP floor as the
  important part, not the exact order.
- The safety rule (HP ≥ 4 × max hit) drives the "hard" results. A bolder rule (3×) narrows
  Str-max's loss. A party with a healer removes most of it. **Str-max is defensible for a grouped
  basher with heals; Witchunters resist friendly heals, so not for them.**
- Spells (monster casters, MR) aren't modelled. That understates Wil, especially for a
  Witchunter.
- Armour kits are fixed per band. Weapon budgets are guesses at what the character can afford.
- Kang's Defence 5 and DR 10 are applied as +5 DV and +1 DR; that mapping is inferred.
