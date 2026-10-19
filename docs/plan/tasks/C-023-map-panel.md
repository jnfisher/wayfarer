---
id: C-023
title: Web UI map panel: rooms around the character on a canvas, the belief, click a room to walk
tier: B
size: L
wave: 3
depends: [C-022]
crate: ui/
touches:
  - ui/src/MapPanel.svelte (new), ui/src/lib/map.ts (new: drawing from a `map_view`)
  - ui/src/App.svelte (the right drawer holds `map` and `dev`), ui/src/lib/{gateway.ts,state.ts,keys.ts}
  - docs/web-ui.md, docs/images/
---

## Goal

A map beside the tiles, in the old-school look: rooms as boxes on a grid, exits as lines,
the character's room lit, the belief shown when the brain is not sure, spawn rooms and shops
marked. `Alt+M` or a `map` button beside `dev` opens it. Click a room and the character walks
there; the route shows on the map and as text; a stop button ends it. John's design call
(2026-10-19): boxes and lines a level at a time, like the mapper programs; changeable once
he has used it.

## Inputs

- `docs/plan/05-ui.md` "Look" (CP437 lines, the 16 colours), "Side panels" (the map: click a
  room to walk there).
- C-022 (`map_view`, `walk_to`, `walk_stop`, `map_request`; `position` and `walking` in the
  snapshot), C-018 (`DevPanel.svelte` and the drawer wiring in `App.svelte`; `Alt+D`).
- `kb/navigation.md` (exit kinds the lines distinguish; up and down).

## Behaviour

- **Drawer.** One right-hand drawer with two tabs, `map` and `dev`; `Alt+M` opens it on the
  map (again: closes), `Alt+D` on dev; the two buttons in the top bar do the same. It follows
  the focused tile. Open state and tab remembered as the dev panel's are.
- **Drawing** (`map.ts`, pure: a `map_view` plus view state in, draw calls on a 2D canvas
  out): each room a box of 3x2 cells at its (x, y); exits as lines between boxes, solid for
  plain, a gap with a `+` glyph for a door, dotted for hidden or text exits, an arrow head for
  a one-way exit; `▲`/`▼` in a box with an up or down exit. The believed room bright white
  with a double frame; other candidates (when the belief has several) framed in yellow; rooms
  that spawn monsters with a dim red fill; shops, trainers and banks with a letter (S, T, B)
  from the room type and the MegaMud waypoint flags; rooms visited this session brighter than
  rooms not visited; collided rooms (C-021's flag) drawn with a dashed frame and no exits.
  The current route as a highlighted path with step numbers. One level at a time: the
  believed room's level, with a `level` indicator. The letters and flags come from the
  `map_view` (`kind`, `spawns`, `collides`); the panel computes nothing.
- **Navigation.** The view centres on the believed room and recentres when it changes
  unless the user has panned (then a `centre` button). Mouse wheel zooms between three cell
  sizes; drag pans. Hover shows a tooltip: name, map:room, what spawns, exits.
- **Walking.** Click a room: `walk_to`; the route appears; a bar under the canvas shows the
  commands ("n, n, open e, e, borrow skiff") and a **stop** button sends `walk_stop`.
  `walking` in the snapshot drives the bar (steps left). While walking, clicking another room
  replaces the walk.
- **Without a brain** the panel says so and draws nothing; without a position it says
  "position unknown: re-sync or look".
- **Keys and docs**: `Alt+M` (`KeyM`) in `keys.ts`, the help overlay and `docs/web-ui.md`
  ("The map"), with a screenshot.

## Design notes

- Canvas, not SVG: a few hundred boxes redraw in a millisecond, and the crisp VGA look is
  pixel work. Draw at device pixel ratio so Retina is sharp; the text in boxes uses the VGA
  font at 16 px like the headers.
- Keep `map.ts` free of Svelte so it can be unit-tested with a fake 2D context (record the
  calls) and reused by a future terminal renderer.
- The `map_view` is the brain's; the panel never computes layout or routes.

## Done when

- With the brain running on the rig: the map shows the character's surroundings after a
  re-sync; walking by hand (typing `n`) moves the lit box; clicking a room three steps away
  walks the character there with the route drawn and the bar updating; `stop` stops it.
- Belief with several candidates shows them all framed; it collapses as the character moves.
- `Alt+M`, the button, the tabs and the remembered state work; the keys test passes.
- `npm run check`, `npm test`, the UI workflow green; screenshots at 1, 2 and 4 tiles in the
  PR; `docs/web-ui.md` updated.

## Tests

- `ui/tests/map.test.ts`: drawing a small `map_view` records the expected boxes, lines and
  glyphs; hover hit-testing; zoom levels; the route bar text.

## Notes for the agent

- Two tiles may walk at once; the panel shows the focused one. Nothing party-wide yet.
- Loop listing and "walk this loop" come with C-024 and plug into this panel.
