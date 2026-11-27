---
id: C-135
title: "A walk that set off unsneaked stops in the first clear room to sneak, and a cast that does no damage ends the sneak"
tier: A
size: S
wave: 24
depends: [C-048, C-134]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/sneak.rs (`due`, `clear_to_try`; `YouCast` ends the belief)
  - crates/wayfarer-brain/src/modes.rs (`sneak_due`)
  - crates/wayfarer-brain/src/session.rs (stop the walk when a sneak is due)
  - crates/wayfarer-parse/src/typed.rs (`GameEvent::YouCast`)
  - docs/brain.md "Sneaking"
---

## Goal

On v0.1.55 (C-134 in), Penny the Gypsy still walked 110 of 285 moves unsneaked in an hour
(silver-mirror, 2026-11-27), mostly in runs of 20, 33 and 43 moves. Only about six were the
unavoidable kind (a silent failure that the next move shows). The rest came from two gaps:

- **A walk that set off unsneaked stayed unsneaked.** The bot sneaks before a walk starts, and
  stops a walk only when a sneak it believed in breaks. A walk that could not sneak at its
  start (a monster in the room, or `You may not sneak right now!` after a kill or a rest) had
  nothing to stop it, and went unsneaked to its end: 46 empty rooms back from a rest after
  one refusal; 4 on the loop after a kill.
- **A blessing did not end the sneak belief.** Every cast ends sneaking (`kb/spells.md`), but a
  cast that does no damage (`You cast blur on Penny!`, `You cast ethereal shield, and a
  shimmering field forms about you.`) was read as nothing, so the bot set off believing it
  sneaked. After a pause and resume it cast blur and walked 33 rooms plain.

## Behaviour

- A bot walk (a loop leg, a Go to, Recover's walk; never Flee) that is under way while
  *Sneak* is on and the character is not believed sneaking stops in the first room with no
  monster in it and nothing attacking, once no refusal wait holds, narrated "Stopped walking:
  clear to sneak". The bot then sneaks before the next move, as before any walk.
- No attempt goes with a monster in the room (John, 2026-11-27: it always fails there): the
  walk keeps going through such rooms until it reaches a clear one.
- After five failures in a row the bot still moves on unsneaked until the next fight: no stop.
- The character's own cast that did no damage is `GameEvent::YouCast { spell }` and ends the
  sneak belief like any other cast, which marks it broken and stops a walk under way.

## Design notes

- `Sneak::due(presence, now)`: not on, not given up, no refusal wait, no monster here, no
  attacker. `may_try` shares the first four through `clear_to_try`.
- `Bot::sneak_due(settings, presence, now)` adds `movement.sneak` on and not Flee acting. The
  session checks it beside `sneak_broken` in both places (after each chunk, and after each
  walker step) and stops the bot's walk.
- The parser's `spell_cast_you` / `you_cast_generic` without damage, and the `mon_hits_you`
  line that opens "You cast " without damage (message 17), become `YouCast` instead of
  `Other`. Consumers that match the raw event name (`OWN_COMBAT`) are unchanged.

## Done when

- A walk that set off unsneaked sneaks in the first room with no monster in it.
- A blessing's cast ends the sneak belief.

## Tests

- `tests/sneak.rs`: `a_walk_that_set_off_unsneaked_stops_to_sneak_in_the_first_clear_room`
  (refused at the start, the bot sneaks in the next room before moving on; fails without the
  change).
- `sneak.rs` unit: `a_cast_that_does_no_damage_ends_the_sneak`,
  `due_only_in_a_clear_room_with_nothing_holding_the_attempt`.
- `wayfarer-parse` `typed.rs`: `a_cast_that_does_no_damage_is_the_characters_cast`.
