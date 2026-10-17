# 07. Development and testing

Two wishes from John shape this: watch the bot-run character in the real UI while it is being
built, and never rebuild or restart the game server for client work.

## Four restart domains

| Part | Restarted when | Cost |
|---|---|---|
| Game server (silvermere, MBBSEmu) | Never for client work. Upgraded on purpose to a pinned build | None during development |
| Gateway | Only when its own code changes (rare: it is small) | Characters log off cleanly (`x`, then `X`, `X`, `Y`) and log back on through their logon scripts |
| Brain | On every engine change, automatically | None: characters stay logged in; the brain re-syncs and resumes (`03-engine.md`, section 7) |
| Web UI | On every UI change, by Vite's hot reload | None: the page updates in place |

So the usual loop is: edit the engine, save, a watcher rebuilds and restarts the brain, and a
few seconds later the character in the browser carries on with the new code. The UI header
shows the brain's build (git commit, built-at, restarted-at), so it is always clear which code
is playing.

## The development machine: John's MacBook (John, 2026-10-17)

Agents, the game server and the UI all run on John's laptop; GitHub runs CI. The laptop stays
open during development; when it sleeps, everything pauses and carries on when it wakes.

- **silvermere**, built from the silvermere checkout (`~/repos/silvermere`) and run from its own
  directory, `~/repos/silvermere-wayfarer` (its `silvermere.toml` and `./data` database), with
  telnet on `127.0.0.1:2323`, started in a terminal tab (`silvermere serve`; Ctrl-C saves
  everyone and stops it). It is long-lived: rebuilt and restarted only when John upgrades it,
  never by a card. Keeping it outside the checkout means `git pull` or a rebuild never touches
  its world or characters.
- **The gateway** in another terminal tab, holding the characters' sessions.
- **The brain** under a file watcher (`cargo watch` or the repo's own script), from M1.
- **The browser** on `http://localhost:7373`.
- A **dev panel** in the UI: the live event stream, the executor's actions with their
  confirmations and timeouts, unclassified lines, the position belief.

## Where work runs

| Work | Runs on | Not on |
|---|---|---|
| Building the client (agent sessions) | Local Claude Code sessions on the laptop, one per card in its own git worktree (`tasks/README.md`) | Cloud sessions: they can't reach the local server |
| Fast checks on every PR | GitHub-hosted runners (one job, below) | Self-hosted runners |
| Live tests against a game server | The card's own session, against the local silvermere, before it pushes | GitHub Actions |
| Watching and playing | The laptop, in a browser | — |
| Soak (hours of play) | By hand, when wanted, on the laptop | A schedule |

Why the laptop: Claude Code cloud sessions reach the internet only through an HTTPS proxy, so
they cannot open a telnet connection to any game server, and the proxy does not support
WebSocket bridges either. A server on the laptop is just `127.0.0.1:2323` to a local session.
silvermere's CI grew to eight workflows on a self-hosted runner; wayfarer keeps CI on
GitHub's hosted runners and everything else on the laptop.

## CI: one fast job

**Budget: under 3 minutes on a warm cache, under 8 cold.** If a check can't fit, it moves
out of CI (to the live check on the laptop, below), and less testing is an acceptable price.

- **One workflow, one job**, on `ubuntu-latest` (GitHub-hosted): `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo nextest run`. A second small workflow runs the web UI's
  type check and unit tests only when `ui/` changed.
- **Only in-process tests**: no game server, no network, no database service, no Docker, no
  browser. The whole Rust suite runs in under a minute; a test that takes over a second needs
  a reason in its comment.
- **Fast by construction**: `Swatinem/rust-cache`; `CARGO_INCREMENTAL=0` and no debug info in
  the test profile; a fast linker; no feature matrix, no doctests, no Windows or macOS.
- **Run only when needed**: on pull requests only, not also on branch pushes; `concurrency`
  with `cancel-in-progress` so a new push cancels the old run; skip changes to `docs/` and
  Markdown; skip draft PRs.
- **Releases are separate**: the installer builds (`08-packaging.md`) run on a tag or by hand,
  never per PR; they are slow, not costly.
- **Speed still matters** though minutes are free (below): agents wait on CI before they can
  finish, so the budget stays.

## Public repository (John, 2026-10-17)

`jnfisher/wayfarer` is public, so GitHub Actions on the standard hosted runners (Linux,
Windows, macOS) costs nothing and has no minute limit; that was silvermere's wall. Only
larger runners would be billed, and we don't use them. What this means for the work:

- Everything in the repository is visible to anyone: code, the plan, and the fixtures copied
  in (captures, transcripts, the stock-world export). MegaMud's data stays in
  `mmud-exploration` (John, 2026-10-17). Nothing personal
  goes in: no real hostnames, account names or passwords; `examples/` uses placeholders.
- CI needs no secrets, so pull requests from forks run safely as they are.
- `mmud-exploration` and `silvermere` stay as they are; wayfarer never needs them at build or
  test time (fixtures are copied in with provenance).

## Live tests against the local server

Tests that need a game server run in the card's own session on the laptop, never in Actions:
`scripts/live-check.sh` connects to the silvermere that is **already running** at
`WAYFARER_LIVE_ADDR` (default `127.0.0.1:2323`). It never starts, stops, rebuilds or resets the
server. It logs on with test characters from a pool created once on that server (race, class
and stats from a fixture file John approves), and runs a short set of scenarios: log on, walk
a route, fight, rest, flee, train, log off. A card that changes the engine runs it before
pushing and pastes the summary into the PR. It should take a minute or two.

- **Shared, long-lived world.** Several cards may run at once against the same server, and the
  world changes over time (monsters killed, characters levelling). Scenarios check behaviour,
  not exact outcomes; the exact, seeded replays are recorded sessions in CI.
- **Characters don't collide.** The server refuses a second logon to the same account, so a
  session takes the first character in the pool that logs on.

Recorded sessions from these runs can be played back in the UI's replay mode, so John can
watch what an agent's bot did without running anything himself.

## Test layers

| Layer | What | Where it runs |
|---|---|---|
| 1. Policies | Pure functions: flee check, target choice, rest and heal decisions, routing, setting layers | CI |
| 2. Parser | Golden tests over a small set of captured sessions (transcripts and raw captures); the pattern table checked against the message catalogue | CI |
| 3. Replay | A recorded session fed through the brain with its recorded clock; the decisions must match | CI (a few short recordings) |
| 4. Live scenarios | Against the local, long-lived silvermere | The card's session on the laptop, before pushing |
| 5. Soak | Hours of play: exp/hr, deaths, flees, relocations, unclassified-line rate, compared with `progress.py`'s measured numbers | By hand, occasionally |
| 6. Simulator | The engine against a model world to tune policies offline | Later, if wanted |

**Test characters** for live scenarios get race, class and stats from a fixture file John
approves; this is test setup, not the bot spending CP.

**Server differences.** When the bot behaves differently on silvermere than on MBBSEmu with
the original, that is a silvermere bug until shown otherwise: file it there with both captures.
