---
id: C-124
title: "Combat numbers for the blows taken and for each monster fought"
tier: A
size: S
wave: 18
depends: [C-123]
crate: wayfarer-brain
touches:
  - crates/wayfarer-parse/src/typed.rs (Blow, AttackedBy.blow)
  - crates/wayfarer-brain/src/combat_stats.rs (Taken, Foe, foe_event)
  - crates/wayfarer-brain/src/session.rs (count_combat)
  - crates/wayfarer-proto/src/lib.rs (TakenRow, FoeRow)
  - ui/src/lib/stats.ts, ui/src/StatsPanel.svelte
  - docs/brain.md "Combat statistics", docs/web-ui.md, docs/protocol.md
---

## Goal

The combat numbers only count the character's own blows. Nothing says how often monsters hit
it, so there is no way to see whether protection from evil or a new armour helps. Nothing
says what each kind of monster costs and pays: rounds to kill it, damage each way, exp per
kill. John agreed to both on 2026-11-20.

## Behaviour

- `AttackedBy` carries how a monster's blow came out: hit (with damage), miss, dodged, or
  deflected (`mon_hits_you`, `mon_miss_you`, `mon_dodge_you`, `mon_deflect_you`, and their
  built-in forms).
- The combat numbers count the blows taken: hits with their damage, misses, dodges and
  deflections. Every value of the damage is counted too, for C-125's histograms.
- Each kind of monster, by name without adjectives, counts:
  - the character's damage to it;
  - the rounds with a blow or cast at it;
  - its blows at the character and their damage;
  - its deaths in the character's fight (one of the death text's monsters struck in the last
    60 s);
  - the exp line within 5 s after each death.
- The stats pane shows a line for the blows taken. The per-monster table is in C-125's view.

## Done when

- Parser tests for the blow outcomes.
- Unit tests for blows taken, and for a monster's rounds, death and exp.
- A UI test for the taken line.
- Docs say so, CI green.
