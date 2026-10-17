# wayfarer-proto

The messages the gateway, the brain and the UIs exchange over WebSocket
(`docs/plan/02-architecture.md`, "The wire protocol"): Rust types with serde JSON, and the
TypeScript for the web UI generated from them. No I/O. The full list with what each message
means is in [`docs/protocol.md`](../../docs/protocol.md).

- `PROTOCOL` is the version both sides send in the handshake; a mismatch is refused with both
  numbers in the reason (`protocol_mismatch`). Adding a message bumps nothing; removing or
  changing one bumps `PROTOCOL`.
- `ToClient` (gateway → client) and `ToGateway` (client → gateway) are enums tagged by a
  `"type"` field. `from_json` returns a `DecodeError` (`NotJson`, `MissingType`,
  `UnknownType`, `Invalid`) for anything it can't read; the receiver reports it and carries on.

## TypeScript

The generated files are committed in `ui/src/proto/` (`index.ts` re-exports them all and
holds `PROTOCOL`). A test fails when they drift from the Rust types; regenerate with

```sh
cargo test -p wayfarer-proto -- --ignored regenerate
```

## Examples

One of each message, exactly as serialized (a test checks they match).

Gateway → client:

```json
{"type":"welcome","protocol":1,"server_build":"0.1.0","sessions":[{"id":"kasimir","name":"Kasimir","server":"bbs.example.org:23","state":"realm","last_seq":42}]}
{"type":"refused","reason":"protocol mismatch: client speaks 2, gateway speaks 1"}
{"type":"session_list","sessions":[{"id":"kasimir","name":"Kasimir","server":"bbs.example.org:23","state":"connecting","last_seq":0}]}
{"type":"session_state","session":"kasimir","state":"lost"}
{"type":"output","session":"kasimir","seq":43,"text":"\u001b[1;32mObvious exits: north, south\u001b[0m\r\n"}
{"type":"history","session":"kasimir","from_seq":41,"chunks":[{"seq":41,"text":"You notice nothing here.\r\n"},{"seq":42,"text":"[HP=30/30]: "}]}
{"type":"lagged","session":"kasimir","missed":120}
{"type":"error","message":"unknown session \"nobody\""}
```

Client → gateway:

```json
{"type":"hello","protocol":1,"client":"ui","token":"TOKEN"}
{"type":"input_line","session":"kasimir","text":"look north"}
{"type":"input_keys","session":"kasimir","bytes":[13]}
{"type":"history_request","session":"kasimir","after_seq":42,"max_chunks":2000}
{"type":"subscribe","sessions":["kasimir"]}
```
