# Task cards: index

The cards that build wayfarer, in the format of [CARD-FORMAT.md](CARD-FORMAT.md). Wave 1 is
milestone M0, "gateway and a live terminal" (`../09-milestones.md`). Later waves are written
when the wave before them is running (see "Waves" below).

## Before the first card (John)

1. Create the empty **public** repository `jnfisher/wayfarer` (public so GitHub Actions has no
   minute limit; see `../07-development-and-testing.md` "Public repository"). Done.
2. The laptop setup (`../07-development-and-testing.md`, "The development machine"): the
   repositories side by side in `~/repos/wayfarer-dev/` (moved there 2026-10-19), Claude Code
   installed, and silvermere running from `~/repos/wayfarer-dev/silvermere-wayfarer` with
   telnet on `127.0.0.1:2323`. Done 2026-10-17.

## How cards run (simpler than silvermere's)

- One **local** Claude Code session per card on John's laptop, in its own git worktree, on
  branch `card/C-NNN-slug`, started with the card file as its brief. Either a background
  session from `~/repos/wayfarer-dev/wayfarer`:

  ```sh
  claude --bg "Run task card C-005 for wayfarer: read CLAUDE.md, docs/plan/tasks/README.md and docs/plan/tasks/C-005-logon-scripts.md, work on branch card/C-005-logon-scripts, and finish with a pull request against main with green CI. Do not merge."
  ```

  or a desktop-app session (Cmd+N) with the worktree option. It ends by opening a pull request
  and never merges.
- **Dispatching.** John, or a conductor session he starts in `~/repos/wayfarer-dev/wayfarer` with
  `/loop /conduct` (`.claude/skills/conduct/SKILL.md`), starts the cards whose dependencies
  are merged (one `claude --bg` each), reads the PRs, and merges the ones whose CI is green
  and whose review passes, without asking; anything marked "Needs John" waits for him. The
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
- **Knowledge**: the KB is `docs/kb/` (since 2026-10-19). A confirmed fact about the game or
  MegaMud that it lacks is written there by the card itself, in the same PR: the right file,
  the provenance tag, a line in `docs/kb/changelog.md`, and a summary under "KB notes" in
  the PR description. silvermere's own behaviours go to mmud-exploration's
  `kb/server-behaviours.md` as a note for John.
- **Docs**: each crate has a short `README.md`; behaviour visible to a user goes in `docs/`.
- **Commits and PRs**: no model names; the attribution lines the session is given.

## Waves

| Wave | Milestone | Written |
|---|---|---|
| 1 | M0: gateway and a live terminal | Now (this index) |
| 2 | M1: parser, events, status; brain process | C-010 and C-011 early (they needed nothing from M0); C-012 to C-019 on 2026-10-19, after John tried M0 and said go |
| 3 | M2: world and navigation, plus "fight here" pulled forward from M3 and the server-run page | 2026-10-19, after John tried M1 and said go |
| 4 | M3: the solo bot: Flee, the store, Task mode running a loop, typing beside the bot, replay, the timed run | 2026-10-19, after John accepted M2 |
| 5 | M4 onward | When the wave before is merged and John has played with it |
| 6 | M7: party play, brought forward (`../10-party-play.md`) | 2026-10-22, after John approved the design |

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

## Index (wave 2, M1)

