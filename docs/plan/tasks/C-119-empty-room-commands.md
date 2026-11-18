---
id: C-119
title: "A room command that needs an empty room says so when refused"
tier: A
size: S
wave: 16
depends: [C-052]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/nav.rs (room_commands: nomonsters; Edge::needs_empty)
  - crates/wayfarer-world/src/route.rs (Step::needs_empty)
  - crates/wayfarer-brain/src/walker.rs (the refusal stops the walk with its reason)
  - docs/brain.md
---

## Goal

The way into the Darkened Tunnels is `push panel` in the Black Fortress's Dark Chamber
(7:1283), a room command the game takes only with no monster in the room (`nomonsters`). The
router already took it (C-052); refused, the walker pushed twice and stopped saying it had
"lost the way".

## Behaviour

- A room command with `nomonsters` before another clause is marked as needing an empty room
  (as the last clause the game never looks).
- Refused (the look after it shows the same room), the walk stops: "`push panel` works only with
  no monster in Dark Chamber". With fighting on the way the bot clears the room before it.

## Done when

- The panel routes from the Dark Chamber into the Tunnels and is marked; pushed, the walk
  arrives; refused, it stops saying why.
