---
id: C-050
title: "Combat statistics: hits, misses, crits, backstabs, casts and rounds on the stats panel, kept until the player resets them"
tier: B
size: M
wave: 8
depends: [C-029, C-048]
crate: wayfarer-parse, wayfarer-store, wayfarer-brain, wayfarer-proto, ui
touches:
  - crates/wayfarer-parse/src/typed.rs (`YouHit` keeps `crit` and `surprise`)
  - crates/wayfarer-store/src/lib.rs (migration 8: `combat_stats`)
  - crates/wayfarer-brain/src/{combat_stats.rs (new),stats.rs,session.rs}
  - crates/wayfarer-proto/src/lib.rs (`Stats.combat`, `CombatStats`, `DamageRow`)
  - ui/src/{StatsPanel.svelte,lib/stats.ts}
  - docs/brain.md "Statistics"; docs/web-ui.md "Stats"; docs/protocol.md
---

## Goal

The stats panel shows each character's combat numbers in the shape of MegaMud's Combat tab:
how often its blows miss, hit, crit and backstab, the damage of each (min, max, average), its
attack spells, and the damage it does in a round. The numbers build up over every fight,
across runs, stops and brain restarts, until the player presses the panel's reset. (Added
2026-10-24 at John's request: "reset when I change weapons or settings, start scripting, or
after a level up".)

## Inputs

- MegaMud's Combat tab (`MegaRes.dll` dialog 383, read on 2026-10-24): rows Miss, Hit, Extra,
  Crit, BS, Pre, Cast and Round, with count, Min-Max and Avg columns; then Killed, Had to run,
  Health low and Mana low; Copy and Reset buttons. `docs/kb/megamud-settings.md` §4 names the
  dialog; the layout is not in the KB yet (see "Notes").
- Patterns: `you_hit` already captures `crit` (`critically`) and `surprise` (a backstab):
  `You critically beat filthbug for 34 damage!` (`tests/fixtures/transcripts/kasimir-early.txt`).
  `you_miss`, `you_dodged`, `you_glance`; `you_cast_generic` with `dmg` (the typed
  `SpellCast`); `cast_fail`, `cast_resisted`, `spell_fails`.
- `docs/kb/combat.md` "The round": a round every 5 s. In John's logs (Penny, 2026-10-23) one
  round's blows arrive together and rounds are 5.00 s apart.
- C-029 (`stats.rs`, the `Ledger`, `stats_reset`), C-048 (`bs` as the opening attack).

## Behaviour

- **What counts**: only the character's own lines (`You …`), whoever started the fight,
  whether the bot or the player was driving. A party member's blows count on its own panel.
- **Rows** (each with a count and its share of swings; the damage rows also min, max and
  average):
  - *Swings*: every own melee attempt: hits, crits, backstabs, glances, misses and dodges.
  - *Hit*: `you_hit` without `crit` or `surprise`.
  - *Crit*: `you_hit` with `crit` and without `surprise`.
  - *BS*: `you_hit` with `surprise` (a backstab that also crits counts here, and in a crit
    count of its own shown as "of which crit"). Attempts are the `bs` / `backstab` commands
    sent (by the bot or typed); BS accuracy is landed / attempts.
  - *Glance*: `you_glance` (landed, no damage). *Miss*: `you_miss` and `you_dodged` together,
    with the dodges shown in brackets.
  - *Cast*: `SpellCast` with damage, per cast; failed casts (`cast_fail`, `cast_resisted`,
    `spell_fails` after an attack cast) count as cast misses, giving cast accuracy.
  - *Round*: the total damage the character did in one round (blows and spells); min, max,
    average, and the number of rounds.
  - *Accuracy*: landed (hit + crit + BS + glance) / swings. *Crit rate*: crits / landed.
- **Rounds**: a round starts with the character's first damaging or missing line and takes in
  every own line within 2.5 s of it (half the 5 s round); the next line after that starts the
  next round. A round with only misses counts, with 0 damage.
