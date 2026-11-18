---
id: C-115
title: "Boss fight mode on or off for the whole party at once"
tier: A
size: S
wave: 16
depends: [C-100]
crate: ui
touches:
  - ui/src/Tile.svelte (the skull sets the whole party; Shift+click one)
  - ui/src/App.svelte (Alt+K)
  - ui/src/lib/parties.ts (partySessions), ui/src/lib/bot.ts (the title)
  - ui/src/lib/keys.ts, docs/web-ui.md
  - ui/tests/parties.test.ts
---

## Goal

A party goes into a boss fight together: turning boss fight mode on tile by tile, six times,
is slow, and a member left out flees or walks off mid-fight.

## Behaviour

- In a party, the boss button on any member's tile turns boss fight mode on or off for every
  member, as that tile's state goes (off there: on for all; on there: off for all).
- `Alt+K` does the same for the focused tile's party. Out of a party, both set the one character.
- `Shift`+click sets that character alone.

## Done when

- `partySessions` gives every member, leader first, from any member; just the character out of
  a party.
- The key is in the help table and docs/web-ui.md.
