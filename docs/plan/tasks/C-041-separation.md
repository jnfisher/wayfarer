---
id: C-041
title: "Separation: the leader holds and waits, the separated follower walks to the leader and re-joins, with a time limit and an unreachable stop"
tier: B
size: M
wave: 6
depends: [C-039]
crate: wayfarer-brain, wayfarer-settings
touches:
  - crates/wayfarer-brain/src/{party.rs,modes.rs,task.rs,session.rs}
  - crates/wayfarer-settings/src/lib.rs (`Group::Party`, `party.wait_separated_minutes`)
  - docs/brain.md "Parties"
---

## Goal

When the game drops a follower from the party, the leader stops and waits, the follower walks
to wherever the leader is, they re-form, and the leader carries on. If it can't happen, the
party stops and John is told.

## Inputs

- `docs/plan/10-party-play.md` §6, and §3 "Who counts" and "A leader walked by hand".
- C-039 (`PartyRole`, `MemberState`, forming, `par` tracking), C-022 (the walker, routes,
  refused exits), C-030 (Task's holds and narration), C-037 (unreachable stops).

## Behaviour

- A joined follower that the leader's `par` no longer lists, or whose own `PartyLeft` came,
  is **separated** (`MemberState::separated`).
- **Leader**: while any counted member is separated, the bot's own moves are held (Task's
  next leg and Go to wait; Fight and Flee still run), narrated "Waiting for Lauren to catch
  up". Moves John types are not held.
- **Follower**: walks to the leader's believed room by the walker (state `walking`, detail
  "4 rooms"), fighting what attacks it on the way (Fight and Flee outrank the walk, as for
  Task); re-targets when the leader's room changes. Arrived, C-039's forming re-joins it.
- **Unreachable**: no route, or the walker gives up on every route (a key it lacks, a level
  or class exit, a climb it fails): the follower stops, state `unreachable` with the reason
  ("no way past the locked gate west of 2:118"), and a `notice` goes out.
- **Time limit**: the new setting `party.wait_separated_minutes` (Party group; default 5,
  range 1 to 60; help: "How long the leader waits for a separated member before stopping
  the loop and telling you"). At the limit the leader's Task stops with a warning notice
  ("Lauren did not catch up in 5 minutes"); the follower keeps trying. The leader never
  walks on and leaves a member behind.
- Several separated members are waited for together.
- **Joining from elsewhere**: a character added with `party_join` (C-039) that is not in the
  leader's room walks to it the same way, then forms.
- **The leader away**: when the leader is logged off, dead or paused, its followers don't walk
  anywhere; they stand and fight where they are (state `away` on the leader) until it is back.

## Done when

- In-process: a scripted follower drop (an exit refusal with no message, then `par` without
  it) makes the leader hold, the follower walk two rooms and re-join, the leader resume; an
  unreachable leader room gives `unreachable` and a notice; the time limit stops the leader.
- Live: on John's rig, Lauren is walked two rooms away by hand mid-loop and the party re-forms.

## Tests

- `party.rs`: separation from `par` and from `PartyLeft`; hold and release; the time limit
  with the test clock; two separated at once; an away member isn't waited for.
- `task.rs`: a held leg resumes where it was.
