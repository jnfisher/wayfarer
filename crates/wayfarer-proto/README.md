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
  `Control`: a UI; `Brain` and `BrainToGateway`: the brain) and `into_relayed()` turns it into the `ToClient` the
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
{"type":"logging_off","session":"kasimir"}
{"type":"user_input","session":"kasimir","text":"a rat","from":"ui-3"}
{"type":"keys_sent","session":"kasimir"}
```

Client → gateway:

```json
{"type":"hello","protocol":1,"client":"ui","token":"TOKEN"}
{"type":"input_line","session":"kasimir","text":"look north"}
{"type":"input_keys","session":"kasimir","bytes":[13]}
{"type":"history_request","session":"kasimir","after_seq":42,"max_chunks":2000}
{"type":"subscribe","sessions":["kasimir"]}
{"type":"session_control","session":"kasimir","action":"log_off"}
{"type":"resync","session":"kasimir"}
{"type":"state_request","session":"kasimir"}
{"type":"walk_to","session":"kasimir","map":1,"room":2150}
{"type":"walk_stop","session":"kasimir"}
{"type":"journey_fight","session":"kasimir","fight":false}
{"type":"map_request","session":"kasimir","radius":15}
{"type":"bot_control","session":"kasimir","action":"start"}
{"type":"loops_request","session":"kasimir","radius":8}
{"type":"loops_request","session":"kasimir","radius":15,"doors":true,"query":"keep courtyard","limit":12}
{"type":"walk_loop","session":"kasimir","id":"1:2140:8:doors:0"}
{"type":"run_loop","session":"kasimir","id":"1:2140:8:doors:0","skip":[{"map":1,"room":2156}]}
{"type":"stats_request","session":"kasimir"}
{"type":"spell_plan_request","session":"kasimir"}
{"type":"stats_reset","session":"kasimir"}
{"type":"settings_request","session":"kasimir","layer":"server"}
{"type":"settings_save","session":"kasimir","layer":"character","changes":[{"key":"health.flee_below_hp","value":35},{"key":"spells.heal"}]}
{"type":"save_loop","session":"kasimir","name":"Hidden Keep with Leo the Quick","base":"HKEPLOOP.mp","stops":[{"map":9,"room":749},{"map":9,"room":870}]}
{"type":"delete_loop","session":"kasimir","id":"custom:hidden-keep-with-leo-the-quick"}
{"type":"find","session":"kasimir","query":"chief"}
{"type":"set_monster_rule","session":"kasimir","pattern":"giant rat","rule":"never_fight"}
{"type":"parties_request"}
{"type":"party_save","party":{"id":"","leader":"kasimir","followers":["isolde","brynja"]}}
{"type":"party_delete","id":"party-1"}
{"type":"party_leave","session":"isolde"}
{"type":"party_join","session":"isolde","party":"party-1"}
{"type":"party_resync"}
{"type":"blessings_request"}
{"type":"blessings_save","session":"isolde","spells":["bles","aura"]}
```

Brain → gateway → UIs (the same JSON both ways):

```json
{"type":"event","session":"kasimir","seq":43,"line_no":88,"name":"you_hit","cat":"combat","fields":{"dmg":"1","target":"fat giant rat","verb":"punch"},"line":"You punch fat giant rat for 1 damage!"}
{"type":"character","session":"kasimir","state":{"name":"Kasimir","race":"Human","class":"Warrior","level":3,"hp":30,"hp_max":35,"exp":1520,"stats":{"strength":60,"agility":50},"room_title":"Newhaven, Village Entrance","room_exits":["north","south"],"in_combat":false,"statuses":[],"prompt_form":"hp","parse":{"lines":412,"unclassified":3},"synced_at":"2026-10-19T12:00:05Z","updated_seq":43,"coins":{"copper":320,"carried":[{"coin":"silver nobles","count":32}]},"carrying":["padded vest","dagger"],"encumbrance":{"current":520,"max":3792,"level":"None"},"kills":4,"position":{"map":1,"room":2140,"candidates":1,"title":"Newhaven, Village Entrance"},"walking":{"to":{"map":1,"room":2150},"name":"Newhaven, Arena","steps_left":3,"step":1,"commands":["w","w","d"]},"bot":{"state":"running","mode":"fight","narration":"Fight: large giant rat","doing":"attacking large giant rat","journey":{"name":"Newhaven, Arena","to":{"map":1,"room":2150}},"monster_rules":[{"pattern":"gigantic black ooze","rule":"flee_from","source":"default"}]}}}
{"type":"character_patch","session":"kasimir","patch":{"hp":23,"in_combat":true,"room_title":null}}
{"type":"brain_info","build":"0.1.0 (3f2a9c1)","started_at":"2026-10-19T12:00:00Z","world_pack":{"path":"worlds/stock-1.11p.sqlite","hash":"sha256:5d41402abc4b2a76"}}
{"type":"notice","session":"kasimir","text":"re-synced"}
{"type":"warning","text":"world pack not found; walking blind"}
{"type":"map_view","session":"kasimir","center":{"map":1,"room":2140},"level":0,"rooms":[{"map":1,"room":2140,"name":"Newhaven, Village Entrance","x":0,"y":0,"level":0,"spawns":false,"visited":true,"candidate":true,"collides":false},{"map":1,"room":2141,"name":"Newhaven, Weapons Shop","x":0,"y":-1,"level":0,"spawns":false,"kind":"shop","visited":false,"candidate":false,"collides":false}],"exits":[{"from":{"map":1,"room":2140},"to":{"map":1,"room":2141},"dir":"n","kind":"plain","one_way":false}],"route":[{"map":1,"room":2140},{"map":1,"room":2141}]}
{"type":"loops","session":"kasimir","anchor":{"map":1,"room":2140},"radius":8,"doors":true,"generated":[{"id":"1:2140:8:doors:0","name":"Newhaven","stops":[{"map":1,"room":2150,"name":"Newhaven, Arena","monsters":["giant rat"],"arena":true,"lair":false,"regen":0,"bosses":[]},{"map":1,"room":2152,"name":"Dungeon, Entrance","monsters":["giant rat"],"arena":false,"lair":true,"regen":1,"bosses":[]}],"path":[{"map":1,"room":2150},{"map":1,"room":2152},{"map":1,"room":2150}],"steps":2,"doors":1,"distance":1}],"megamud":[{"file":"NARNLOO2.mp","description":"Newhaven Arena","author":"Spork ZSpoon","path":[{"map":1,"room":2150},{"map":1,"room":2152},{"map":1,"room":2150}],"steps":2,"exact":2,"stops":[{"map":1,"room":2150,"name":"Newhaven, Arena","monsters":["giant rat"],"arena":true,"lair":false,"regen":0,"bosses":[]},{"map":1,"room":2152,"name":"Dungeon, Entrance","monsters":["giant rat"],"arena":false,"lair":true,"regen":1,"bosses":[]}],"runnable":true,"distance":1}],"custom":[],"matches":2}
{"type":"stats","session":"kasimir","runs":[{"started_at":"2026-10-19T12:00:00Z","minutes":2,"exp_per_hour":540,"kills":2,"deaths":0,"flees":0,"rests":1,"exp_gained":18}],"samples":[{"at":"2026-10-19T12:01:00Z","exp":355,"hp":30}],"combat":{"since":"2026-10-24T12:00:00Z","swings":4,"accuracy_per_mille":750,"crit_rate_per_mille":333,"rows":[{"kind":"hit","count":2,"per_mille":500,"min":21,"max":84,"avg_tenths":525}],"misses":1,"dodged":0,"glances":0,"backstab_tries":0,"backstab_crits":0,"cast_misses":0,"cast_accuracy_per_mille":0,"kills":1,"flees":0,"lowest_hp":50},"time":{"fight_secs":52,"flee_secs":0,"rest_secs":30,"walk_secs":25,"upkeep_secs":3,"wait_secs":10,"paused_secs":0,"away_secs":0}}
{"type":"spell_plan","session":"kasimir","cash":520,"items":[{"spell":"mend","spell_level":3,"scroll":"scroll of mend","shop":"Newhaven Spell Shop","map":1,"room":2144,"price":800,"steps":12,"usable":true,"affordable":false}]}
{"type":"settings","session":"kasimir","layer":"character","files":[{"layer":"mine","path":"/data/settings.toml","exists":false},{"layer":"character","path":"/data/characters/kasimir.toml","exists":true}],"rows":[{"key":"health.flee_below_hp","label":"Flee below","help":"Leave the fight and run when HP falls below this.","group":"health","tier":"essential","kind":{"kind":"int","min":0,"max":90},"unit":"%","default":25,"value":35,"source":"character","own":35,"megamud":"HP Run If Below"}]}
{"type":"found","session":"kasimir","query":"chief","results":[{"kind":"monster","name":"minotaur chieftain","rooms":[{"map":1,"room":1865,"name":"Large Wooden Room","steps":46,"boss":true}],"count":1,"monsters":["minotaur chieftain"]}]}
{"type":"parties","parties":[{"party":{"id":"party-1","leader":"kasimir","followers":["isolde","brynja"]},"members":[{"session":"kasimir","state":"joined","rank":"front"},{"session":"isolde","state":"joined","rank":"back"},{"session":"brynja","state":"not_joined","detail":"in another room"}]}]}
{"type":"blessings","characters":[{"session":"isolde","spells":[{"short":"bles","name":"bless","mana":4,"chosen":true,"on":true},{"short":"aura","name":"holy aura","mana":15,"chosen":false,"on":false}]}]}
```

The control messages above are relayed to the brain the same way.

Brain → gateway (the answer to `logging_off`, and which sessions' input the brain
arbitrates; never relayed):

```json
{"type":"ready_to_leave","session":"kasimir"}
{"type":"arbitrate","session":"kasimir","on":true}
```
