# 02. Architecture

## The shape

```
  silvermere / MBBSEmu / a BBS
            │ telnet (CP437, ANSI)
 ┌──────────┴───────────┐
 │ gateway              │ holds up to N telnet sessions, logs every byte, fans output out,
 │                      │ serves the web UI. Small; rarely changes.
 └───┬──────────────┬───┘
     │ WebSocket    │ WebSocket (same protocol)
 ┌───┴────────┐ ┌───┴────────────────────────────────┐
 │ brain      │ │ UIs: web (browser), desktop (Tauri),│
 │ parser,    │ │ terminal (ratatui). Any number,     │
 │ world,     │ │ attached at once.                   │
 │ modes,     │ └─────────────────────────────────────┘
 │ settings,  │
 │ party      │
 └────────────┘
```

- **Gateway.** Owns the sockets and nothing else that changes often: telnet negotiation (refuse
  everything except NAWS, answered 80x24; `kb/connection.md`), CP437 to UTF-8, a raw capture
  per session, a sequence-numbered output log, input arbitration (below), and fan-out to every
  attached client. It also serves the web UI's static files. It runs the logon script for a
  session (`06-accounts-and-setup.md`) so a character can be reconnected without a brain.
- **Brain.** One process for all characters: the parser, the world model, the executor and
  mode stack for each character, the settings, measured statistics, and later the party
  coordinator. It attaches to the gateway as a privileged client. See `03-engine.md`.
- **UIs.** Render the terminal stream and the brain's state, and send input and control. They
  hold no game logic. See `05-ui.md`.

**Two processes in development, one in a release.** During development the gateway and the
brain run as separate processes so the brain can be rebuilt and restarted while every
character stays logged in (`07-development-and-testing.md`). The released app runs both as
modules of one process (`wayfarer serve`), talking over in-process channels with the same
message types. The desktop app embeds that process in Tauri's Rust side, so there is no
sidecar to manage.

## Input arbitration

Every line sent to the game passes through one place per session:

- **No brain attached, or the character is in manual mode**: the gateway sends UI input
  straight to the game. This is what makes the gateway a plain multi-session client on its own.
- **Brain attached and playing**: the gateway forwards UI input to the brain as `user_input`.
  The brain's executor decides when it goes out and what happens to the bot
  (`03-engine.md`, "Player input beside the bot"). The bot's own commands and the player's
  never interleave mid-exchange.
- Raw keys (the full-screen character editor, `Hit any key`) are a separate message that
  always goes straight through and pauses the bot for that character.

## The wire protocol

WebSocket, JSON messages, one connection per UI, versioned in the handshake. The message types
are Rust types in `wayfarer-proto`; the TypeScript types for the web UI are generated from
them (`ts-rs`), so the two cannot drift.

| Family | Direction | Messages |
|---|---|---|
| Hello | both | protocol version, client kind (ui/brain), auth token, server build |
| Sessions | gateway → client | list, opened, closed, connection state (connecting, logon, menu, realm, lost) |
| Terminal | gateway → client | output chunk `{session, seq, text}` (UTF-8 with ANSI kept); history range on request |
| Input | client → gateway | `line {session, text, origin}`, `keys {session, bytes}` |
| Events | brain → UIs | typed game events per character (`03-engine.md`), with the terminal `seq` they came from |
| State | brain → UIs | character snapshot (HP, mana, exp, level, room belief, mode stack, intent narration, exp/hr), party snapshot; sent whole on attach, then as diffs |
| Control | UI → brain | pause/resume (one, party, all), manual/auto, choose plan, settings get and patch, "walk to room" |
| Dev | brain → UIs | brain build id and start time, restart notices, warnings |

Every output chunk has a sequence number per session, so a UI or brain that reconnects asks
for "everything since `seq`" and misses nothing. The gateway keeps a recent window in memory
and the rest on disk.

**Security.** The gateway binds to localhost by default. Remote access needs a token (shown in
the UI, rotatable) and is off until enabled. Anything that can reach the gateway can play the
characters.

