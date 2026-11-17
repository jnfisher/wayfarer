---
id: C-111
title: "After a restart the party forms up again before the leader goes on"
tier: A
size: S
wave: 16
depends: [C-107]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/party.rs (stored followers count as joined at a start)
  - docs/brain.md "Separation"
---

## Goal

The v0.1.27 update (2026-11-17 17:05) restarted wayfarer and relogged all six characters. The
relog dropped Penny from Lexi's party in the game; her `follow lexi` went before Lexi's
invite and was refused ("You must be invited first!"), and Lexi's bot went on with its loop
at once. Lexi's `invite penny` 90 s later found Penny in another room. Lexi fought on alone
and hung up at 6% HP, chased by bugbears; Penny fought wyverns where she stood (John: "what
just happened with lexi? how did she get separated?").

The party coordinator already handles a follower dropped from its party (an exit, a flee):
the leader holds, the follower walks back, they form up. It does so only for a follower it
saw joined since the brain started, which a restart forgets.

## Behaviour

- At a start, every follower of a party in the store counts as one that was in its party in
  the game. One the game no longer has following is `separated`: the leader holds ("Waiting
  for Penny to catch up"), invites it while they share a room, and a follower elsewhere walks
  to the leader; after `party.wait_separated_minutes` the leader's loop stops, as before.
- A quicksilver restart without a wayfarer restart already worked this way.

## Done when

- A party loaded from the store, its follower in the leader's room and out of the party:
  the leader holds and invites it; the leader moved on: the follower walks to it and the
  leader still holds.
