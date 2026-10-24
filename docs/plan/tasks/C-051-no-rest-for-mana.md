---
id: C-051
title: "Rest for mana is off by default: `health.rest_mana_below` defaults to 0"
tier: C
size: S
wave: 8
depends: []
crate: wayfarer-settings, wayfarer-brain
touches:
  - crates/wayfarer-settings/src/lib.rs (`health.rest_mana_below`)
  - crates/wayfarer-brain/src/{modes.rs,bless.rs,session.rs} (the hard-coded fallbacks)
  - docs/brain.md
---

## Goal

A character never rests for mana unless the player sets a line: the default of "Rest for mana
below" is 0 for every class and style. (John, 2026-10-24: resting for mana is almost always
the wrong answer; tune the other settings so a caster does not need it, and set a line only
for room scripting.)

## Behaviour

- `health.rest_mana_below` defaults to 0 (it was 30). No style or class preset sets it.
- At 0 mana plays no part in resting: the bot never starts a rest for mana, and a rest started
  for HP ends on the HP lines alone. (Before this card every rest also waited for
  `health.rest_mana_until`, whatever started it; with a line above 0 that stays as it was.)
  The same goes for the party rest: a member at 0 is not waited for to get its mana back.
- Wherever the brain falls back to a literal when the setting is missing, the literal is 0.
- The help says "0 (the default) never rests for mana".
- Files that already set a value keep it.

## Done when

- `wayfarer settings` and the settings tab show 0 as the default.
- A caster with low mana and full HP does not rest; with the line set to 30 it does.
- A caster resting for HP gets up at the HP line with its mana still low.

## Tests

- `wayfarer-settings`: the default.
- Brain: a caster below 30% mana with the default does not rest; with `rest_mana_below = 30`
  it does (the existing tests that rely on 30 set it).
