---
id: C-123
title: "Combat numbers by attack: each attack's hit, crit, miss and resist rates"
tier: A
size: S
wave: 18
depends: [C-050]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/combat_stats.rs (Attack, Hist, KnownSpell)
  - crates/wayfarer-brain/src/session.rs (sent lines, the spells screen)
  - crates/wayfarer-proto/src/lib.rs (AttackRow, CombatStats.attacks)
  - ui/src/lib/stats.ts, ui/src/StatsPanel.svelte
  - docs/brain.md "Combat statistics", docs/web-ui.md, docs/protocol.md
---

## Goal

The combat numbers count misses and crits for all blows together, so there is no backstab hit
rate, no backstab miss rate and no resist rate per spell. John: "attack events should have
min/max/avg/miss values so a 'bs' has hit %, miss %, crit %, etc; casts could have a resist
counted as a 'miss'" (2026-11-20). He also wants each spell's damage per mana, and rates on
few tries marked as such.

## Behaviour

- Each attack (the swing, the backstab, each attack spell by name) counts its hits (crits
  among them), misses (dodges among them), glances and resists, with the hits' damage.
- A miss or glance is the backstab's when a `bs` sent within 10 s waits for its first blow. A
  plain hit while one waits means the stab became a plain attack.
- A cast with damage is a hit of its spell. `cast_fail` in a fight is a miss of the named
  spell when it is an attack spell. `cast_resisted` is a miss of the attack spell sent within
  10 s.
- Each spell's mana is the `spells` screen's. Damage per mana is the hits' damage over the
  mana of every hit and miss.
- Every value of round damage, of every hit and of each attack's hits is counted, for C-125's
  histograms. All of it is kept in the store with the rest and zeroed by the combat reset.
- The stats pane shows a "by attack" table with tries, hit %, crit %, miss %, min-max, average
  and a note (dodges, glances, damage per mana). A row with fewer than 30 tries is dim.

## Done when

- Unit tests for backstab outcomes, spell resists and damage per mana, and the histograms.
- A UI test for the table and the dim rows.
- Docs say so, CI green.
