---
id: C-008
title: Development setup on the laptop: docs/dev-rig.md, gateway and watcher scripts, worktrees and background sessions
tier: C
size: S
wave: 1
depends: [C-005, C-006]
crate: scripts/, docs/
touches:
  - docs/dev-rig.md
  - scripts/dev/gateway.sh
  - scripts/dev/card.sh (start a card as a background Claude Code session)
  - examples/ (the dev gateway config, placeholders only)
---

## Goal

One page John can follow to run the whole loop on his MacBook: the local silvermere, the
gateway, the UI with hot reload, and card sessions running in parallel worktrees. Plus two
small scripts so the routine steps are one command each. Nothing runs on a schedule.

## Inputs

- `docs/plan/07-development-and-testing.md` "Four restart domains", "The development machine",
  "Where work runs", "Live tests against the local server".
- `docs/plan/tasks/README.md` "How cards run" (worktrees, `claude --bg`, permissions).
- The setup John already did (2026-10-17): repositories in `~/repos/`, silvermere built in
  `~/repos/wayfarer-dev/silvermere` and run from `~/repos/wayfarer-dev/silvermere-wayfarer` with
  `[server] telnet_bind = "127.0.0.1:2323"`, `admin_bind = "127.0.0.1:8080"`.
- `silvermere/README.md` and `docs/install.md` (build, `import`, `serve`, `account`, `backup`).
- C-006 (`wayfarer gateway`), C-007 (the UI, `npm run dev`).

## Behaviour

- `docs/dev-rig.md`, written for macOS:
  - **silvermere**: build, first-time `import`, `serve` in a terminal tab from
    `~/repos/wayfarer-dev/silvermere-wayfarer`; Ctrl-C saves and stops; upgrading (pull, rebuild,
    restart); backups with `silvermere backup`. Why it lives outside the checkout.
  - **The gateway**: `scripts/dev/gateway.sh` builds `wayfarer` in release mode and runs
    `wayfarer gateway` with the dev config (`examples/dev.toml`, placeholders; the real
    config and `credentials.toml` live in the user's data directory, never in the repo).
  - **The UI**: `http://localhost:7373`, or `npm run dev` in `ui/` for hot reload.
  - **Cards**: `scripts/dev/card.sh C-005` starts a card as a background Claude Code session
    (`claude --bg`) with the standard brief from `docs/plan/tasks/README.md`, so it runs in
    its own worktree; how to watch, answer and stop background sessions; two or three at a
    time.
  - **Permissions**: what `.claude/settings.json` pre-approves and why.
  - **Sleep**: closing the lid pauses everything; what reconnects on wake (C-005).
  - What restarts when (the four domains); the server is never restarted for client work.
- A placeholder section "Brain (from M1)" saying the brain will run under a watcher here.

## Done when

- Following `docs/dev-rig.md` on the laptop brings up the gateway against the running
  silvermere, and a tile shows a character in the realm (screenshot in the PR).
- `scripts/dev/card.sh` starts a card session that appears in the background session list,
  shown in the PR.
- The scripts are idempotent and `shellcheck` clean; nothing in the repository holds a real
  host, account or password.

## Tests

- None in CI (scripts for one machine).

## Notes for the agent

- This card runs on John's laptop. Don't stop, rebuild or reset the running silvermere.
- Keep the scripts small; they wrap commands John could type, they don't replace the doc.
