---
id: C-056
title: "Blessings are kept on in a fight: a buff that wore off is cast mid-fight, after a heal, on its own mana line"
tier: B
size: S
wave: 8
depends: [C-047]
crate: wayfarer-settings, wayfarer-brain
touches:
  - crates/wayfarer-settings/src/lib.rs (`spells.bless_in_combat`, `spells.bless_mana_above`)
  - crates/wayfarer-brain/src/{bless.rs,fight.rs,modes.rs}
  - crates/wayfarer-brain/tests/fight.rs; docs/brain.md
---

## Goal

A blessing the player ticked stays on through fights, not only between them. (John,
2026-10-24: "maintaining buffs should be a pretty strong priority"; "buffs don't cost rounds,
it's an instant spell".)

## Inputs

- C-047's Bless mode: it casts only when not engaged and nothing attacks, and only while the
  cast leaves mana at or above `health.rest_mana_below`, which C-051 made 0.
- `kb/spells.md`: every cast ends the caster's auto-combat first (`*Combat Off*`); general
  spells go once per 5-s round. Fight's heal in a fight (C-043) already casts mid-fight and
  attacks the same monster again at once, without a `look`.
- MegaMud: "Bless during combat" (`Other/BlessCombat`, off by default) and "Mana Bless If
  Above" (`Health/ManaBless%`, 80) (`kb/megamud-settings.md`, `kb/megamud-manual.md`).

## Behaviour

- **`spells.bless_in_combat`** ("Bless in a fight", on by default): in a fight, a kept
  blessing that is off is cast. Order in a fight: flee, then a heal (its own or a party
  member's), then the blessing, then the fight goes on. Off: as C-047, between fights only.
- After the cast the fight goes on at once with the same monster: the `*Combat Off*` the cast
  causes is the bot's own, as after a heal, so no `look` and no new pick; the attack goes
  out at the cast's prompt.
- **`spells.bless_mana_above`** ("Bless only above mana", 20%): a blessing is cast, in a fight
  or out of one, only while the cast leaves the mana at or above this per cent of the
  maximum. It replaces `health.rest_mana_below` as the blessing's line, so a 0 rest line no
  longer lets blessings spend the mana a heal needs.

## Done when

- A priest in a fight whose bless wore off casts `bles` and attacks the same monster again
  right after, without a `look`.
- Under its heal line in the same fight it heals first; the blessing comes after.
- With `bless_in_combat = false` nothing is cast in the fight; with mana under the line
  nothing is cast.

## Tests

- `wayfarer-settings`: the defaults.
- Brain (`tests/fight.rs`): the three cases above.
