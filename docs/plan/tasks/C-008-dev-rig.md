---
id: C-008
title: Development rig: run a pinned silvermere, the gateway as a service, a watcher; README
tier: C
size: S
wave: 1
depends: [C-005, C-006]
crate: scripts/, docs/
touches:
  - scripts/dev/{silvermere-up.sh,gateway-up.sh}
  - scripts/dev/wayfarer-gateway.service (systemd user unit)
  - docs/dev-rig.md
---

## Goal

Written instructions and a few scripts that let John set up his rig once and then watch
characters in the browser: a pinned silvermere with its own database, the gateway as a
long-lived service, the UI with hot reload, and the way in from his laptop. Nothing here runs
automatically; John starts what he wants.

## Inputs

- `docs/plan/07-development-and-testing.md` "Four restart domains", "John's development rig",
  "Where work runs".
- `silvermere/README.md` and `silvermere/docs/install.md` (build, `import --from`, `serve`,
  `account create`, `backup`), `silvermere/silvermere.example.toml` (telnet 2323, admin 8080).
- `mmud-exploration/dist/WCCMMUD_1.11p_DOS_MBBSEmu_v2.zip` (the game files the importer needs)
  and `mmud-exploration/HANDOFF.md` "Server setup" (for MBBSEmu, the reference server).

## Behaviour

- `scripts/dev/silvermere-up.sh [--version TAG | --commit SHA]`: fetch the pinned silvermere
  release binary (or build that commit once, if no release exists), import the world from the
  MBBSEmu zip into its own data directory if not done, and start `silvermere serve` on 2323
  (in tmux by default). Re-running it never re-imports or wipes the database.
- `scripts/dev/gateway-up.sh`: build `wayfarer` in release mode and start `wayfarer gateway`
  with the rig's config (tmux), or install and start the systemd user unit.
- `docs/dev-rig.md`:
  - one-time setup (the two scripts; creating accounts with `silvermere account create`;
    creating characters by logging on in a tile with the keys toggle, since the stat editor
    is the player's);
  - the config and credentials files, from `examples/`;
  - watching from the laptop: `ssh -L 7373:localhost:7373 rig`, then `http://localhost:7373`;
  - UI work: `npm run dev` in `ui/` for hot reload;
  - what restarts when (the four domains), and that the game server is never restarted for
    client work; upgrading silvermere on purpose (`--version`).
- A placeholder section "Brain (from M1)" saying the brain will run under a watcher here.

## Done when

- On a fresh Linux container, following `docs/dev-rig.md` start to finish (with the game zip
  available) brings up silvermere and the gateway, and a tile shows a character in the realm
  (screenshot in the PR, as in C-007).
- The scripts are idempotent: running each twice changes nothing the second time.

## Tests

- None in CI (shell scripts on a rig). `shellcheck` clean, run locally.

## Notes for the agent

- Do not add anything that runs on a schedule or registers a CI runner.
- If silvermere has no release yet, `--commit` with a cargo build is the path; say so in the
  doc and keep the release path ready for when it exists.
