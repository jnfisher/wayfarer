---
id: C-110
title: "Collect counts the keys on the key ring"
tier: A
size: S
wave: 16
depends: [C-102]
crate: wayfarer-brain
touches:
  - crates/wayfarer-parse/src/typed.rs ("3 black star keys" is three black star keys)
  - crates/wayfarer-brain/src/character.rs, modes.rs, session.rs (the key ring counts as carried)
  - crates/wayfarer-parse/tests/typed.rs, crates/wayfarer-brain/tests/loot.rs
  - docs/kb/screen-parsing.md, docs/kb/changelog.md
---

## Goal

Rocco collects "black star key (up to 1)" and picked up every one he saw: he has three
(John, 2026-11-17: "seems like rocco is grabbing too many black star keys"). A key taken goes
on the key ring, which `i` lists on its own line (`You have the following keys:  glowing red
amulet, 3 black star keys.`), not among the carried items that Collect counted, so the count
was always 0. The ring also writes copies as "3 black star keys", which matched no item name,
so a route through a key door missed a key the character held two or more of.

## Behaviour

- The key list is read one entry a copy: "3 black star keys" is three black star keys (the
  count first, an "s" added to the name, LJNDISP.C `display_users_keys`).
- Collect counts the keys on the ring with the carried items: at the count, a key on the floor
  is left.
- KB: `screen-parsing.md` "Loot and money".

## Done when

- With "2 black star keys" on the ring and 2 to collect, a black star key on the floor is left.
- The key list parses "3 black star keys" as three "black star key".
