# The brain

The brain is the part of wayfarer that understands the game. It attaches to a running gateway
as its one brain client, parses every session's output, keeps a model of each character and
sends the UIs events and snapshots of it. With a world pack it also knows where each character
is, serves the map panel, walks a character to a room when a UI asks ("Walking", below), and
plays a character where it stands when the player presses Start ("Fight here"). Otherwise it
plays nothing: the lines it sends to the game are a re-sync's, a walk's and the bot's. It can
be stopped and started at any time; the gateway keeps the characters logged in meanwhile.

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
  session's or all) and `brain_info` again; `walk_to` and `walk_stop` walk ("Walking");
  `bot_control` starts, stops, pauses and resumes the bot ("Fight here").
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
  (as the model knows them): the shortest route, through rooms that spawn monsters as readily
  as any other (a setting later), avoiding only the exits the game refused this session. Narrated: "Walking to Newhaven,
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
break them: no attack command (`a`, `bash rat`, `bs`, ...) unless the character is known to be
out of combat (engaged, or not known either way, refuses it), and never `train stats`
(`docs/plan/01-goals.md` "Rules").

## Loops

The first of the three loop sources (`docs/plan/03-engine.md` section 4): loops generated from
the world data, beside MegaMud's verified loops. Nothing runs them yet (the Task mode, M3,
will); a UI lists them and walks one once so the player can watch the route.

`loops_request {session, anchor?, radius, doors?}` answers with `loops` (`docs/protocol.md`):

- **The area**: the rooms within `radius` steps (at most 30) of `anchor`, the believed room
  by default, by exits the character may take: a gated exit outside its level, class or race
  leaves what is behind it out, and with `doors: false` so does every door.
- **Hunt rooms**: the area's rooms whose spawn pool (`genmonster`, or the room's `mongroup`
  within its `minlevel`..`maxlevel`, plus its `specmon`; only Normal, Arena and Lair rooms
  spawn) holds a monster worth hunting (evil, not an NPC, worth exp). A shop whose pool is its
  shopkeeper is none. Each stop says its huntable monsters, whether it is an Arena (monsters
  back at once after a kill) or a Lair, and its `regendelay`.
- **Generated loops**: a closed circuit through the hunt rooms, nearest neighbour from the stop
  nearest the anchor, then 2-opt; each leg the character's cheapest route between two stops
  inside the area. One loop over every hunt room; when the area splits (stops that can't
  reach each other inside it: the river off Newhaven is one-way above level 3) one more per
  part of two or more stops, and the loop over all of them may leave the area between parts,
  or is left out when it can't close. Named after the anchor's place ("Newhaven"; a part adds
  its first stop: "Newhaven: The Silver River, Silvermere Docks"). Within 8 steps of Newhaven,
  Village Entrance: the Arena, the Dungeon Entrance behind its locked door and the Small Cavern
  north of that, 4 steps; without doors, the Arena alone.
- **MegaMud's loops** through the area: those whose verification walk passes one of its rooms,
  only ever ones that walked to the end and closed in their start room. None unless the world
  pack was built with MegaMud's paths.
- Made once per anchor, radius, level and doors for the session, so asking again is cheap.

`walk_loop {session, id}` walks a generated loop once: a `walk_to` to its stop nearest the
character, then to each stop in turn, and back to that first one. Narrated: "Walking loop
Newhaven: 3 stops, 4 steps, from Newhaven, Arena", the walker's own lines for each leg, "Loop
stop 2/3: Dungeon, Entrance" at each stop, and "Loop Newhaven done: back at Newhaven, Arena".
Whatever stops a walk stops the loop ("Loop Newhaven stopped: combat started"), and a
`walk_to` replaces it. The `id` says the area and the loop, so a restarted brain walks the same
one. MegaMud's loops are listed, not walked, until the Task mode runs them.

## Fight here

