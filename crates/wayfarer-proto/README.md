# wayfarer-proto

The messages the gateway, the brain and the UIs exchange over WebSocket
(`docs/plan/02-architecture.md`, "The wire protocol"): Rust types with serde JSON, and the
TypeScript for the web UI generated from them. No I/O. The full list with what each message
means is in [`docs/protocol.md`](../../docs/protocol.md).

- `PROTOCOL` is the version both sides send in the handshake; a mismatch is refused with both
  numbers in the reason (`protocol_mismatch`). Adding a message bumps nothing; removing or
  changing one bumps `PROTOCOL`.
- `ToClient` (gateway → client) and `ToGateway` (client → gateway) are enums tagged by a
  `"type"` field. The brain's messages and the UIs' control messages are in both, since the
  gateway relays them: `ToGateway::family()` says who may send one (`Family::Gateway`: anyone;
  `Control`: a UI; `Brain`: the brain) and `into_relayed()` turns it into the `ToClient` the
  gateway passes on. `ToClient::session()` names the session a message is about, for
  subscription filtering.
- `CharacterState` is the brain's snapshot of a character: flat, every field optional and left
  out of the JSON until known. `character_patch` carries a JSON merge patch (RFC 7386) over
  its shape; `event.fields` and `patch` are plain `serde_json` values the gateway never reads. `from_json` returns a `DecodeError` (`NotJson`, `MissingType`,
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
{"type":"sent","session":"kasimir","seq":42,"text":"********"}
{"type":"brain_state","attached":true}
{"type":"brain_lagged","missed":17}
```

Client → gateway:

```json
{"type":"hello","protocol":1,"client":"ui","token":"TOKEN"}
{"type":"input_line","session":"kasimir","text":"look north"}
{"type":"input_keys","session":"kasimir","bytes":[13]}
{"type":"history_request","session":"kasimir","after_seq":42,"max_chunks":2000}
{"type":"subscribe","sessions":["kasimir"]}
{"type":"resync","session":"kasimir"}
{"type":"state_request","session":"kasimir"}
```

Brain → gateway → UIs (the same JSON both ways):

```json
{"type":"event","session":"kasimir","seq":43,"line_no":88,"name":"you_hit","cat":"combat","fields":{"dmg":"1","target":"fat giant rat","verb":"punch"},"line":"You punch fat giant rat for 1 damage!"}
{"type":"character","session":"kasimir","state":{"name":"Kasimir","race":"Human","class":"Warrior","level":3,"hp":30,"hp_max":35,"exp":1520,"stats":{"strength":60,"agility":50},"room_title":"Newhaven, Village Entrance","room_exits":["north","south"],"in_combat":false,"statuses":[],"prompt_form":"hp","parse":{"lines":412,"unclassified":3},"synced_at":"2026-10-19T12:00:05Z","updated_seq":43,"coins":{"copper":320,"carried":[{"coin":"silver nobles","count":32}]},"carrying":["padded vest","dagger"],"encumbrance":{"current":520,"max":3792,"level":"None"},"kills":4}}
{"type":"character_patch","session":"kasimir","patch":{"hp":23,"in_combat":true,"room_title":null}}
{"type":"brain_info","build":"0.1.0 (3f2a9c1)","started_at":"2026-10-19T12:00:00Z","world_pack":{"path":"worlds/stock-1.11p.sqlite","hash":"sha256:5d41402abc4b2a76"}}
{"type":"notice","session":"kasimir","text":"re-synced"}
{"type":"warning","text":"world pack not found; walking blind"}
```

The control messages above are relayed to the brain the same way.