- **Monsters** (as MegaMud): killed, times fled, lowest HP and lowest mana seen during the
  period (a caster's lowest mana only).
- **Since**: the time of the last reset, shown as "combat since 10:42 (3h 12m)".
- **Reset**: the panel's existing reset button resets everything: it ends the run (as now)
  and zeroes the combat numbers. It shows whenever a brain is attached, run or not. With no
  run it zeroes the combat numbers only, and the notice says so. Nothing else resets them: not
  Stop, Start, a death, a level-up, a brain restart or a gateway restart.
- **Persistence**: the tally lives in the store, one row per session, saved at most every
  few seconds while it changes and when the session goes away, so a brain restart loses at
  most the last few seconds.
- **On the wire**: `stats` gets `combat` (absent when nothing has been counted). The snapshot
  does not carry it; the panel asks with `stats_request` as now (and on its refresh).

## Design notes

- `typed.rs`: `YouHit { target, damage, crit: bool, backstab: bool }` (from the groups'
  presence). Existing matches on `YouHit { target, damage }` get `..`.
- `combat_stats.rs`: `CombatTally` with a `Damage { count, min, max, sum }` per kind
  (`hit`, `crit`, `backstab`, `cast`, `round`), counters for misses, dodges, glances,
  backstab attempts, backstab crits, cast misses, kills, flees, lowest HP and mana, `since`
  (unix seconds), and the open round (its start `Instant` and damage so far). `record(&GameEvent,
  now)`, `sent_backstab()`, `close_round_if_due(now)`, serde to and from JSON for the store,
  `to_wire()`. The round closes lazily (on the next line, a stats request, or the save), so it
  needs no timer of its own.
- Store: migration 8, `combat_stats (session TEXT PRIMARY KEY, tally TEXT NOT NULL,
  updated_at INTEGER NOT NULL)`, with `save_combat_stats` and `combat_stats(session)`.
- `Ledger` (or `Session`) owns the tally; `count()` feeds it; `stats_reset` zeroes it
  whatever the run; kills and flees come from the same places the run counts them.
- Proto: `CombatStats` with `since`, `swings`, `accuracy` (per cent, one decimal as an
  integer tenth or an `f64`, the builder's call), `crit_rate`, the `DamageRow`s, the counters
  and the monsters block. Regenerate the TypeScript.
- UI: a "Combat" section under the current run: a small table (row, count, %, min–max, avg)
  and a line for the monsters block. Rows with nothing counted are left out. No new button:
  the reset beside "run since" moves to a header that is always there, its title saying it
  resets the run and the combat numbers.
- Leave out MegaMud's *Extra*, *Pre* and *Multi* rows (wayfarer has no pre-attack spells or
  weapon procs yet) and *Copy*.

## Done when

- A fight replayed in-process (hits, a crit, a backstab, a miss, a dodge, a cast) gives the
  right counts, min, max, averages, rounds and accuracy.
- Combat numbers survive Stop, Start and a brain restart (store round trip); reset zeroes them
  with and without a run.
- The stats panel shows the combat table on John's rig after a few fights.

## Tests

- `typed.rs`: `you_hit` with `critically`, with `surprise`, with both.
- `combat_stats.rs`: per-kind damage, rounds grouped by time (two blows 0 s apart are one
  round, 5 s apart two), a miss-only round, accuracy and crit rate, reset.
- Store: save and load, migration from version 7.
- Brain: `stats_reset` with no run zeroes combat and says so; `stats` carries `combat`.
- `ui/tests/stats.test.ts`: the table's rows and formatting; empty rows left out.

## Notes for the agent

- KB: the Combat tab's layout above is a MegaMud fact the KB lacks. Add it to
  `docs/kb/megamud-settings.md` §4 ([M], from `MegaRes.dll` dialog 383) with a changelog line.
- Backstab misses: if the game prints a line for a missed backstab, find it in the world
  pack's game texts or the KB and count it; otherwise BS accuracy comes from attempts, as
  above.
