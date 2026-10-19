# The brain

The brain is the part of wayfarer that understands the game. It attaches to a running gateway
as its one brain client, parses every session's output, keeps a model of each character and
sends the UIs events and snapshots of it. In M1 it plays nothing: the only lines it sends to
the game are the four of a re-sync. It can be stopped and started at any time; the gateway
keeps the characters logged in meanwhile.

## Running it

```sh
# once: the world pack, from the stock-world export in this repository
wayfarer import-world --export tests/fixtures/world --out "$WAYFARER_DATA_DIR/world.sqlite"

# with the gateway already running (scripts/dev/gateway.sh)
wayfarer brain
```

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
  of it is read as an echo. A line nothing recognises is counted (`parse.unclassified`), never
  an error.
- **Model.** The character applies each typed event: the prompt's HP, mana and resting mark;
  the status screen (name, lives and CP, race, class, level, HP and mana with their maxima,
  stats, poison); the `exp` screen; the room's title and exits (a dark room has neither); combat
  on and off; exp gained; death and lives; blindness; inventory, wealth and encumbrance (kept
  in the brain; the M1 snapshot has no fields for them). After each chunk that changed
  anything it sends a `character_patch`, the merge patch from the last snapshot sent.
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

No bot, no modes, no executor; no input arbitration (every client's input still goes straight
to the game, `docs/protocol.md`); no position belief from the world pack (M2). `wayfarer
serve` running the brain in-process and the watcher script are C-019.
