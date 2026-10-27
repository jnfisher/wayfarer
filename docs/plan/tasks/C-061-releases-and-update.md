---
id: C-061
title: "Releases and `update`: a tag builds Linux x86_64 and aarch64 tarballs; `scripts/update.sh` installs one on a server and restarts the systemd unit"
tier: B
size: M
wave: 10
depends: []
crate: .github/, scripts/, packaging/, docs/
touches:
  - .github/workflows/release.yml (new)
  - scripts/update.sh (new), scripts/tests/update-test.sh (new), .github/workflows/ci.yml (one step)
  - packaging/wayfarer.service
  - bin/wayfarer/src/main.rs (only `--version`, if it needs the tag)
  - docs/server.md, docs/dev-rig.md (one line), docs/plan/08-packaging.md (one line)
---

## Goal

A box that only runs wayfarer, such as a Raspberry Pi 5 running Ubuntu, never builds it.
Pushing a `v*` tag makes GitHub Actions build Linux tarballs for x86_64 and aarch64 and
attach them to a GitHub Release. On the box, `scripts/update.sh` downloads the release for its
architecture, installs it beside the previous one, and restarts the `wayfarer serve` systemd
user unit, which logs every character off cleanly first. Each step prints as it goes. (John,
2026-10-26: "my plan is to actually have it use a GH release of wayfarer so it doesnt have to
compile"; systemd, not tmux; the name is "update".)

The dev box keeps `scripts/dev/refresh.sh` (pull, rebuild, restart the tmux rig), which asks
before building on an ARM machine and points at `update.sh`.

## Inputs

- `docs/server.md` (C-026): the data directory, the token and bind, the unit, "Upgrading and
  backups". `packaging/wayfarer.service`: the user unit, `KillSignal=SIGTERM`,
  `TimeoutStopSec=390` (the gateway waits up to 360 s for the log-offs).
- `scripts/dev/refresh.sh`: the feedback style to copy (numbered steps, ✓ and ! lines, a
  status line with the timer and the last log line while waiting, a red ✗ on failure), and
  "check everything before stopping anything".
- `scripts/dev/gateway.sh`: how `ui/dist` is built and how the world pack is imported (with
  `data/megamud` for MegaMud's loops). The pack is opened read-only
  (`crates/wayfarer-world/src/pack.rs`), so a release may ship its own.
- `.github/workflows/ci.yml` and `ui.yml`: the toolchain setup and caching to reuse.
- `docs/plan/08-packaging.md` "Release builds" and "Headless": the plan this is a small part
  of. The installers, signing and the update manifest stay out.

## Behaviour

### The release workflow (`.github/workflows/release.yml`)

- Runs on a pushed tag `v*`, and by hand (`workflow_dispatch`). Run by hand, it builds and
  uploads the tarballs as workflow artifacts but publishes no release, so it can be tried
  without a tag.
- Builds natively on two runners: `ubuntu-22.04` (x86_64) and `ubuntu-22.04-arm` (aarch64).
  22.04's glibc (2.35) also runs on newer Ubuntu, Debian 12 and Raspberry Pi OS (bookworm).
  No cross-compiling.
- Each runner runs `cargo build --release --locked -p wayfarer`, builds the UI (`npm ci`,
  `npm run build`) and the world pack (`wayfarer import-world --export tests/fixtures/world
  --megamud data/megamud --out world.sqlite`), then packs
  `wayfarer-<tag>-linux-<x86_64|aarch64>.tar.gz`. The tarball holds one top directory,
  `wayfarer-<tag>/`, containing:
  - `wayfarer` (the binary)
  - `ui/dist/`
  - `world.sqlite`
  - `wayfarer.service` (the unit template)
  - `update.sh`
  - `VERSION` (the tag)
- A last job puts `SHA256SUMS` beside the tarballs and publishes the release (the tag's
  message as its notes, marked pre-release while the tag has a `-` in it, e.g. `v0.1.0-rc1`).
- `wayfarer --version` prints the tag in a release build (the crate version is fine
  elsewhere).

### `scripts/update.sh` (on the server)

```sh
update.sh              # the latest release
update.sh v0.2.0       # that release
update.sh --rollback   # back to the release installed before this one
```

It needs only `bash`, `curl`, `tar`, `sha256sum`, `systemctl` and `tailscale`: no git, Rust
or npm. It runs from anywhere (it ships in the tarball, and `releases/current/update.sh` is
always there).

1. **Checks**: the tools; the machine's architecture (`uname -m`: x86_64 or aarch64; anything
   else stops with a message); the data directory (`~/.local/share/wayfarer`, or
   `WAYFARER_DATA_DIR`) has `wayfarer.toml`, `credentials.toml` and `gateway-token`. When one
   is missing it names the `docs/server.md` step that makes it and stops.
2. **Finds the release**: from the GitHub API (`WAYFARER_REPO`, default the project's own
   repository), the latest one or the tag given. It prints the tag and the one installed now.
   It stops with "already installed" when they match, unless `--force` is given.
3. **Downloads and checks** the tarball for this architecture and `SHA256SUMS`, then unpacks
   to `~/.local/share/wayfarer/releases/<tag>/`. It prints sizes and progress, and stops
   before anything is stopped if the checksum fails.
4. **Installs the unit the first time**: when `~/.config/systemd/user/wayfarer.service` is
   missing, it writes it from the template with the paths and the Tailscale address
   (`tailscale ip -4`, or `WAYFARER_BIND_IP`; port 7373 or `WAYFARER_PORT`) filled in, runs
   `systemctl --user daemon-reload` and `enable`, and prints `sudo loginctl enable-linger
   $USER` for John to run (it never runs sudo). An existing unit is left alone. When its
   `ExecStart` doesn't use `releases/current`, it says so and how to fix that.
5. **Stops wayfarer**: `systemctl --user stop wayfarer` in the background, while it follows
   `journalctl --user -u wayfarer` and shows the last line and a timer ("logging off (42s):
   … the brain is ready: logging off session=…"), until the unit is inactive. Skipped when
   the unit isn't running.
6. **Switches**: `releases/previous` → the old `current` target, `releases/current` → the
   new release (both symlinks, each replaced in one step, so it is never half switched).
7. **Starts** the unit and waits until the port answers (or the unit fails: then it prints
   the last 20 journal lines, says `update.sh --rollback`, and exits non-zero).
8. **Tidies**: keeps the current, previous and one more release, and deletes older ones.
9. Ends with `✓ wayfarer <tag> is running. The UI is at http://<ip>:7373/ ; logs: journalctl
   --user -u wayfarer -f`.

`--rollback` does steps 5 to 7 with `current` and `previous` swapped. With no previous
release it says so and changes nothing.

### The unit (`packaging/wayfarer.service`)

It points at the release layout: `%h/.local/share/wayfarer/releases/current/wayfarer serve
--config %h/.local/share/wayfarer/wayfarer.toml --bind @BIND@ --ui-dir
%h/.local/share/wayfarer/releases/current/ui/dist --world
%h/.local/share/wayfarer/releases/current/world.sqlite`. `@BIND@` is the one placeholder
`update.sh` fills in. The comments say how to fill it by hand, and how a box built from source
(C-026) points it at its checkout instead.

## Design notes

- `update.sh` is plain bash in the style of `refresh.sh`, and as careful: `set -euo
  pipefail`; nothing is stopped until the release is downloaded, checked and unpacked; it
  never kills wayfarer by PID, only `systemctl --user stop` (the unit's SIGTERM is the clean
  log-off).
- For tests, `WAYFARER_RELEASE_BASE` replaces the GitHub download URLs (a `file://` directory
  holding a fake `latest` answer, the tarballs and `SHA256SUMS`), and `PATH` can put stub
  `systemctl`, `journalctl` and `tailscale` first.
- The world pack ships in the release and is not copied into the data directory: the data
  directory keeps only what is the player's (config, credentials, token, the store, the logs).
  `docs/server.md` says that an older `world.sqlite` left in the data directory is no longer
  read once the unit uses `--world …/current/world.sqlite`.
- Leave out: signing, an update manifest, auto-update, installers, a Docker image, macOS and
  Windows (`08-packaging.md` keeps those). The release workflow is the first piece of its
  "Release builds"; add a line there saying so.

## Done when

- The workflow run by hand on the PR branch produces both tarballs as artifacts. Each holds
  the files listed, the aarch64 `wayfarer` is an ARM64 ELF (`file`), and `wayfarer --version`
  in the x86_64 one runs on the runner. The PR links the run.
- `scripts/tests/update-test.sh` passes locally and in CI (one step in `ci.yml`, under ten
  seconds, no network).
- `docs/server.md` has a "Running a release" section before "Build": the first install
  (download `update.sh` once from the release page, or copy it, run it, then `enable-linger`)
  and later updates and rollback. "Build" is presented as the way for a box that builds from
  source. "Upgrading" points at both `update.sh` and building from source.
- `docs/dev-rig.md` "Refreshing everything at once" links the new section.
- **Needs John**: pushing the first tag (e.g. `v0.1.0-rc1`), then running `update.sh` on the
  Pi: it installs the unit, and the characters log on and are watched from the laptop's
  browser. A second `update.sh --force` logs them off cleanly and back on, and `--rollback`
  works.

## Tests

`scripts/tests/update-test.sh`, against a temporary `HOME` and fake releases built in the
test (a stub `wayfarer` that listens on the port and logs a log-off line on SIGTERM) and stub
`systemctl`, `journalctl` and `tailscale` on `PATH`:

- First install: the unit is written with the address filled in, the `current` link is set,
  the unit is started, and the output has the steps and the final URL.
- An update to a second tag: stop before switch, `previous` is set, and the old release is kept.
- The same tag again: "already installed", nothing stopped. `--force`: reinstalled.
- A bad checksum: stops before stopping anything, and `current` is unchanged.
- A missing `wayfarer.toml`: stops and names the `docs/server.md` step.
- `--rollback`: the links swap and the unit is restarted. With no previous release: nothing
  changes.
- An unknown architecture (a stub `uname`): a clear refusal.

## Notes for the agent

- The repository is public: no hostnames, addresses or account names. Use `100.x.y.z` and
  `USER` in docs.
- Don't start, stop or touch the dev rig on John's laptop or the local silvermere. Every test
  is in a temporary `HOME` with stubs.
- `refresh.sh` (dev box) and its ARM prompt already exist (they came with this card's PR).
  Only its pointer in `docs/dev-rig.md` changes here.
- `ubuntu-22.04-arm` is a free hosted runner for public repositories. If the name has changed,
  use the current one for Ubuntu 22.04 on ARM, and say so in the PR.
- Release asset names are fixed by this card. `update.sh` and the workflow must agree;
  `update-test.sh` builds its fakes with the same names.
