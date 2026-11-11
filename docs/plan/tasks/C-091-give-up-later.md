---
id: C-091
title: "Give up later: bashes by the door's odds, write-offs that expire, stalemates over ten rounds, legs that get closer are not failures"
tier: B
size: M
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/position.rs (`BLOCKED_FOR`, `clear_blocks`, `expire_blocks`)
  - crates/wayfarer-brain/src/session.rs (`walk_to`, `walk_loop`, `run_loop` clear the blocks)
  - crates/wayfarer-brain/src/walker.rs (`bash_limit`)
  - crates/wayfarer-brain/src/fight.rs (`STALEMATE_ROUNDS`, a party member's damage)
  - crates/wayfarer-brain/src/rules.rs (a stalemate's rule is not stored)
  - crates/wayfarer-brain/src/journey.rs, crates/wayfarer-brain/src/task.rs (progress)
  - docs/brain.md
---

## Goal

John (2026-11-10): "this '3 failure' rule keeps causing LOTS of false negatives." The same
evening:

- A Go to to the ice sorceress found no route. Earlier that session, two of the tower's three
  locked doors had each failed three bashes. The walker had written them off for the whole
  session, and they are the only way in. After a brain restart the route was there again.
- The party stopped walking because a Go to gave up ("3 walks ended elsewhere") on legs that
  each got closer.

He reviewed the full list of early give-ups and asked for these, in one card. It takes in
C-071 (loops not barred by this session's blocks), which was planned but never built.

## Behaviour

- **Bashes by the odds.** A locked door gets `bash` until failing that often in a row would be
  under 1% at the door's odds (Str + `exitpar2` per cent, `Traveller::lock_odds`): 7 bashes at
  50%, 13 at 30%, 44 at 10%. The count is between 3 and 50, and 15 while the strength is
  unknown (`bash_limit`). Doors at 0% odds are still not bashed.
- **Write-offs expire.** An exit the walker gave up on (a door that would not bash open, a key
  that would not turn, a hidden exit not found or not opened, a refused move, a phrase heard as
  speech) is kept out of routes for `BLOCKED_FOR`, 10 minutes, not the whole session. A Go to,
  `walk_loop` or `run_loop` the player starts clears them all first (C-071).
- **Stalemates over ten rounds.** `STALEMATE_ROUNDS` goes from 4 to 10 (50 s). A blow or
  damage spell on the target by anyone else (`other_hits`, a damaging `spell_cast_room`)
  starts the count again. The *ignore* rule a stalemate teaches lasts the session only: it is
  not stored, and one an older brain stored is dropped on load (Penny's "stone golem" since
  2026-11-03).
- **Progress is not a try.** A Go to's or a loop's leg that ended closer to its stop than it
  began resets the count of legs, whatever stopped it. Only legs that got no closer, with no
  fight between, count toward the three.

## Done when

- `walk.rs` `a_locked_door_is_bashed_as_often_as_its_odds_call_for`, the task and party bash
  tests: as many bashes as `bash_limit` gives.
- `walk.rs` `a_refusal_marks_the_exit_and_the_replan_avoids_it`: a second Go to tries the
  refused exit again.
- `position.rs` `a_blocked_exit_is_taken_again_after_ten_minutes_or_at_once_when_cleared`.
- `rules.rs` `the_recorded_ooze_fight_ends_by_its_tenth_round` and
  `a_party_members_blow_on_the_target_puts_the_stalemate_off`. The rules unit tests: a
  stalemate's rule is not stored, and a stored one is dropped.
- `goto.rs` `a_go_to_whose_legs_each_get_closer_is_not_given_up`: three legs stopped for a
  monster gone before the fight, each closer. It arrives; before the change it gave up.
- `cargo fmt`, `cargo clippy -D warnings` and `cargo nextest run --workspace` pass.
