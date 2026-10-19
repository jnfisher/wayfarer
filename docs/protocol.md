# The wire protocol

The gateway, the brain and the UIs talk over one WebSocket per client, in JSON text frames.
Every message is an object whose `"type"` field names it. The types are defined in
`crates/wayfarer-proto`; the web UI's TypeScript in `ui/src/proto/` is generated from them.
There are two kinds of client: UIs (the browser, the desktop app, the terminal UI) and at
most one brain, which parses the game and keeps each character's state. The gateway handles
sessions, terminal output and input itself, and relays the brain's events, state and dev
messages to the UIs and the UIs' control messages to the brain.

## Handshake

The client's first message is `hello`. The gateway answers `welcome` and starts sending, or
`refused` and closes the connection. `protocol` is an integer (currently **1**); when the two
sides' numbers differ the gateway refuses, naming both. `client` is `ui` or `brain`. `token`
may be left out where the gateway doesn't check it (it listens on localhost only); otherwise a
wrong or missing token is refused.

Only one brain may be attached at a time: another `hello` with `client: brain` while one is attached
gets `refused` with the reason `a brain is already attached`. A UI's `welcome` is followed at once
by `brain_state` (below).

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
| `connected` | Connected, with no logon script running (none configured, or it stopped and the player has the session) |
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

A client that reads too slowly does not hold the session up: the gateway drops the live output
it could not take and tells it with `lagged {session, missed}`, the number of chunks it did not
send. Live output carries on after the gap; the client fetches the gap with `history_request`
`after_seq` the last `seq` it saw, as after a reconnect.

```json
{"type":"lagged","session":"kasimir","missed":120}
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

Any client may send input, the brain included; it goes straight to the game. (Once a bot
plays, a UI's input will go through the brain instead; not yet.)

## The brain

A brain connection receives everything a UI does (output, session states, history, for every
session unless it subscribes), and also `sent`: each line that went to the game, whoever sent
it.

| Message | Direction | Meaning |
|---|---|---|
| `sent {session, seq, text}` | gateway → brain | A line went to the game: typed by any client (`input_line`; not `input_keys`), or sent by a logon or log-off step, after it was written to the socket. `seq` is the session's latest output `seq` at that moment. A password, or a typed line equal to it, is `********` |
| `brain_state {attached}` | gateway → UIs | Whether a brain is attached: right after `welcome`, then whenever one attaches or leaves |
| `brain_lagged {missed}` | gateway → UI | The UI read too slowly and `missed` relayed brain messages were dropped for it. Recover with `state_request` |

```json
{"type":"sent","session":"kasimir","seq":42,"text":"********"}
{"type":"brain_state","attached":true}
{"type":"brain_lagged","missed":17}
```

### Events, state and dev (brain → UIs)

The gateway relays each to every UI subscribed to its `session`; one without a `session` goes
to every UI. It relays by message type and never reads `fields` or `patch`.

| Message | Meaning |
|---|---|
| `event {session, seq, line_no, name, cat, fields, line}` | One parsed event, as the parser produced it: `name` (`you_hit`, `room_title`, `prompt`, ...), category `cat`, the named `fields`, the `line` it was read from, its `line_no` in the session's text and the output `seq` it came from |
| `character {session, state}` | The whole character snapshot (below) |
| `character_patch {session, patch}` | A JSON merge patch (RFC 7386) to apply to the last snapshot; `null` clears a field |
| `brain_info {build, started_at, world_pack?}` | Who the brain is: version and build id, start time (RFC 3339), the world pack's `path` and `hash` |
| `notice {session?, text}` | Something the brain did or saw worth a line in the dev panel ("re-synced", "parser reset") |
| `warning {session?, text}` | Something wrong but not fatal |

```json
{"type":"event","session":"kasimir","seq":43,"line_no":88,"name":"you_hit","cat":"combat","fields":{"dmg":"1","target":"fat giant rat","verb":"punch"},"line":"You punch fat giant rat for 1 damage!"}
{"type":"character","session":"kasimir","state":{"name":"Kasimir","race":"Human","class":"Warrior","level":3,"hp":30,"hp_max":35,"exp":1520,"stats":{"strength":60,"agility":50},"room_title":"Newhaven, Village Entrance","room_exits":["north","south"],"in_combat":false,"statuses":[],"prompt_form":"hp","parse":{"lines":412,"unclassified":3},"synced_at":"2026-10-19T12:00:05Z","updated_seq":43}}
{"type":"character_patch","session":"kasimir","patch":{"hp":23,"in_combat":true,"room_title":null}}
{"type":"brain_info","build":"0.1.0 (3f2a9c1)","started_at":"2026-10-19T12:00:00Z","world_pack":{"path":"worlds/stock-1.11p.sqlite","hash":"sha256:5d41402abc4b2a76"}}
{"type":"notice","session":"kasimir","text":"re-synced"}
{"type":"warning","text":"world pack not found; walking blind"}
```

The character snapshot: every field is optional and left out until the brain knows it.

| Field | What |
|---|---|
| `name`, `race`, `class` | Strings |
| `level`, `lives`, `cp` | Numbers (`cp`: unspent character points) |
| `hp`, `hp_max`, `mana`, `mana_max` | Numbers; `hp` goes below zero while dying |
| `exp`, `exp_next`, `exp_percent` | Experience, what the next level still needs, and how far through the level |
| `stats` | `{strength, intellect, willpower, agility, health, charm}`, each optional |
| `room_title`, `room_exits` | The room as last seen: its title and its exits |
| `in_combat`, `resting` | Booleans |
| `statuses` | A list: `poisoned`, `blind`, `held`, `diseased`, ... |
| `prompt_form` | Which form the last prompt had |
| `parse` | `{lines, unclassified}`: lines parsed and lines nothing recognised, since the brain attached |
| `synced_at` | When the last re-sync finished (RFC 3339) |
| `updated_seq` | The output `seq` the snapshot reflects |

### Control (UI → brain)

Relayed to the brain. With no brain attached the gateway answers `error` `no brain attached`.

| Message | Meaning |
|---|---|
| `resync {session}` | Re-sync that character (`st`, `exp`, `i`, look) |
| `state_request {session?}` | Send a fresh `character` snapshot (of one session, or of all when `session` is left out) and a `brain_info` |

```json
{"type":"resync","session":"kasimir"}
{"type":"state_request","session":"kasimir"}
```

A UI that sends a brain message (`event`, `character`, ...), or a brain that sends a control
message, gets an `error`; the message is dropped and the connection stays up.

## Errors

A message the gateway can't read (not JSON, no `type`, an unknown `type`, a missing field) or
can't act on (an unknown session) gets an `error` reply; the connection stays up.

```json
{"type":"error","message":"unknown session \"nobody\""}
```

## Versioning

Adding a message type changes nothing: receivers report types they don't know. Removing or
changing a message bumps the protocol number.
