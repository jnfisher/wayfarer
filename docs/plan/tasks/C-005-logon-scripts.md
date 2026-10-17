---
id: C-005
title: Logon scripts: prompt → reply steps, presets, reconnect, clean log-off
tier: B
size: M
wave: 1
depends: [C-004]
crate: wayfarer-gateway
touches:
  - crates/wayfarer-gateway/src/{logon.rs,presets/}
  - crates/wayfarer-gateway/src/session.rs (states, reconnect, shutdown)
---

## Goal

Each character logs itself on: the gateway runs a logon script from connect to the realm
prompt, tracks where the session is (logon, menu, realm), reconnects after a lost connection,
and logs every character off cleanly when the gateway stops, so nobody is punished for
dropping in the realm.

## Inputs

- `tests/fixtures/captures/entry-existing/script.json` and `logoff-in-realm/script.json`
  (copied in by C-001): the exact dialogue. Logon: `Username:` →
  user, `Password:` → password, `Make your selection` → `M`, `[MAJORMUD]:` → `E`, then the
  realm prompt `\[HP=-?\d+[^\]\n]*\]:`. Log-off: `x` → wait for `[MAJORMUD]:` (up to 90 s:
  the character meditates silently first), `X` → `Make your selection`, `X` → `Are you sure`,
  `Y` → the server closes.
- `mmud-exploration/kb/connection.md` "Login flow (MBBSEmu)": the disconnect punishment, the
  "already logged in" refusal (`<user> is already logged in -- only 1 connection allowed per
  user.`), `Invalid Credentials`, unlimited retries at `Username:`.
- `silvermere/crates/silvermere-auth/src/login.rs` and
  `silvermere/crates/silvermere-engine/src/state/login.rs`: silvermere's prompts (the same
  `Username: `, `Password: `, BBS menu shim, `Are you sure (Y/N, or R to re-logon)?`).
- `docs/plan/06-accounts-and-setup.md` "Logon scripts".
- `mmud-exploration/kb/megamud-manual.md` 1.2-1.3 (MegaMud's 20 prompt/reply pairs, its
  reconnect options), for what players expect.

## Behaviour

- A logon script is a list of steps `{expect, send, optional, timeout}`; `expect` is a
  substring or `re:` regex matched against decoded text since the last step; `send` may use
  `{user}` and `{password}`. A final `realm` regex marks success.
- Presets `mbbsemu` and `silvermere`, in TOML files compiled into the binary, selected by the
  server's `preset`. A server may also give its own steps inline.
- States: `connecting` → `logon` → `menu` (at `Make your selection` or `[MAJORMUD]:`) →
  `realm` (at the realm prompt). After entry, the session follows the text: the main menu
  prompt sets `menu`, a realm prompt sets `realm`.
- Failure handling:
  - `Invalid Credentials` (or the server's equivalent): state `closed` with reason "bad
    credentials"; no retry.
  - "already logged in": retry with backoff (the old connection may still be closing).
  - A step timeout: close and retry with backoff.
- Reconnect after `lost` with backoff 5 s, 10 s, 30 s, then every 60 s; never after a
  deliberate `close`. Each attempt is a line in the gateway log.
- The password send is written to `sent.bin` as `********`.
- **Clean log-off**: `close(id)` from `realm` runs the log-off steps (above), from `menu` the
  remaining ones, then closes. Gateway shutdown (SIGINT or SIGTERM) logs every session off in
  parallel, waits up to 120 s, then closes what is left. A second signal closes at once.
- Typed input still works during logon (the steps keep matching).

## Design notes

- The script runner is a small state machine fed with decoded text; testable without
  sockets. The session task feeds it and acts on its decisions.
- Keep step matching on a rolling window of recent text, cleared after each matched step.

## Done when

- Against an in-test fake server speaking the MBBSEmu dialogue (prompt strings from the
  captures), a session goes `connecting → logon → menu → realm` with the right sends.
- Bad credentials stop without retrying; "already logged in" retries; a dropped connection in
  the realm reconnects and logs on again; the backoff schedule is tested with a paused clock.
- Shutdown from the realm sends `x`, `X`, `X`, `Y` in order, each after its prompt; from the
  menu, only the remaining ones.
- `sent.bin` never contains the password.

## Tests

- Runner unit tests per preset; fake-server tokio tests for the flows above, using
  `tokio::time::pause()` for timeouts and backoff (no real waiting).

## Notes for the agent

- Character creation (a new account with no character) is not handled here; the script stops
  at whatever the game shows and the player takes over in the tile (`06-accounts-and-setup.md`
  makes creation a later wizard).
- Do not add "learn by doing" recording; that is a later card.
