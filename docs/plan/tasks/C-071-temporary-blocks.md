---
id: C-071
title: "Loops are not barred by this session's blocked exits: three failed bashes don't lock a loop out"
tier: B
size: S
wave: 14
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/session.rs (mm_not_runnable, run_loop, the loop list)
  - crates/wayfarer-brain/src/position.rs (clearing the blocks)
  - crates/wayfarer-brain/tests/megamud.rs, crates/wayfarer-brain/tests/task.rs
---

## Goal

An exit the walker gave up on this session (a door that wouldn't bash open, a refused move, a
hidden exit not found) no longer stops a loop from being run. The loop check ignores those
blocks altogether, and starting a loop starts with none, so a few unlucky bashes cost one round
at most, not the rest of the brain's life.

## Inputs

- **What happened** (Penny on John's server, 2026-11-03): she ran `HKEPLOOP.mp` (Bandit Keep).
  At Inner Keep Entrance the east door was locked (`The door is closed!`, lower case). Penny and
  Sasha each bashed three times and failed. The walker marked the exit blocked
  (`walker.rs`, `Doing::Bash` after `MAX_BASHES`), the running loop skipped the five stops behind
  it, and from then on the loop list said: `no way from Stone-Strewn Courtyard to Hallway, Keep
  Entrance: the door east of Inner Keep Entrance would not open`. Run was refused, every time,
  until the server was restarted. Penny had been through that door many times before.
- **Why:** `Traveller::blocked` ("Exits the game refused this session", `wayfarer-world/src/
  route.rs`) is filled by `Position::block` and `Position::refused` and never cleared.
  `Session::mm_not_runnable` routes each leg of a MegaMud loop with a clone of the session's
  traveller, blocks included, and `run_loop` refuses a loop it calls not runnable. `run_loop`'s
  pick of the nearest ticked stop routes with the same traveller.
- **The game:** a bash succeeds when genrdn(0,100) < Str + exitpar2. A door that failed three
  bashes is unlucky, not shut for good (`docs/kb/navigation.md`, the Door/Gate row). A door the
  character can never open is `Traveller::door_odds(e) == Some(0)`, which is separate from
  `blocked` and stays as it is.
- C-037 (doors, hidden exits, unreachable stops) and C-036 (MegaMud loops as tasks) made the
  pieces this card changes.

## Behaviour

- **The loop check ignores `blocked`.** Everything that decides whether a loop can be run, or
  where it starts, plans with the session's traveller minus its `blocked` set:
  - `mm_not_runnable`, so the loop list's reason and `run_loop`'s refusal;
  - `run_loop`'s nearest ticked stop, and its "no way to any of its ticked stops" warning.

  Everything else still counts: gates, levels, class and race, a door whose odds are 0, items
  and keys, hidden exits.
- **Starting a loop clears the blocks.** A `run_loop` that starts a run (not one that only
  changes the skip list of the run in progress) empties `traveller.blocked` first, so the first
  round tries the door again. Say it once when it drops any:
  `Trying again the exits given up on earlier: 1`.
- **Within a run, nothing changes.** A door that fails `MAX_BASHES` mid-run is still skipped for
  the rest of that run's rounds, as C-037 made it (`stops_it_cannot_reach_are_passed_by_each_
  round_without_giving_up` stays as it is). Stopping and starting the loop is how the player
  says "try it again".
- Walks, Go to and Flee keep using `blocked` as now: within one journey, an exit given up on
  should not be tried again at once.

## Design notes

- One helper does it: `Traveller::without_blocks(&self) -> Traveller` in `wayfarer-world`, or
  clearing the set on a clone in `mm_not_runnable` and `run_loop`. Prefer the helper, so a
  third caller can't forget.
- `Position::clear_blocks(&mut self) -> usize` for `run_loop`, returning how many it dropped
  (for the note).
- `why_unreachable` needs no change: with no blocks in the traveller, it no longer says "would
  not open" or "was refused" from the loop check.

## Done when

- After three failed bashes on a door a loop crosses, the loop list shows that loop runnable,
  and Run starts it. The first round walks to the door and tries it again.
- A door with `door_odds == Some(0)` still makes the loop not runnable, with its "cannot be
  opened" reason.
- The mid-run skipping of C-037 is unchanged.
- `cargo fmt`, clippy and nextest are green.

## Tests

- `megamud.rs`: a loop whose leg crosses a door. Bash it `MAX_BASHES` times and fail. The
  loop's `not_runnable` reason is `None`, and `run_loop` starts it.
- `task.rs`: run a loop, fail the bashes (the stops behind the door are skipped), stop it, run
  it again. The first leg sends `bash` at the door again, and the note says one exit is tried
  again.
- `megamud.rs`: a door the character's strength can never open still gives the loop a "cannot
  be opened" reason.

## Notes for the agent

- John's rig runs a brain that watches `~/repos/wayfarer`: work in the card's worktree only.
- Not in this card: blocks that wear off by time mid-run. If a long run losing a door for the
  rest of the run turns out to matter, that's a follow-up for John to ask for.
