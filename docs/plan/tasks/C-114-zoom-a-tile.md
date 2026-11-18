---
id: C-114
title: "Alt+Z zooms the focused tile to fill the window"
tier: A
size: S
wave: 16
depends: []
crate: ui
touches:
  - ui/src/App.svelte (the zoom: every other tile takes no cell)
  - ui/src/lib/minimized.ts (offGrid)
  - ui/src/lib/keys.ts, docs/web-ui.md (the key)
  - ui/tests/minimized.test.ts
---

## Goal

With six tiles, one character's screen is small; making its font larger only crops it. The
player wants to look at one tile at full size for a while and go back.

## Behaviour

- `Alt+Z` (Option on a Mac) zooms the focused tile to fill the grid, and again goes back. The
  other tiles stay mounted, as minimized ones do: their terminals keep running.
- While zoomed, `Alt+1` to `Alt+6` zoom that tile instead (the zoom follows the focus).
- Minimizing the zoomed tile ends the zoom. Minimized tiles stay in the dock.

## Done when

- `offGrid` takes the minimized tiles and, while zoomed, every other out of the grid; zooming
  on a minimized tile zooms nothing.
- The key is in the help table and in docs/web-ui.md.
