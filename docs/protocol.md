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
| `user_input {session, text, force?}` | A line as `input_line`, with `force: true` for the UI's `send anyway` of a line the brain held (`input_held`, below) |

```json
{"type":"input_line","session":"kasimir","text":"look north"}
{"type":"input_keys","session":"kasimir","bytes":[13]}
{"type":"user_input","session":"kasimir","text":"a rat","force":true}
```

Any client may send input, the brain included. It goes straight to the game, except a UI's
line for a session the brain **arbitrates** (its bot is running or paused, and it said so with
`arbitrate`): that goes to the brain as `user_input`, and the brain decides when it goes and
what the bot does meanwhile ([brain.md](brain.md) "Typing beside the bot"). Keys always go
straight through; for an arbitrated session the brain is told (`keys_sent`) and pauses the bot.
When the brain goes, every session's input goes straight through again at once; a new brain
arbitrates nothing until it says so.

## The brain

A brain connection receives everything a UI does (output, session states, history, for every
session unless it subscribes), and also `sent`: each line that went to the game, whoever sent
it. A session's `sent` and `output` reach the brain in the order they happened: a brain that
receives `sent {seq: n}` has received every chunk up to `n` and none after, so the echo of a
typed line always comes after its `sent`. UIs never get `sent`.

| Message | Direction | Meaning |
|---|---|---|
| `sent {session, seq, text}` | gateway → brain | A line went to the game: typed by any client (`input_line`; not `input_keys`), or sent by a logon or log-off step, after it was written to the socket. `seq` is the session's latest output `seq` at that moment. A password, or a typed line equal to it, is `********` |
| `brain_state {attached}` | gateway → UIs | Whether a brain is attached: right after `welcome`, then whenever one attaches or leaves |
| `brain_lagged {missed}` | gateway → UI | The UI read too slowly and `missed` relayed brain messages were dropped for it. Recover with `state_request` |
| `logging_off {session}` | gateway → brain | The gateway is stopping and is about to log this session off (sent only for a session in the realm). The brain stops playing the character and answers `ready_to_leave` |
| `ready_to_leave {session}` | brain → gateway | The brain sends the session nothing more: the log-off may start. The gateway waits for it up to 30 s after `logging_off`, or until the brain goes; never relayed, and refused from a UI |
| `arbitrate {session, on}` | brain → gateway | Whether the brain arbitrates the session's input: on while its bot runs or is paused, off when it stops. Never relayed, and refused from a UI |
| `user_input {session, text, from, force?}` | gateway → brain | A line a UI typed (`input_line`, or `user_input` with `force`) for a session the brain arbitrates; the gateway sent nothing to the game. `from` names the UI's connection (`ui-3`). The brain's own `input_line` is never turned into one |
| `keys_sent {session}` | gateway → brain | A UI's `input_keys` went to the game for a session the brain arbitrates |

```json
{"type":"sent","session":"kasimir","seq":42,"text":"********"}
{"type":"brain_state","attached":true}
{"type":"brain_lagged","missed":17}
{"type":"logging_off","session":"kasimir"}
{"type":"ready_to_leave","session":"kasimir"}
{"type":"arbitrate","session":"kasimir","on":true}
{"type":"user_input","session":"kasimir","text":"a rat","from":"ui-3","force":true}
{"type":"keys_sent","session":"kasimir"}
```

When the gateway stops, the brain's connection (and every UI's) stays open until every
log-off has finished or given up; only then does the gateway close it.

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
| `map_view {session, center, level, rooms, exits, route?}` | The rooms around the believed position, for the map panel (below) |
| `loops {session, anchor, radius, doors, generated, megamud}` | The answer to a `loops_request`: the loops of the area (below) |
| `stats {session, runs, samples}` | The answer to a `stats_request`: the character's last ten runs, newest first, and the newest run's samples (below) |
| `input_held {session, text, reason}` | A line the player typed was not sent: it would break a rule while the bot plays (an attack while combat is engaged). The UI shows it with a `send anyway`, which sends `user_input` with `force` |
| `found {session, query, results}` | The answer to a `find`: rooms, well-known places and monsters by name, best first (below) |

