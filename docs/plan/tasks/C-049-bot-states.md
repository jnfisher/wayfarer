---
id: C-049
title: "The bot's states: on or off, and a route; Go to and walk-once are the bot's journeys, ■ drops the route"
tier: B
size: M
wave: 7
depends: [C-048]
crate: wayfarer-brain, wayfarer-proto, ui
touches:
  - crates/wayfarer-brain/src/{journey.rs (new),modes.rs,session.rs,walker.rs}
  - crates/wayfarer-proto/src/lib.rs (`Bot.journey`, `Journey`)
  - ui/src/lib/{bot.ts,keys.ts}
  - tests/live/src/{main.rs,soak.rs}; tests/fixtures/replay/{walk-2,fight-2}/
  - docs/brain.md "The bot's states"; docs/web-ui.md "The bot"; docs/protocol.md
---

## Goal

A Go to and a loop are both things the player starts, so the bot treats them alike (John,
2026-10-23). The bot is on or off, and it may have a route (a loop, or a journey: a Go to or a
loop walked once). `║`/`►` turn it off and on and keep the route; `■` (and the map's `stop`)
drops the route and leaves it on or off. A Go to or a loop turns it on from any state; a Go
to arriving leaves it on there. Of the lines the player types, only a move changes what the
bot does: it drops the route and the bot plays where the character goes.

## Behaviour

| State | `║` / `►` | `■`, the map's `stop` |
|---|---|---|
| On, with a route | paused, the route kept | the route dropped, on where it is |
| On, no route | off | off |
| Off, with a route (paused) | on, going on with the route (a re-sync first) | off, the route dropped |
| Off, no route | on where it stands | nothing |

- `walk_to` and `walk_loop` give the bot a journey (`Journey`), in Task's place in the mode
  stack: it fights on the way, walks on after, rests first when low; its moves are not
  pipelined. At the end it plays there, that room its home. The walker giving the walk up
  (lost, a door) or finding no route ends it: "Not going to X: the walk stopped; playing here".
- A typed move (or keys) with the bot on drops the route; the room the character goes to
  becomes home. Other typed lines (acts, attacks) go between the bot's actions and pause
  nothing.
- `F9` is `║`/`►` (and starts a bot that is off); `F10` is `■` (with no route it turns the bot
  off, or on). The tile shows `[■]` only with a route.

## Done when

- In-process: pause on a Go to keeps it and resume walks on; `■` while paused drops it and the
  bot stays off; a Go to with the bot off turns it on; `■` on a loop leaves the bot on; pause
  with no route turns it off; a typed move keeps the bot on; the walker tests run with the
  bot on.
- Live: walk, fight, typing, goto, loop, restart, flee-from, hidden, under-attack and brain
  pass; the replay fixtures re-recorded (`walk-2`, `fight-2`).
