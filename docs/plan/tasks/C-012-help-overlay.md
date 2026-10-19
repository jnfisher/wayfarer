---
id: C-012
title: Web UI help: `?` opens an overlay of wayfarer's keys and controls, with a `?` button beside `keys`
tier: C
size: S
wave: 2
depends: [C-007]
crate: ui/
touches:
  - ui/src/lib/keys.ts (new: the one table of keys and controls)
  - ui/src/Help.svelte (new), ui/src/App.svelte, ui/src/Tile.svelte
  - ui/tests/keys.test.ts (new), ui/package.json (test dev dependencies), docs/web-ui.md
---

## Goal

A player who forgets what wayfarer itself responds to can see it without leaving the tile:
`?` on an empty input line, or a small `?` button beside `keys`, opens an overlay listing
wayfarer's keys and controls. It is wayfarer's help only, never the game's (John, 2026-10-19).
The list comes from one table in code, and a test keeps `docs/web-ui.md` in step with it.

## Inputs

- `docs/plan/05-ui.md` "Input", "Pause and control" (the key table that grows later; the
  overlay is where it will be read).
- `docs/web-ui.md` "Keys" (the rows the overlay shows today).
- `ui/src/App.svelte` (the window key handler: `Alt+1`..`Alt+6`, `Ctrl +`/`-`/`0`) and
  `ui/src/Tile.svelte` (the input line, history, the `keys` toggle).

## Behaviour

- **Opening.** `?` typed into a tile's input line when the line is empty opens the overlay and
  is not sent to the game. With anything already on the line, `?` is an ordinary character
  (so `?` can still be sent, and in keys mode every key goes to the game as now). A `?`
  button in the tile footer, right of `keys`, opens and closes it too. `Esc`, clicking
  outside, or `?` again closes it. Focus returns to where it was.
- **Content.** One overlay for the whole page, over the focused tile's area, in the same VGA
  look (box-drawn frame, the 16 colours): a table of keys and what they do (Enter, Up and
  Down, `Alt+1`..`Alt+6` with the Mac's `Option` named, `Ctrl +`, `Ctrl -`, `Ctrl 0`, `keys`
  mode and how to leave it, `?`); what the input line does versus keys mode; what each state
  badge means (`connecting` .. `closed`); where `wayfarer.toml`, `credentials.toml` and the
  logs live (the data directory, as a path the gateway reports or the documented default).
  Nothing about MajorMUD's own commands.
- **One source.** `ui/src/lib/keys.ts` exports the table (`{ keys, does, group }` rows and
  the badge meanings). The overlay renders it; the key handlers in `App.svelte` and
  `Tile.svelte` read their key names from the same module where that is practical (at least
  the modifier and code constants), so a rebinding later changes both.
- **Docs in step.** A vitest test parses the "Keys" table in `docs/web-ui.md` and fails when
  its rows differ from `keys.ts` (same keys, same order; wording of "does" may differ only in
  whitespace). Update the doc in this card so the test passes, and add the `?` rows.

## Design notes

- A `Help.svelte` component mounted once in `App.svelte`, shown by a `$state` flag; the tile
  tells the app "open help" through a prop callback, as `onfocus` does now.
- Keep the overlay keyboard-only friendly: it is a `<dialog>` or a `role="dialog"` with focus
  trapped while open and `aria-label`.
- No new runtime dependencies. For component tests add dev dependencies only: a DOM for
  vitest (`jsdom` or `happy-dom`) and `@testing-library/svelte`; nothing of either ships in
  `ui/dist`.

## Done when

- `?` on an empty line opens the overlay and nothing reaches the game (check the gateway's
  `sent.bin` or the UI's `input_line` calls in a test); `?` after other text is sent as text.
- The `?` button opens and closes it; `Esc` closes it; focus goes back to the input line (or
  the terminal in keys mode).
- The overlay lists every key the UI handles today and the badge meanings.
- The docs test passes and `docs/web-ui.md` "Keys" matches `keys.ts`.
- `npm run check` and `npm test` pass; the UI workflow is green.

## Tests

- `keys.test.ts`: the docs table matches `keys.ts`; the overlay's rows equal the table.
- A component test (vitest with the DOM and `@testing-library/svelte` added above) for `?`
  on an empty versus non-empty line, and for `Esc`. Tests live in `ui/tests/`, where
  vitest's `include` points.

## Notes for the agent

- Help about the game (commands, spells) is out of scope and stays out: John wants wayfarer's
  help only.
- C-013 (resizable tiles) adds rows to the same table; leave room for a "Layout" group.
