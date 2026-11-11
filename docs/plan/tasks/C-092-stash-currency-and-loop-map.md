---
id: C-092
title: "Stash currency at the loop's first stop, the party's too; the running loop on the map"
tier: C
size: M
wave: 16
depends: []
crate: wayfarer-brain, ui
touches:
  - crates/wayfarer-brain/src/stash.rs (new), crates/wayfarer-brain/src/modes.rs, task.rs, party.rs, session.rs
  - crates/wayfarer-settings/src/lib.rs (`cash.stash_on_loop`)
  - crates/wayfarer-proto/src/lib.rs (`BotMode::Stash`), ui/src/proto/BotMode.ts
  - ui/src/lib/map.ts (`loopBar`, the running stop), ui/src/MapPanel.svelte
  - docs/brain.md "Stash currency", docs/web-ui.md "The map"
---

## Goal

John (2026-11-10):

- "An option for stashing all silver/gold/copper at the start of a loop, and if the party
  leader has this option on then all characters in the group drop their silver/gold/copper at
  the beginning of the leader's loop … never stash platinum and runic … always stash at the
  first room in the loop. This allows us to collect all currency on the loop and later pick it
  up. Default to on." He chose `hide` over `drop`: hidden coins are not seen by the bot's own
  pickups.
- "A way to see where we're at in a loop in the map view."

## Behaviour

- `cash.stash_on_loop` (Cash, on). Each time a loop's Task arrives at its first stop, the bot
  hides its silver, gold and copper there: `hide <n> <coin>` for each it carries, then `i`. A
  new mode, `stash`, sits below Fight and above Task, so a fight there comes first. Platinum
  and runic are never hidden.
- A leader with it on publishes a stash round (`MemberInfo::stash`) while its loop is at the
  first stop. A follower with it (`PartyRole::Follower::stash`) hides its own once for each
  round, whatever its own setting.
- The map draws the running loop when none is hovered or picked in the list. The stop it is
  at or walking to is framed and numbered in light cyan, and unticked stops are grey. The bar
  under the map shows `↻ Loop <name>`, `stop n/N: <room>`, the leg's steps and commands, and
  **stop**.

## Done when

- `task.rs` `at_the_loops_first_stop_the_silver_gold_and_copper_are_hidden` and
  `with_stashing_off_nothing_is_hidden`.
- `party.rs` `at_the_leaders_first_stop_every_member_with_it_hides_its_coins`.
- `map.test.ts` "the loop bar (C-092)"; `map-panel.test.ts` "says where the running loop is in
  the bar".
- Rust: fmt, clippy `-D warnings`, nextest. UI: `npm run check`, `npm test`.
- Live: check that `hide 7 gold` hides coins on the server. The KB writes the coin form as
  `stash <n> <coin>`.
