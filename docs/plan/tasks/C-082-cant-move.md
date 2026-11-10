---
id: C-082
title: "Knocked flat or stunned: a refused move stops the leg, not the Go to"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-parse, wayfarer-brain
touches:
  - crates/wayfarer-parse/src/typed.rs (Refusal::CantMove)
  - crates/wayfarer-brain/src/walker.rs, session.rs (`Next::held`)
  - crates/wayfarer-parse/tests/typed.rs, crates/wayfarer-brain/tests/walk.rs
  - docs/kb/screen-patterns.md, docs/kb/changelog.md
---

## Goal

A Go to whose move is refused because the character can't move for now carries on once it can.
That covers being knocked flat, held or stunned. On 2026-11-10 at 15:10, Lexi's passive Go to to
Markus (2/2518, 309 steps) was on the Southwestern Road in the Dragon's Teeth when an ankheg
charged. "You are knocked off your feet ... You are flat on your back!" The next `n` was answered,
about a second later, with "You can't seem to move anywhere!". No refusal was mapped to that
line, so the walker heard no answer and stopped ("no answer to `n`"). That ended the journey
("the walk stopped; playing here"), and the party waited there.

## Behaviour

- `cant_move` ("You can't seem to move anywhere!") and `stunned` ("You are too stunned to move
  anywhere!") are a move refusal, `Refusal::CantMove`. Both come only from `move_user`
  (`kb/grouping.md` "Can't move").
- The walker stops the walk with "`n` refused: can't move for now (knocked down, held or
  stunned)". It doesn't block the exit or plan again.
- The session counts the stop as halted, like a walk stopped for a fight (`Next::held`), not as
  failed. A journey therefore begins the leg again from where the character is, under its usual
  `LEG_TRIES`, which an attack resets.

## Done when

- The parse test maps both texts to `MoveRefused { CantMove }`.
- `walk.rs`: a Go to whose first move is refused that way walks on and arrives.
- KB: the live sighting goes on `screen-patterns.md` `cant_move`, and a line in `changelog.md`.
