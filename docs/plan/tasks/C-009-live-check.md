---
id: C-009
title: scripts/live-check.sh: against the local silvermere, test-character pool, logon/log-off scenario
tier: B
size: S
wave: 1
depends: [C-005, C-006]
crate: scripts/, tests/live/
touches:
  - scripts/live-check.sh
  - tests/live/ (scenario runner and the fixture definitions)
  - tests/live/fixtures.toml (test characters John approves; no passwords)
  - .gitignore (the local credentials file)
---

## Goal

The test that needs a game server, run by a card's own session on John's laptop before it
pushes, never in GitHub Actions: connect `wayfarer gateway` to the silvermere that is already
running locally, log a test character on, check the M0 scenario (reach the realm, type a
command, log off cleanly), and print a summary for the PR, in a minute or two.

## Inputs

- `docs/plan/07-development-and-testing.md` "The development machine" and "Live tests against
  the local server": the server is long-lived, shared by every card, and never started,
  stopped, rebuilt or reset by a card.
- The gateway (C-004, C-005) and its WebSocket server (C-006).
- `mmud-exploration/kb/connection.md` (on the laptop: `../mmud-exploration/kb/connection.md`):
  the `NEW` account dialogue, character creation (race and class lists, the lawful question,
  the full-screen editor's cursor positions and name rules), the disconnect punishment.
- `tests/fixtures/captures/entry-new-account/` and `creation-fsd/`: the exact dialogues.

## Behaviour

- `scripts/live-check.sh` (optionally `--char NAME`):
  1. Checks the server answers at `WAYFARER_LIVE_ADDR` (default `127.0.0.1:2323`). If not, it
     stops with "start silvermere first" and the command from `docs/dev-rig.md`; it never
     starts one itself.
  2. **Test-character pool.** `tests/live/fixtures.toml` lists test characters: account
     name, character name, race, class and stats (**John approves the values**; the card
     proposes them in the PR). Passwords are not in the repository: the first time a
     character is needed the script creates its account through the `NEW` dialogue with a
     generated password, saved to the untracked `tests/live/.credentials.toml` (mode 0600,
     git-ignored), then creates the character through the game's own editor with the fixture
     stats. This is test setup John approved, not the bot spending CP.
  3. **Claiming a character.** It tries the pool in order and uses the first one that logs
     on; the server refuses a second logon to an account already in use, so concurrent cards
     never share one.
  4. Starts `wayfarer gateway` on a free local port with a config for that character, and
     through its WebSocket checks: the session reaches `realm`; an `input_line` of `st`
     produces the status screen; on shutdown the character logs off cleanly; a second logon
     shows no "disconnected while playing" notice.
  5. Prints a short summary (pass or fail per step, timings, which character) to paste into
     the PR, and stops the gateway it started. The server keeps running.
- Total time once the pool exists: under two minutes.

## Design notes

- The scenario runner is a small Rust test binary under `tests/live/` (a separate package or
  an `#[ignore]`d integration test the script runs), so CI never runs it.
- The world is shared and long-lived (other cards' characters, monsters killed, levels
  gained): check behaviour, never exact numbers or room contents.

## Done when

- The script passes twice in a row on the laptop, the first run creating the pool and the
  second reusing it; the PR shows both summaries and times.
- With the server stopped, it fails fast with the "start silvermere first" message.
- Nothing in `.github/workflows/` runs it, and no password is committed.

## Tests

- The scenario itself; unit tests for the fixture parsing and the claiming order.

## Notes for the agent

- This card runs on John's laptop, where the server is. Don't touch
  `~/repos/wayfarer-dev/silvermere-wayfarer` or the server process.
- Keep scenarios few in M0; later cards add theirs (walk, fight, rest, flee) here.