The first slice of the bot (`docs/plan/03-engine.md` sections 4 and 5). With the character
standing in a room the player chose, **Start** (`F10` or the tile's `[►]`, `bot_control
start`) makes it fight what comes there, rest when low, and stop itself when HP is
dangerously low. Every character starts **stopped**; the snapshot's `bot` says `{state, mode,
narration, reason?}`, and every change of what the bot is doing is a `notice` (its narration).
It needs a world pack (who is worth fighting, which rooms spawn) and the realm.

- **Start** re-syncs first (`st` for the HP maximum, `look` for who is here), remembers the room
  as *home*, then decides at every prompt with nothing in flight. If the room is not known at
  Start (a brain just attached, its history showing none), home is the first room the bot
  believes it is in after that, narrated "Playing from <room>".
- **The modes**, highest first; the first that wants control has it:

  | Mode | Wants control when | Does |
  |---|---|---|
  | Recover | HP below 80% (or poisoned) and not fighting; or a rest it began is not finished | Rests: in this room if nothing spawns here; else walks (through the walker, each step confirmed) to the nearest room within 5 steps where nothing spawns, and rests there; else rests here anyway. `rest` again when the next prompt lacks `(Resting)` (three times, then a 30 s wait; `You are too sick to rest!` waits 30 s too). Rested at 95%, it walks back home |
  | Fight | Engaged; or a monster worth fighting is here | Picks one that is attacking the character first, else the first listed; sends `a <its last word>` once and lets auto-combat run. After `*Combat Off*` or a kill it looks (`look`) and picks the next. A target the game refuses (`You do not see rat here!`, `Your weapon has no effect`) is left alone for the rest of the session. Never leaves the room |
  | Idle | Otherwise | Sends nothing: "Waiting here" |

- **Worth fighting**: every record the name finds (adjectives stripped: `fat giant rat` is a
  giant rat) is evil, not an NPC and worth experience (`kb/pitfalls.md` 1). **A room spawns**
  when the spawn pass makes monsters there or it holds one worth fighting; a room the position
  is not sure of counts as spawning. A shopkeeper's room is quiet.
- **Engaged** comes from `*Combat Engaged*` and `*Combat Off*`. Auto-combat can also end without
  a word (pitfall 42): 12 s with none of the character's own blows and it counts as over. A
  monster's blows are not the character's: one attacking a character that is not fighting
  back is fought at once. Before the brain has seen either, the character counts as engaged
  until 12 s pass without its blows.
- **A monster that attacks while resting** wakes the bot: Fight takes the character (in the
  quiet room, if it is there), then Recover carries on and walks home when rested. One that
  arrives and doesn't attack leaves the rest alone.
- **Stop in place of Flee**: below 30% HP the bot stops itself with a warning, "Stopped at 24%
  HP: flee is not built yet; fight or run by hand". Auto-combat goes on in the game: the fight
  is the player's. So does dying (`You have been killed!`) and leaving the realm.
- **Pause and take-over**: a line the player types (not the brain's own) while the bot runs
  pauses it after the action in flight: "You took over Waydeva", then "Paused: you took over".
  So do a `walk_to` from the map and `pause` (`F9`, `[║]`). Paused, it sends nothing. **Resume**
  (`F9`, `[►]`) re-syncs first, then narrates what it does next. **Stop** (`F10`, `[■]`)
  sends nothing more; a walk of the bot's stops after the move in flight.
- **Constants** in this slice, settings in M4 (`docs/plan/04-settings.md` section 3): rest
  below 80%, rest until 95%, stop below 30%, a quiet room within 5 steps
  (`crates/wayfarer-brain/src/policy.rs`).

Not in this slice: coins and loot, healing spells and healers, bash and the other attacks,
flee, "guard while paused" (paused means nothing is sent), and a fight the bot thinks it
cannot win.

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
re-sync; the bot's state is kept only while the brain runs (a restarted brain's bots are
stopped). The store arrives with the first card that has state to keep across restarts (the
bot's modes and plans, M2 onward); `docs/plan/03-engine.md` section 7 lists what it will
hold.

## Not yet

No modes beyond Fight, Recover and Idle (no Flee, Maintain or Task); no input arbitration
(every client's input still goes straight to the game, `docs/protocol.md`; a typed line stops
a walk and pauses the bot); moves are not pipelined along a known route (one in flight); the
refused exits are forgotten when the brain restarts; MegaMud's loops and player-recorded ones
are not walked (only generated loops are, once).