## World data

The bot is data-driven (`kb/architecture.md`): it plans routes, judges monsters and prices
gear from the world's own records. The client never reads a server's internals, so the data
comes as a **world pack**: one SQLite file with rooms and exits, monsters, items, shops,
spells, classes, races, messages and the text patterns that go with them.

| Source | When |
|---|---|
| Stock 1.11p "Realm of Legends" pack, shipped with the installer | The default. Most servers run the stock world |
| Built from a server's `.DAT` files (`wayfarer import-world <dir>`) | A sysop or a player with the files of a customised world |
| Learned overlay | Rooms, exits and texts seen in play that the pack lacks, kept per server and merged over the pack |
| MegaMud's path set (`kb/megamud-formats.md`) | Optional: imported from the player's own MegaMud copy (`wayfarer import-world --megamud`), never shipped or kept in this repository (John, 2026-10-17); for development, `jnfisher/mmud-exploration` `reference/megamud/`. Gives the community loop library and waypoints, each checked against the pack's rooms |

The stock pack is built at release time by a tool that reads the game files (the work in
`test/play/mmud_data.py`, and silvermere's stock-world export, T-006, as the reference for
the record layouts). That tool is a build-time dependency only.

## Tech stack

| Part | Choice |
|---|---|
| Core | Rust (stable, pinned), tokio, axum (HTTP and WebSocket), serde, rusqlite |
| Wire types | `wayfarer-proto`, TypeScript generated with `ts-rs` |
| Web UI | TypeScript, Svelte 5, Vite, xterm.js (WebGL, fit and search add-ons) |
| Desktop | Tauri 2, embedding the core library |
| Terminal UI | ratatui, crossterm, `vt100` for the game panes |
| Storage | per-user data directory: `config.toml`, `characters/*.toml`, `logs/`, `store.sqlite` |

Rust matches silvermere (one toolchain and set of habits for the agents), gives one binary per
platform, and lets Tauri embed the core directly.

## Crates

| Crate | What it is |
|---|---|
| `wayfarer-proto` | Wire messages and shared types; no I/O |
| `wayfarer-telnet` | Telnet options, CP437, raw capture |
| `wayfarer-gateway` | Sessions, logon scripts, output log, arbitration, WebSocket server, static files |
| `wayfarer-parse` | Pattern table and incremental parser: terminal text to events |
| `wayfarer-world` | World pack, map, routing, position belief, learned overlay |
| `wayfarer-settings` | Setting declarations, layers, presets, persistence |
| `wayfarer-brain` | Executor, mode stack, policies, statistics, party coordinator |
| `wayfarer-store` | Log index and search, measured statistics (SQLite) |
| `bin/wayfarer` | `serve`, `gateway`, `brain`, `tui`, `import-world` |
| `ui/web`, `ui/desktop` | Svelte app; Tauri shell |
| `tools/` | Pattern-table generator, world-pack builder |

Dependency direction: proto ← telnet ← gateway; proto ← parse ← world ← brain; settings and
store are leaves used by brain. Nothing in the shipped client depends on silvermere's crates.
The test harness and simulator may (`07-development-and-testing.md`).

## Repositories (*Needs John* on the KB move)

- **`jnfisher/wayfarer`** (new, public): the client.
- **`jnfisher/mmud-exploration`**: research, the recovered source, the prototype in
  `test/play/` (frozen once wayfarer reaches parity; it stays silvermere's soak driver until
  then), and today the KB.
- **`jnfisher/silvermere`**: the test server, run from a pinned build.
- **The KB.** Recommendation: when wayfarer reaches milestone M1, move the client-facing KB
  files to `wayfarer/docs/kb/` and point CLAUDE.md's "keep `kb/` updated" rule there.
  `kb/server-behaviours.md` stays here (it serves the server). Until then the KB stays here
  and wayfarer sessions attach this repository read-only.
