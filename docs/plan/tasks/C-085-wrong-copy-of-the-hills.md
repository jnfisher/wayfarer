---
id: C-085
title: "Sure of the wrong copy: a position log, the re-sync's look starts afresh, a contradicted belief looks everywhere, a guessed plan's surprise is not being lost"
tier: B
size: M
wave: 16
depends: [C-084]
crate: wayfarer-brain
touches:
  - crates/wayfarer-world/src/belief.rs (`observe` returns how it moved; the widening order)
  - crates/wayfarer-brain/src/position.rs (the trace of each room seen)
  - crates/wayfarer-brain/src/walker.rs (`guessed`; the plan and surprise traces)
  - crates/wayfarer-brain/src/session.rs (the re-sync's `look`; traces out)
  - crates/wayfarer-proto/src/lib.rs (`position_trace`)
  - crates/wayfarer-gateway/src/gateway.rs, server.rs, session.rs (`position.log`)
  - docs/brain.md "Look-alike rooms", docs/protocol.md, docs/server.md (the logs)
---

## Goal

C-084 did not get Lexi through the Dragon's Teeth Hills. On 2026-11-10, from 16:31, her Go to
to Markus stopped again with "lost the way: expected Dragon's Teeth Hills, the room was Dragon's
Teeth Hills", the s/n and nw/se moves in between going back and forth. This card finds out why
from the logs, and fixes what that shows.

The game's output and the world pack agree on every room. Replayed with every move, her rooms
were 2/191, 190, 189, 188, 186, 187, 180, 181, 182. The rooms the brain believed (the `kill`
events' rooms in the store) were 2/482, 352, 322, 221. Each one had the same name and exits as
the right one, in a copy of the same stretch of maze elsewhere in the hills (2/191 → 192 → 193
has the same layout as 2/482 → 483 → 484). The brain was **sure of the wrong copy**:

1. whenever a room fitted none of the candidates, `Belief::observe` looked first among rooms
   within 12 steps of the old candidates. That is the wrong copy's neighbourhood, so it settled
   on another room there;
2. a brain that attaches rebuilds the position from the output since, and the re-sync's `look`
   only checks that the room fits. In the hills nearly every guess fits;
3. a walk planned from a guessed candidate (C-084's vote, or the lowest key) took that guess's
   first surprise as being lost, once the belief after the move was certain. Two of those
   stopped the walk.

Nothing recorded which rooms the brain believed when, so this took a replay by hand. A
`position.log` does that from now on.

## Inputs

- C-084, `docs/brain.md` "Look-alike rooms" and "Position and the map".
- `kb/navigation.md` "Position tracking".
- Lexi's logs on 2026-11-10 16:31–16:34 (not in the repository: they name the account).

## Behaviour

- **`position.log`** beside `notes.log` (`logs/<id>/`): one JSON line per entry, with the
  session's latest output `seq` and `ts`, like `notes.log`. The brain sends each entry as
  `position_trace {session, entry}` (brain → gateway, never relayed, refused from a UI).
  - Every room seen: `{"kind":"room","move":"n","title":…,"exits":"n,sw","how":…,"count":3,"cands":["2/190",…]}`.
    `move` is the move it was carried through, if any. `how` is how the belief changed:
    `dark` (nothing told), `followed` (the candidates after the move that fit),
    `exits` (no title: the exits narrowed it), `not_a_title`, `not_on_map` (a whole room the
    pack lacks), `anywhere` (from unknown: every room of that name and exits),
    `relocated` (nothing fitted: every room of that name and exits), `near` (no exits seen:
    near the old candidates), `near_by_name` and `anywhere_by_name` (exits fitted nowhere).
    `cands` lists the first 8.
  - The re-sync's fresh start: `{"kind":"afresh","why":"re-sync"}`.
  - A walk's plan and each replan: `{"kind":"plan","from":"2/190","to":"2/2518","steps":230,"guessed":true,"count":2,"why":…}`.
  - A surprise: `{"kind":"surprise","expected":"2/189","count":1,"cands":[…],"guessed":true,"unsure":false,"counted":"unsure"|"drift"}`.
- **The re-sync's `look` starts afresh.** The room it brings is placed among every room of its
  name and exits, not just the ones history led to. The belief stands until that room comes:
  forgetting it when `look` goes leaves the bot a moment with no position, and a bot started
  then re-syncs a second time ("Starting in an unknown room"). Where the name and exits are unique, the belief is as sure as before.
  Among look-alikes it is honestly unsure, and moves narrow it (C-084).
- **A contradicted belief looks everywhere.** When the room seen fits none of the candidates
  and its exits were seen, the belief becomes every room in the world of that name and exits.
  The 12-step widening is left for a room seen without exits (a title behind a prompt) and for
  exits that fit nowhere: near by name, then anywhere by name, as before.
- **A guessed plan's surprise is not being lost.** A plan is *guessed* when the belief held
  several candidates when it was made. A surprise on a guessed plan counts as an unsure move
  (up to `MAX_UNSURE_MOVES`), however sure the belief is after the move. Only a surprise on a
  plan made from a certain belief counts toward "lost the way".

## Design notes

- `Belief::observe` returns `How` (an enum, `as_str` for the log). Callers that ignore it are
  unchanged.
- `Position` keeps a small queue of trace entries (`serde_json::Value`); the session drains it
  after each event, and the walker's `Next` gets `traces` too. The session sends them as
  `position_trace`.
- The gateway writes `position_trace` to a `LineLog` (`position.log`) as `note_from_brain`
  does `notes.log`.
- `Walk` gets `guessed: bool`, set at the walk's first plan and at each replan from the belief
  then.

## Done when

- `walk.rs`: Lexi's walk, replayed on the fake game's map from 2/191 with the belief sure of
  2/482, gets to 2/182 rather than stopping with "lost the way".
- `belief.rs` tests: a contradicted belief with exits becomes every room of that name and
  exits; without exits it widens near as before.
- A re-sync's `look` in a look-alike room leaves the belief with every room of that layout.
- The gateway writes `position_trace` entries to `position.log` and relays none; a UI that
  sends one is refused.
- `cargo fmt`, `clippy -D warnings`, `nextest` pass.

## Tests

- `wayfarer-world` `belief`: `observe` widening order and `How`.
- `wayfarer-brain` `walk.rs`: the replay above; a guessed plan's surprise is an unsure move.
- `wayfarer-brain`: the re-sync forgets; position traces are sent.
- `wayfarer-proto` `messages.rs`: `position_trace` round-trips, family `BrainToGateway`.
- `wayfarer-gateway`: `position.log` is written.

## Notes for the agent

- The size of `position.log`: one line per room seen and per plan, small next to `output.log`.
- Found on the way: forgetting the belief when the re-sync's `look` goes (before its room
  came) made six `fight.rs` tests hang on the paused clock (`a_go_to_keeps_the_bot_on_…`,
  `logging_off_stops_the_bot_…` and others). Not the bot: it re-synced a second time, the
  scripts got out of step, and the harness's `until` waited for ever for a line that never
  came (C-088 found this and makes such a wait fail).
