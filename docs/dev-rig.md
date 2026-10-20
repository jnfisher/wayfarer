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
from your data directory, `~/Library/Application Support/wayfarer/` (override with `WAYFARER_DATA_DIR`). The
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

The brain needs the world pack in the data directory. Both `scripts/dev/gateway.sh` and
`scripts/dev/brain.sh` build it from `tests/fixtures/world` if it is missing, so either may go
first. To rebuild it by hand: `wayfarer import-world --export tests/fixtures/world --out
"$WAYFARER_DATA_DIR/world.sqlite"`.

Without the watcher, `wayfarer brain` by hand does the same once. A release runs both in one
process instead, `wayfarer serve` ([brain.md](brain.md#running-it)); the dev rig keeps them
apart so that the brain restarts alone.

After the laptop sleeps, the brain reattaches by itself (it retries with the web UI's
backoff, half a second doubling to ten) and catches up from the gateway's history.

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

Not yet working: `--char` takes only pool characters until a follow-up lets `--credentials`
be a gateway `credentials.toml` (`[character.<id>] user, password`); the commands above are
the target. The check logs the character on through its own gateway, so stop Thorgrim's tile on the rig
first (the server refuses a second logon to an account). Put the anchor in a quiet room: the
character is walked back to it at the end.

A short smoke run on a pool character (three minutes in Newhaven, the Lairs unticked):

```sh
scripts/live-check.sh --scenario soak --char Waychecka --minutes 3 --anchor 1:2140 \
    --skip 1:2152 --skip 1:2156
```
