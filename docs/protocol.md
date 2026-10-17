# The wire protocol

The gateway, the brain and the UIs talk over one WebSocket per client, in JSON text frames.
Every message is an object whose `"type"` field names it. The types are defined in
`crates/wayfarer-proto`; the web UI's TypeScript in `ui/src/proto/` is generated from them.
This page lists the messages of milestone M0: sessions, terminal output and input. Events,
state and control (the brain's families) come later.

## Handshake

The client's first message is `hello`. The gateway answers `welcome` and starts sending, or
`refused` and closes the connection. `protocol` is an integer (currently **1**); when the two
sides' numbers differ the gateway refuses, naming both. `client` is `ui` or `brain`. `token`
may be left out where the gateway doesn't check it (it listens on localhost only); otherwise a
wrong or missing token is refused.

```json
{"type":"hello","protocol":1,"client":"ui","token":"TOKEN"}
{"type":"welcome","protocol":1,"server_build":"0.1.0","sessions":[{"id":"kasimir","name":"Kasimir","server":"bbs.example.org:23","state":"realm","last_seq":42}]}
{"type":"refused","reason":"protocol mismatch: client speaks 2, gateway speaks 1"}
```

## Sessions

A session is one character's connection to a game server. Its `id` is a short string from the
gateway's config (the character key), stable across restarts; `last_seq` is the `seq` of its
latest output, 0 before any. `state` is one of:

| State | Meaning |
|---|---|
| `connecting` | Opening the connection |
| `logon` | Connected; the logon script is running |
| `menu` | At the game's menus, outside the realm |
| `realm` | In the realm |
| `lost` | The connection dropped without a clean log-off |
| `closed` | Logged off and disconnected on purpose |

Gateway → client:

| Message | When |
|---|---|
| `session_list {sessions}` | The list of sessions changed (the whole list) |
| `session_state {session, state}` | One session's state changed |

```json
{"type":"session_list","sessions":[{"id":"kasimir","name":"Kasimir","server":"bbs.example.org:23","state":"connecting","last_seq":0}]}
{"type":"session_state","session":"kasimir","state":"lost"}
```

Client → gateway: `subscribe {sessions}` chooses which sessions' output and state this
connection receives, replacing the previous choice. Before any `subscribe`, or with
`sessions` left out, it receives all of them.

```json
{"type":"subscribe","sessions":["kasimir"]}
```

## Terminal output

`output {session, seq, text}` is live output, sent as it arrives. `text` is UTF-8 with the
game's ANSI escapes kept (the gateway has already decoded CP437). `seq` starts at 1 for each
session and never resets for the life of the session's log, so a client knows exactly what it
has seen.

```json
{"type":"output","session":"kasimir","seq":43,"text":"\u001b[1;32mObvious exits: north, south\u001b[0m\r\n"}
```

`history_request {session, after_seq?, before_seq?, max_chunks}` asks for stored output:

- `after_seq`: the chunks after it. After a reconnect, the last `seq` the client saw; repeat
  until the answer comes back empty.
- `before_seq`: the chunks just before it, when scrolling back past the oldest chunk held.
- Neither: the latest chunks.

The answer is `history {session, from_seq, chunks}`: contiguous chunks in ascending `seq`, at
most `max_chunks`. `from_seq` is the first chunk's `seq` (where the range would have started,
if `chunks` is empty), so `from_seq` 1 means the start of the log.

```json
{"type":"history_request","session":"kasimir","after_seq":42,"max_chunks":2000}
{"type":"history","session":"kasimir","from_seq":41,"chunks":[{"seq":41,"text":"You notice nothing here.\r\n"},{"seq":42,"text":"[HP=30/30]: "}]}
```

## Input

| Message | What it does |
|---|---|
| `input_line {session, text}` | A line of input; the gateway adds the line ending |
| `input_keys {session, bytes}` | Raw bytes (an array of 0–255), sent straight through and never line-buffered: the character editor, `Hit any key` |

```json
{"type":"input_line","session":"kasimir","text":"look north"}
{"type":"input_keys","session":"kasimir","bytes":[13]}
```

## Errors

A message the gateway can't read (not JSON, no `type`, an unknown `type`, a missing field) or
can't act on (an unknown session) gets an `error` reply; the connection stays up.

```json
{"type":"error","message":"unknown session \"nobody\""}
```

## Versioning

Adding a message type changes nothing: receivers report types they don't know. Removing or
changing a message bumps the protocol number.
