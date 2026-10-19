---
id: C-016
title: `wayfarer-brain` and `wayfarer brain`: attach to the gateway, parse every session, the character model, snapshots to UIs, re-sync on attach
tier: B
size: L
wave: 2
depends: [C-009, C-014, C-015]
crate: wayfarer-brain, bin/wayfarer
touches:
  - crates/wayfarer-brain/ (new)
  - bin/wayfarer/src/main.rs (`brain` subcommand)
  - tests/live/ (an M1 scenario), tests/fixtures/ (nothing new expected)
  - docs/brain.md (new)
---

## Goal

The brain process exists: `wayfarer brain` attaches to a running gateway as its brain client,
parses every session's output, keeps a character model per session, publishes snapshots and
events to the UIs, and re-syncs a character by asking the game (`st`, `exp`, `i`, look) when
it attaches or is told to. It can be stopped and started at any time while everyone stays
logged in (`03-engine.md` section 7). It sends nothing else: no bot, no modes, no executor.

## Inputs

- `docs/plan/02-architecture.md` "The shape" (the brain as a privileged client; two processes
  in development, one in a release), "Input arbitration" (not built: no bot yet).
- `docs/plan/03-engine.md` sections 1 (events), 2 (the character model), 6 (talk-and-look
  commands go at a prompt boundary: the re-sync commands are these), 7 (restarts), 8 (all
  characters in one brain).
- `docs/plan/07-development-and-testing.md` "The development machine" (the brain under a
  watcher; the gateway keeps the sessions), "Test layers" (layer 3, replay).
- C-014 (`docs/protocol.md`: `event`, `character`, `character_patch`, `brain_info`,
  `notice`, `warning`, `resync`, `state_request`, `sent`, `brain_state`), C-015
  (`GameEvent`, `WorldPack::game_texts`), C-010 (`Parser`, `parser.sent`), C-011
  (`WorldPack::open`).
- `crates/wayfarer-gateway/README.md` (history, `lagged`, the WebSocket routes); C-009's
  `tests/live/` (the scenario runner to extend).

## Behaviour

- **Command.** `wayfarer brain [--gateway ws://127.0.0.1:7373/ws] [--world PATH] [--token
  TOKEN]`. `--world` defaults to `world.sqlite` in the data directory; when it is missing the
  brain stops at once and prints the `wayfarer import-world` command that builds it. When
  `--token` is not given the brain reads the gateway's `gateway-token` file beside the
  config (`Paths`, as `wayfarer gateway --new-token` writes it); a loopback gateway with no
  token needs none.
- **Attach.** `hello {client: brain}`; on `welcome`, send `brain_info`, then for each session:
  create a `Parser`, fetch the last 2,000 chunks of history and feed them **silently** (no
  `event` messages, model updated) so the latest prompt's vitals are known before anything
  is sent; then follow live output. On `lagged`, fetch the gap and feed it. The parser is
  told of each `sent` line (`parser.sent`) so echoes classify as echoes. A session that
  appears later (`session_list`) gets the same treatment; a closed one keeps its last
  snapshot, marked by the session state the UI already shows.
- **Model.** `Character` applies every `GameEvent` (prompt vitals, status screens, exp
  screen, room title and exits, combat on and off, kills and exp gain, death and lives,
  statuses, inventory and wealth, resting) and counts lines and unclassified lines. After
  each chunk, if the snapshot changed, send `character_patch` (the merge patch between the
  old and new snapshot); send a whole `character` after attach, after a re-sync, and on
  `state_request`. Every parsed event goes out as `event` with its `seq` and `line_no`.
- **Re-sync.** When a session is in the realm and the brain has not completed a re-sync for
  it since attaching, when the session enters the realm, and on `resync`: at an **idle
  prompt** (the last chunk ended in a prompt and nothing was `sent` for 2 s), send `st`, wait
  for the status screen (or 5 s), then `exp`, then `i`, then `look` (an empty line
  redisplays the room only in statline FULL modes, `kb/screen-parsing.md`; `look` always
  does), one at a time, each at the next prompt. Then a whole `character` with `synced_at`, and a `notice "re-synced"`. If the
  player types during a re-sync, the rest waits for the next idle prompt. Never send while
  the session is `logon`, `menu`, `connected`, `lost` or `closed`.
