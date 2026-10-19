---
id: C-017
title: Web UI status header: live HP and mana bars, level and class, exp and room in each tile; the frame corners line up
tier: B
size: M
wave: 2
depends: [C-013, C-016]
crate: ui/
touches:
  - ui/src/Tile.svelte (header, frame), ui/src/lib/state.ts (new: snapshots and patches)
  - ui/src/lib/gateway.ts (the new messages), ui/src/App.svelte
  - docs/web-ui.md
---

## Goal

Each tile's header shows the character as the brain sees them: HP and mana as bars with
numbers, level and class, exp with the percent to the next level, the room title, and the
state badge as now. Without a brain the header is what M0 showed. While the header is
rebuilt, the tile frame is redrawn so the `╔` and `╗` corners meet the side borders (John,
2026-10-19: the corners sit a few pixels in from the sides). HP and mana maxima arrive with
the first re-sync (the standard prompt has none), so a bar shows `--` until then.

## Inputs

- `docs/plan/05-ui.md` "Views" (the header mock: name, level and class, bars, mode and
  narration later), "Look" (CP437 box characters and shading blocks, the 16 colours).
- C-014 (`character`, `character_patch`, `brain_state`, `brain_info`; `CharacterState`),
  C-016 (what fills it and when).
- `ui/src/Tile.svelte` (the header spans and the `.tile` border CSS), `ui/src/lib/theme.ts`
  (the VGA palette), `ui/public/fonts/` (the VGA font: measure where the double-line glyphs `║` and `═`
  put their strokes in the 8x16 cell; in the IBM VGA ROM font `║` is near columns 3 and 5).

## Behaviour

- **State in the UI.** `lib/state.ts` keeps one `CharacterState` per session: `character`
  replaces it, `character_patch` merge-patches it (RFC 7386: objects recurse, `null`
  deletes, anything else replaces). `brain_state` is kept too. On a reconnect the UI sends
  `state_request` once the socket is open, so a page that loads after the brain gets the
  snapshots.
- **Header.** On one 16 px line, in this order, dropping items from the right when the tile
  is narrow: `╔═ 1 Waydeva  L3 Warrior ═ HP ███████░░░ 28/40  MA ░░░░░░░░░░ 0/0 ═ 1,230 (45%)
  ═ Town Square ═ 127.0.0.1:2323 ═ realm ═╗`. Bars are ten CP437 blocks (`█` full, `░`
  empty), green above two thirds, yellow above one third, red below (my call; the thresholds
  become settings in M4). Unknown values show as `--`. When no brain is attached the header is
  the M0 header plus a dim `no brain` after the badge.
- **Live.** A patch updates only what changed; no flicker, no re-layout of the terminal.
- **Frame.** The side borders are drawn so the double lines sit in the same columns as the
  header's `╔` and `╗` glyphs: two 1 px lines, 8 px apart from the glyph's cell edge as the
  font draws them, at every tile size (the header is a fixed 16 px font, so the side borders
  are fixed too). The bottom corners get `╚` and `╝` the same way. Check at 1, 2 and 4 tiles
  and with the browser zoomed.
- **Docs.** `docs/web-ui.md` describes the header and its colours; a new screenshot replaces
  `docs/images/dev-rig-realm.png`'s role in `docs/dev-rig.md` if the look changed.

## Design notes

- Merge patch in twenty lines of TypeScript with a unit test; no library.
- Header items are spans with fixed roles; measure with `ch` units (the font is monospace)
  and hide from the right with a `ResizeObserver` or CSS `container` queries, whichever is
  simpler in Svelte 5.
- The frame: `::before`/`::after` pseudo-elements on `.tile` for the sides, each 8 px wide
  with a `linear-gradient` giving the two lines, instead of the CSS `double` border; or an
  SVG border image. Pick the one that matches the glyph exactly; say which columns you
  measured in the PR with a zoomed screenshot.

## Done when

- With the brain running against the rig: the header shows HP, mana, level, class, exp and
  room for a character in the realm; taking damage (or resting) moves the bar; `x` to the
  menu keeps the last values and the badge changes.
- Reloading the page shows the header filled within a second (via `state_request`).
- Without a brain: the M0 header plus `no brain`.
- Corners line up at 1, 2 and 4 tiles and at 90%, 100% and 125% browser zoom (screenshots in
  the PR).
- `npm run check`, `npm test`, the UI workflow green; the keys test (C-012) still passes.

## Tests

- `ui/tests/state.test.ts`: merge patch cases (nested, null, replace), `character` replaces,
  `state_request` on reconnect.
- A header rendering test: the right spans for a full snapshot and for an empty one.

## Notes for the agent

- Keep the game's 80 columns untouched; the header is outside the terminal.
- Do not add the bot controls (start, pause, settings) yet: those are M3 and M4.
