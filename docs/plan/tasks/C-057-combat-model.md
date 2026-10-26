---
id: C-057
title: "The combat model: a character's blows against a monster, per round, from the game's formulas"
tier: B
size: M
wave: 9
depends: []
crate: wayfarer-world
touches:
  - crates/wayfarer-world/src/combat.rs (new)
  - crates/wayfarer-world/src/lib.rs
  - crates/wayfarer-world/README.md
  - docs/kb/combat.md (only if the card confirms a fact the KB lacks)
---

## Goal

A pure module that answers, for one character, one weapon and one monster: what each blow
costs, how often it lands, crits or is dodged, how much damage it does, how many blows land in
each round of a fight, and how likely the monster is to be dead after round 1, round 2, and so
on, for a plain attack, a bash and a smash. It is the engine of the gear planner (C-059, C-060);
nothing in the brain uses it yet. Solo play only (John, 2026-10-26): no party damage, no
monster turns.

## Inputs

- `docs/kb/combat.md` "Attack modes" and "How a fight runs"; `docs/kb/stat-builds.md`.
  The formulas below are taken from the recovered source (`LJNCOMBT.C` 200-251
  `compute_energy_used`, 338-943 `move_player_to_fighter`, 1015-1204
  `move_monster_to_fighter`, 1226-1427 `calculate_attack`, 2640-2830 the auto-combat blow
  loop; `LJNUTIL.C` 4990-5010 `mabase`, 8870-8975 `get_armour_rating`). The card states
  them in full so it needs nothing outside this repository.
- The world fixtures (`tests/fixtures/world/`): item records (`mindmg`, `maxdmg`, `speed`,
  `energy`, `strreq`, `accuracy`, `ac`, `dr`, `armtype`, `worn`, `abilities`/`abilvals`,
  `weight`), monster records (`ac`, `dr`, `maxhp`, `magicres`, `level`, abilities), class
  records (`combat`), race records (abilities).
- The conversation that asked for this (John, 2026-10-26): "it's pretty nuanced what would be
  the best weapon … we ONLY need to take solo play into account … we select scripting areas
  where we kill most things in 1 round, possibly 2."

## Behaviour

### The character for one blow (`move_player_to_fighter`)

Inputs: class `combat` (C), level (L), Str, Int, Agi, Cha, the wielded weapon (or none), the
worn items, the encumbrance percentage (E, `carried weight × 100 / capacity`, truncated),
`maxenergy` (1000 for every stock character), and the buffs (below). All arithmetic is C
integer arithmetic (truncation toward zero); keep every intermediate as the C does.

- **Energy per blow.** `skill = L*C + 45`, `quick = Agi + 150`,
  `divisor = skill*quick*1500/9000`, `cost = weapon.energy + weapon.speed*1000/divisor`
  (no weapon: speed 1200, energy 0). Then:
  1. Str short of the weapon's `strreq`: `cost = cost*((strreq-Str)*3 + 200)/200`, and the
     character is *too weak* (no fast-blow crit bonus).
  2. Alter Energy Usage (ability 87) above 0: `cost = cost*pct/100`.
  3. Encumbrance: `cost = cost*(E/2 + 75)/100`.
  4. Fast blow: if `cost < 200`, `E <= 66` and not too weak:
     `n = min(20, 200 - cost + (Agi-50)/10)`; crit `+= n` when `E < 33`, `+= n/2` otherwise.
  5. Bash: `cost *= 2`. Smash: `cost = maxenergy`.
  6. `cost = max(cost, 200)`, then `min(cost, maxenergy)`.
  (Leave out the ×1.1 "cast in this round" and the slowed weapon: the planner never casts
  between blows.)
- **Damage range.** The weapon's `mindmg..maxdmg` (fists 1..4). `max += (Str-50)/10`;
  `min += (Str-100)/10*2` when that is above 0; then `min = min(min, max)`, both at least 0.
  `max +=` Alter Damage (ability 4) from items and buffs.
