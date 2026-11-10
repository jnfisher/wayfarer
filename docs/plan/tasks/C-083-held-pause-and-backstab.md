---
id: C-083
title: "A moment's wait after a knockdown; backstab only while sneaking, then the attack command"
tier: B
size: S
wave: 16
depends: [C-082]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/modes.rs (`Bot::walk_held`, `HELD_PAUSE`), journey.rs, session.rs
  - crates/wayfarer-brain/src/fight.rs (the backstab opener and the switch after it)
  - crates/wayfarer-brain/tests/walk.rs, sneak.rs
  - docs/brain.md
---

## Goal

- **A knockdown doesn't use up a Go to.** v0.1.1 (C-082) began a held leg again at once. On
  2026-11-10 at 15:55, Lexi's Go to to Markus was refused three times in 20 ms while she was
  still flat on her back. That used up the journey's three tries, and it gave up ("3 walks …
  ended elsewhere").
- **Backstab** (John, 2026-11-10):
  - `bs <monster>` is only the first round, and only while the bot believes the character is
    sneaking;
  - after that round the attack command takes over (`a`, `bash` …);
  - with a sneak known to have failed, the fight opens with the attack command.

## Behaviour

- **Held** (`Next::held`, a bot walk):
  - `Bot::walk_held` takes one off the journey's or loop's tries, so the refused leg is not
    counted;
  - for `HELD_PAUSE` (4 s) the bot decides nothing, unless it is engaged or attacked, and its
    `wake` brings it back then.
  - Holds wear off, so repeated refusals keep retrying every few seconds and never give the walk
    up.
  - Every bot walk is covered: a follower's walk back to its leader after the leader moved on
    while it was down, a loop's legs and Recover's walks.
- **Backstab**:
  - `bs` only when `backstab` is on and `sneaking` is believed; otherwise the attack command, or
    the attack spell as usual.
  - Once the backstab's strike is seen (a hit or miss line after `bs`), the next engaged step
    sends `<attack command> <monster>` once ("Fight: backstab done, now bash"), as the spell
    fight switches to the weapon.
  - `*Combat Off*`, a kill or exp clears it.

## Done when

- `walk.rs`: a Go to refused five times in a row walks on and arrives, saying "Can't move for
  now: waiting a moment".
- `sneak.rs`:
  - a sneaking arrival opens with `bs rat`, then `bash rat` after the strike;
  - an arrival whose move showed no `Sneaking...` opens with `a rat`.
- `docs/brain.md` covers "Knocked down, held or stunned" and the backstab rules.
