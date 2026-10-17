# wayfarer web UI

Svelte 5, TypeScript and Vite; each session is an [xterm.js](https://xtermjs.org/) terminal
(WebGL renderer, xterm's DOM renderer as the fallback). How to use it:
[`docs/web-ui.md`](../docs/web-ui.md).

```sh
npm ci
npm run dev        # http://localhost:5173, /ws proxied to a gateway on 127.0.0.1:7373
npm run build      # dist/, served by `wayfarer gateway` (default --ui-dir ./ui/dist)
npm run check      # svelte-check (types, Svelte warnings fail it)
npm test           # vitest
```

| Path | What |
|---|---|
| `src/proto/` | The wire types, generated from `wayfarer-proto` (`cargo test -p wayfarer-proto -- --ignored regenerate`); never edit by hand |
| `src/lib/gateway.ts` | The protocol client, no Svelte: hello, dispatch, backfill, catching up after a gap or a reconnect, backoff, the token |
| `src/lib/layout.ts` | The tile grid by count and screen shape, and "fit to tile" font sizing |
| `src/lib/input.ts`, `src/lib/sinks.ts` | A tile's command history; terminal text waiting for its terminal |
| `src/lib/theme.ts`, `src/app.css` | The 16 VGA colours and the IBM VGA font |
| `src/App.svelte`, `src/Tile.svelte` | The page and one tile |
| `public/fonts/` | The VGA web font and its licence |
| `scripts/fake-bbs.mjs` | A stand-in game server replaying `tests/fixtures/captures/*/raw.bin` |
| `scripts/screenshot.mjs` | A Playwright screenshot of a running gateway's page (`npx playwright install chromium` once) |

**Output, once each and in order.** The client keeps each session's last written `seq`. A new
tile asks for the latest 2,000 chunks (`before_seq` = the session's `last_seq` + 1, from
`welcome`) and holds live output until they arrive. A live chunk past the next `seq`, or a
`lagged`, fetches the gap (`after_seq`); after a reconnect it asks for everything after the
last `seq`, a page of 2,000 at a time. Chunks it already has are dropped.

**Looking at it without a game server:**

```sh
node ui/scripts/fake-bbs.mjs 4001=tests/fixtures/captures/fight-arena/raw.bin:6898
# a wayfarer.toml whose server is 127.0.0.1:4001, then:
cargo run -- gateway --config /path/to/wayfarer.toml --ui-dir ui/dist
npm --prefix ui run screenshot -- http://127.0.0.1:7373/ tile.png 1280x800
```
