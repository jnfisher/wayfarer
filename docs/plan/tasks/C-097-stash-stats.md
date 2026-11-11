---
id: C-097
title: "The stats tab shows the coins stashed this run, and where"
tier: B
size: S
wave: 16
depends: [C-093]
crate: wayfarer-parse, wayfarer-store, wayfarer-proto, wayfarer-brain, ui
touches:
  - crates/wayfarer-parse/src/typed.rs (`CoinsHidden`)
  - crates/wayfarer-store/src/lib.rs (`EventKind::Stash`, `Store::stashed`)
  - crates/wayfarer-proto/src/lib.rs (`Stashed`, `stats.stashed`)
  - crates/wayfarer-brain/src/stats.rs, session.rs, modes.rs, stash.rs
  - ui/src/StatsPanel.svelte, ui/src/lib/stats.ts
  - docs/brain.md, docs/protocol.md, docs/web-ui.md, docs/kb/economy.md, docs/kb/changelog.md
---

## Goal

The player sees how much gold, silver and copper the bot's stashes hid this run, and in which
rooms, without reading the log.

John (2026-11-11): "track amount of gold/silver/copper (g/s/c) stashed and in what room(s) it
was stashed (e.g., "15:12" is fine) in a session under stats- maybe above the combat stats in
the window with 'per kill' statistics."

## Inputs

- The game confirms each hide: "You hid 76 gold crowns." (`hid_item`, `LJNCMDB.C:3855`; live on
  silver-mirror, 2026-11-11). A refused hide says something else ("You don't have 1859 gold
  crowns to hide!") and hides nothing.
- C-093's stash hides only in a room it is sure of, after counting the coins there.

## Behaviour

- The parser reads a `You hid <n> <coin>.` line as `CoinsHidden {n, coin}`; a hidden item stays
  `Other`.
- While the bot's stash is under way and has counted its coins (`Stash::hiding_in`), each
  `CoinsHidden` is a `stash` event of the run: the count, the coin's word, the stash's room. A
  hide the player types is not counted. The events are kept in the store with the run's other
  events, so a brain restart keeps the totals; a run reset starts them at zero.
- `stats` has `stashed`: `{room, gold, silver, copper}` by room (`map:room`), first stashed in
  first; absent when none.
- The stats tab shows, under where the time went and above the combat numbers, "stashed this
  run: 1,276g 40s 306c" and a line per room ("in 15:12 1,200g 40s 300c"), or "nothing stashed
  this run". No run, no section.

## Done when

- `typed.rs` `coins_hidden_are_counted_and_a_hidden_item_is_not`.
- `store.rs` `a_runs_stashes_sum_by_room_and_coin`.
- `task.rs`: the stash tests read `stashed` from `stats` (a refused hide counts nothing).
- `stats-panel.test.ts` "the coins stashed".
- fmt, clippy `-D warnings`, nextest, vitest, svelte-check.
