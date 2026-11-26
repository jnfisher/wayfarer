---
id: C-121
title: "Followers type the leader's room command with it"
tier: A
size: S
wave: 17
depends: [C-116]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/session.rs (note_party_command, party_command, party_notice)
  - crates/wayfarer-brain/src/party.rs (MemberInfo::command, the dispatch, pending lines)
  - crates/wayfarer-brain/src/brain.rs (plan.notices)
  - crates/wayfarer-brain/tests/party.rs
  - docs/brain.md "Parties"
---

## Goal

The game's follow moves a follower through directions only. When the leader takes a Text exit
or a room command (`go vortex` in the Royal Palace kitchen, 3:668, the skiff at Newhaven's
docks), it goes alone and its followers stay behind: Riley's party was left wandering the
Black Wasteland on its own (2026-11-19). John: "whenever the leader is using a command like
`go vortex` and the followers also need to issue that command to follow, the brain makes all
members issue the command simultaneously."

## Inputs

- `docs/brain.md` "Parties": the coordinator, "Separation", C-116 left behind.
- `crates/wayfarer-brain/src/session.rs` `apply` (the walker's actions), `sent` (the player's
  lines), `party_info`; `party.rs` `plan`, `sent`.
- `wayfarer_world::ExitType::is_phrase`, `Edge::taken_by`.

## Behaviour

- A phrase the leader types that takes a Text exit, a room command or a crossing out of its
  believed room, whether its walk typed it or the player did, is noted with the room and a
  running number.
- The coordinator, the same tick, has every follower that is in the leader's party in the
  game, in the realm with its bot running and believed in that room type the same phrase as
  its own line between its bot's actions, then `look`. A follower's belief may lag the drag by
  a tick, so followers are checked for ten seconds after the leader typed; one still elsewhere
  after that, or one the exit refuses, is separated as before and walks back.
- The leader's tile says who went: "`go vortex` with the party: Asa, Rocco".
- Queued lines a follower cannot send within 20 s (busy) are dropped.

## Design notes

- `Session::note_party_command` from `sent` only (every line that goes out, the walker's or
  the player's), not from `apply` as well: a line the executor held back (a fight) was noted
  twice, as two commands, and the followers went through twice (2026-11-26).
  `MemberInfo::command: Option<LeaderCommand>` (room, phrase, number, the script's wait).
- `Parties::commanded` per leader (number, when, followers sent and when each was);
  `Parties::pending` per follower, a queue the plan sends one line at a time and `sent` pops.
  A follower is crossing from its send until it has seen a room after the wait, or for the
  window after the wait; the leader is held for it, and `Session::stop_walk` keeps the
  walker's queued look after a phrase (`Walker::arriving`).
- `Plan::notices` beside `notes`: a notice, not a warning.

## Done when

- The party test: the leader's Go to from the Arena to the Small Pier ends with the docks'
  skiff; the follower types `borrow skiff` right after the leader, looks, and both are on the
  pier; the leader's notice names the follower.
- `docs/brain.md` says so; CI green.

## Tests

- `tests/party.rs`: `a_follower_takes_a_room_command_exit_with_its_leader`.

## Notes for the agent

- The first cut checked the follower's room only at the tick the leader typed, and the
  follower's belief was still one room back: nothing went, and the follower reached the pier
  only by the separation walk half a minute later. Hence the ten-second window.
- The second cut read `adddelay 5` as the teleport firing five seconds after the phrase.
  quicksilver teleports at once and holds the character's lines for the five seconds
  (`kb/navigation.md` Clauses); the look after the wait is right either way, but a follower's
  crossing ends with its look, not with a clock, and the leader waits for it.