- **Attack value.** Accuracy = weapon + worn items' `accuracy`, 1 if that is 0; when `E < 33`
  add `15 - E/10`. `skill = (C-1)*2 + L/2 + acc/2`; `training = (C-1)*isqrt(L)`;
  `AV = (skill + training)*2 + (Str-50)/3 + (Agi-50)/6`, plus the AV buffs (abilities 22,
  105, 106 from buffs and items).
- **Crit chance.** `mabase = clamp(L/10 + (Int-50)/10 + (Agi-50)/20 + (Cha-50)/30, 1, 75)`;
  `crit = max(1, mabase + critbonus)`, where `critbonus` is Alter Critical (58) from items and
  buffs; then the fast-blow bonus above.

### The monster (`move_monster_to_fighter`)

`DV = ac + Alter Defence (2)`; `DR = dr*10 + Alter DR (7)` in tenths, a tenth of it taken off
each blow; `dodge = Dodge (34)`; `defmod = Defence Modifier` (the ability that
`calculate_attack` adds to the attacker's hit modifier); `maxhp`. The alignment defences are
left out (`defalign` 0): the planner does not know the character's alignment group.

### One blow (`calculate_attack`)

- Move modifiers: attack 0 / 0%; bash −15 to hit, +10% damage, crit 0; smash −25, +20%, crit 0.
  `hitmod += defmod`. The damage bonus scales both ends: `min = min*(100+bonus)/100`, the same
  for max.
- Hit chance: `power = (AV+hitmod)²/14/10`; `hit = 100 - DV²/power` (5 when `power` is 0),
  clamped to 10..99. The roll is `genrdn(1,100)` and lands when below `hit`.
- Crit: above 40, `crit = (crit-40)/3 + 40`; a landed attack crits when `genrdn(0,100) < crit`;
  then the range is `2*max .. 4*max`.
- Damage: `genrdn(0, max-min+1) + min - DR/10`, then ×3 for a bash, ×5 for a smash; 0 or less
  is a glance (0 damage).
- Dodge: a landed blow is dodged when `dodge > 0` and `genrdn(0,100) < min(95, dodge*10/(AV/8))`
  (0 when AV is 8 or less), after the damage roll.
- **`genrdn(lo, hi)` returns `lo..hi` with both ends included** (John, 2026-10-26; the KB
  calls this unproven, `combat.md` "Does `genrdn(lo, hi)` include `hi`?"). So a hit lands with
  probability `(hit-1)/100`, a crit `crit/101`, a dodge `dodgechance/101`, and the damage roll
  is uniform over `min..max+1`. Keep this in one place (a `ROLL_TOP_INCLUDED` constant with a
  comment) so it can be flipped if the fact is settled the other way.

### Rounds (the auto-combat loop)

Engaging sets the character's energy to 0; every round adds `maxenergy`; within a round blows
are struck while the energy left is at least the blow's cost, at most **six** a round; what is
left carries into the next round. So a bash costing 632 strikes 1, 2, 1, 2, … and one costing
496 strikes 2, 2, 2, ….

### Smash knock-down

A landed smash knocks the monster down when `Str + L > monster level + magicres + 30`; while
it is down its DV is `max(0, DV - 35)`. Model it as down from the blow after the first landed
smash until the fight ends (a smash every round keeps it down, `combat.md` "SMASH knock-down").

### Buffs

A `Buff` is a set of modifiers `{av, crit, max_damage, str, agi, energy_pct}`. Str and Agi
buffs change the stats before everything above. The model takes a list of them; where they
come from (spells, the gear planner's checkboxes) is C-059's business.

### Results

For (character, weapon, monster, move):
- `cost`, `blows_per_round` for the first 8 rounds, `hit`, `crit` (after the 40 rule),
  `dodge`, the damage range of a normal blow and of a crit;
- `expected_damage_per_blow`, expected damage in round 1 and in round 2;
- `p_kill_by_round[k]` for k = 1..8, and `expected_rounds` (capped at 20, with a flag when
  the cap is reached).

The kill probabilities are **exact**, not sampled: keep the distribution of damage dealt so far
as an array over `0..=maxhp` (everything at or above `maxhp` in the last cell), and fold in one
blow at a time. A blow's damage is a mixture of point masses (miss, glance, dodge) and two
uniform ranges (normal, crit) over multiples of 3 or 5 for bash and smash; folding a uniform
range in is a sliding window over a running sum, O(maxhp) per component. Smash's knock-down
makes its second and later blows use the lowered DV.

## Design notes

- `combat.rs` in `wayfarer-world`: `Fighter` (built from a class, race, level, stats, items
  and buffs), `Defender` (from a monster record), `Move { Attack, Bash, Smash }`,
  `Blow` (one blow's numbers), `Fight` (the round results above). No I/O, no world pack
  access inside the maths: the caller hands in parsed records (small structs with the fields
  above, read from `raw` by helpers in this module).
- Performance: the gear planner will run about 150 weapons × 3 moves × 10 monsters per
  request. One `Fight` for a 300-HP monster over 4 blows must stay well under 100 µs on CI;
  add a test that times 4,500 fights and asserts under 1 s (comment: why it is there).
- Leave out: casting, monster turns, party damage, backstab (`bs` has its own AV and damage
  rules), martial-arts moves, Protective Shield, Shadow Stealth, blindness, the knocked-down
  *player*, Quickness and Slowness, rank (solo play has no rank).

## Done when

- The formulas above are implemented with C integer arithmetic and covered by tests.
- The fixture numbers below come out exactly.
- `p_kill_by_round` agrees with a brute-force check (enumerate every outcome) for a small
  monster within 1e-9.

## Tests

Energy and blows (independent of the roll question):
- Kang Witchunter (combat 7), L1, quarterstaff (speed 1200, energy 0), Agi 42, 18% load: the
  KB's worked costs (`combat.md` "Fast weapons crit more": 238 before the load factor at L16
  and 199 after it; +1 crit at L16, +9 at L17, +17 at L18, +20 from L19). Recompute from the
  formula in the test and assert the KB's numbers.
- Witchunter (combat 7), Str 95, Agi 56, L15: Magebane sword (speed 1600, energy 0, Str 60)
  at 55% load: attack cost 316, bash 632, bashes per round 1, 2, 1, 2, 1, 2, …;
  bone-handled headcutter (speed 2200, Str 80) at 51% load: attack 427, bash 854, bashes
  1, 1, 1, 1, 1, 2, …. At L20 (Str 97, Agi 62): Magebane at 54% load bash 496 (2 every
  round), headcutter at 50% load bash 672. (Worked with the card's formulas on 2026-10-26.)
- A Gnome Druid (combat 4), Str 60, Agi 45, L10, maul (speed 2600, Str 90) at 42% load:
  attack cost 1309 before the cap, so attack and bash both strike once a round.
- Hit chance: AV 58 against DV 40 gives `hit` 34 (`combat.md`).
- Dodge: Dodge 15 against AV 58: `min(95, 150/7)` = 21 (`combat.md`).
- Kill probability: a 10-HP monster, a weapon that always lands for 4..6 with no crit or dodge
  (a fixture-built fighter): `p_kill_by_round` after exactly two blows is the brute-force
  value; a monster with more HP than eight rounds can deal gives `expected_rounds` 20 with the
  cap flag.

## Notes for the agent

- The KB's own attack-vs-bash thresholds for the L1 Kang (AC ≤ 32 fresh, ≤ 29 sustained) were
  worked out with the top value excluded; with it included they may move by one. Don't assert
  them; say in the PR whether they moved.
- If the source and this card disagree, the source wins: say so in the PR and fix the card.
- KB notes: if you confirm a combat fact the KB lacks, add it to `docs/kb/combat.md` with [C]
  and its line numbers, and a line in `docs/kb/changelog.md`.
