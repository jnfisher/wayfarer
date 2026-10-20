# wayfarer-brain

The brain (`docs/brain.md`): attaches to the gateway as its brain client, parses every
session's output, keeps a character model per session, sends the UIs events and snapshots,
re-syncs a character (`st`, `exp`, `i`, `look`) at an idle prompt, and with a world pack keeps
each character's position, serves the map panel (`map_view`), walks a character to a room
(`walk_to`), lists the loops of an area and walks one once (`loops_request`, `walk_loop`),
plays it where it stands (`bot_control`: "fight here"), and runs a loop (`run_loop`: Task
mode). When the gateway is about to log a
character off (`logging_off`) it stops that character's bot, sends it nothing more and answers
`ready_to_leave`. With a store (`Brain::with_store`) it saves each bot's state and counts each
run, and a restarted brain resumes the bots. Depends on `wayfarer-proto`, `wayfarer-parse`,
`wayfarer-world` and `wayfarer-store`, not on the gateway.

```rust
let world = World::open(path)?;                          // game texts, rooms (`Atlas`)
let mut brain = Brain::new(world, Arc::new(SystemClock), token);
let mut link = WsLink::connect("ws://127.0.0.1:7373/ws").await?;
brain.run(&mut link).await?;                             // Ok when the gateway goes; run again
```

