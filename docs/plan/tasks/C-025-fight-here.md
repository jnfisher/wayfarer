---
id: C-025
title: Fight here: the Fight and Recover modes in the current room, start/stop/pause per character, typed lines pause the bot
tier: B
size: L
wave: 3
depends: [C-022, C-023]
crate: wayfarer-brain, wayfarer-proto, ui/
touches:
  - crates/wayfarer-brain/src/{modes.rs,fight.rs,recover.rs,policy.rs} (new), session.rs
  - crates/wayfarer-proto/src/ (`bot_control`; `CharacterState.bot`)
  - ui/src/Tile.svelte (start, pause, stop and the narration line), ui/src/lib/{keys.ts,gateway.ts,state.ts},
    ui/src/proto/ (regenerated), ui/tests/
  - tests/live/ (scenario "fight"), docs/brain.md, docs/web-ui.md
---

## Goal

The first slice of the bot, pulled forward from M3 (John, 2026-10-19): with a character
standing in a room John chose, pressing Start makes it fight what comes, keep auto-combat
on, rest when low (in the nearest quiet room if this one spawns), and stop itself when HP is
dangerously low in place of the Flee mode that is not built yet. Any line John types pauses
it; one key resumes. The mode stack, policies and narration are the ones `03-engine.md`
describes, with only the modes this slice needs.

## Inputs

- `docs/plan/03-engine.md` sections 3 (executor, guards), 4 (modes: `wants`, `step`,
  `on_event`; narration), 5 (running, paused, stopped; "guard while paused"), 6 (typed lines:
  here, any typed line pauses), `01-goals.md` "Rules" (never train stats; no attack while
  engaged), `04-settings.md` section 3 (the defaults: rest below 80%, rest until 95%, flee
  below 30%; they are constants in this card and become settings in M4), `05-ui.md` "Views"
  (the bot controls on every tile, the narration line) and "Pause and control" (`F9`
  pause/resume, `F10` start/stop).
- `kb/combat.md`, `kb/recovery.md`, `kb/pitfalls.md` 11, 20, 24, 27, 31, 33, 41, 42, and
  `kb/screen-patterns.md` for the texts: `*Combat Engaged*`, `*Combat Off*`, `You gain N
  experience.`, `<Name> drops to the ground!`, `You have been killed!`, `You do not see <x>
  here!`, `Your weapon has no effect against this monster!`, `You are now resting.`, `You
  are too sick to rest!`, the `(Resting)` prompt mark, monster arrivals (per-monster WCCMSG
  texts and the generic `X moves into the room from …`), `Also here:` (players capitalised,
  monsters lower case with an optional adjective), a bash needs a weapon.
- The prototype for mechanics only: `progress.py` `monsters` (l.331: parse `Also here`, keep
  evil non-NPC monsters with exp), `fight_all` (l.535: no attack while engaged; the fight ends
  on `*Combat Off*` or four quiet reads), `rest` (l.961: retreat to a non-spawn room first;
  re-send `rest` if the prompt lacks `(Resting)`; wake on arrivals), `_seen` (l.132: engaged
  from the last engaged/off event).
- C-022 (executor, walker, `RoomSeen`; `World` keeps the pack and the graph), C-020 (events
  applied to the model), C-021 (`spawn_pool`, `huntable`, `monsters_by_name` with adjective
  stripping), `crates/wayfarer-parse/src/typed.rs`
  (add `MonsterArrives { name }`, `AlsoHere { players, monsters }`, `YouDoNotSee`,
  `NoEffect`, `RestStart`, `TooSick`).

## Behaviour

**Protocol:** `bot_control {session, action}` with `start`, `stop`, `pause`, `resume` (UI →
brain). `CharacterState.bot { state: stopped|running|paused, mode: idle|fight|recover,
narration, reason? }`. The default for every character is `stopped`.

**Mode stack** (highest first), each a small struct with `wants(&State) -> Option<Reason>`,
`step(&State) -> Option<Action>`, `on_event`:

