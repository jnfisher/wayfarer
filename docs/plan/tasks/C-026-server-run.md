---
id: C-026
title: Running wayfarer on a server: build, data directory, token, systemd unit, Tailscale
tier: C
size: S
wave: 3
depends: [C-019]
crate: docs/, packaging/
touches:
  - docs/server.md (new), packaging/wayfarer.service (new), docs/README.md
  - bin/wayfarer/src/main.rs (only if a message needs a better hint)
---

## Goal

John can run `wayfarer serve` on a Linux box, keep the characters logged in around the
clock, and watch them from a browser on another machine over Tailscale (John, 2026-10-19).
This card writes the page that gets him there and the service unit, and checks the binary's
behaviour for that case; it ships no installer (M6).

## Inputs

- `docs/plan/08-packaging.md` (what M6 will do; this card stays below it), `02-architecture.md`
  "Security" (localhost by default; a token for remote access; anyone who reaches the gateway
  can play the characters).
- `docs/gateway.md`, `docs/brain.md`, `docs/dev-rig.md` (what exists: `--bind`, `--new-token`,
  `token_for_bind` refusing a non-loopback bind without a token, `--ui-dir`, `--world`, the
  data directory on Linux `~/.local/share/wayfarer/`), `bin/wayfarer/src/main.rs`.
- silvermere's `docs/install.md` "silvermere service install" (the user-unit pattern with
  `loginctl enable-linger`; copy the shape, not the installer).

## Behaviour

`docs/server.md` covers, in order, each step tried on a real Linux machine or VM:

1. **Build**: `rustup`, a C linker, Node LTS; `git clone`, `cargo build --release`,
   `npm --prefix ui ci && npm --prefix ui run build`; where the binary and `ui/dist` end up.
   Or: build `ui/dist` on the laptop and copy it.
2. **Data directory**: `~/.local/share/wayfarer/` with `wayfarer.toml` (from
   `examples/dev.toml`, pointing at the game server: the local silvermere, or a BBS on the
   internet), `credentials.toml` (0600), `world.sqlite` (`wayfarer import-world --export
   tests/fixtures/world`), the logs.
3. **Token and bind**: `wayfarer gateway --new-token`; bind to the Tailscale address
   (`--bind 100.x.y.z:7373`), why not `0.0.0.0`; the page asks for the token once; a
   Tailscale ACL limiting the port to John's devices. No TLS, and why that is fine inside the
   tailnet (and what to do if ever exposed otherwise: don't).
4. **The unit**: `packaging/wayfarer.service`, a user unit running `wayfarer serve --config
   … --bind … --ui-dir … --world …`, `Restart=on-failure`, `KillSignal=SIGTERM` and a
   `TimeoutStopSec` above the gateway's 120 s log-off patience so a stop logs everyone off;
   `loginctl enable-linger`; `systemctl --user` commands; where logs go (`journalctl --user`).
5. **Running silvermere beside it** on the same box (its own unit, `127.0.0.1:2323`), or a
   remote BBS; sleep is not an issue on a server, reconnects are (the logon scripts).
6. **Upgrading**: pull, rebuild, `systemctl --user restart` (everyone logs off and back on);
   backing up the data directory.
7. **Development stays on the laptop**: cards and the live check need the local rig.

Binary checks: a non-loopback bind without a token says exactly what to run; `serve` under
SIGTERM from systemd logs everyone off within the patience; a missing `ui/dist` gives the 503
page but `/ws` still works (already true); `--world` missing says the import command.

## Design notes

- Write for the person who ran `docs/dev-rig.md`: short, one command per step, no
  distribution-specific detours beyond Debian/Ubuntu package names.
- The unit file is a template with placeholders for the paths; say which to replace.

## Done when

- The page and the unit tried end to end on a Linux VM or container (multipass, a cloud
  VM, or Docker with systemd): fresh clone to a browser on another machine showing a tile
  logged into a silvermere running beside it; `systemctl --user stop` logs the character off
  cleanly (the gateway log); the PR records what was used and how long it took.
- The token refusal message names `wayfarer gateway --new-token`.
- `docs/README.md` links the page.

## Tests

- None in CI beyond what exists; the end-to-end run is the evidence.

## Notes for the agent

- Do not add an installer, a Docker image or a release workflow: M6.
- Nothing personal in the page: placeholder addresses and names.