- **`Link`**: `send` (never waits: a task of the link's own writes) and `recv`. `WsLink` over
  the WebSocket; `ChannelLink` over a pair of tokio channels (`Hub::connect`'s), for tests and
  the in-process brain of `wayfarer serve`.
- **`Brain::run`**: one task and one loop, `tokio::time` for its timers (tests pause the clock).
  The brain's sessions, parsers and characters live in `Brain` across runs, so a reattach
  carries on from the last `seq` seen. `Err(Refused)` when the gateway says no.
- **`Character`**: `apply(&GameEvent)` and `snapshot() -> CharacterState`; `merge_patch` and
  `apply_patch` (RFC 7386) between snapshots.
- **The re-sync** (`session.rs`): `Off`, `Want {next, idle}`, `Waiting {step, deadline}`.
- **`executor`**: an `Action` is a command, the events that confirm it (`Expect`) and a
  timeout; one in flight per session, sent at a prompt in the realm; outcomes confirmed,
  refused or timed out. A move may go *ahead* of the move in flight (`submit_ahead`,
  `take_ahead`: Task's pipelined legs), never anything else. The hard guards (`guard`): no attack unless known to be out of combat,
  never `train stats`. The re-sync and the executor never have a line in flight at once; the
  session knows whose action is in flight (the walker's or the bot's) and hands it the outcome.
- **`commands`**: the game's command words as runs of prefixes (`TABLE`, from silvermere's
  `commands/table.rs`, tested against its oracle in `tests/fixtures/command-words/`),
  `parse_command`, and `classify` a typed line into a `Kind` (talk, act, attack, no command)
  for typing beside the bot (`Session::user_input`: talk goes between the bot's actions, an act
  pauses it, an attack while engaged is held). The executor's attack guard uses it too.
- **`position.rs`**: the `Belief` fed by every room seen and every move sent, the `Traveller`
  with the exits refused this session, the rooms visited, and the `map_view`.
- **`walker.rs`**: the route and the per-step policy (doors, refusals, relocation, stops), and
  pipelining on a walk started with `start_with(.., pipelined)`: `Next::ahead` proposes the
  next plain move, the session sends it if the moment allows (`went_ahead`), and a step that
  goes wrong voids it and replans once its answer is in.
- **Loops** (`session.rs`): `loops_request` makes an area's loops with `wayfarer_world::loops`
  (cached per anchor, radius, level and doors); `walk_loop` is a `walk_to` per stop, on to the
  next when the walker says it arrived.
- **The bot** (`modes.rs`): running, paused or stopped, `Presence` (who is in the room), and the
  mode stack, each mode `wants`, `step`, `on_event` and `outcome`: `flee.rs`, `recover.rs`,
  `fight.rs`, `task.rs`, then Idle. A step asks for an action (through the executor) or a walk (through
  the walker), and narrates. Flee hops one move at a time through the executor; it also runs
  alone before a log-off under attack (`Bot::leave`). **`policy`**: the constants (rest below
  80%, until 95%, flee below 30% and at most three rooms, a 1.5 s watch for a follower, a quiet
  room within 5 steps, 12 s of quiet ends a fight) and pure functions over them and the pack
  (`target`, `hostile`, `spawns_here`, `rest_at`, `flee_exits`, `hp_percent`).
- **`task.rs`**: Task mode: the loop, the stop cursor, when each stop was cleared, the skip set;
  it asks the walker for each leg and hands the stop to Fight and Recover as home.
  `stop_ready(now, cleared_at, regen, is_arena)` and `ready_in` are the respawn rule
  (`STOCK_REGEN`, `STOP_QUIET`, `LEG_TRIES`). `TaskLoop` is what the store keeps of it.
- **`Atlas`** (`brain.rs`): the `RoomGraph`, room kinds, class and race numbers, the
  `Monsters` and MegaMud's verified loops, loaded once
  per brain and shared by its sessions; `World` keeps the `WorldPack` open beside it.
- **`stats.rs`**: the `Ledger` per session: the bot's state saved when it changes (the brain
  calls `Session::persist` after every message and timer), the run from Start to Stop with its
  events and a sample a minute, and the run's numbers for the snapshot's `run`. A stored bot
  that was running or paused is put back by `Session::resume` once the catch-up is done.
- `BUILD`: the version and commit (`build.rs`), for `brain_info`.

Tests: `cargo nextest run -p wayfarer-brain`. `tests/brain.rs` runs the brain against a fake
gateway: the fixtures replayed as live output (the snapshot, the patches, the events), history
catch-up, the re-sync's timing on a paused clock, reattach, restart and refusal.
`tests/walk.rs` walks against a fake game (`tests/game/mod.rs`: rooms from the `room-walk`
capture and the pack built from `tests/fixtures/world`): confirmed steps, doors, bashing,
refusals, "no exit", the stops, the map view. `tests/fight.rs` starts the bot in the Arena
against the same fake game (texts from the `fight-arena` capture): one attack per target,
nothing while engaged, the look after a kill, an arrival, resting in a quiet room and walking
back, pause, take-over and resume, determinism, `logging_off`, Recover's
two fallbacks narrated, knocked down not counting as quiet, players not taken for attackers,
`in_combat` going false when a fight lapses, and the policies on the pack. `tests/flee.rs`:
under the line mid-fight a hop, a follower and a second hop, safe, rest and back; a shut door
passed over or opened; three hops then `break` and a stop; cornered; the check while a walk is
in flight; nothing while paused or stopped; the log-off under attack; determinism; and
`flee_exits` on the pack. `tests/task.rs`: `run_loop` round the fixture's Newhaven loop: to
the nearest stop with moves pipelined, a fight, on through the door, an unticked stop and a
not-ready one passed by, the wait for a respawn on the paused clock, a rest before the wait,
a wrong room voiding the move ahead, combat and a hostile monster on the way, pause and
resume at the same stop, unticking the stop it walks to, stop, a brain restart going on to
the same stop, determinism, and never an attack while engaged. `tests/loops.rs`: the Newhaven loops, `walk_loop` round the stops and back, from the
nearest stop, and its stops. `tests/store.rs`: a second brain on the same store resumes the bot
paused with its home and the same run, a running bot resumes and fights, a stored loop's fields
survive and its stops win, nothing is resumed outside the realm or when stopped, a run's kills,
death, samples and exp per hour, flees (a log-off's too), and the run counted without a store. The live scenarios: `scripts/live-check.sh --scenario brain`,
`--scenario walk`, `--scenario fight` and `--scenario loop` (which runs the Newhaven loop for
five minutes).