| Id | Title | Tier | Size | Depends | Review |
|---|---|---|---|---|---|
| [C-010](C-010-parser.md) | `wayfarer-parse`: pattern table, generated game-text patterns, incremental parser, golden parity with `patterns.py` | B | L | C-001 | review |
| [C-011](C-011-world-pack.md) | `wayfarer-world`: the world pack (SQLite) from the stock-world export, MegaMud waypoints and verified loops | B | M | C-001 | |
| [C-012](C-012-help-overlay.md) | Web UI help: `?` opens an overlay of wayfarer's keys and controls, with a `?` button beside `keys` | C | S | C-007 | |
| [C-013](C-013-resizable-tiles.md) | Web UI: drag the boundaries between tiles to resize them; the layout is remembered | C | M | C-012 | |
| [C-014](C-014-proto-m1.md) | Protocol M1: event, state, control and dev messages; the gateway relays between the brain and the UIs | B | M | C-002, C-006 | review |
| [C-015](C-015-parse-typed-events.md) | `wayfarer-parse`: game texts from the world pack; typed events for the character model | B | S | C-010, C-011 | |
| [C-016](C-016-brain.md) | `wayfarer-brain` and `wayfarer brain`: attach, parse every session, the character model, snapshots, re-sync | B | L | C-009, C-014, C-015 | review |
| [C-017](C-017-status-header.md) | Web UI status header: live HP and mana bars, level and class, exp and room; the frame corners line up | B | M | C-013, C-016 | review |
| [C-018](C-018-dev-panel.md) | Web UI dev panel: the event stream, unclassified lines and rate, brain build and restarts, re-sync | B | M | C-017 | |
| [C-019](C-019-serve-and-watcher.md) | `wayfarer serve` runs the brain in-process; `scripts/dev/brain.sh` under a watcher; the dev rig doc | C | S | C-008, C-014, C-016 | |

## Index (wave 3, M2 plus two pulls)

| Id | Title | Tier | Size | Depends | Review |
|---|---|---|---|---|---|
| [C-020](C-020-m1-follow-ups.md) | M1 follow-ups: `sent` ordered with output, more events applied to the model, inventory on the wire | B | S | C-016, C-018 | |
| [C-021](C-021-routing-and-belief.md) | `wayfarer-world`: the room graph, routing, the position belief, and the map layout | B | L | C-011, C-015 | review |
| [C-022](C-022-walker.md) | The executor and the walker: protocol M2 (map view, walk to, stop), confirmed moves, doors, relocation | B | L | C-020, C-021 | review |
| [C-023](C-023-map-panel.md) | Web UI map panel: rooms around the character on a canvas, the belief, click a room to walk | B | L | C-022 | review |
| [C-024](C-024-generated-loops.md) | Generated loops: circuits over an area's spawn rooms, listed and walkable once | B | M | C-021, C-023, C-025 | |
| [C-025](C-025-fight-here.md) | Fight here: the Fight and Recover modes in the current room, start/stop/pause, typed lines pause the bot | B | L | C-022, C-023 | review |
| [C-026](C-026-server-run.md) | Running wayfarer on a server: build, data directory, token, systemd unit, Tailscale | C | S | C-019 | |

## Index (wave 4, M3)

| Id | Title | Tier | Size | Depends | Review |
|---|---|---|---|---|---|
| [C-027](C-027-logoff-under-attack.md) | Log-off under attack, and the M2 brain follow-ups | B | M | C-025 | |
| [C-028](C-028-flee.md) | Flee: below the flee line, leave by a confirmed exit to a quiet room, keep running if followed, rest, return | B | L | C-025, C-027, C-029 | review |
| [C-029](C-029-store.md) | `wayfarer-store`: the bot's state survives a brain restart; measured statistics; a stats tab | B | M | C-025, C-018, C-027 | |
| [C-030](C-030-task-mode.md) | Task mode: run a loop: walk to the area, go stop to stop, fight at each, rest between, respawn delays, narrate | B | L | C-024, C-028, C-029, C-031 | review |
| [C-031](C-031-typing-beside-the-bot.md) | Typing beside the bot: input arbitration; talk and look lines go between actions, acting lines pause | B | L | C-025, C-027, C-028 | review |
| [C-032](C-032-replay.md) | Replay: play a recorded session back into the UI and the brain on its recorded clock; the CI replay layer | B | M | C-029, C-030, C-031 | |
| [C-033](C-033-soak-scenario.md) | The timed run: `--scenario soak`: a character runs a loop for N minutes and the numbers come out | C | M | C-030, C-029 | |
| [C-034](C-034-goto.md) | Go to: find a room, a place or a monster by name, walk there, stop one room short of a boss lair (added 2026-10-20 at John's request) | B | M | C-030 | |
| [C-035](C-035-monster-lists.md) | Monster lists: never fight, flee from and avoid by name; a stalemate ends a fight; the gigantic black ooze is never fought (added 2026-10-20 after the M3 soak met it) | B | M | C-028, C-030, C-034 | review |
| [C-036](C-036-megamud-loops.md) | Run a MegaMud loop as a task: the verified path's hunt rooms are the stops, its step actions become knowledge for the router and walker, the soak and the map panel can pick one (added 2026-10-20 at John's request) | B | L | C-030, C-037 | review |
| [C-037](C-037-doors-hidden-exits-unreachable-stops.md) | Doors the character cannot open are not routed through; the walker searches for a hidden exit on its route; unreachable stops do not end a loop (added 2026-10-20 after the keep soak) | B | M | C-030, C-022 | |

