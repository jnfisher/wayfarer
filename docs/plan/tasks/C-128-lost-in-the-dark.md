---
id: C-128
title: "A move refused in the dark means lost: the walk stops and says so, once"
tier: A
size: S
wave: 19
depends: [C-127]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/walker.rs (`NoExit` on a dark belief; `relocations` across restarted walks)
  - crates/wayfarer-brain/src/modes.rs (the walk back to the leader is not begun again at once)
  - crates/wayfarer-brain/src/party.rs (a follower lost in the dark: the leader's loop stops with a warning)
  - crates/wayfarer-brain/tests/party.rs, crates/wayfarer-brain/tests/walk.rs
  - docs/brain.md "Walking", "Parties" (Separation)
---

## Goal

After C-127's mistake put Sasha in Redstone Tunnel 6/1412 while the brain believed 6/1408
(silver-mirror, 2026-11-21 15:18:58), the walk back to Rocco planned `n` from 6/1408. 6/1412
has no north exit: "There is no exit in that direction!". The walker's answer to a refused
move is a look to relocate (pitfall 34), but a look in the dark shows the darkness line and
nothing else; the "You are no longer sneaking." on the same refusal ended the walk ("Stopped
walking: the sneak failed"), the next tick began "Walking back to Rocco" afresh from the same
wrong room with the relocation count back at zero, and the sneak came between. She sent `n`
and `sneak` once a second, 367 refusals in 2 minutes 20 seconds, until John paused her.

When this card is done, a move refused where the character cannot see ends the walk with
"lost in the dark", nothing starts the same walk again until the position is known, and the
leader's loop stops with a warning that names the member.

## Inputs

- The host's `logs/sasha/output.log` and `position.log` seq 2981888 to 2983330 (the loop), and
  the brain journal 15:18:58 to 15:21:13 (the three notices a second).
- `crates/wayfarer-brain/src/walker.rs`: `(Doing::Move, Refused::Move(Refusal::NoExit))`,
  `MAX_RELOCATIONS` (2), `relocations: 0` on a new walk.
- `crates/wayfarer-brain/src/session.rs`: `stop_walk("the sneak failed", ..)` (C-048, C-122).
- `crates/wayfarer-brain/src/modes.rs`: "Out of its party: back to the leader" and
  `Step::walk(to, "Walking back to {leader}")`.
- `crates/wayfarer-brain/src/party.rs`: `rejoin`, `hold`, the give-up after
  `party.wait_separated_minutes` ("did not catch up in N minutes").
- `docs/brain.md` "Walking" (relocating), "Parties" (Separation).

## Behaviour

- A move the game refuses with "There is no exit in that direction!" while the belief rests on
  dark rooms only (no room with a title or exits since the character last entered the dark)
  ends the walk at once: "Lost in the dark: `n` from Redstone Tunnel (6/1408) was refused". No
  look, no second try. The belief is marked unknown, as after a room the pack lacks (C-053):
  the next lit room relocates it.
- With the belief unknown nothing begins a walk: not the walk back to the leader, not a loop or
  a Go to. The follower waits where it is ("Waiting: lost in the dark") and looks every 10 s
  (as the blind wait does, C-108), ready to be relocated by the first room it is shown. It
  still fights back, heals and rests.
- A refused sneak on the same line as a refused move does not end the walk first: the move's
  refusal is read before the sneak's, so the walk stops for the right reason and the count of
  relocations is not thrown away.
- Across walks begun again from the same belief (a stop for combat, a sneak, a held moment), the
  relocation count carries over: two "no exit" refusals from one believed room without a lit
  room between, lit or dark, is "lost", as `MAX_RELOCATIONS` already intends.
- The leader: a follower lost in the dark is separated and cannot walk back. The leader's loop
  stops at once with a warning naming it ("Sasha is lost in the dark: walk it to a lit room, or
  the party to it") rather than holding `party.wait_separated_minutes` for a walk that will not
  come. The leader's bot otherwise stays on (fights, rests) where it stands.
- The player can always type moves beside the bot (C-079); a lit room puts the belief back and
  the follower rejoins as before.

## Design notes

- `Position` knows whether the belief is "dark only": a flag set when a dark `RoomSeen`
  follows a lit one, cleared on any `RoomSeen` with a title or exits, and reported with
  `best()` for the walker and the modes. Expose it as `Position::in_the_dark()`.
- `walker.rs`: in the `NoExit` arm, `if pos.in_the_dark() { pos.lose(); return Err("lost in the
  dark: ...") }`; keep the look-to-relocate for lit rooms. Move `relocations` from the walk to
  a `(Key, u32)` kept on the `Walker` and reset when the believed room changes or a lit room is
  seen.
- `modes.rs`: the "walking back to the party" arm requires `v.room.is_some()`; with the belief
  unknown it falls to a new `Step::wait("Waiting: lost in the dark")` with a look every 10 s
  (reuse `fight.blind_look`'s cadence or lift it into a shared helper).
- `session.rs`: in the chunk handler, read the executor's outcome for the move before
  `sneak_broken` is consulted, or let `stop_walk` keep the walker's `relocations` state.
- `party.rs`: a follower whose position is unknown and in the dark is `separated` with detail
  "lost in the dark"; the leader's hold becomes a loop stop with the warning, once; the gave-up
  set prevents a repeat.

## Done when

- Replaying Sasha's refusal (dark belief 6/1408, `n` refused, "You are no longer sneaking." on
  the same line) stops the walk once with the lost notice, sends no further `n`, and the
  belief is unknown; the next lit room relocates it (`tests/walk.rs`).
- Over two minutes of ticks after that, the follower sends only `look` (at most every 10 s)
  and `sneak` per its own rules; never a move.
- Coordinator test: the leader's loop stops with the warning within a second of the follower
  being lost; the follower shown a lit room in the leader's room sends `follow` and is joined.
- `docs/brain.md` says so; CI green.

## Tests

- `tests/walk.rs`: `a_move_refused_in_the_dark_ends_the_walk_as_lost`,
  `relocations_carry_over_walks_from_one_believed_room`.
- `tests/party.rs`: `a_follower_lost_in_the_dark_waits_and_the_leader_stops_with_a_warning`,
  `a_lit_room_relocates_a_lost_follower_and_it_rejoins`.

## Notes for the agent

- Do this after C-127: with the follow moves tracked, Sasha would not have been "walking back"
  at all; this card is the safety net for the next wrong belief in the dark (a flee, a stun,
  a hidden exit the game took).
- "The room is very dark" is also what a search answers in the dark ("too dark to search",
  `docs/brain.md` "Walking"); that stop stays as it is.
- The 367 refusals were harmless to the character (no HP lost, the game only refused) but
  the party stood apart for two and a half minutes in a spawning area; the warning is what
  John wants to see on the tile.
