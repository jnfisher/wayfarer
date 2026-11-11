---
id: C-094
title: "Blind is not a refusal; a refused target is skipped in its room, not for the session"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/fight.rs, modes.rs, character.rs
  - crates/wayfarer-brain/tests/fight.rs
  - docs/brain.md "Fight here"
---

## Goal

A blind character skips no monster, waits and looks until it sees, then fights on. A target
the game refuses is left alone only while the character stays in that room.

John (2026-11-11): "penny just went through a stretch where she failed to attack with a
"Waiting" message … we should never be "skipping" a regular monster we're grinding like a
nomadic shaman".

## Inputs

- Live, 2026-11-11 10:03 (Torchlit Caverns). A nomadic shaman's black curse blinded Penny. Her
  `bash shaman` came back as speech (`You say "bash shaman"`), twice, and her `look` said
  "You are blind.". Fight counted two unanswered attacks and skipped "nomadic shaman" for the
  session. At 10:26 she stood by one while Lexi fought it alone.
- Lexi, blind at 10:04: "You do not see shaman here!", then "You are blind.". "large nomadic
  shaman" and "large nomadic tribesman" were skipped for the session.
- `LJNPEND.C` / `kb/recovery.md`: "You are blind." is the answer to a look.

## Behaviour

- Fight tracks blindness: set by "You are blind." or by an attack echoed as speech; cleared by
  the next room shown (title or `Also here:`).
- Blind, a refused or unanswered attack skips nothing. Skips made in the room before the
  blindness showed are cleared. Out of combat it waits ("Fight: blind, waiting to see again")
  and looks every 10 s ("Fight: blind, looking whether it can see"). In combat, auto-combat
  goes on.
- A skip (refused, or unanswered `ATTACK_TRIES` times) lasts while the character stays in the
  room it was made in. Leaving clears it.
- The character's "blind" status clears when a room is shown.

## Done when

- `fight.rs` tests: `a_refused_target_is_skipped_in_its_room_and_fought_again_elsewhere`,
  `a_blind_character_skips_nothing_and_looks_until_it_sees`,
  `a_refusal_that_turns_out_to_be_blindness_skips_nothing`.
- fmt, clippy `-D warnings`, nextest.

## Notes for the agent

- Follow-ups not in this card: confusion and the other "lost action" texts (C-095), and making
  automatic monster exclusions (the stalemate's session rule) manual, with defaults taken from
  MegaMud's monster relationships.