| Mode | Wants control when | Does |
|---|---|---|
| Recover | HP below 80% (constant) and not engaged; or poisoned | If this room spawns monsters (pack data) and a room with no spawn pool is within 5 steps, walk there (C-022's walker) and rest; else rest here. `rest`; re-send if the next prompt lacks `(Resting)`; wake and yield to Fight on a monster arrival that attacks; done at 95% |
| Fight | Engaged; or a hostile monster is in `Also here` | Pick a target (C-021's `huntable` on the record `monsters_by_name` finds, adjectives stripped; one that is attacking us first); send `a <last word>` once; then wait: no attack command while engaged (the executor's guard). Fight ends on `*Combat Off*`; after a kill (`You gain N experience.`) look again (Enter or `look`) and pick the next. A refused target (`You do not see`, `no effect`) goes on a skip list for the session |
| Idle | Otherwise | Sends nothing; narrates "Waiting here" |

**Stop in place of Flee.** HP below 30% (constant) while running: the bot goes to `stopped`
with a `warning` ("Stopped at 24% HP: flee is not built yet; fight or run by hand"). Nothing
else leaves the character in the game's hands.

**Pause and take-over.** A typed line from the player (`sent`, not the brain's own) while
`running` pauses after the action in flight ("You took over Waydeva"). Resume (`F9` or the
button) re-syncs first (`st`, `look`) and narrates what it will do next. `pause` also comes
from the UI. While paused, no mode sends anything (the "guard while paused" setting is
deferred to M4: paused means nothing).

**Guards** in the executor (C-022): never `train stats`; no attack command while engaged.
Fight never leaves the room; Recover may leave for a quiet room and must come back before
Fight resumes (it walks back when rested).

**UI.** In each tile header: `▶` (start), `⏸`/`▶` (pause/resume), `■` (stop), from `05-ui.md`;
`F10` start/stop and `F9` pause/resume for the focused tile; a second 16 px line under the
header while the bot is running or paused: the mode and the narration ("Fight: a rat",
"Recover: resting to 95% in Narrow Path", "Paused: you took over"). Keys and docs updated.

**Narration.** Every mode change and action emits a `notice` and updates `bot.narration`.

## Design notes

- `modes.rs`: the stack and the tick (on every event and on the clock); `fight.rs`,
  `recover.rs`: the two modes; `policy.rs`: pure functions (target choice, rest decision,
  quiet-room choice) over the state, the pack and the constants, unit-tested without a
  clock. The constants live in one place with a comment that `04-settings.md` replaces them.
- The executor's "one action in flight" rule holds across modes: a mode asks for an action,
  gets the outcome, decides again.
- Whether this room spawns is C-021's `spawn_pool` of the believed room (non-empty means it
  spawns, NPC or not: resting beside a shopkeeper is fine, so use the hunt-room flag for the
  "quiet room" choice); with an uncertain belief, treat the room as a hunt room.

## Done when

- In-process tests on the `fight-arena` capture (and a scripted fight with two monsters and
  an arrival): Start in a room with a rat sends exactly one `a rat`; nothing is sent while
  engaged; after `*Combat Off*` and a kill the bot looks and attacks the next; an arrival
  while resting wakes Fight; HP under 80% with no monster rests, in a quiet room when the
  current one spawns (the walk is through the walker, confirmed); 95% ends the rest and the
  bot walks back; HP under 30% stops the bot with the warning; a typed line pauses after the
  action in flight; resume re-syncs; `stop` stops; the executor refuses an attack while
  engaged (the guard test from C-022 extended to the mode).
- Determinism: the same events and clock give the same commands (replay test: two runs equal).
- **Live** (`scripts/live-check.sh --scenario fight`): walk the test character to the
  Newhaven Arena (level 3 or below; the pool characters are level 1), Start, expect
  `*Combat Engaged*` within a minute and at least one kill or a stop, HP tracked, then Stop,
  walk out, and log off clean. Summary in the PR.
- `docs/brain.md` "Fight here", `docs/web-ui.md` (buttons, keys, narration line).

## Tests

- As above; policies as plain unit tests; the mode tick on a paused clock.

## Notes for the agent

- No coins, no loot, no healing spells, no bash, no flee: each is a later card or a setting.
- The character is John's; the live scenario uses the pool character, in the arena only, and
  stops before leaving it with a dying character (if HP falls under 30% the bot stops itself;
  the scenario then rests by hand through the gateway before logging off).
