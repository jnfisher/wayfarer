# The brain

The brain is the part of wayfarer that understands the game. It attaches to a running gateway
as its one brain client, parses every session's output, keeps a model of each character and
sends the UIs events and snapshots of it. With a world pack it also knows where each character
is, serves the map panel, and walks a character to a room when a UI asks ("Walking", below).
Otherwise it plays nothing: the lines it sends to the game are a re-sync's and a walk's. It
can be stopped and started at any time; the gateway keeps the characters logged in meanwhile.

## Running it

```sh
# once: the world pack, from the stock-world export in this repository
wayfarer import-world --export tests/fixtures/world --out "$WAYFARER_DATA_DIR/world.sqlite"

# with the gateway already running (scripts/dev/gateway.sh)
wayfarer brain
```

In a release, one process runs both: `wayfarer serve` takes the gateway's flags plus `--world`
and attaches the brain to the gateway over in-process channels, with no socket. The UIs see
the same `brain_state` and messages as with two processes, and stopping `serve` logs everyone
off as `gateway` does. With no world pack at `--world` (default: `world.sqlite` in the data
directory) it warns, prints the `import-world` command and runs the gateway alone. In
development the two stay apart (`scripts/dev/brain.sh`, [dev-rig.md](dev-rig.md)) so the
brain restarts without the characters.

`wayfarer brain` takes:

| Flag | Default | What |
|---|---|---|
| `--gateway URL` | `ws://127.0.0.1:7373/ws` | The gateway's WebSocket |
| `--world PATH` | `world.sqlite` in the data directory | The world pack. Missing, the brain stops at once and prints the `import-world` command that builds it |
| `--token TOKEN` | the `gateway-token` file in the data directory | The gateway's token, as `wayfarer gateway --new-token` made it. A gateway on a loopback address checks none |
| `--config PATH` | the user's data directory | The gateway's `wayfarer.toml`; its directory is the data directory |

The data directory is the gateway's: on macOS `~/Library/Application Support/wayfarer/`
(`WAYFARER_DATA_DIR` in the dev scripts). Logs go to stderr (`RUST_LOG` sets the level): one
line when it starts and attaches, one per re-sync. Nothing the player types is logged.

## What it does

- **Attach.** It says `hello` as the brain and sends `brain_info` (its build, start time and
  world pack). For each session it fetches the last 2,000 chunks of output and feeds them to a
  fresh parser *silently*: the character model is filled (so the last prompt's HP and mana are
  known) but no `event` goes out for that old text. Then it sends the whole `character` and
  follows the live output. A session that appears later gets the same.
- **Parse.** Every line of live output goes out as an `event` (with its output `seq` and
  `line_no`). Each line the gateway reports as `sent` is told to the parser, so the game's echo
  of it is read as an echo; the gateway sends a line's `sent` before the output that echoes it.
  The brain's own re-sync lines are told to the parser when they go; their `sent` is matched
  off and not told twice, and one whose `sent` has not come within 10 s (or after a `lagged`,
  or once the re-sync is done) is forgotten, so a line the player types later is never taken
  for the brain's. A line nothing recognises is counted (`parse.unclassified`), never an error.
  If the gateway's output log started again (its `last_seq` is below what the brain fed), the
  parser and the line count start afresh and events go out again from the new log's live
  output.
- **Model.** The character applies each typed event: the prompt's HP, mana and resting mark;
  the status screen (name, lives and CP, race, class, level, HP and mana with their maxima,
  stats, poison); the `exp` screen; the room's title and exits (a dark room has neither); combat
  on and off; exp gained; damage taken and a healer's healing (HP until the next prompt); the
  character's own collapse (HP at most 0, out of combat); monster deaths (`kills`); death and
  lives; poison (until a cure or death) and blindness (until death); inventory, wealth and
  encumbrance (`coins`, `carrying`, `encumbrance`). After each chunk that changed anything it
  sends a `character_patch`, the merge patch from the last snapshot sent.
- **Fell behind.** On `lagged`, or a gap in the live `seq`s, it fetches the missed chunks with
  `history_request` and holds live output until they are in.
