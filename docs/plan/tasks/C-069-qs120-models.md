---
id: C-069
title: "1.20 numbers in the models: the backstab opener, openings and weaves, the knees, the casting stat, area resists and the class tables, as data"
tier: B
size: L
wave: 12
depends: [C-064, C-066]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/data/quicksilver-1.20/balance.toml (new), class.toml (new)
  - crates/wayfarer-world/src/{combat.rs,combat/,qs120.rs}
  - crates/wayfarer-brain/src/{gear_plan.rs,regen.rs,sneak.rs,fight.rs}
  - docs/brain.md, docs/kb/quicksilver-1.20.md, docs/kb/changelog.md
---

## Goal

Wayfarer's damage and recovery estimates use 1.20's rules on a 1.20 server: the stab's energy and
the blows after it, the stab crit, openings and weaves, the crit, magic-resistance and dodge
knees, the casting-stat bonus to spell damage, area spells cut by resists, the race regen
bonuses, the class damage knobs and the level-60 scaling cap. The numbers are data. On 1.11p and
unknown, every estimate is as today.

## Inputs

- `docs/plan/quicksilver-1.20/handoff.md` 5.4-5.10, 6 and 7 (the `[balance]` table, the per-class
  tables, the band targets).
- `crates/wayfarer-world/src/combat.rs` and `combat/` (C-057's combat model: blows per round from
  the game's formulas), `crates/wayfarer-brain/src/gear_plan.rs` (C-059), `regen.rs`, `sneak.rs`
  (C-048, the opener).
- **The class tables are changing** (the orchestrator, 2026-11-01): handoff 6.2 and 7.1
  (`class_melee_pct`, `class_spell_pct`, `class_weave_pct` as four level bands) are being replaced
  before 1.20.0 is tagged by **anchor points with straight-line values between them, never
  falling as level rises**. The final values come later.

## Behaviour

- **`balance.toml`** holds every key of handoff section 7's table with its 1.20.0 value, and
  `class.toml` the per-class knobs as **anchor points** `(level, value)` per class and knob,
  interpolated in a straight line between anchors and held flat outside them. Until the final
  values arrive, the 7.1 bands are written as anchors that reproduce them exactly at whole
  levels (`(1,v1),(29,v1),(30,v2),(39,v2),(40,v3),(49,v3),(50,v4)`), with a comment saying they
  are the interim values; a class not listed is 100. Nothing in the code assumes four bands.
- **The opener** (5.4): on 1.20 a stab costs 150% of the same character's normal blow's energy,
  at most the round's 1000, and the round goes on with normal blows: the opener estimate is the
  stab plus `(1000 − stab energy) / normal energy` normal blows. The stab's damage uses
  `stealth / 4` and `× (100 + 2 × L)%` with L held to 60, and can crit (max to 2 × max).
- **Openings** (5.5): a Thief's expected extra damage per normal hit on a surviving target is
  10% of the opening formula, L held to 60, minus target DR / 10, never critting.
- **Weaves** (5.6): the expected weave damage per normal hit for a weaving class, from its pick
  (the known single-target attack spell with the best volley, C-066's 1.20 view), its budget
  `3 × L` (L held to 60), power 25-100%, half the casting-stat bonus, `class_spell_pct` and
  `class_weave_pct`; the Bard's as one hit of its budget.
- **Knees** (5.8, 5.9): crit past 50 counts a third (1.11p's knee stays 40); players' MR stat part
  past 110 and dodge Agility past 150 count a third. Monsters unchanged.
- **Casting stat** (6.3): player damage spells × `(1000 + max(0, stat' − 50) × 5) / 1000`, stat'
  with the knee at 100; heals and buffs unchanged.
- **Area resists** (5.7): an area spell's expected damage on a monster of MR is its share ×
  (1 − P(resist) × min(25, MR/4)/100), with the stock resist chance (capped at 98%).
- **The MR fix** (5.10): a player under 50 MR takes `amount + amount × (50 − MR)/100` from ability
  17 spells; the danger estimate of a caster monster uses it.
- **Race regen** (6.7): Human +15 healing and +15 mana regen, Dwarf +15 healing, Elf +15 mana,
  Gaunt One +25 mana, summed with the regen tier (6.10), in `regen.rs`'s 1.20 rates.
- **Class knobs** (6.2): `class_melee_pct` scales the min and max of normal blows (not the stab),
  `class_spell_pct` damage spells and weaves, `class_weave_pct` weaves on top.
- **Stats above the race maximum** are normal on 1.20 (6.11); no model caps a stat there. The bot
  still never trains stats.
- **Gear plan** (C-059): on 1.20 its scores use the above; the planner view says "1.20 rules" in
  its header when they apply.

## Design notes

- One `Balance` value (loaded from the TOML, embedded) passed into the model next to the rule set;
  1.11p's equivalents are a `Balance::stock()` so the same code paths run with 1.11p's numbers
  where that is exact, and keep today's code where it isn't.
- Interpolation: integer arithmetic, rounding down. The interim 1.20.0 values fall with level for
  some classes (Paladin 95 → 71 → 88), so the loader accepts any anchors; the test that the
  shipped anchors never fall comes with the final values.
- When the final class values arrive, they replace `class.toml` in a follow-up PR; nothing else
  should need to change.

## Done when

- On 1.11p every existing combat-model and gear-plan test gives the same numbers.
- On 1.20, the handoff's worked numbers come out: a 400-energy normal blow gives a 600 stab and
  one more blow; a Warrior's guard chance is 45%; the casting stat gives +25% at 100, +30% at 130,
  +50.5% at 255; crit 80 counts as 60; MR 0 takes 150% of an armour-piercing nuke, MR 30 120%.
- The class knob lookup reproduces the 7.1 table at L1, 29, 30, 39, 40, 49, 50, 60.
- KB "Numbers" and "Models" sections ([Q]), noting the class tables are interim; changelog.
- fmt, clippy and nextest pass.

## Tests

- Each formula above, at the handoff's examples and at L60/L61 (the scaling cap) and 65.
- The anchor interpolation, flat outside anchors, and between two anchors a level apart.
- The opener estimate per rule set; the gear plan per rule set for one Thief and one Warlock.

## Notes for the agent

- The band targets of 7.2 are a check on the model, not an input: a test may compare the model's
  steady-state ratio for a class against its band loosely, but only if the model is close enough
  to make it meaningful; otherwise leave it out and say so.
- "Where next?" isn't built yet; when it is, it reads these models.