## Index (wave 6, M7 party play)

| Id | Title | Tier | Size | Depends | Review |
|---|---|---|---|---|---|
| [C-038](C-038-party-lines.md) | Party lines: typed events for invite, follow, drop and `par`; each session's in-game party | B | M | — | |
| [C-039](C-039-parties.md) | Parties: definitions saved by the brain, the coordinator, forming up, `par` tracking, followers don't walk, Leave and Join | B | L | C-038 | review |
| [C-040](C-040-parties-panel.md) | Web UI: the Parties panel, party chips on the tiles, followers' walking controls greyed | B | M | C-039 | |
| [C-041](C-041-separation.md) | Separation: the leader waits, the follower walks back and re-joins; a time limit and an unreachable stop | B | M | C-039 | |
| [C-042](C-042-party-rest-flee-assist.md) | Party rests, party flees and assisting the leader's target | B | L | C-041 | review |
| [C-043](C-043-healing-members.md) | Healing party members: heal-a-member lines, party heals chosen by how many are hurt | B | M | C-042 | |

Wave 6: C-038; then C-039; then C-040 and C-041 together (UI and brain); then C-042; then
C-043. **Wave 6 is done** when Penny leads Lauren, Faye and Manuel round a loop for an hour on
John's rig: they form up by themselves, rest and flee together, a healer keeps the others up,
and a follower walked away by hand comes back and re-joins.

Tier: B Opus, C Sonnet. Size: S under half a day of agent time, M about a day, L two to three
days.

Parallelism, wave 1: C-002, C-003, C-010 and C-011 together after C-001; then C-004; then
C-005 and C-006 together; then C-007, C-008 and C-009 together. Wave 2: C-012, C-014 and
C-015 together; then C-013 and C-016; then C-017 and C-019; then C-018. Wave 3: C-020,
C-021 and C-026 together; then C-022; then C-023; then C-025; then C-024 (C-023, C-025 and
C-024 all edit the UI's keys table, docs and generated protocol files, so they run one at a time).
Wave 4: C-027; then C-029; then C-028; then C-031; then C-030; then C-032 and C-033 together
(every card to C-030 changes the brain's session or modes, so one at a time; C-032's clock
refactor goes last).

**M0 is done** when John opens the browser on his rig, sees one tile per configured
character, each logged on by its logon script, types into them, scrolls back, reloads the
page without anyone being logged off, and stops the gateway with every character logged off
cleanly.

**M1 is done** when the tiles on John's rig show live HP, mana, exp, level and room for a
character in the realm, the dev panel shows the event stream and the unclassified lines with
their rate, the help overlay and resizable tiles work, and John stops and starts the brain
(`scripts/dev/brain.sh`) while nobody is logged off and the headers fill again.

**M2 is done** (with the two pulls) when, on John's rig: the map panel shows the character's
surroundings with the believed room lit, he clicks a room a few steps away and the character
walks there confirming each step, with the route drawn and a stop that works; a generated
loop is listed for the area and walks once; in a room he picks, Start makes the character
fight what comes and rest when low, typing pauses it and `F9` resumes it, and it stops itself
below 30% HP; and `docs/server.md` has been tried end to end on a Linux machine.

**M3 is done** when Thorgrim runs a Wooden Passage loop for thirty minutes on John's rig
(`scripts/live-check.sh --scenario soak`, with and without the Chieftain's room), fights,
rests and flees on his own, survives, the stats tab shows exp per hour and kills, a brain
restart mid-run resumes the loop, John types `st` into the tile without pausing him, and the
run can be replayed afterwards in the browser.
