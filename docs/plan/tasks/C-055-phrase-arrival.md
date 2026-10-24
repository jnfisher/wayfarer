---
id: C-055
title: "A phrase's way through is looked at, not waited for: `ring chime` prints no room"
tier: B
size: S
wave: 8
depends: [C-052]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/walker.rs (`Doing::Arrive`)
  - crates/wayfarer-brain/tests/goto.rs; docs/brain.md
---

## Goal

The walker knows where a room command or a MegaMud crossing took the character. (John,
2026-10-24: "it worked but the bot kept attempting to ring it rather than recognizing it got
moved".)

## Inputs

- Penny and Manuel, 2026-10-24 15:08, walking back to Faye: `ring chime` printed "You ring the
  chime... You find yourself...elsewhere." and no room; the walker waited for one, stopped on
  "no answer to `ring chime`", believed itself still on Slum Street, and walked back again every
  5 s, saying "ring chime" in the Entrance 26 times.

## Behaviour

- A room command or a crossing is sent expecting its prompt, then `look`; the room shown is
  checked as a move's room (arrived at the step's room, or a drift: plan again). A phrase that
  was only speech there comes to the same: the look relocates, and the walk plans again.

## Tests

- `goto.rs`: the chime with no room shown, then `look`; the chime as speech in the wrong room.
