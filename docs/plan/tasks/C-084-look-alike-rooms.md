---
id: C-084
title: "Walking among look-alike rooms: vote the first step, and a surprise while unsure is not being lost"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/walker.rs (`origin`, `MAX_UNSURE_MOVES`)
  - crates/wayfarer-brain/tests/game/mod.rs (`Game::map_at`: the fake game walks the map)
  - crates/wayfarer-brain/tests/walk.rs
  - docs/brain.md "Look-alike rooms"
---

## Goal

A Go to through the Dragon's Teeth Hills gets there. On 2026-11-10, from 16:07, Lexi's Go to to
Markus stopped six times in five minutes with "lost the way: expected Dragon's Teeth Hills, the
room was Dragon's Teeth Hills". Each try:
1. planned from the lowest-numbered candidate;
2. its first move reached a room the route didn't expect ("Replanned");
3. its next move did the same, and the walk gave up.

The hills are 931 rooms named "Dragon's Teeth Hills" with only 54 exit layouts between them. The
rooms the game showed (2/191 → 2/189, then 2/189 → 2/186) fit only two or three places in the
pack, so the pack and the game agree. The belief was right to be unsure. The walker ignored that.

## Behaviour

- `origin` (each replan): when the belief is certain, its room.
  - If the step's `from` is still a candidate (a refused move, a phrase that was only speech),
    it plans from there.
  - Otherwise the walker routes (`route_within`, avoiding a boss lair it stops short of) from
    each of up to 16 candidates, groups them by the route's first command, and plans from the
    first candidate of the largest group.
  - A walk's first plan stays the believed room, so existing walks don't change. A wrong guess
    there is put right by the rule below.
- A move into a room the route didn't expect, while the belief after the move still holds
  several candidates:
  - plans again ("Replanned (not sure which Dragon's Teeth Hills)");
  - doesn't count as a surprise, nor against `MAX_REPLANS`;
  - is allowed up to `MAX_UNSURE_MOVES` (20) times a walk.

  With a certain belief the old rule stands: the second surprise is "lost the way".
- Test support: `Game::map_at`. A direction the script has no line for moves the character
  along the pack's exit from that room and answers with the room it reaches, so a test can walk
  the hills without scripting every move.

## Done when

- `walk.rs`: from 2/651 (one of 32 rooms of its name and layout, not the lowest), a Go to to
  2/565 (20 steps) arrives. With the old walker this test fails with Lexi's message.
