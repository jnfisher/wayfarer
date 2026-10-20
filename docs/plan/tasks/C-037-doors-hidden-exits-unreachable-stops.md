---
id: C-037
title: Doors the character cannot open are not routed through; the walker searches for a hidden exit on its route; stops it cannot reach do not end the loop
tier: B
size: M
wave: 4
depends: [C-030, C-022]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/{route.rs,nav.rs} (`Traveller` strength and picklocks; door odds; `Costs`)
  - crates/wayfarer-brain/src/{walker.rs,position.rs,task.rs,session.rs} (search before a hidden exit; unreachable stops)
  - crates/wayfarer-brain/src/executor.rs (`search` outcome), crates/wayfarer-parse (the search texts)
  - docs/brain.md "Walking" and "Running a loop", docs/kb/navigation.md (KB notes)
---

## Goal

The 2026-10-20 keep soak (Thorgrim, anchor 9:724, the generated "doors" loop) ended at 2:43:
the loop's leg into the courtyard went through the Hidden Keep's east door, a Door exit with
`exitpar2` -250. The KB's bash rule is `genrdn(0,100) < Str + exitpar2`, so 97 - 250 never
opens it; the walker bashed three times ("Your attempts to bash through fail!", 2 damage),
blocked the exit, and every courtyard stop became "no route known". Three stops passed by in
a row tripped C-030's give-up rule and the bot stopped itself with 6 kills. The real way in is
a hidden exit north of 9:724 (`search n`, then along the Keep Wall and `climb tree`), which
MegaMud's `Ftrehkep.mp` takes. Three fixes:

1. The router knows which doors this character can open.
2. The walker can search for a hidden exit its route uses.
3. A loop keeps going while any stop is reachable.

## Inputs

- `docs/kb/navigation.md` "Door / Gate" (lock states, bash odds `Str + exitpar2`, pick odds
  `exitpar2 + picklocks`, relocking; a locked door with `exitpar2 > 0` never relocks) and
  "Hidden" (search; found exits hide again after about five minutes), `docs/kb/combat.md`
  (bash self-damage).
- C-021 (`Traveller`, `Gate`, `Category::Hidden`, `found`, `Costs`), C-022 (the walker's
  `Doing::{Move, Open, Bash, Look}`, `MAX_BASHES`, blocked exits), C-030 (`LEG_TRIES`,
  `GIVE_UPS`, `passed`), C-016/C-029 (the character's stats on the snapshot: strength; the
  class's picklocks if the pack has it, else none).

## Behaviour

- **Door odds** (`wayfarer-world`): `Traveller` gains `strength: Option<i64>` and
  `picklocks: Option<i64>` (the character's; `None` unknown, not checked). A Door or Gate
  edge whose lock state is locked is passable when the character can open it: bash chance
  `strength + exitpar2 > 0`, or pick chance `picklocks + exitpar2 > 0`; a shut-unlocked door
  is always passable (`open`). An impassable door is left out of routes, the way a refused
  exit is; the loop generator (C-024) uses the same traveller, so a "doors" loop never plans
  through one. The cost of a bashable door scales with its odds (a door at 20 % costs more
  hops than one at 90 %), so a sure way round is preferred; say the formula in the PR.
- **The walker and locked doors**: before the first `bash`, the walker checks the odds and
  gives the door up at once when they are nil, narrating "The door east is beyond my
  strength (needs more than 250)"; `MAX_BASHES` stays for doors that can open. A bash that
  fails three times marks the exit blocked for this session, as today.
- **Searching** (`wayfarer-brain`): a route may use a Hidden exit the traveller allows (one
  in `found`, or one the loop's path knowledge names, C-036) and the walker, arriving at the
  room, sends `search <dir>` (or plain `search` if the game needs it; the KB says which) when
  the exit is not in the exits line, waits for the result ("You notice a hidden exit to the
  north" or the game's text; the parser gets the typed event), then moves. Found exits are
  remembered in `found` with the time and forgotten after `HIDDEN_FOR` (5 min); a search that
  finds nothing twice blocks the exit for the session. Searching costs time, so a Hidden exit
  costs more than a plain one in `Costs`.
- **Unreachable stops** (`task.rs`): a leg that cannot be routed ("no route known") passes the
  stop by for this round with its own narration and **does not count** towards `GIVE_UPS`;
  only legs that were walked and ended elsewhere do. The task gives up when no stop at all is
  reachable ("loop X: none of its stops can be reached from here") or when `GIVE_UPS` walked
  legs fail in a row, as before. A stop passed by as unreachable is tried again next round
  (the belief may have changed: a door opened, an exit found).
- **Narration**: the first time a round skips unreachable stops it says how many and why,
  once ("Skipping 31 stops behind the courtyard door this round: the door east of the
  Hidden Keep cannot be opened").
- Docs: `docs/brain.md` "Walking" (doors and searching) and "Running a loop" (what ends a
  loop); KB notes in `docs/kb/navigation.md` for the search texts and the hidden-exit timing
  as confirmed live, with provenance.

## Design notes

- Strength is on the status screen (`st`, parsed since C-016); picklocks is a class skill
  the pack's `classes` table may carry; if it does not, `None` and only bashing counts.
- Keep "impassable" a property of the traveller, not the graph: another character with Str
  150 may bash the same door.
- The three-in-a-row rule exists for walks that keep failing (a monster blocking, a wrong
  belief); unreachable stops are a different signal and were never meant to trip it.

## Done when

- World tests on the stock pack: with strength 97 the route from 9:724 to 9:798 is none (the
  east door is impassable) and the generated loop around 9:724 with doors has no courtyard
  stops; with strength 300 the route is one step; a traveller with the hidden exit north of
  9:724 in `found` routes into the keep that way; a shut-unlocked door routes regardless.
- Brain tests (fake game): the walker does not bash a hopeless door and narrates; it searches
  before a hidden exit, moves on "You notice...", and blocks the exit after two empty
  searches; a loop with half its stops unreachable runs the reachable half round twice
  without giving up, and gives up at once when none is reachable; a loop whose walked legs
  fail three times still stops as before.
- Live (pool character, Newhaven): a walk that uses a hidden exit if Newhaven has one the
  pool character can search (name it; if none, say so and test the search text against the
  nearest one reachable), else in-process only; summary in the PR.
- Checks green; docs updated.

## Tests

- As above; the door-odds cost as unit tests on `Costs`.

## Notes for the agent

- John's own characters (Waydeva, Waydevb, Thorgrim) are not yours to log in; the live check
  uses the pool character only. Never start, stop or reset the silvermere server.
- The keep itself is not reachable for the pool character; the door and hidden-exit cases are
  tested on the stock pack in-process.
