---
id: C-002
title: wayfarer-proto: M0 wire messages, handshake and version, TypeScript generation
tier: B
size: S
wave: 1
depends: [C-001]
crate: wayfarer-proto
touches:
  - crates/wayfarer-proto/src/lib.rs (and modules)
  - ui/src/proto/ (generated TypeScript, committed)
---

## Goal

The message types that the gateway, UIs and (later) the brain exchange over WebSocket, as
Rust types with serde JSON, plus generated TypeScript for the web UI and a test that fails
when the two drift. M0 needs sessions, terminal output and input; the other families are
reserved, not designed.

## Inputs

- `docs/plan/02-architecture.md` "The wire protocol" (the families table) and "Input
  arbitration".
- `docs/plan/05-ui.md` (what the UI needs to draw a tile: session name, connection state,
  output, history).

## Behaviour

- Every message is a JSON object with a `"type"` field (serde internally tagged,
  snake_case). Unknown types are an error the receiver reports, not a crash.
- Handshake: the client sends `hello {protocol, client: "ui" | "brain", token}`; the gateway
  answers `welcome {protocol, server_build, sessions: [...]}` or `refused {reason}` and closes.
  `protocol` is an integer; a mismatch is refused with both numbers in the reason.
- M0 messages:
  - gateway → client: `welcome`, `refused`, `session_list {sessions}`, `session_state
    {session, state}` (`connecting | logon | menu | realm | lost | closed`), `output {session,
    seq, text}`, `history {session, from_seq, chunks: [{seq, text}]}`, `error {message}`.
  - client → gateway: `hello`, `input_line {session, text}`, `input_keys {session, bytes}`
    (bytes as an array of u8), `history_request {session, after_seq?, before_seq?,
    max_chunks}` (after a reconnect: everything after the last `seq` seen; when scrolling
    back: the chunks before the oldest one held), `subscribe {sessions}` (default: all).
- `SessionInfo {id, name, server, state, last_seq}`. `id` is a short string chosen by the
  gateway's config (the character key, e.g. `"kasimir"`), stable across restarts.
- `text` in `output` is UTF-8 with ANSI escapes kept (CP437 already decoded by the gateway).
  `seq` starts at 1 per session and never resets for the life of the session's log.

## Design notes

- `pub const PROTOCOL: u32 = 1;` and two enums: `ToClient`, `ToGateway`. Reserved variants
  for later families (events, state, control) are **not** added now; adding a variant bumps
  nothing, removing or changing one bumps `PROTOCOL`.
- TypeScript with `ts-rs` (`#[derive(TS)] #[ts(export)]`), exported to `ui/src/proto/`. A test
  regenerates into a temp directory and compares with the committed files, failing with
  "run `cargo test -p wayfarer-proto -- --ignored regenerate`" (or a small `xtask`) when they
  differ. Keep it simple; one command to regenerate, documented in the crate README.
- No I/O, no tokio in this crate.

## Done when

- Round-trip tests for every message (Rust → JSON → Rust), and a JSON example of each in the
  crate README.
- The generated TypeScript is committed and the drift test passes.
- A one-page `docs/protocol.md` lists the M0 messages with one example each.

## Tests

- Serde round trip per variant; a fixed JSON string per variant (so field renames are caught);
  an unknown `type` gives a typed error; the TypeScript drift test.

## Notes for the agent

- Keep the message set small; C-004, C-006 and C-007 will tell you if something is missing,
  and adding a variant later is cheap.
- `input_keys` exists for the character editor and `Hit any key` (`docs/plan/05-ui.md`); it is
  sent straight through, never line-buffered.
