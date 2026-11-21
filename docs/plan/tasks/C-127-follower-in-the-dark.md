---
id: C-127
title: "A follower keeps its place in the dark: the game's follow moves advance its belief"
tier: A
size: S
wave: 19
depends: [C-108, C-116]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/position.rs (`PartyMoved` is a move in flight)
  - crates/wayfarer-brain/src/session.rs (`room_seen_at`: a dark room confirms nothing)
  - crates/wayfarer-brain/src/party.rs (left behind: not on a dark room alone)
  - crates/wayfarer-brain/tests/party.rs, crates/wayfarer-brain/tests/walk.rs
  - docs/brain.md "Position and the map", "Parties" (Separation)
---

## Goal

Rocco led Sasha from the Spiral Stairwell west into the Redstone Tunnels above Khazarad
(6/1407 to 6/1431, `light` -200: every room answers "The room is very dark - you can't see
anything"), on the loop Upper Redstones (silver-mirror, 2026-11-21 15:18). Rocco walked the
tunnels blind without trouble (C-108: his own moves advance his belief). Sasha followed him
eight rooms, the game saying ` -- Following your Party leader west --` and then the darkness
line each time, and her belief never left 6/1406: her position log shows `how: dark`, one
candidate, 6/1406, for all eight follow moves. Ten seconds after they parted the coordinator
took her for left behind (C-116): listed in Rocco's `par`, believed eight rooms away, and "a
room seen since they parted", the darkness line counting as one. She went to "Walking back to
Rocco", typed `w` herself, which broke the follow ("Rocco stops guarding you") and really did
separate her, and walked two rooms the wrong way to 6/1412 while the brain thought 6/1408.
C-128 is what happened next.

When this card is done, a follower that the game moves through dark rooms knows where it is,
and a dark room never tells the coordinator that a follower is apart from its leader.

## Inputs

- The logs of the run on the host: `logs/sasha/position.log` seq 2981870 to 2981894 and
  `logs/sasha/output.log` the same range (the eight follow moves, then `w`, `w`, `n`);
  `logs/rocco/position.log` seq 3121017 onwards (a leader's own moves in the dark, tracked).
- `crates/wayfarer-brain/src/position.rs`: `sent` pushes the character's own move onto
  `moving`; `event` pops it on the next room seen (`RoomDark` included) and moves the belief
  through it before `observe`. `GameEvent::PartyMoved { dir }` (the parser's `follow_leader`)
  reaches `party_game.rs` only, as a timestamp.
- `crates/wayfarer-brain/src/party.rs` around `LEFT_BEHIND` (C-116): listed, apart 10 s, and
  `room_seen_at > since` decides; else `look`.
- `crates/wayfarer-brain/src/session.rs`: `room_seen_at = Some(now)` for every `RoomSeen`,
  dark ones too.
- `docs/kb/grouping.md` §2a (a follower the game does not move stays on the list with no line).
- C-108 (a blind move that was not refused went; the darkness line in place of the room counts
  as that room) and C-116.

## Behaviour

- ` -- Following your Party leader <dir> --` is a move the character made: the belief moves
  through `<dir>` when the room that follows it is seen, lit or dark, exactly as for a move the
  character typed. In a lit room nothing changes (the title and exits relocate it anyway); in
  the dark the follower now steps along with its leader, one candidate per room when the map
  has one exit that way.
- A follow move and a typed move do not both count for one room shown: a `w` the player or the
  bot typed that the game answered with a follow line is one move, not two.
- A room too dark to see confirms nothing about being apart. For the left-behind test (C-116),
  "a room seen since they parted" means a room with a title or exits; dark rooms since the
  parting leave the belief "may only lag", and the follower looks (as now) instead of walking.
  A look in the dark answers the darkness line, so a follower in the dark whose belief lags
  stays put and keeps looking at most every 10 s; it never walks back on a dark belief alone.
- The leader's own dark walking is unchanged.

## Design notes

- `Position::event`: on `GameEvent::PartyMoved { dir }`, if `moving` is empty, push the
  direction (`mm::direction(dir)`'s short name, so `belief.moved` finds the edge); if the
  front of `moving` is already a move the same way (the player typed `w` and the game followed
  west), leave it. The room that follows pops it as today. Add a `"follow": true` field to the
  room trace so the position log shows which moves were the leader's.
- `RoomSeen` gains nothing; `session.rs` sets `room_seen_at` only for `!seen.dark`, or keeps
  a second `room_shown_at` for the coordinator. Pick the one that leaves C-108's blind-walk
  code (`presence.moved_unseen`) untouched.
- `party.rs`: no change of logic if `room_seen_at` means a lit room; otherwise the test at
  `room_seen_at.is_some_and(|t| t > since)` reads the new field. Keep the `look` fallback.
- Leave the walker alone: C-128 handles a refused move on a dark belief.

## Done when

- Replaying Sasha's eight follow lines with the darkness line after each, from 6/1406, ends with
  one candidate, 6/1414 (`tests/walk.rs`, a small graph with the same shape is fine).
- A typed `w` answered by ` -- Following your Party leader west --` and a room moves the belief
  one room, not two.
- Coordinator test: a listed follower apart from the leader for 10 s whose only rooms since are
  dark is not separated; it sends `look` (at most every 10 s) and the leader goes on holding;
  a lit room then decides as before.
- `docs/brain.md` says so under "Position and the map" and "Separation"; CI green.

## Tests

- `tests/walk.rs`: `a_follower_moved_by_the_game_in_the_dark_keeps_its_place`,
  `a_typed_move_the_game_followed_counts_once`.
- `tests/party.rs`: `dark_rooms_since_the_parting_do_not_separate_a_follower`.
- `party.rs` unit test beside C-116's: the lagging-belief case with `room_seen_at` unset and
  dark rooms only.

## Notes for the agent

- The darkness line is the parser's `room_dark` (`GameEvent::RoomDark`); the pitch-black
  variant maps to the same event.
- The host's logs are not fixtures: copy the two dozen relevant lines into a new
  `tests/fixtures/<subdir>/` with a `PROVENANCE.md` only if a test wants them verbatim; a
  hand-built transcript is enough.
- **Needs John:** when a follower's bot is paused mid-walk (as Sasha's was at 15:21:13), the
  leader stops waiting for it ("its bot is not running" is not a member who counts) and walks
  on; Rocco went eight stops alone in the dark before John stopped his loop. Keep that (a
  paused member is being driven by hand), or hold the leader while a listed member's bot is
  paused in another room? Not changed by this card either way.
- KB: `docs/kb/` has no note that the Redstone Tunnels (6/1407 to 6/1431) are dark; add one to
  the areas file with its [C] tag and a changelog line if the card confirms it on the live
  server.