```json
{"type":"event","session":"kasimir","seq":43,"line_no":88,"name":"you_hit","cat":"combat","fields":{"dmg":"1","target":"fat giant rat","verb":"punch"},"line":"You punch fat giant rat for 1 damage!"}
{"type":"character","session":"kasimir","state":{"name":"Kasimir","race":"Human","class":"Warrior","level":3,"hp":30,"hp_max":35,"exp":1520,"stats":{"strength":60,"agility":50},"room_title":"Newhaven, Village Entrance","room_exits":["north","south"],"in_combat":false,"statuses":[],"prompt_form":"hp","parse":{"lines":412,"unclassified":3},"synced_at":"2026-10-19T12:00:05Z","updated_seq":43,"coins":{"copper":320,"carried":[{"coin":"silver nobles","count":32}]},"carrying":["padded vest","dagger"],"encumbrance":{"current":520,"max":3792,"level":"None"},"kills":4,"position":{"map":1,"room":2140,"candidates":1,"title":"Newhaven, Village Entrance"},"walking":{"to":{"map":1,"room":2150},"name":"Newhaven, Arena","steps_left":3,"step":1,"commands":["w","w","d"]},"bot":{"state":"running","mode":"fight","narration":"Fight: large giant rat","monster_rules":[{"pattern":"gigantic black ooze","rule":"flee_from","source":"default"}]}}}
{"type":"character_patch","session":"kasimir","patch":{"hp":23,"in_combat":true,"room_title":null}}
{"type":"brain_info","build":"0.1.0 (3f2a9c1)","started_at":"2026-10-19T12:00:00Z","world_pack":{"path":"worlds/stock-1.11p.sqlite","hash":"sha256:5d41402abc4b2a76"}}
{"type":"notice","session":"kasimir","text":"re-synced"}
{"type":"warning","text":"world pack not found; walking blind"}
{"type":"map_view","session":"kasimir","center":{"map":1,"room":2140},"level":0,"rooms":[{"map":1,"room":2140,"name":"Newhaven, Village Entrance","x":0,"y":0,"level":0,"spawns":false,"visited":true,"candidate":true,"collides":false},{"map":1,"room":2141,"name":"Newhaven, Weapons Shop","x":0,"y":-1,"level":0,"spawns":false,"kind":"shop","visited":false,"candidate":false,"collides":false}],"exits":[{"from":{"map":1,"room":2140},"to":{"map":1,"room":2141},"dir":"n","kind":"plain","one_way":false}],"route":[{"map":1,"room":2140},{"map":1,"room":2141}]}
{"type":"loops","session":"kasimir","anchor":{"map":1,"room":2140},"radius":8,"doors":true,"generated":[{"id":"1:2140:8:doors:0","name":"Newhaven","stops":[{"map":1,"room":2150,"name":"Newhaven, Arena","monsters":["giant rat"],"arena":true,"lair":false,"regen":0,"bosses":[]},{"map":1,"room":2152,"name":"Dungeon, Entrance","monsters":["giant rat"],"arena":false,"lair":true,"regen":1,"bosses":[]}],"path":[{"map":1,"room":2150},{"map":1,"room":2152},{"map":1,"room":2150}],"steps":2,"doors":1}],"megamud":[{"file":"NARNLOO2.mp","description":"Newhaven Arena","author":"Spork ZSpoon","path":[{"map":1,"room":2150},{"map":1,"room":2152},{"map":1,"room":2150}],"steps":2,"exact":2,"stops":[{"map":1,"room":2150,"name":"Newhaven, Arena","monsters":["giant rat"],"arena":true,"lair":false,"regen":0,"bosses":[]},{"map":1,"room":2152,"name":"Dungeon, Entrance","monsters":["giant rat"],"arena":false,"lair":true,"regen":1,"bosses":[]}],"runnable":true}],"custom":[]}
{"type":"stats","session":"kasimir","runs":[{"started_at":"2026-10-19T12:00:00Z","minutes":2,"exp_per_hour":540,"kills":2,"deaths":0,"flees":0,"rests":1,"exp_gained":18}],"samples":[{"at":"2026-10-19T12:01:00Z","exp":355,"hp":30}]}
{"type":"input_held","session":"kasimir","text":"a rat","reason":"no attack command while combat is engaged"}
{"type":"found","session":"kasimir","query":"chief","results":[{"kind":"monster","name":"minotaur chieftain","rooms":[{"map":1,"room":1865,"name":"Large Wooden Room","steps":46,"boss":true}],"count":1,"monsters":["minotaur chieftain"]}]}
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
| `coins` | `{copper?, carried}`: the total value in copper farthings (the inventory's `Wealth:` line) and each kind carried, `[{coin, count}]` (`silver nobles`, 32) |
| `carrying` | The items carried, by name, as the last inventory listed them |
| `encumbrance` | `{current?, max?, level?}`: weight carried, what the character can carry, and the game's word for it (`None`, `Light`, ...) |
| `kills` | Monster deaths seen since the brain attached |
| `position` | `{map?, room?, candidates, title?}`: the room the brain believes the character is in (the lowest key among the candidates; absent when unknown), how many rooms fit what was seen (1 is certain), and the last room title seen. Absent without a world pack |
| `bot` | `{state, mode, narration, reason?, task?, monster_rules}`, with a world pack: `state` is `stopped` (the default), `running` or `paused`; `mode` is `idle`, `fight`, `recover`, `flee` or `task`; `task`, while it runs a loop (`run_loop`), is `{loop_id, loop_name, stop, stops, skip}`: `stop` the index into `stops` (`[{map, room}]`, in loop order) of the stop it is at or walking to, `skip` the stops it passes by; `narration` is what it is doing ("Fight: large giant rat", "Recover: resting to 95% in Newhaven, Narrow Road", "Paused: you took over"); `reason` why it stopped or paused, when it did so itself or for the player ([brain.md](brain.md) "Fight here"); `monster_rules` the rules it plays by, `[{pattern, rule, source}]`: `rule` is `never_fight`, `flee_from` or `avoid`, `source` is `default` (wayfarer's own list), `session` (learned this session: a stalemate) or `player` (`set_monster_rule`, the brain's `--flee-from`) ([brain.md](brain.md) "Monster rules") |
| `run` | `{run_started_at, exp_per_hour, kills, deaths, flees, rests, exp_gained}` while the bot has a run (from Start to Stop or a death; [brain.md](brain.md) "Statistics"): when it started (RFC 3339), exp gained over the wall time since then (a minute at least), and what the run has counted. Absent when there is none |
| `walking` | `{to, name, steps_left, step, commands, short_of?}` while a walk is in progress: the destination (`{map, room}`) and its name, the steps not yet confirmed, the step in progress from 1 (counted over the whole walk, replans included), and what the walker will type for each step left, the one in progress first (`n`, or a text exit's phrase such as `borrow skiff`; a closed door adds an `open` not listed). `short_of`, when the walk stops one room short of a boss lair (`walk_to`'s `stop_short`): `{map, room, name, bosses}`, the lair, with `to` the room before it. Absent when not walking |

`hp` follows the prompt. Between prompts the brain also applies `You took 3 damage.` (lowered),
a healer's `all your wounds are healed` (to `hp_max`) and the character's own `drops to the
ground!` (at most 0, `in_combat` false); the next prompt confirms or corrects them.
`poisoned` comes from `You feel ill.` and the status screen and goes with a cure, death or a
status screen without it; `blind` likewise, without the cure.

The map view: the rooms within `radius` steps of the believed room (15 unless a
`map_request` asked for another), laid out on a grid with the believed room at `center`,
(0, 0) on level 0. It goes out when the believed room changes, when a walk's route starts,
changes or ends, and on `map_request`.

| Field | What |
|---|---|
| `center`, `level` | The believed room (`{map, room}`), and its level: the floor to show first |
| `rooms` | `[{map, room, name, x, y, level, spawns, kind?, visited, candidate, collides}]`: `x` grows east, `y` south, `level` up. `spawns`: monsters come here on their own. `kind`: `shop`, `trainer`, `bank`, `arena` or `lair`, from the room's type and shop and the MegaMud waypoint resolved to it. `visited`: the character was certainly here since the brain started. `candidate`: one of the rooms the character may be in. `collides`: its cell was already taken; drawn there, nothing placed beyond it |
| `exits` | `[{from, to, dir, kind, one_way}]` between rooms of the view, a two-way pair once. `dir` is `n` ... `d`; `kind` is `plain`, `door`, `hidden` or `text` (a phrase such as `borrow skiff`) |
| `route` | While walking: the rooms still to pass, from the one the character is in to the destination |

The loops of an area ([brain.md](brain.md) "Loops"): the area is the rooms within `radius`
steps of `anchor` by exits the character may take (through doors unless `doors` is false).

| Field | What |
|---|---|
| `generated` | `[{id, name, stops, path, steps, doors}]`: closed circuits through the area's hunt rooms. `id` is what `walk_loop` takes. `stops` in walking order, each `{map, room, name, monsters, arena, lair, regen, bosses}` (the huntable monsters it spawns by name; `regen`: minutes nothing spawns after a kill, arenas exempt; `bosses`: `[{name, regen_hours, up_in_minutes?}]`, the bosses spawning there and whether each should be up, from this character's own kills: `up_in_minutes` is absent when it should be up now, never killed by this character or its time passed; a guess, other players kill bosses too). `path`: every room passed, from the first stop back to it; `steps` and `doors` along it |
| `megamud` | `[{file, description, author, path, steps, exact, stops, runnable, reason?}]`: MegaMud's loops whose verification walk passes a room of the area, only those that walked to the end and closed in their start room. `file` is what `walk_loop` and `run_loop` take. `path`: the rooms the walk passed; `steps` it checked, `exact` of them matching MegaMud's room id exactly. `stops`: the hunt rooms on the path, in path order and each once, shaped as a generated loop's. `runnable`: whether `run_loop` runs it for this character; when not, `reason` says why ("nothing to hunt on it", or "no way from A to B: ..." for a leg between two stops no route takes) ([brain.md](brain.md) "Loops"). Empty unless the world pack was built with MegaMud's paths |
| `custom` | `[{id, name, stops, path, steps, doors}]`: the loops the player made (`save_loop`) with a stop in the area, by name, shaped as a generated loop's; `id` is `custom:` and a slug of the name. `run_loop` and `walk_loop` take it ([brain.md](brain.md) "Custom loops") |

What a `find` found ([brain.md](brain.md) "Go to"): at most 30 results, those with no way there
last, then the better match of the name (the whole name, its start, the start of a word,
anywhere), then the nearer, then places before monsters before room names.

| Field | What |
|---|---|
| `kind` | `rooms` (every room with this exact name), `shop`, `bank`, `trainer` (a shop room by its name, or a MegaMud waypoint), `boss` or `place` (another MegaMud waypoint), or `monster` (with the rooms it spawns in) |
| `name` | The room name, the place's name or the monster's |
| `rooms` | `[{map, room, name, steps?, no_route?, boss}]`, the nearest 20 first: `steps` the route length from the character's room when within 200 steps, `no_route` when there is no way there; neither: farther, or the character's room is unknown. `boss`: a boss lair, which `walk_to` stops one room short of by default |
| `count` | How many rooms it names in all |
| `monsters` | The bosses of its boss lairs (for a monster, itself if it is one), by name; absent when none |

The statistics of a character ([brain.md](brain.md) "Statistics"), from the brain's store:

| Field | What |
|---|---|
| `runs` | `[{started_at, ended_at?, loop_id?, minutes, exp_per_hour, kills, deaths, flees, rests, exp_gained}]`: the last ten runs, newest first. `ended_at` is absent while the run goes on; `loop_id` names the loop it ran (absent for "fight here"); `minutes` are whole minutes from start to end (or to now) |
| `samples` | `[{at, exp?, hp?}]`: the newest run's samples of the character's exp and HP, one a minute, oldest first |

### Control (UI → brain)

Relayed to the brain. With no brain attached the gateway answers `error` `no brain attached`.

| Message | Meaning |
|---|---|
| `resync {session}` | Re-sync that character (`st`, `exp`, `i`, look) |
| `state_request {session?}` | Send a fresh `character` snapshot (of one session, or of all when `session` is left out) and a `brain_info` |
| `walk_to {session, map, room, stop_short?}` | Walk the character to that room, confirming each step ([brain.md](brain.md) "Walking"); replaces a walk in progress. `stop_short`: stop in the room before it when it is a boss lair ([brain.md](brain.md) "Go to"); left out, on for a boss lair; it changes nothing for another room |
| `walk_stop {session}` | Stop walking after the move in flight |
| `map_request {session, radius?}` | Send a `map_view` now, of the rooms within `radius` steps (15 when left out); later views use the same radius |
| `bot_control {session, action}` | The bot ([brain.md](brain.md) "Fight here"): `start` (fight here: re-sync, then fight what comes and rest when low), `stop`, `pause` (after the action in flight) or `resume` (re-sync, then carry on) |
| `loops_request {session, anchor?, radius, doors?}` | Send the `loops` of the area within `radius` steps of `anchor` (`{map, room}`; the believed room when left out), through doors unless `doors` is false |
| `walk_loop {session, id}` | Walk a loop once (`id`: a generated loop's id, or a MegaMud loop's file) ([brain.md](brain.md) "Loops"): to its stop nearest the character, round every other stop and back to that one, each leg a `walk_to`; `walk_stop` stops it and a `walk_to` replaces it |
| `run_loop {session, id, skip?}` | Run a loop (`id`: a generated loop's id, or a MegaMud loop's file, `Mwodloop.mp`; one listed not `runnable` is refused with its reason) ([brain.md](brain.md) "Running a loop"), replacing a running "fight here": the bot re-syncs, walks to the loop's ticked stop nearest the character, then round its stops in loop order, fighting at each, passing by the stops in `skip` (`[{map, room}]`) and those whose respawn delay has not passed. Sent again for the loop the bot already runs, it only changes `skip` (left out: none). `bot_control` `stop`, `pause` and `resume` apply |
| `save_loop {session, name, base?, stops}` | Make or change a custom loop: `name` (1 to 60 characters, with a letter or a digit) and its `stops` (`[{map, room}]`, 1 to 300, in order; every room must exist), made from the loop `base` (an id or a MegaMud file, kept for reference). The id is `custom:` and a slug of the name, so the same name again replaces the loop. The brain answers with a notice ("Saved loop X (N stops): custom:x" or "Changed loop ...") or a warning, and the loop is in the next `loops` message ([brain.md](brain.md) "Custom loops") |
| `delete_loop {session, id}` | Delete a custom loop (`custom:...`) |
| `stats_request {session}` | Send a `stats` message: the character's last runs and the newest run's samples |
| `stats_reset {session}` | End the current run and start a new one now, its numbers (exp/hr, kills, deaths, flees, rests) at zero; the ended run stays in the history. A notice says so and a `stats` message follows; with no run (the bot stopped) a warning says so |
| `find {session, query}` | Send a `found` message: the rooms, well-known places and monsters whose name holds `query` (two characters or more, any case) |
| `set_monster_rule {session, pattern, rule}` | Set the player's monster rule for names that hold `pattern` (any case): `never_fight`, `flee_from` or `avoid` ([brain.md](brain.md) "Monster rules"); `null` removes it. Stored with the bot's record; the bot's `monster_rules` shows the result |

```json
{"type":"resync","session":"kasimir"}
{"type":"state_request","session":"kasimir"}
{"type":"walk_to","session":"kasimir","map":1,"room":2150}
{"type":"walk_stop","session":"kasimir"}
{"type":"map_request","session":"kasimir","radius":15}
{"type":"bot_control","session":"kasimir","action":"start"}
{"type":"loops_request","session":"kasimir","radius":8}
{"type":"walk_loop","session":"kasimir","id":"1:2140:8:doors:0"}
{"type":"run_loop","session":"kasimir","id":"1:2140:8:doors:0","skip":[{"map":1,"room":2156}]}
{"type":"stats_request","session":"kasimir"}
{"type":"stats_reset","session":"kasimir"}
{"type":"save_loop","session":"kasimir","name":"Hidden Keep with Leo the Quick","base":"HKEPLOOP.mp","stops":[{"map":9,"room":749},{"map":9,"room":870}]}
{"type":"delete_loop","session":"kasimir","id":"custom:hidden-keep-with-leo-the-quick"}
{"type":"find","session":"kasimir","query":"chief"}
{"type":"set_monster_rule","session":"kasimir","pattern":"giant rat","rule":"never_fight"}
{"type":"set_monster_rule","session":"kasimir","pattern":"giant rat","rule":null}
{"type":"walk_to","session":"kasimir","map":1,"room":1865,"stop_short":false}
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
