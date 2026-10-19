---
id: C-014
title: Protocol M1: event, state, control and dev messages; the gateway relays between the brain and the UIs
tier: B
size: M
wave: 2
depends: [C-002, C-006]
crate: wayfarer-proto, wayfarer-gateway
touches:
  - crates/wayfarer-proto/src/ (new message variants and the `CharacterState` type)
  - ui/src/proto/ (regenerated)
  - crates/wayfarer-gateway/src/{server.rs,session.rs,gateway.rs} (one brain connection;
    relay; the `sent` stream; a connection that runs over channels as well as a WebSocket)
  - docs/protocol.md
---

## Goal

The wire grows the brain's families (`02-architecture.md` "The wire protocol": events, state,
control, dev), and the gateway learns the one thing it must do for them: pass messages from
the attached brain to the UIs and control messages from UIs to the brain, and tell the brain
what the player sends. The brain itself is C-016; this card gives it and the UIs their
vocabulary, testable without either.

## Inputs

- `docs/plan/02-architecture.md` "The shape", "Input arbitration" (the brain-attached path is
  **not** built here: no bot plays in M1, so UI input still goes straight to the game),
  "The wire protocol".
- `docs/plan/03-engine.md` section 2 (the character model's fields: what `CharacterState`
  carries), section 7 (restarts).
- `docs/protocol.md`, `crates/wayfarer-proto/README.md` (examples checked by a test; the
  TypeScript regeneration), `crates/wayfarer-gateway/src/server.rs` (`Connection`).
- `crates/wayfarer-parse/src/event.rs` (`Event`: name, cat, fields, line, seq, line_no; the
  wire event carries the same).

## Behaviour

New messages (names are final; adding messages bumps nothing, `PROTOCOL` stays 1):

**Brain → gateway → UIs** (the gateway relays each to every UI connection subscribed to its
session; messages without a session go to every UI):

| Message | Meaning |
|---|---|
| `event {session, seq, line_no, name, cat, fields, line}` | One parsed event, as `wayfarer-parse` produced it, with the output `seq` it came from |
| `character {session, state}` | The whole character snapshot (`CharacterState`, below) |
| `character_patch {session, patch}` | A JSON merge patch (RFC 7386) to apply to the last snapshot |
| `brain_info {build, started_at, world_pack}` | Who the brain is: version and build id, start time (RFC 3339), the world pack's path and hash |
| `notice {session?, text}` | Something the brain did or saw worth a line in the dev panel ("re-synced", "parser reset") |
| `warning {session?, text}` | Something wrong but not fatal |

**UI → gateway → brain** (relayed to the brain connection; with no brain attached the gateway
answers `error "no brain attached"`):

| Message | Meaning |
|---|---|
| `resync {session}` | Ask the brain to re-sync that character (`st`, `exp`, `i`, look) |
| `state_request {session?}` | Ask for a fresh `character` snapshot (one session, or all) and a `brain_info` |

**Gateway → brain only:**

| Message | Meaning |
|---|---|
| `sent {session, seq, text}` | A line went to the game from a client or a logon/log-off step; `seq` is the session's latest output `seq` at that moment. A step's password, or a typed line equal to it, is `********`, as in `sent.bin` |

**Gateway → UIs:** `brain_state {attached}` whenever a brain attaches or leaves, and right
after `welcome` so a UI knows at once; `brain_lagged {missed}` when the gateway had to drop
relayed brain messages for that UI (it recovers with `state_request`).

`CharacterState` (one struct in `wayfarer-proto`, every field optional until known): `name`,
`race`, `class`, `level`, `hp`, `hp_max`, `mana`, `mana_max`, `exp`, `exp_next`,
`exp_percent`, `lives`, `cp`, `stats {strength, intellect, willpower, agility, health,
charm}`, `room_title`, `room_exits`, `in_combat`, `resting`, `statuses` (a list: `poisoned`,
`blind`, `held`, `diseased`, ...), `prompt_form`, `parse {lines, unclassified}` (counts since
the brain attached), `synced_at` (when the last re-sync finished) and `updated_seq` (the
output `seq` the snapshot reflects). The merge patch is over this JSON shape; `null` clears a
field.

