---
id: C-126
title: "A tile's scrollbar no longer shows through the full-screen views"
tier: A
size: S
wave: 18
depends: [C-125]
crate: ui
touches:
  - ui/src/Tile.svelte (.term)
---

## Goal

John: "when there's a full screen view (like the stat zoom) I can see the scroll handle (the
thing you drag on the character tile for backscroll) effect for each time pulsing through it"
(2026-11-20, v0.1.36).

xterm's scrollbar sits at z-index 11, and the terminal formed no stacking context of its own.
So the scrollbar stacked against the page's layers, above the gear planner and the combat zoom
(z-index 5). It fades in on new output, so each tile's scrollbar pulsed through the overlay.

## Behaviour

- The terminal's host isolates its stacking (`isolation: isolate`). xterm's layers then stack
  inside the terminal, under the full-screen views and the help.

## Done when

- Checked in a browser: an xterm with scrollback, still writing, under a black z-index 5
  overlay. That left 462 non-black pixels before the change and 0 after.
- CI green.
