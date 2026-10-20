# wayfarer-live: the live check

The test that needs a game server. `scripts/live-check.sh` runs it against the server already
running on the laptop (`WAYFARER_LIVE_ADDR`, default `127.0.0.1:2323`): a card's session runs
it before pushing a change to the gateway and pastes the summary into its pull request. It is
never run in CI (its unit tests are, like any crate's), and it never starts, stops, rebuilds or
resets the server.

```sh
scripts/live-check.sh                 # the first free character of the pool
scripts/live-check.sh --char Waycheckb
```

If nothing answers at the address it stops at once with the command that starts silvermere
(`docs/dev-rig.md`).

## What it checks (M0)

1. **Claim a character.** It tries the pool in `fixtures.toml` in order and keeps the first
   that logs on. The server refuses a second logon to an account in use, so checks running at
   once (one per card) each get their own; the summary says which ones were passed over.
2. **Log on**: `wayfarer gateway`, started on a free local port with a config for that
   character, runs the silvermere logon script to the realm prompt.
3. **A command**: an `input_line` of `st` over the gateway's WebSocket shows the status screen.
4. **Log off**: stopping the gateway (SIGTERM, as Ctrl-C) logs the character off: `x`, the
   meditation, out through the menus. The gateway's own log must show the log-off finishing,
   not timing out.
5. **Not punished**: a second gateway logs the character on again, and nothing from connect to
   the realm says "disconnected while playing"; then it is stopped and logs off again.

The world is shared and long-lived, so it checks behaviour, never numbers or room contents.
Later cards add their scenarios (walk, fight, rest, flee) here.

## Scenario "brain" (M1)

```sh
scripts/live-check.sh --scenario brain
```

It claims and logs on a character as above, then, with that gateway running:

1. **The world pack**: `wayfarer import-world` from `tests/fixtures/world` into the run's
   directory.
2. **The brain attaches**: `wayfarer brain` against the gateway; the UI connection sees
   `brain_state true`.
3. **The snapshot**: within 10 s the `character` (with its patches) has the HP of the last
   prompt on the screen.
4. **The re-sync**: the brain sends `st`, `exp`, `i` and `look` at an idle prompt and the UI
   gets the notice "re-synced".
5. **A second brain** exits non-zero: `a brain is already attached`.
6. **Stopping the brain** (SIGTERM) exits cleanly; the UI sees `brain_state false` and the
   session is still in the realm; `st` over the gateway shows the status screen.
7. **A restarted brain** attaches and re-syncs again.
8. **Log off**: the gateway stops as in M0.

The brains' logs are `NN-brain/brain.log`, `brain-2.log` and `brain-3.log`.

## Scenario "walk" (M2)

```sh
scripts/live-check.sh --scenario walk
```

It claims and logs on a character, then:

1. **The world pack** and **a brain**, as in "brain"; it waits for the re-sync, whose `look`
   places the character (the snapshot's `position`).
2. **A walk there**: `walk_to` the Newhaven Narrow Road (1/2146; the Weapons Shop, 1/2141, if
   the character is already there). It passes when the brain narrates "Arrived:", the
   snapshot's room title is that room's, the position is that room, and a `map_view` was
   centred on it. A warning from the brain fails it. The road is two or three steps from where
   characters start, through rooms nothing spawns in. No door: the only one near Newhaven is
   the Arena's into the Dungeon (`docs/kb/pitfalls.md` 36), and the Arena's monsters would
   start a fight, which stops a walk; the in-process tests (`crates/wayfarer-brain/tests/walk.rs`)
   cover doors.
3. **A walk back** to the room it started in, checked the same way.
4. **Stopping the brain**, then the gateway logs off as in M0.

The brain's log is `NN-walk/brain.log`.

## Scenario "fight" (M2, "fight here")

```sh
scripts/live-check.sh --scenario fight
```

It claims and logs on a character (level 3 or below: the pool's are level 1), then:

1. **The world pack** and **a brain**, as in "walk".
2. **To the Arena**: `walk_to` the Newhaven Narrow Road (1/2146), then the Arena (1/2150) down
   the stairs, each checked as in "walk" (skipped when the character is already there).
3. **Start** (`bot_control start`): `*Combat Engaged*` must show within a minute (the bot
   re-syncs first); a warning from the brain fails it.
4. **A kill or a stop**: within three minutes, `You gain N experience.` or the bot stopping
   itself (below 30% HP); the summary lists the HP values the snapshot went through.
5. **Stop** (`bot_control stop`), whatever happened: the snapshot's `bot.state` is `stopped`.
6. **Walk out** to the Narrow Road, once the character is not engaged (a minute at most: a
   fight can lapse without `*Combat Off*`); tried three times, since a fight stops a walk.
7. **Rest by hand** below half HP: `rest` until 80%, sent again if an attack breaks it.
8. **Stopping the brain**, then the gateway logs off as in M0, out of the Arena.

The brain's log is `NN-fight/brain.log`.

## Scenario "loop" (M2, generated loops)

```sh
scripts/live-check.sh --scenario loop
```

It claims and logs on a character, then:

1. **The world pack** and **a brain**, as in "walk".
2. **To the Village Entrance** (1/2140), checked as in "walk" (skipped when already there).
3. **The loops** (`loops_request` around it, radius 8): the generated "Newhaven" loop stops
   at the Arena, the Dungeon Entrance and the Small Cavern, the Arena flagged, under 40
   steps.
4. **`walk_loop`** on it, and `walk_stop` as soon as the brain narrates the walk to its first
   stop (the Arena, three steps away): the brain narrates "Loop Newhaven stopped: asked to
   stop" and the walk stops after the move in flight, short of the Arena. Every stop of the
   loop spawns monsters, and a fight stops a walk, so the check never walks one round; the
   in-process tests (`crates/wayfarer-brain/tests/loops.rs`) walk the whole loop and back.
   If the character did reach the Arena, it walks out as in "fight" and the step fails.
5. **A walk back** to the Village Entrance, then **stopping the brain**, and the gateway logs
   off as in M0.

The brain's log is `NN-loop/brain.log`.

## The test-character pool

`fixtures.toml` lists the characters: account, family name, race, class and the stat totals
(John approves the values). The first time a character is needed the check makes it:

- **The account**, through the `NEW` dialogue, done as logon-script steps laid over the
  silvermere preset (`SIGNUP` in `src/main.rs`). The password is generated and saved to
  `tests/live/.credentials.toml` *before* the account is made, so it is never lost; a saved
  password whose account was never made gets `Invalid Credentials`, and the check signs up
  with it then.
- **The character**, through the game's own editor: race and class picked by name from the
  game's lists, not lawful, the given name as offered (the account name), the family name and
  stats typed field by field, following the editor's cursor. This is test setup John
  approved, not the bot spending CP.

`.credentials.toml` (mode 0600, git-ignored) lives in the **main checkout**, not the worktree,
so every card's worktree uses the same accounts (`WAYFARER_LIVE_CREDENTIALS` overrides the
path). Never commit it.

## Logs

Each run keeps everything under `target/live-check/<time>-<pid>/`, one directory per gateway
it started (`01-logon`, `02-relogon`, ...): the config, the gateway's log (`gateway.log`) and
its session logs (`logs/probe/output.log`, `raw.bin`, `sent.bin`; passwords show as
`********`). A failing step's summary line names what it waited for and the last lines of the
screen.
