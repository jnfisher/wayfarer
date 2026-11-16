---
id: C-106
title: "A class badge on each tile: whose tile is whose at a glance"
tier: B
size: S
wave: 16
depends: []
crate: ui
touches:
  - ui/src/lib/classes.ts (new: the 15 badges)
  - ui/src/lib/header.ts (`fixedWidth` counts the badge)
  - ui/src/Tile.svelte (the badge after the number)
  - ui/tests/classes.test.ts, ui/tests/header.test.ts
  - docs/web-ui.md ("The header")
---

## Goal

Tell the tiles apart at a glance: the characters' names do not say their classes ("Rocco is a
warrior"), so each tile's header shows a class badge in the class's own colour (John,
2026-11-16, option B of four mockups).

## Behaviour

- After the tile number: a block with a CP437 glyph and a three-letter tag, `♠ WAR`, in the
  style of the connection badges.
- Every one of the 15 classes has its own one of the 15 VGA colours (all but black), grouped
  loosely by theme (fighters red, healers blue/white/grey, nature green, arcane magenta, sneaky
  dark grey and blue); black text on the bright colours, white on the dark. The table is in
  docs/web-ui.md "The header".
- No badge until the class is known; the badge is never dropped on a narrow tile (the level
  text and the rest go first). The class name stays in the level text.

## Done when

- Every class has a badge with its own colour, glyph and tag; any case finds it; an unknown
  class has none.
- The tile header shows ` ♠ WAR ` for a Warrior right after the number, and the header's width
  count includes it.
