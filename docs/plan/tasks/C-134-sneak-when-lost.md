---
id: C-134
title: "Sneaking keys off the move, not a change of room, so a sneaker whose position is unknown keeps sneaking instead of walking plain"
tier: A
size: S
wave: 24
depends: [C-048, C-122]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/sneak.rs (`moved` replaces `arrived`; the room field drops)
  - crates/wayfarer-brain/src/position.rs (`took_move`, `just_moved`)
  - crates/wayfarer-brain/src/session.rs (drive `sneak_moved` from the move signal)
  - crates/wayfarer-brain/src/modes.rs (`sneak_moved` wrapper; drop the per-tick `arrived`)
  - docs/brain.md "Sneaking"
---

## Goal

John rerolled Penny to a Gypsy and expected her to sneak, but she walked long stretches plain
(silver-mirror, 2026-11-27: ten sneak attempts in 252 moves). She started sneaking again only
after a `sys goto` teleport. The cause: the sneak's "did this move show `Sneaking...`?" check
fired on a change of the believed room (`Sneak::arrived(room)`), but her position belief was
unknown (`None`) through the sewers and terse corridors. With the room never changing to a
known key, the check never ran, so a stale "believed sneaking" froze on and the bot never
re-sent `sneak`. The teleport put her back on a known room and unfroze it.

## Behaviour

- The sneak's end-of-sneak detection runs on the move itself: when the character takes a move
  (a direction that completed, as against a `look` or a status line) and no `Sneaking...` was
  shown since the last move, the sneak had ended — a failed attempt — and the bot sneaks again
  before the next move. This holds whether or not the belief knows the room.
- Everything else is unchanged: the five-failures give-up, the two-second wait after `You may
  not sneak right now!`, the follower path, backstab.

## Design notes

- `Position` gains `just_moved`, set true when `event` pops a move off `moving` (a real move
  completed) and false otherwise; `took_move()` reads and clears it.
- `Sneak::arrived(Option<Key>)` becomes `Sneak::moved()`: the move-ended logic with no room
  key and no room field.
- The session calls `bot.sneak_moved()` in the event loop whenever `position.took_move()` is
  true, right where each game event is read. The old per-chunk and per-walk-step
  `sneak_arrived(position.best())` calls go (they double-counted or missed when lost); the
  `sneak_broken` checks that stop the walk to re-sneak stay.

## Done when

- A sneaker whose position is unknown re-sends `sneak` after a move that showed no
  `Sneaking...`, instead of believing itself sneaking for ever.

## Tests

- `sneak.rs` unit: `a_move_without_sneaking_shown_means_the_sneak_failed` now drives `moved()`
  with no room key; the refusal-wait test uses `moved()` for the next-room reset.