- **Control.** `resync` starts a re-sync; `state_request` gets the whole `character` (one
  session's or all) and `brain_info` again.
- **Closed sessions** keep their last snapshot; the UI shows the session's state beside it.

## The re-sync

A re-sync asks the game for what the model may lack: `st`, `exp`, `i`, then `look` (not an
empty line, which redisplays the room only in statline FULL modes, `kb/screen-parsing.md`).
They are the "talk and look" kind of command (`docs/plan/03-engine.md` section 6).

- **When**: for a session in the realm that has not been re-synced since the brain attached;
  when a session enters the realm; when a UI sends `resync`.
- **Only at an idle prompt** for the first command: the last chunk ended in a prompt and
  nothing went to the game for 2 s. Each later command goes at the prompt after the previous
  one's answer (the status screen, the exp line, the inventory, the room), or after 5 s
  without one.
- **The player comes first**: if they type during a re-sync, the rest waits for the next idle
  prompt.
- **Never outside the realm**: nothing is sent while a session is `logon`, `menu`,
  `connected`, `connecting`, `lost` or `closed`. A re-sync asked for then waits for the realm;
  one cut short by leaving the realm starts again from `st`.
- **Done**: a whole `character` with `synced_at`, and a `notice` "re-synced".

## Position and the map

With a world pack the brain keeps, per character, a **position belief**: the rooms the
character may be in (`kb/navigation.md` "Position tracking"). Every room the game shows narrows
it (the title, when it was not lost behind a prompt, and the `Obvious exits`); every move that
goes to the game, the brain's or one the player typed (a direction, or the phrase of a text
exit such as `borrow skiff`), carries it through the exit first. So the map follows a character
played by hand as well as one the brain walks. The snapshot's `position` is the believed room
(the lowest key when several fit), how many fit, and the last title seen.

The map panel gets a `map_view`: the rooms within 15 steps (or what `map_request` asked for)
laid out on a grid with the believed room at (0, 0), each with its name, whether it spawns
monsters, its kind (shop, trainer, bank, arena, lair; from the room data and MegaMud's
waypoints), whether the character has certainly been there and whether it is a candidate; the
exits between them; and the walk's route. One goes out when the believed room changes, when a
route starts, changes or ends, and on `map_request` (`docs/protocol.md`).

## Walking

`walk_to {session, map, room}` from a UI walks the character there; it replaces a walk in
progress (after the move in flight).

- **The route**: the cheapest from the believed room for the character's level, class and race
  (as the model knows them), avoiding rooms that spawn monsters where a way round is a few
  steps longer, and the exits the game refused this session. Narrated: "Walking to Newhaven,
  Arena: 3 steps" (", one door" when there are doors). No route, or an unknown position: a
  warning, and nothing is sent.
- **One step at a time**: each move goes at a prompt, in the realm, and only once the room the
  last one led to has been seen and is the one the route expected. One line is in flight at a
  time: a re-sync waits for the walk's action and the walk for a re-sync command. A room that is
  not the one expected replans from the belief once; a second time stops the walk.
- **Doors**: `The door is Closed!` (shut) sends `open <dir>`, then the move again. `The door is
  closed!` (locked), or `The door is locked.` after `open`, sends `bash <dir>`, at most three
  times; then the exit is marked refused and the route planned again.
- **Refusals** (level, alignment, class or race, not permitted, a spell or item needed, a toll
  not covered, a timed exit): the exit is marked refused for the session and the route planned
  again, at most five times a walk.
- **"There is no exit in that direction!"** means the belief is wrong (`kb/pitfalls.md` 34):
  the brain sends `look`, relocates from the room shown, and plans again. A second one in the
  same walk stops it.
- **Stops**: `walk_stop`, `*Combat Engaged*`, a line the player typed, or no answer to a move
  within 5 s stop the walk after the move in flight (the walker never fights); leaving the realm
  or the brain re-attaching stop it at once. Narrated: "Stopped walking: combat started".
- **Arrival**: "Arrived: Newhaven, Arena"; `walking` leaves the snapshot.

**The guards** live in the executor every action goes through, so nothing the bot ever does can
break them: no attack command (`a`, `bash rat`, `bs`, ...) while combat is engaged, and never
`train stats` (`docs/plan/01-goals.md` "Rules"). Nothing sends either yet.

## Stopping, restarting, the gateway going away

- **Ctrl-C or SIGTERM** closes the WebSocket and exits. Nothing is sent to the game; the
  characters stay where they are. The UIs see `brain_state false`.
- **Starting it again** attaches afresh: catch-up, a whole snapshot, a re-sync for every
  session in the realm. The UIs see `brain_state true`.
- **A second brain** is refused by the gateway (`a brain is already attached`) and exits
  non-zero with that reason, as it does for a wrong token or another protocol version.
- **The gateway goes away**: the brain retries with the web UI's backoff (half a second,
  doubling up to ten) and, when it is back, catches up from the last `seq` it saw (those
  chunks' events do go out: they are new to the UIs) and sends a whole snapshot. It re-syncs
  only a session whose realm state was lost meanwhile: one not in the realm any more, one
  that left and came back, or one whose output went by unseen.

## What it keeps

Nothing on disk. M1 has no plan, loop position, death pile or pause state to keep (John,
2026-10-19), so a restarted brain rebuilds everything from the gateway's history and a
re-sync. The store arrives with the first card that has state to keep across restarts (the
bot's modes and plans, M2 onward); `docs/plan/03-engine.md` section 7 lists what it will
hold.

## Not yet

No bot and no modes beyond walking; no input arbitration (every client's input still goes
straight to the game, `docs/protocol.md`; a typed line stops a walk); moves are not pipelined
along a known route (one in flight); the refused exits are forgotten when the brain restarts.
