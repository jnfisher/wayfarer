---
id: C-013
title: Web UI: drag the boundaries between tiles to resize them; the layout is remembered
tier: C
size: M
wave: 2
depends: [C-012]
crate: ui/
touches:
  - ui/src/App.svelte, ui/src/lib/layout.ts, ui/tests/layout.test.ts
  - ui/src/lib/keys.ts (the Layout rows), docs/web-ui.md
---

## Goal

In the tile view the player can drag the boundary between two columns or two rows to give one
tile more room, and the page remembers it. Today every tile gets an equal share
(`repeat(n, 1fr)`); after this card each column and row has a weight the player sets by
dragging, kept per grid shape in the browser (John, 2026-10-19).

## Inputs

- `docs/plan/05-ui.md` "Views" (the tile grid shapes), "Fonts and scaling" (80 columns always;
  the font fits the tile), and the note that layouts can be saved and switched.
- `docs/plan/04-settings.md`: settings are declared once and persisted by the core. The
  browser copy made here is the stopgap until that machinery exists (M4); write it so the
  stored shape (weights per grid shape) can move into a setting unchanged.
- `ui/src/App.svelte` (the grid), `ui/src/lib/layout.ts` (`tileGrid`), `ui/src/Tile.svelte`
  (`refit` already follows the tile's size through a `ResizeObserver`).

## Behaviour

- **Handles.** Between adjacent columns and adjacent rows a handle a few pixels wide (the
  present 2 px gap grows to about 6 px, drawn in the frame colour, cursor `col-resize` or
  `row-resize`). Dragging it moves that boundary; the two neighbours change, the others keep
  their size. Touch works too (pointer events).
- **Limits.** No tile narrower than an 80-column terminal at the minimum font size
  (`fitFontSize`'s 6 px, whose cells are 3 px wide: 240 px, plus the frame) or shorter than
  a few rows; a drag stops at the limit.
- **Reset.** Double-clicking a handle resets that axis to equal shares; a "reset layout" row
  in the help overlay names the key for resetting both (`Alt+0`, my call, rebindable later).
- **Remembered.** Weights are kept per grid shape (`1x2`, `2x2`, `2x3`, ...) in
  `localStorage` under one key (`wayfarer.layout`), read on load and written after a drag.
  Every read and write is in `try`/`catch`; with storage unavailable the layout still works
  and is simply forgotten on reload. Adding or removing a character changes the shape, so
  the shape's own weights apply.
- **Resizing the window** keeps the weights (they are proportions, not pixels); tiles refit
  their font as now.

## Design notes

- `layout.ts` grows a pure `Weights` model: `{ cols: number[], rows: number[] }` per shape,
  `drag(weights, axis, index, deltaPx, totalPx, minPx)` returning new weights, and
  `toTemplate(weights)` giving the `grid-template-*` strings (`1.4fr 0.6fr`). Keep the DOM
  out of it so it is unit-tested.
- The grid stays CSS grid; handles are absolutely positioned elements over the gaps, or
  explicit gutter tracks (`1.4fr 6px 0.6fr`). Pick the one that keeps `Tile`'s refit simple.
- The stored form: `{ "2x2": { "cols": [1.4, 0.6], "rows": [1, 1] } }`, versioned with a
  `v: 1` field so a later migration into settings is mechanical.

## Done when

- Two tiles side by side: dragging the boundary makes one wider and the other narrower; the
  terminals refit (80 columns still, font sizes differ); reloading the page restores it.
- Four tiles: a column drag moves both tiles in the column; a row drag likewise.
- The minimum stops a drag; double-click resets the axis; `Alt+0` resets both.
- With `localStorage` throwing (simulate in a test), nothing breaks.
- `npm run check`, `npm test` and the UI workflow are green; `docs/web-ui.md` describes it
  and the keys test from C-012 still passes.

## Tests

- `layout.test.ts`: `drag` arithmetic (limits, neighbours only), `toTemplate`, storage
  round-trip with a fake storage, and the throwing-storage case.

## Notes for the agent

- Do not write the layout to `wayfarer.toml` from the UI: the gateway owns that file and has
  no message for settings yet. Say in `docs/web-ui.md` that the layout is per browser until
  settings arrive.
- Keep the handle out of the way of xterm's scrollbar and the tile's click-to-focus.
