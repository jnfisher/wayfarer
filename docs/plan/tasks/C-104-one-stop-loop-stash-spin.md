---
id: C-104
title: "A one-stop loop held in its stop does not arrive again: no stash round, no clear, many times a second"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/task.rs (`stepping_out`, `observe`, `step_out`)
  - crates/wayfarer-brain/tests/party.rs
  - docs/brain.md ("Running a loop")
---

## Goal

A party leader running a one-stop loop no longer spins between clearing its stop, starting a
stash round and waiting for its members to hide their coins. Each arrival at the stop starts
one round.

John (2026-11-16): "it looks like the bot is freaking out trying to hide coins (see riley and
asa's group).. what's going on?" He asked for the fix.

## Inputs

What Riley's (leader) and Asa's logs show (2026-11-16, 12:26:09 to 12:26:30), on the
one-stop loop CaveWorm in the Dark Cave:

- About 500 notices in 20 s and no game commands: "Stash: nothing to hide", "Stop 1/1
  cleared: nothing there" and "Waiting for Asa to hide their coins" from Riley, and "Stash:
  nothing to hide" from Asa, round after round, several times a second.
- A one-stop loop's cleared stop is stepped out of and back into. `step_out` set the phase to
  `Going` (to the stop) while the character still stood in it. If the step out did not go at
  once (a fight came first, then the party's hold for Asa's stash), the next tick's `observe`
  saw "going to the stop, and in it" and took that for an arrival: a new stash round (C-092),
  so a new hold; Task cleared the stop again, stepped out again, and the hold held the walk
  back again. Only when the round's hold happened to clear before the next tick did the step
  out go.

## Behaviour

- `step_out` marks the task as stepping out. While it is, being in the stop is not an
  arrival: no stash round, no clear. Seen in any other room, the mark goes, and coming back in
  is an arrival as before.

## Done when

- `tests/party.rs` `a_step_out_that_does_not_go_starts_no_new_stash_round`: a party on a
  one-stop loop on the Narrow Road; the leader, once arrived, cannot move. Of the leader's next
  60 notices none is a new stash round (19 before the change).
- The fake party game takes a following member along when the leader moves from their room
  (`World::drag`), as the real one does.
- `docs/brain.md` "Running a loop" says so.
