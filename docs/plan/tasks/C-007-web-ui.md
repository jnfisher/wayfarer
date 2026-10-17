---
id: C-007
title: Web UI scaffold: Svelte, Vite, xterm.js; a tile per session, input, scrollback, reconnect
tier: B
size: M
wave: 1
depends: [C-006]
crate: ui/
touches:
  - ui/ (new: package.json, vite.config.ts, src/)
  - ui/public/fonts/ (the VGA font, with its licence file)
  - .github/workflows/ui.yml
---

## Goal

The first thing John sees: a browser page with one tile per character, each a live 80-column
terminal in the old DOS look, with an input line, scrollback that includes what happened
before the page opened, and a connection that survives page reloads and gateway restarts.

## Inputs

- `docs/plan/05-ui.md` "Look", "Views" (Tile only, for now), "Fonts and scaling",
  "Scrollback and logs", "Input".
- `docs/plan/02-architecture.md` "The wire protocol"; C-002's generated types in
  `ui/src/proto/`; C-006 for the endpoint.
- The prototype viewer, for what John liked: a read-only tmux mirror of the game with the
  bot's notes injected (`mmud-exploration/test/play/README.md` "Watch").

## Behaviour

- **Tiles**: one per session, laid out by count (1; 1x2; 2x2; 2x3 or 3x2 by screen shape).
  Header: character name, server, a state badge (connecting, logon, menu, realm, lost, closed).
- **Terminal**: xterm.js, 80 columns, rows to fill the tile; the font size is computed so 80
  columns fit the tile's width ("fit to tile"); `Ctrl +/-` zooms the focused tile.
- **Look**: `Px437 IBM VGA 8x16` from the Ultimate Oldschool PC Font Pack (int10h.org) as a
  web font, falling back to a system monospace; the 16 DOS colours as the xterm theme; dark
  chrome with CP437-style box borders.
- **Input**: an input line under each tile sends `input_line` on Enter, with per-tile history
  (Up/Down). A "keys" toggle sends keystrokes typed in the terminal as `input_keys` instead
  (for the stat editor and `Hit any key`). Clicking a tile focuses it; `Alt+1`..`Alt+6`
  focus by number.
- **Backfill**: on load, each tile requests the last 2,000 chunks (`history_request` with
  `before_seq` = its first live `seq`) and writes them before the live output, so the screen
  shows what happened before the page opened. xterm's scrollback is 10,000 lines.
- **Reconnect**: when the WebSocket drops, show it on every tile, retry with backoff, and on
  reconnect request everything after each tile's last `seq`. Nothing is lost or duplicated.
- **Token**: if the gateway refuses for a token, ask once and keep it in `localStorage`.

## Design notes

- Svelte 5 with TypeScript, Vite, `@xterm/xterm` with the fit and WebGL add-ons (canvas
  fallback). No UI framework beyond Svelte; plain CSS with variables for the theme.
- `src/lib/gateway.ts`: the protocol client (connect, hello, dispatch, reconnect, history) as
  plain TypeScript, independent of Svelte, so it can be unit-tested.
- Vite dev server proxies `/ws` to `127.0.0.1:7373`, so `npm run dev` gives hot reload
  against a running gateway (`07-development-and-testing.md`, "Four restart domains").
- `npm run build` writes `ui/dist/`, which `wayfarer gateway` serves.
- `ui.yml`: on pull requests touching `ui/**` only; Node LTS with npm cache; `npm ci`,
  `npm run check` (svelte-check / tsc), `npm test` (vitest). Budget: under 2 minutes. No
  browser tests in CI.

## Done when

- `npm run build` succeeds and the gateway serves the page.
- Vitest covers the protocol client: handshake, output dispatch to the right tile,
  reconnect requesting `after_seq`, no duplicate chunks; and the tile layout function.
- **A screenshot in the PR**: the agent runs the gateway against a fake server (or a local
  silvermere if C-009's binary is available), opens the page with Playwright (Chromium is
  preinstalled in cloud sessions; not in CI), and attaches screenshots of one tile and of
  four tiles, showing the VGA font and colours.

## Tests

- Vitest unit tests above; `svelte-check` clean.

## Notes for the agent

- Do not build an older-history viewer beyond the backfill; deeper scrollback (split view,
  search) is a later card (`05-ui.md`, M5).
- Keep the visual design simple and faithful to the DOS look; John will iterate on it once
  he can see it.
