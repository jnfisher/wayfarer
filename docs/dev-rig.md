# The development rig (macOS laptop)

How to run the whole loop on one MacBook: the local silvermere, the gateway, the web UI and
card sessions in parallel worktrees. Nothing here runs on a schedule. The reasoning is in
[plan/07-development-and-testing.md](plan/07-development-and-testing.md).

Repositories sit side by side under one parent directory, `~/repos/wayfarer-dev/` (since
2026-10-19; before that, `~/repos/`): `wayfarer`, `silvermere`, `mmud-exploration` and
`silvermere-wayfarer` (the server's run directory, below).

## silvermere (the game server)

It is built from `~/repos/wayfarer-dev/silvermere` and run from its own directory,
`~/repos/wayfarer-dev/silvermere-wayfarer`, so that a `git pull` or rebuild never touches the
world or the characters. Its `silvermere.toml` there has:

```toml
[server]
telnet_bind = "127.0.0.1:2323"
admin_bind = "127.0.0.1:8080"
```

First time:

```sh
cd ~/repos/wayfarer-dev/silvermere && cargo build --release
cd ~/repos/wayfarer-dev/silvermere-wayfarer
~/repos/wayfarer-dev/silvermere/target/release/silvermere import        # the built-in stock world
~/repos/wayfarer-dev/silvermere/target/release/silvermere account create NAME --sysop
```

`account create` asks for the password on its own line. Make a few test accounts; they are what
`credentials.toml` (below) holds.

Run it in its own terminal tab:

```sh
cd ~/repos/wayfarer-dev/silvermere-wayfarer && ~/repos/wayfarer-dev/silvermere/target/release/silvermere serve
```

Ctrl-C saves everyone and stops it. It is long-lived: **client work never restarts it, and
cards never start, stop, rebuild or reset it.**

**Upgrading** is a deliberate act: stop it, `git pull` and `cargo build --release` in
`~/repos/wayfarer-dev/silvermere`, start it again. **Backups**: `silvermere backup` from
`~/repos/wayfarer-dev/silvermere-wayfarer` writes to `data/backups/`; take one before an upgrade.

## The gateway

```sh
scripts/dev/gateway.sh
```

builds `wayfarer` in release mode, rebuilds the web UI (`npm ci` and `npm run build` in
`ui/`, so the page always matches the checkout), and runs `wayfarer gateway` with the config
from your data directory, `~/Library/Application Support/wayfarer/` on macOS or `~/.local/share/wayfarer/` on Linux (override with `WAYFARER_DATA_DIR`). The
first run copies [`examples/dev.toml`](../examples/dev.toml) there as `wayfarer.toml`; after
that the script never overwrites it. Edit the copy: change the placeholder characters to your
test characters, and put their accounts in `credentials.toml` beside it
([`examples/credentials.toml`](../examples/credentials.toml); the gateway creates it readable
only by you). The real config, credentials and logs stay in that directory, never in the
repository. See [gateway.md](gateway.md) for the file formats.

Extra arguments pass through: `scripts/dev/gateway.sh --bind 127.0.0.1:7374`. Ctrl-C (or
SIGTERM) logs every character off cleanly before the process exits.

## The UI

With the gateway running, the built UI is at `http://localhost:7373` (the gateway serves
`ui/dist`; build it with `npm run build` in `ui/`). For hot reload, run `npm run dev` in `ui/`
and open the address Vite prints; it talks to the gateway on 7373.

Two test characters in the realm, from `scripts/dev/gateway.sh` and `scripts/dev/brain.sh`
against the local silvermere, with the dev panel open (M1, 2026-10-19):

![Two tiles with filled status headers, and the dev panel beside them](images/dev-rig-realm.png)

## Cards

```sh
scripts/dev/card.sh C-005
```

starts the card as a background Claude Code session (`claude --bg`) with the standard brief
from [plan/tasks/README.md](plan/tasks/README.md). The session makes its own git worktree under
`.claude/worktrees/`, works on `card/C-005-<slug>` and ends with a pull request against `main`;
it never merges. The session is named after the card (`C-005` in `claude agents`). Run the script from the main checkout. Running it again for a card that
already has a session does nothing.

| To | Command |
|---|---|
| list sessions | `claude agents` (`--json` for scripts) |
| watch one | `claude logs ID` (the short id `claude --bg` printed) |
| answer or steer one | `claude attach ID`, type, detach |
| stop one | `claude stop ID`; the conversation is kept and `claude attach ID` resumes it |

Two or three cards at a time: each is a full Rust build. The conductor session
(`/loop /conduct`) does the same dispatching for you.

## Permissions

`.claude/settings.json` pre-approves the routine commands so a background session does not
stall on a prompt nobody is there to answer: `cargo` build, test, fmt and clippy; read-only
`git` plus add, commit, push and worktree handling; `npm` scripts; `gh pr` create, view,
checks, comment and merge; `scripts/live-check.sh`; and `claude --bg`, `agents` and `logs`.
It denies force-pushes and pushes to `main`. `gh pr merge` is pre-approved so that the
conductor can merge on its own once CI is green and its review passes, without a prompt
stalling the loop; it still waits for John on anything marked "Needs John".

## Sleep

Closing the lid pauses everything: silvermere, the gateway, card sessions. On wake the
gateway's sessions reconnect and log on again through their logon scripts (C-005), and the
browser reconnects and asks for the output it missed by sequence number, so nothing is lost
from the scrollback. A card session carries on where it stopped; a CI wait simply resumes.

## Refreshing everything at once

`scripts/dev/refresh.sh` brings the rig up to date with `main` in one go, when the gateway and
brain run in a tmux session `wayfarer` (windows `gateway` and `brain`):

```sh
scripts/dev/refresh.sh            # then: tmux attach -t wayfarer
```

It checks the checkout can pull (on `main`, no uncommitted changes, no commits `origin` lacks)
before stopping anything; stops the gateway first, so it logs every character off cleanly
while the brain is still attached (up to six minutes); stops the brain; pulls; starts the
gateway on this machine's Tailscale address (`tailscale ip -4`, port 7373) and waits for it to
listen; then starts the brain against it. Each step prints as it goes. A window whose program
exits stays open, so a failed build can be read there. `WAYFARER_BIND_IP`, `WAYFARER_PORT` and
`WAYFARER_TMUX_SESSION` override the address, the port and the session. On an ARM machine it
asks before building: a release box (a Raspberry Pi) updates from a GitHub release with
`scripts/update.sh` instead ([server.md](server.md), C-061).

## What restarts when

| Part | Restarted when |
|---|---|
| silvermere | Never for client work; on purpose, to upgrade |
| Gateway | Only when its own code changes: Ctrl-C, run `scripts/dev/gateway.sh` again. Characters log off cleanly and back on |
| Brain | On every engine change, by `scripts/dev/brain.sh`; characters stay logged in |
| Web UI | On every UI change, by Vite's hot reload |

## Brain (from M1)

`scripts/dev/brain.sh` in a third terminal tab runs `wayfarer brain` ([brain.md](brain.md))
under `cargo watch`, restarting it whenever a file under `crates/` or `bin/` changes. The
gateway keeps the characters logged in meanwhile; the UIs see `brain_state false`, then `true`,
and the brain re-syncs each character when it attaches. Install the watcher once with `cargo
install cargo-watch` or `brew install cargo-watch` (a developer tool, not part of the build).

### The world pack

The brain needs the world pack in the data directory. Both `scripts/dev/gateway.sh` and
`scripts/dev/brain.sh` build it from `tests/fixtures/world` if it is missing, so either may go
first. They add **MegaMud's path set** from [`data/megamud/`](../data/megamud/PROVENANCE.md):
the paths that ship with stock MegaMud v2.2 (1,957 paths, 195 of them loops, 183 verified
against the stock world). `$WAYFARER_MEGAMUD` points them at another set, a `Default/`
directory's parent or a MegaMud zip. Without one the pack has no MegaMud loops and the map
panel lists only generated ones.

The scripts never rebuild an existing pack. To rebuild it by hand, with the gateway and the
brain stopped:

```sh
wayfarer import-world --export tests/fixtures/world \
  --megamud data/megamud \
  --out "$WAYFARER_DATA_DIR/world.sqlite"
```

Without the watcher, `wayfarer brain` by hand does the same once. A release runs both in one
process instead, `wayfarer serve` ([brain.md](brain.md#running-it)); the dev rig keeps them
apart so that the brain restarts alone.

After the laptop sleeps, the brain reattaches by itself (it retries with the web UI's
backoff, half a second doubling to ten) and catches up from the gateway's history.

## Replay (from M3)

Every gateway session is recorded in `logs/<id>/`, so what a bot did while you were away can
be watched afterwards:

```sh
wayfarer replay "$WAYFARER_DATA_DIR/logs/<id>" --config "$WAYFARER_DATA_DIR/wayfarer.toml" \
    --brain --bind 127.0.0.1:7374
```

serves the normal page on a port of its own beside the running gateway, with the session
playing back at its recorded pace (`--speed 10`, `--instant`). See [replay.md](replay.md).

## A timed run

`scripts/live-check.sh --scenario soak` makes a character run a loop for a number of minutes
and prints what happened: exp per hour, kills by monster, deaths, flees, rests, relocations,
the unclassified-line rate, the longest gap without a prompt, brain restarts and the run's id
in the store. It prints a line a minute while it goes, stops at once on a death or when the
bot stops itself, and ends with a Stop, a walk back to the anchor and a clean log-off. The
scenario is described in `tests/live/README.md`.

The M3 acceptance command, for Thorgrim on the Wooden Passage (the Minotaur Chieftain's room is
Large Wooden Room, 1:1865), run by John, never by a card:

```sh
# the Chieftain's room in
scripts/live-check.sh --scenario soak --char Thorgrim --minutes 30 --anchor 1:1839 --radius 10 \
    --credentials "$WAYFARER_DATA_DIR/credentials.toml"
# the Chieftain's room out
scripts/live-check.sh --scenario soak --char Thorgrim --minutes 30 --anchor 1:1839 --radius 10 \
    --skip 1:1865 --credentials "$WAYFARER_DATA_DIR/credentials.toml"
```

`--char` takes a pool character, or an account of your own when `--credentials` is a gateway
`credentials.toml` (`[character.<id>] user, password`): the check reads the password from it
and never writes to it, and never signs up or creates such an account. The check logs the
character on through its own gateway, so stop Thorgrim's tile on the rig first (the server
refuses a second logon to an account). Put the anchor in a quiet room: the
character is walked back to it at the end.

A short smoke run on a pool character (three minutes in Newhaven, the Lairs unticked):

```sh
scripts/live-check.sh --scenario soak --char Waychecka --minutes 3 --anchor 1:2140 \
    --skip 1:2152 --skip 1:2156
```


### A 30-minute run on a MegaMud loop (the Bandit Keep)

```sh
scripts/live-check.sh --scenario soak --char Thorgrim --minutes 30 --loop HKEPLOOP.mp \
    --anchor 9:750 --credentials "$WAYFARER_DATA_DIR/credentials.toml"
```

Anchor `9:750` is a Keep Wall Edge room inside the keep, on the loop's path (the area listing
is built around the anchor, and from the Hidden Keep outside the keep it does not reach the
loop). The bot searches north at the Hidden Keep until the hidden exit shows, then walks the
loop; the way in is `Ftrehkep.mp`'s, not the loop's own.

There is no respawn clock (see `brain.md`, "Running a loop"). Two soaks on 2026-10-20
(Thorgrim, level 20), one with a five-minute clock on every stop and one without, came out the
same: 15,700 exp (31.4k/hr, 122 kills, 6 rests, 40 waits for respawns, longest gap 51 s) with
the clock, 15,800 exp (31.6k/hr, 124 kills, 0 rests, 0 waits, longest gap 8.5 s) without. What
limits exp per hour on that loop is the supply of monsters, not the walk. One run of each is a
sample, not a verdict: a run starts with every room stocked.

### A custom loop: the Bandit Keep with Leo the Quick

A loop of the player's own, made from Bandit Keep by adding Leo the Quick's Elaborate Bedroom
(9:870) after the last Living Quarters stop (32). Leo is a boss (240 HP, 10,000 exp, Hit 80, back
about two hours after a kill); his room is 15 steps and seven doors beyond the Living Quarters,
behind a hidden exit the walker searches for. A short run, to see that the loop closes:

```sh
scripts/live-check.sh --scenario soak --char Thorgrim --minutes 15 --loop HKEPLOOP.mp \
    --anchor 9:750 --save-as "Hidden Keep with Leo the Quick" --add-stop 9:870@32 \
    --credentials "$WAYFARER_DATA_DIR/credentials.toml"
```

The loop is kept in the store, so it is in the web UI's loops list under "My loops" afterwards
(run it from there, or `--loop custom:hidden-keep-with-leo-the-quick`). Watch for: the walk to
Leo's room ("Searching for the hidden exit west", doors bashed), the fight, the way back out to
stop 33, and the next lap starting at stop 1. The stop's tooltip says whether Leo should be up.
