---
id: C-116
title: "A follower the game left behind, still on the leader's list, walks back"
tier: A
size: S
wave: 16
depends: [C-111]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/party.rs (LEFT_BEHIND; a look to tell a lagging belief)
  - crates/wayfarer-brain/src/session.rs (when a room was last seen)
  - crates/wayfarer-brain/tests/party.rs (the lagging-belief test's world agrees)
  - docs/brain.md
---

## Goal

A relog put the party in the Steel Chamber, Stairwell (the Mirrored Hall's hang-up room).
Lexi and Sasha followed Riley there; Riley's group went on by the hidden south exit and the
game did not move them. A follower the game does not move stays on the leader's list with no
line (`kb/grouping.md` §2a), so `par` kept listing them, wayfarer counted them as joined in
another room, nobody walked back and Riley did not wait. Riley fell to 18% with both healers
rooms away.

## Behaviour

- A follower the leader's `par` lists, out of the leader's room for 10 s, that has seen a room
  since they parted, is separated: it walks back and the leader holds, as for a dropped one.
- With no room seen since, its belief may only lag: it sends `look` (every 10 s at most).
- Fighting or resting, it is left where it is until done.
- In the leader's room again it sends `follow` and is joined after it.

## Done when

- The coordinator test: listed, apart, no room seen, a `look`; a room seen, separated, a walk
  back and the leader waiting; back, `follow`, joined.
- A joined follower whose belief only lags (the game moved it) still never flees on its own.
