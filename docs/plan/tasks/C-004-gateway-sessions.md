---
id: C-004
title: Gateway sessions: connect, sequence-numbered output log, history, direct input
tier: B
size: M
wave: 1
depends: [C-002, C-003]
crate: wayfarer-gateway
touches:
  - crates/wayfarer-gateway/src/{config.rs,session.rs,log.rs,gateway.rs}
  - examples/wayfarer.toml, examples/credentials.toml
---

## Goal

The gateway's core, with no network server yet: it reads a config naming characters and
servers, holds one telnet connection per character, logs every byte and every output chunk
with a sequence number, answers history requests from memory and disk, fans output out to any
number of subscribers, and sends input to the game. C-006 puts WebSocket in front of it;
C-005 adds logon scripts.

## Inputs

- `docs/plan/02-architecture.md` "The shape", "Input arbitration" (only the no-brain path in
  M0), "The wire protocol" (sequence numbers and replay).
- `docs/plan/06-accounts-and-setup.md` "Credentials" (one local `credentials.toml`, separate
  from settings).
- `docs/plan/07-development-and-testing.md` "Four restart domains" (the gateway is the
  long-lived process; its log must survive its own restart).
- C-002 (message and `SessionInfo` types), C-003 (`Telnet`, `Capture`).
- The prototype's equivalent, for what worked: `mmud-exploration/test/play/bridge.py`
  (a daemon holding the socket, `raw.bin`/`transcript.txt` logs, named sessions).

## Behaviour

- Config (`wayfarer.toml`, path from `--config`, default in the user's data directory):
  `[[server]] id, host, port, preset` and `[[character]] id, name, server, autostart`.
  Credentials (`credentials.toml`, same directory): `[character.<id>] user, password`. On Linux
  and macOS the gateway creates `credentials.toml` with mode 0600 and warns if it is wider.
- Each session: connect, run the bytes through `Telnet`, write replies back, decode, append
  the text as one **chunk** per socket read with the next `seq`, publish it to subscribers.
- Output log per character on disk (`logs/<id>/output.log`, append-only, one record per
  chunk with its `seq` and a timestamp) plus the raw and sent captures from C-003
  (`logs/<id>/raw.bin`, `sent.bin`). On gateway start, `seq` continues from the last record on
  disk, so a restarted gateway never reuses a `seq`.
- Memory: the most recent chunks per session (default 4 MB of text) for fast history; older
  history is read from disk.
- History: `after_seq` returns the chunks after it (up to `max_chunks`), `before_seq` the
  chunks before it, oldest first.
- Subscribers that fall behind get a "lagged" notice and must fetch history; a slow UI never
  blocks the session or other UIs.
- Input (no brain in M0): a line goes out through `Telnet::encode_line`; keys through
  `encode_keys`. The gateway serialises writes per session so two UIs typing at once never
  interleave bytes within a line.
- States: `connecting`, `lost` (socket closed or failed), `closed` (deliberately). C-005 adds
  `logon`, `menu` and `realm`.
- Passwords are never written to logs, captures or tracing output (C-005 masks the logon
  sends; this card makes sure nothing else prints the credentials).

## Design notes

- `Gateway` (an `Arc` handle) with `sessions()`, `open(id)`, `close(id)`, `subscribe(id) ->
  broadcast::Receiver<Chunk>`, `history(id, query)`, `send_line(id, text)`, `send_keys(id,
  bytes)`. One tokio task per session owns the socket.
- Log record format: a simple length-prefixed or JSON-lines file is fine; pick the simplest
  that reads the last record quickly at start-up. Document it in the crate README.
- Data directory with the `directories` crate; everything overridable by flags for tests.

## Done when

- Against an in-test loopback server (tokio listener on 127.0.0.1:0, part of the test, not an
  external service) that sends the MBBSEmu negotiation then a vendored capture's `raw.bin` in
  random-sized pieces: the subscriber receives exactly the decoded text, `seq` counts up from
  1, and the NAWS reply reaches the server.
- History by `after_seq` and `before_seq` is correct across the memory/disk boundary.
- After the gateway is dropped and started again on the same data directory, new chunks
  continue the `seq`.
- Two subscribers, one never reading, do not stall the other.
- `credentials.toml` is created 0600 (Unix) and no test output contains the test password.

## Tests

- The scenarios above as tokio tests, each under a second (use small memory limits to force
  the disk path).

## Notes for the agent

- No reconnect logic here (C-005). On a lost socket, just go to `lost`.
- Keep the M0 gateway single-process and simple; the brain, events and control come later
  and will attach through the same `Gateway` handle.
