---
id: C-118
title: "A sealed hidden exit is opened by going to its levers elsewhere first"
tier: A
size: M
wave: 16
depends: [C-077]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/nav.rs (Edge::levers, Lever, find_far_levers)
  - crates/wayfarer-world/src/route.rs (Traveller::no_levers, LEVER_TOUR_COST, the lever tour)
  - crates/wayfarer-brain/src/session.rs (loops take no lever tours)
  - docs/brain.md, docs/kb/navigation.md
---

## Goal

Some hidden passages open only when levers in other rooms are pulled: the Darkened Tunnels'
passage north of 15:477 has two blocks, taken off by `pull lever` in 15:730 and 15:604, far off
in the maze; the Crypt's chamber is the same. C-077 pulls a door's lever a few steps away; a
sealed hidden exit was walked only through a MegaMud path window.

## Behaviour

- A sealed Hidden exit whose every block a Remote Action of another room takes off (no item
  needed) carries those levers, in the game's order, or any order.
- A walk's route through it goes from the exit's room to each lever, types the phrase, and back,
  then through. In any order the lever nearest the exit goes last: the passage closes again one
  exit step after its last block comes off.
- It costs 1000 steps, so it is taken only when there is no other way; a path window wins.
- Loops never take it.

## Done when

- The Tunnels' passage carries the two levers; a route through it pulls both and goes north.
- The Crypt without its path window is reached by the tour; with it, by the path.
