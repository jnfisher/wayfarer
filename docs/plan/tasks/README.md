# Task cards: index

The cards that build wayfarer, in the format of [CARD-FORMAT.md](CARD-FORMAT.md). Wave 1 is
milestone M0, "gateway and a live terminal" (`../09-milestones.md`). Later waves are written
when the wave before them is running (see "Waves" below).

## Before the first card (John)

1. Create the empty **public** repository `jnfisher/wayfarer` (public so GitHub Actions has no
   minute limit; see `../07-development-and-testing.md` "Public repository"). Done.
2. The laptop setup (`../07-development-and-testing.md`, "The development machine"): the
   repositories side by side in `~/repos/`, Claude Code installed, and silvermere running from
   `~/repos/silvermere-wayfarer` with telnet on `127.0.0.1:2323`. Done 2026-10-17.

## How cards run (simpler than silvermere's)

- One **local** Claude Code session per card on John's laptop, in its own git worktree, on
  branch `card/C-NNN-slug`, started with the card file as its brief. Either a background
  session from `~/repos/wayfarer`:

  ```sh
  claude --bg "Run task card C-005 for wayfarer: read CLAUDE.md, docs/plan/tasks/README.md and docs/plan/tasks/C-005-logon-scripts.md, work on branch card/C-005-logon-scripts, and finish with a pull request against main with green CI. Do not merge."
  ```

  or a desktop-app session (Cmd+N) with the worktree option. It ends by opening a pull request
  and never merges.
- **Dispatching.** John, or a conductor session he starts in `~/repos/wayfarer` with
  `/loop /conduct` (`.claude/skills/conduct/SKILL.md`), starts the cards whose dependencies
  are merged (one `claude --bg` each), reads the PRs, and merges
  the ones whose CI is green and whose review passes; anything marked "Needs John" waits
  for him. The
  conductor stops at the end of a wave. Two or three cards at a time suits a laptop (each is
  a full Rust build).
- **Permissions.** The repository's `.claude/settings.json` pre-approves the routine commands
  (cargo, git, npm, gh, the live check) so background sessions don't stall on prompts.
- Review: the session re-reads its own diff against "Done when" before opening the PR. A
  second review session only for cards marked **review** below.
- CI is the one fast job (`../07-development-and-testing.md`), on GitHub's hosted runners.

## Conventions every card inherits

Cards don't repeat these:

- **Toolchain**: Rust stable, pinned in `rust-toolchain.toml` (C-001 picks the newest
  stable); `cargo fmt`, `cargo clippy --all-targets -- -D warnings`; `cargo nextest run`.
  Node LTS and npm for `ui/`.
- **CI budget**: the whole Rust suite in under a minute; a test over one second says why in
  a comment. Tests are in-process: no network, no game server, no Docker.
- **The live server** is the long-lived local silvermere at `127.0.0.1:2323`
  (`WAYFARER_LIVE_ADDR`). Cards connect to it; they never start, stop, rebuild or reset it.
- **No dependency on mmud-exploration or silvermere at build or test time.** Test data from
  those repositories is copied into `tests/fixtures/<subdir>/` with a `PROVENANCE.md` per
  subdirectory (source repository, path, commit). C-001 copies the shared ones (captures,
  transcripts, patterns, world); a card that needs more adds its own subdirectory. Read the
  other repositories freely while working; never clone them in CI.
- **Rules**: only those in `docs/plan/01-goals.md` "Rules". Nothing from the prototype
  (`test/play/`) or `mmud-exploration/CLAUDE.md` applies unless that section lists it.
- **Knowledge**: a confirmed fact about the game or MegaMud that the KB lacks goes in the PR
  description under "KB notes"; the planning session writes it to `mmud-exploration/kb/`.
- **Docs**: each crate has a short `README.md`; behaviour visible to a user goes in `docs/`.
- **Commits and PRs**: no model names; the attribution lines the session is given.

## Waves

| Wave | Milestone | Written |
|---|---|---|
| 1 | M0: gateway and a live terminal | Now (this index) |
| 2 | M1: parser, events, status; brain process | Partly now: C-010 (parser) and C-011 (world pack) need nothing from M0 and run beside wave 1. The rest once M0 is live |
| 3+ | M2 onward | When the wave before is merged and John has played with it |

## Index (wave 1, M0)

| Id | Title | Tier | Size | Depends | Review |
|---|---|---|---|---|---|
| [C-001](C-001-repo-scaffold.md) | Repository scaffold: workspace, toolchain, the fast CI job, fresh CLAUDE.md, plan moved to `docs/plan/` | B | S | — | |
| [C-002](C-002-proto.md) | `wayfarer-proto`: M0 wire messages, handshake and version, TypeScript generation | B | S | C-001 | review |
| [C-003](C-003-telnet.md) | `wayfarer-telnet`: option negotiation (NAWS 80x24), CP437 both ways, raw capture | C | S | C-001 | |
| [C-004](C-004-gateway-sessions.md) | Gateway sessions: connect, sequence-numbered output log, history, direct input | B | M | C-002, C-003 | review |
| [C-005](C-005-logon-scripts.md) | Logon scripts: prompt → reply steps, presets, reconnect, clean log-off | B | M | C-004 | |
| [C-006](C-006-websocket-server.md) | WebSocket server: token auth, localhost by default, static files, `wayfarer gateway` | C | S | C-002, C-004 | |
| [C-007](C-007-web-ui.md) | Web UI scaffold: Svelte, Vite, xterm.js; a tile per session, input, scrollback, reconnect | B | M | C-006 | review |
| [C-008](C-008-dev-rig.md) | Development setup on the laptop: `docs/dev-rig.md`, gateway and watcher scripts, worktrees and background sessions | C | S | C-005, C-006 | |
| [C-009](C-009-live-check.md) | `scripts/live-check.sh`: against the local silvermere, test-character pool, logon/log-off scenario | B | S | C-005, C-006 | |

## Index (wave 2, written early)

| Id | Title | Tier | Size | Depends | Review |
|---|---|---|---|---|---|
| [C-010](C-010-parser.md) | `wayfarer-parse`: pattern table, generated game-text patterns, incremental parser, golden parity with `patterns.py` | B | L | C-001 | review |
| [C-011](C-011-world-pack.md) | `wayfarer-world`: the world pack (SQLite) from the stock-world export, MegaMud waypoints and verified loops | B | M | C-001 | |

Tier: B Opus, C Sonnet. Size: S under half a day of agent time, M about a day, L two to three
days.

Parallelism: C-002, C-003, C-010 and C-011 together after C-001; then C-004; then C-005 and
C-006 together; then C-007, C-008 and C-009 together.

**M0 is done** when John opens the browser on his rig, sees one tile per configured
character, each logged on by its logon script, types into them, scrolls back, reloads the
page without anyone being logged off, and stops the gateway with every character logged off
cleanly.
