---
id: C-112
title: "An entry during the catch-up puts the bot back after it"
tier: A
size: S
wave: 16
depends: [C-107]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/session.rs (the entry waits for the catch-up; no hang-up on it)
  - crates/wayfarer-brain/tests/store.rs
  - docs/brain.md
---

## Goal

The v0.1.28 update (2026-11-17 17:50) restarted wayfarer. Penny entered the realm while the
new brain was still reading her session's old output; C-107 put her bot back on at the entry,
so the bot ran on that old output as it was read, and a low-HP fight in it hung her up at "2%"
with 311/357 HP, leaving Lexi alone among bugbears again. The resumes at 17:05 and 17:50
named rooms from that old output ("Temple, Halls of the Dead").

## Behaviour

- An entry while the brain catches up on the output puts the bot back (or starts it) when
  the catch-up ends, as a brain restart's resume does, narrated as at an entry. The fights of
  the old output are the old connection's: nothing attacks the character after it.
- The hang-up line is never checked during a catch-up.

## Done when

- A brain whose catch-up holds a fight at 5% HP, with the entry coming during it, does not
  hang up and resumes the stored loop.
