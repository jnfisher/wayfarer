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
   itself (cornered below 30% HP, say); the summary lists the HP values the snapshot went
   through.
5. **Stop** (`bot_control stop`), whatever happened: the snapshot's `bot.state` is `stopped`.
6. **Walk out** to the Narrow Road, once the character is not engaged (a minute at most: a
   fight can lapse without `*Combat Off*`); tried three times, since a fight stops a walk.
7. **Rest by hand** below half HP: `rest` until 80%, sent again if an attack breaks it.
8. **Stopping the brain**, then the gateway logs off as in M0, out of the Arena.

The brain's log is `NN-fight/brain.log`.

## Scenario "under-attack" (C-027, log-off under attack)

```sh
scripts/live-check.sh --scenario under-attack
```

Steps 1 to 4 of "fight", then:

5. **A monster attacking, HP to spare, attacked just now**: within ten minutes the bot is
   engaged again (the Arena's monsters come back at once) with HP at 60% or more, and a
   monster's attack (a hit or a miss, from the brain's events) came within the last 6 s; it
   rests first if it must. The summary names the attacker and quotes its line.
6. **Stop the gateway under attack**: SIGTERM with the brain still attached and the fight
   going on. The brain flees before it answers `logging_off` (C-028): the summary gives its
   narration from "Logging off under attack: fleeing first" to "Safe in ..." (from the brain's
   log); no flee, or one that does not get away, fails it. Then it passes when the gateway's log
   shows the brain answering and the log-off finishing (`x` sent again after each `Your
   meditation has been interrupted`, counted in the summary) with the session `Closed`; a
   time-out, a hang-up or "dropped in the realm" fails it.
7. **Stopping the brain**.
8. **Log on again: not punished**, as in M0; then **out of the Arena by hand** (`u` to the
   Narrow Road, so the next check starts somewhere quiet; done punished or not) and the
   gateway logs off.

Before C-028 it failed at step 6: every attack on a character waiting to exit cancels the
exit, a miss as much as a hit (`docs/kb/recovery.md`, "An attack cancels the exit"), and `x`
ends the character's own auto-combat, so an Arena monster interrupted every try until the
gateway dropped the character. Now the brain takes the character up to the Narrow Road first.
Run it on purpose, with a pool character, not as a routine check.

If step 5 never comes (the bot stopped itself, say), it fails there and tidies up as "fight"
does.

## Scenario "flee" (C-028)

```sh
scripts/live-check.sh --char Waychecka --scenario flee
```

The brain runs with `--flee-below 100` (a hidden test knob of `wayfarer brain`) and the
character rests to full HP by hand first, so the first blow that takes any HP sets a flee off
(95 would not: a level-1 warrior has 40 HP, an Arena rat takes 1 or 2, and 38/40 is 95%). The
room is the Newhaven Arena (1/2150), the one hunt room within 8 steps of the Village Entrance
with a plain exit to a quiet room (up the stairs to the Narrow Road; the Dungeon Entrance and
the Small Cavern lead only to rooms that spawn). Steps 1 and 2 of "fight", **rest to full HP**
before the walk, and Start (no wait for `*Combat Engaged*`: a blow can come first), then:

4. **The first blow: flee to a quiet room**: within three minutes the brain narrates "Fleeing:
   HP ..%: up to Newhaven, Narrow Road" and then "Safe in ..." (followed, it hops on); the
   summary names the attacker, quotes its line and gives the narration. The bot stopping
   itself fails it.
5. **Rests there, walks back**: within ten minutes "Recover: resting ...", "Recover: rested;
   walking back to Newhaven, Arena" and "Arrived: Newhaven, Arena". (Rested is 95%, under the
   raised line, so it flees again as soon as it is back: the scenario stops it there.)
6. Then as "fight" ends: **Stop**, **walk out**, **rest by hand**, and **no death** (`You have
   been killed!` nowhere on the screen; the summary counts the flees and attacks seen),
   **stopping the brain**, the gateway logs off, and **log on again: not punished** (out of
   the Arena by hand first, if it is there).

The brain's log is `NN-flee/brain.log`.

## Scenario "restart" (C-029, the bot survives a brain restart)

```sh
scripts/live-check.sh --scenario restart
```

Steps 1 to 4 of "fight" (the brain's store is `store.sqlite` in the run's directory), then:

5. **The bot is running**: a kill came, not a stop (the bot stopping itself fails it here).
6. **Restart the brain mid-run**: SIGTERM, then `wayfarer brain` again in the same directory,
   so on the same store. It passes when the new brain narrates `Resumed: ...`, re-syncs, and
   the snapshot shows the bot `running`, its home the Arena (it may be resting nearby: Recover
   walks it back once rested).
7. **`stats_request`**: the newest run has not ended and has at least the kills the snapshot
   showed before the restart; the summary gives its kills, exp, exp per hour and samples.
8. Then as "fight" ends: **Stop**, **walk out**, **rest by hand**, **stopping the brain**, and
   the gateway logs off.

The brains' logs are `NN-restart/brain.log` and `NN-restart/brain-2.log`.

## Scenario "typing" (C-031, typing beside the bot)

```sh
scripts/live-check.sh --scenario typing
```

Steps 1 to 3 of "fight" (to the Arena, Start), then, from the check's own WebSocket connection
(a UI, so the gateway forwards its lines to the brain while the bot plays):

4. **The bot fighting**: its narration `Fight: ...` and `*Combat Engaged*` on the screen.
5. **`st` beside the bot**: the status screen comes, and no "took over" or "Paused" notice: the
   bot is still running.
6. **`u` beside the bot**: "You took over ...", then "Paused: you took over", the bot paused,
   and the character up the stairs on the Narrow Road (the Arena's way out is up, not north).
7. **Stop**, **walk out** (already out), **rest by hand** if HP is low, **stopping the brain**,
   and the gateway logs off.

The brain's log is `NN-typing/brain.log`.

## Scenario "loop" (M2, generated loops; C-030, running one)

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
5. **A walk back** to the Village Entrance.
6. **`run_loop`** on it for five minutes with its two Lairs unticked, so the bot hunts the
   Arena: it walks there (moves pipelined), fights what comes, clears the stop, passes the
   Lairs by ("Skipping Dungeon, Entrance (unticked)") and waits there for respawns. The Lairs
   are unticked because a level-1 character is no match for them: the Small Cavern has cave
   bears, and in two runs on 2026-10-20 with the Dungeon Entrance ticked its three monsters
   took the character under the flee line and the bot stopped itself, followed through three
   rooms (no death). The respawn delays of non-arena stops are checked in-process
   (`crates/wayfarer-brain/tests/task.rs`, on the paused clock). The step passes when, after
   the five minutes, the bot is still running (no death, never stopped itself), the Arena was
   cleared with at least one kill, the unticked stops were narrated and never walked to, and
   the Arena (no respawn delay) was never called not ready. Its note has the run's numbers:
   kills, exp per hour, flees, rests, waits.
7. **Stop**, **walk out** to the Narrow Road, **rest by hand** if HP is low, **stopping the
   brain**, and the gateway logs off as in M0.

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
