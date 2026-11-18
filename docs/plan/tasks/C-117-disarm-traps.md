---
id: C-117
title: "A character with the traps skill disarms the traps on its route"
tier: A
size: S
wave: 16
depends: []
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/nav.rs (Edge::armed_trap)
  - crates/wayfarer-world/src/route.rs (Traveller::disarm, Costs::trap, Step::disarm)
  - crates/wayfarer-brain/src/walker.rs (Doing::Disarm), executor.rs (Expect::DisarmResult)
  - crates/wayfarer-brain/src/position.rs (the status screen's Traps), session.rs (the loop key)
  - docs/brain.md, docs/kb/navigation.md
---

## Goal

The Ancient Ruin and the Darkened Tunnels are full of traps that go off on every crossing.
A Missionary leading the party has the disarm-traps skill (119 at level 25) and could make
every one safe for the party; the walker walked into them instead.

## Behaviour

- An armed trap (Trap or Spell Trap, `exitpar2` kind 0 or 3) costs 10 steps on a route for a
  character without the skill, so a trap-free way of a few more steps wins.
- A walker with the skill (`Traps` above 0 on the status screen) sends `disarm trap <dir>`
  before the move, the trap costing one step. A failure is tried again, three tries in all,
  then the walk goes through.
- Followers move through behind the leader while the trap is disarmed.

## Done when

- The Demolished Doors' west trap: disarmed then walked with the skill; walked into without it.
- Three failures, then the walk goes through, narrated.
