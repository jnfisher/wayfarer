---
id: C-122
title: "A refused sneak is tried again in two seconds, or in the next room"
tier: A
size: S
wave: 17
depends: [C-048]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/sneak.rs (NOT_NOW, Sneak::arrived)
  - crates/wayfarer-brain/tests/party.rs
  - docs/brain.md "Sneaking"
---

## Goal

A follower that killed a salamander in the Lava Tubes sent `sneak` at `*Combat Off*` and got
`You may not sneak right now!`, then was dragged five rooms unsneaked behind Rocco before it
tried again, into the next pair of salamanders (2026-11-19). The refusal held the next attempt
five seconds, a "round"; a party on the move is rooms away by then. John: "we may need more
aggressive re-stealth after a kill when the room is empty and the character is following, or
after a 'You may not sneak right now!' message."

## Inputs

- silvermere `commands/misc.rs` `can_sneak`: the game refuses while the player still has
  attackers counted, is being attacked from the room, or a monster in the room could attack;
  a kill clears these within a round or two.
- `docs/brain.md` "Sneaking"; `crates/wayfarer-brain/src/sneak.rs`.

## Behaviour

- `You may not sneak right now!` and `You must wait before you may do that!` hold the next
  attempt two seconds, not five.
- Arriving in a new room ends the hold at once: whatever the game counted against the sneak
  was left behind. The same room again does not.
- Everything else as before: no attempt with a monster in the room, five failures give up until
  the next fight, a fight starts the attempts afresh.

## Done when

- `NOT_NOW` is two seconds and `Sneak::arrived` clears the wait on a new room (unit test).
- Party test: a follower refused once sends its second `sneak` two to four seconds later and
  then stays sneaking.
- `docs/brain.md` says so; CI green.

## Tests

- `sneak.rs`: `a_refusal_waits_two_seconds_or_until_the_next_room`.
- `tests/party.rs`: `a_follower_refused_a_sneak_tries_again_within_two_seconds`.