- **Restart.** Stopping the brain (Ctrl-C, SIGTERM) closes the WebSocket and exits; nothing
  is sent to the game. Starting it again attaches, re-syncs, and the UIs see
  `brain_state false` then `true`. A brain refused because one is attached exits non-zero
  with the gateway's reason.
- **Gateway gone.** When the WebSocket drops, retry with the same backoff as the UI and
  attach afresh (history catch-up from the last `seq` seen, no re-sync unless the session's
  realm state was lost meanwhile).
- **Logs.** `tracing` to stderr as the gateway does; one line per re-sync and per restart;
  never the text of what the player typed.

## Design notes

- `wayfarer-brain` depends on `proto`, `parse`, `world`; not on `gateway`. The connection
  is behind a small `Link` trait (`send(ToGateway)`, `recv() -> ToClient`) with a
  `WsLink` (tokio-tungstenite) and a `ChannelLink` (tokio mpsc pair). Tests and C-019's
  `wayfarer serve` use `ChannelLink`.
- `Brain::run(link, world, clock)`: one task reading the link, a `Session` struct per
  session (parser, character, re-sync state machine), all in one `tokio` task to keep it
  deterministic; time through `tokio::time` so tests can pause it.
- `Character` is a plain struct with `apply(&GameEvent) -> bool` (changed?) and
  `snapshot() -> CharacterState`; the merge patch from `serde_json` values of two snapshots
  (a small function, not a crate, unless `json-patch`'s merge is already a transitive
  dependency).
- The re-sync is a state machine driven by events and a timer: `Idle`, `Waiting {cmd,
  deadline}`; it owns the only path that sends to the game in M1.
- Keep the brain's own persisted state at nothing: M1 has no plan, loop position, pile or
  pause state to keep (John, 2026-10-19); say so in `docs/brain.md` and where C-0xx will add
  the store.

## Done when

- In-process tests with `ChannelLink` and a fake gateway that replays the fixtures
  (`status-screens`, `fight-arena`, `room-walk`, `death-rebirth`, a transcript): the
  snapshot after each fixture has the expected HP, mana, level, class, race, stats, room
  title and exits, combat flag, lives; the patches applied to the first snapshot equal the
  last snapshot; the events sent equal the parser's.
- Re-sync: sends `st` only at an idle prompt; a `sent` from the player defers it; the four
  commands go out in order, each after the previous answer; `resync` from a UI triggers it;
  nothing is sent in `menu`.
- Attach catch-up: with history available, the first snapshot already has the vitals from the
  last prompt before anything is sent.
- Restart: two attaches in a row against the fake gateway give the same snapshot.
- Refusal: a second brain exits non-zero.
- **Live**: `scripts/live-check.sh` gains scenario "brain": start the gateway as the M0
  scenario does, start `wayfarer brain` against it, expect `brain_state true`, a `character`
  snapshot whose `hp` equals the prompt's within 10 s, then stop the brain and see the
  character still in the realm (`st` works over the gateway). Paste the summary into the PR.
- `docs/brain.md`: how to run it, what it does and does not do in M1, the re-sync rules.

## Tests

- As listed, under a second each on a paused clock; the live scenario by hand in the card's
  session.

## Notes for the agent

- Nothing in this card sends an acting command. The re-sync commands are the "talk and
  look" kind (`03-engine.md` section 6); keep the list to `st`, `exp`, `i` and the look.
- An unclassified line is data for the dev panel (C-018), never an error.
- The world pack for the rig: `wayfarer import-world --export tests/fixtures/world --out
  "$WAYFARER_DATA_DIR/world.sqlite"` (C-019 scripts it); say so in `docs/brain.md`.
