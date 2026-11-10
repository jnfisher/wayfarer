---
id: C-087
title: "The walk bar stays through a Go to: between legs, while the bot fights or plans, with its stop button"
tier: C
size: S
wave: 16
depends: []
crate: ui
touches:
  - ui/src/lib/map.ts (`walkBar`)
  - ui/src/MapPanel.svelte (the walk bar)
  - ui/tests/map.test.ts, ui/tests/map-panel.test.ts
  - docs/web-ui.md "Walking"
---

## Goal

John (2026-11-10): the bar under the map shows a Go to's destination, step count and
directions, then disappears when the bot stops to fight, or when it re-routes. It should stay,
and so should its stop button, which keeps doing what it does.

The bar follows `character.walking`, the walker's leg. A Go to (`bot.journey`) is made of legs.
The bot ends the leg to fight and starts a new one afterwards, and a replan after a stop is a
new leg, so between legs there was no bar.

## Behaviour

- While the bot is not stopped and has a `journey`, the bar shows:
  - the destination;
  - the last leg seen for this session, its step and steps left, and its commands, greyed;
  - what the bot is doing instead, in yellow (its narration, "Fight: giant rat"; "planning
    the next leg" when it says nothing);
  - **stop**.
- **stop** sends `walk_stop` as before. With a journey the brain stops the bot and drops the Go
  to (`Session::walk_stop`), whether or not a leg is on. No brain change.
- Once the journey is over, or the bot is stopped, the bar goes back to "Click a room to walk
  there."

## Done when

- `map.test.ts`: `walkBar` is the leg on; between legs it is the last leg with the bot's
  narration; without a journey, or with the bot stopped, it is nothing.
- `map-panel.test.ts`: a leg, then the bot fighting with no leg: the bar, the steps, the route
  and the narration stay, and stop sends `walk_stop`. With the journey over, the bar goes.
- `npm run check` and `npm test` pass.
