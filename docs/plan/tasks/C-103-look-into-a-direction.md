---
id: C-103
title: "A look into the next room is not a move; an attack said aloud is refused at once"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/peek.rs (new)
  - crates/wayfarer-brain/src/session.rs (`feed`, `sent`, `send_own`)
  - crates/wayfarer-brain/src/executor.rs (`said`)
  - crates/wayfarer-brain/tests/typing.rs
  - crates/wayfarer-brain/tests/fight.rs
  - docs/brain.md ("Typing beside the bot")
---

## Goal

Looking into the next room by hand (`l s`, `look north`) leaves the bot where it is: it does
not believe the character moved, and it does not attack a monster it saw there. An attack of
the bot's that the game takes for speech no longer holds the player's typed lines for 5
seconds.

John (2026-11-16), playing Riley with the bot on: "when I look into a room with a monster it
tries to attack, then for a few seconds its not sending my messages". He asked for the fix.

## Inputs

What Riley's logs show (2026-11-16, 12:11:07 and again at 12:11:14):

- The player typed `l s`. The game showed the Alchemy Laboratory, the room south, with "Also
  here: mad wizard, acid slime." It shows a looked-at room just as it shows the room the
  character stands in.
- The brain took that for the character's own room ("You moved: playing in Alchemy
  Laboratory", "Fight: mad wizard is here: switching from acid slime") and sent `bash wizard`.
- No wizard was in Riley's room, so the game read the line as speech: `You say "bash
  wizard"`. The fight already reads that as blindness, but the attack itself waited for
  `*Combat Engaged*` until its 5 s timeout. Typed lines go only with nothing in flight, so
  the player's lines (four empty lines the first time) went 3-4 s late.

## Behaviour

- A line that looks into a direction (`l`, `lo`, `loo` or `look`, then one direction word),
  typed or the bot's, marks the next room shown as looked-at. From its title to its exits (or
  `too dark` at once), the room's events go to the UIs as usual but to nothing else in the
  brain: position, fight, presence, loot, executor. A prompt before any title ends the wait
  (no exit that way).
- An attack in flight (`Expect::Engaged`) whose own command comes back as `You say "<it>"` is
  refused at once (`Refused::Target`), as `You do not see wizard here!` is. The fight's
  blindness handling of that line is unchanged, and blind it skips nothing.

## Done when

- `tests/typing.rs` `a_look_into_the_next_room_is_not_a_move_and_its_monsters_are_not_fought`:
  with the bot waiting in the Arena, `l u` shows the Narrow Road with a fat giant rat; the bot
  sends nothing for it, says nothing of the Narrow Road, and fights a rat that arrives in the
  Arena after. It fails without the change (the bot sends `a rat` at the look).
- `tests/typing.rs` `a_line_typed_after_an_attack_said_aloud_goes_at_once`: the bot's `a rat`
  comes back as `You say "a rat"`, and `gossip hi` typed behind it goes within a second.
- `tests/fight.rs` `an_attack_the_game_took_for_speech_is_refused_at_once`: the executor
  refuses the attack on its own words, and not on someone else's.
- `src/peek.rs` unit tests: which lines look into a direction; the looked-at room ends at its
  exits or at `too dark`.
- `docs/brain.md` "Typing beside the bot" says so.