Gateway rules:

- At most one brain connection. A second `hello` with `client: brain` while one is attached
  is `refused` with the reason "a brain is already attached". When the brain's connection
  closes, `brain_state {attached: false}` goes to the UIs and the next brain may attach.
- A brain connection receives everything a UI does (output, states, history) for every
  session; `subscribe` works for it too.
- Relay is by message family, not by inspecting contents: the gateway never parses `fields`
  or `patch`.
- A UI message of a brain family, or a brain message of a UI family, gets an `error` and is
  dropped.
- `sent` is emitted for every `input_line` from any client, and for every line a logon script
  or log-off sends, after the line has gone to the socket.

## Design notes

- `ToClient` gains the relayed variants and `brain_state`; `ToGateway` gains `resync`,
  `state_request`, and the brain-originated variants (`event`, `character`, ...). Keep one
  enum per direction as now; a `family()` method says which variants a UI or a brain may
  send, and the relay matches on it.
- `fields` and `patch` are `serde_json::Value` (ts-rs: `Record<string, unknown>` /
  `unknown`); the UI's typed view of `CharacterState` is generated.
- `server.rs`: the brain connection is a `Connection` with a flag and a registered
  `mpsc::Sender<ToClient>` in `AppState` (`brain: Mutex<Option<...>>`). Relay to UIs goes
  through a `broadcast` channel of brain messages in `AppState`, with one forwarding task
  per UI connection filtering by its subscription into its own outbox, the way output
  chunks are forwarded today: a slow UI stalls only its own forwarder, never the brain, and
  on `RecvError::Lagged` it gets `brain_lagged`. (Today's forwarders block on the outbox
  and nothing is dropped; keep that for output.)
- `sent` is produced where the bytes are written: the session task in `session.rs` (typed
  lines, logon and log-off steps, with the `MASK` it already applies for `sent.bin`) gets a
  `broadcast` of `Sent {session, seq, text}` exposed by `Gateway::sent_lines()` beside
  `state_changes()`; the server forwards it to the brain connection only.
- **Transport.** `Connection::run` takes a sink and a stream of messages rather than a
  `WebSocket`, so the same code runs over a pair of tokio channels: C-019's `wayfarer serve`
  attaches its in-process brain through that path with no socket. Make `AppState` (or a
  small public function) able to accept such a connection; test it here with a channel
  client.
- `docs/protocol.md` gets the new sections with one example of each message; the README's
  example test covers them.

## Done when

- `wayfarer-proto`: every new message round-trips JSON, the README examples match, the
  TypeScript is regenerated and the drift test passes.
- Gateway tests (in-process, the test WebSocket client used by C-006's tests): a brain
  attaches and a UI sees `brain_state true`; a `character` from the brain reaches a UI
  subscribed to that session and not one subscribed to another; `resync` from a UI reaches
  the brain; with no brain, `resync` gets `error`; a second brain is refused; the brain's
  disconnect sends `brain_state false`; `sent` arrives at the brain for a UI line and for a
  log-off step with the password masked.
- A channel-attached client (no WebSocket) goes through the same handshake, relay and
  refusal rules as a WebSocket one.
- `docs/protocol.md` documents everything above; `PROTOCOL` is still 1.

## Tests

- As listed in "Done when", each under a second, on the existing in-memory connector.

## Notes for the agent

- Do not build arbitration (forwarding UI input to the brain as `user_input`): M1 has no
  bot, so input keeps going straight to the game. Leave a comment where it will hook in.
- `sent` must never carry a password: reuse the masking the gateway already does for
  `sent.bin`.
- Keep `CharacterState` flat and optional-everywhere; C-016 fills it, C-017 draws it.
