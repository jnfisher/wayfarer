---
id: C-101
title: "A typed line goes once the game is quiet after a stray line, not 5 s later"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/session.rs (`send_typed`, `TYPED_QUIET`)
  - crates/wayfarer-brain/tests/typing.rs
  - docs/brain.md ("Typing beside the bot")
---

## Goal

A line the player types beside a running bot goes to the game promptly when the game has
shown a stray whole line with no prompt after it, instead of being held for 5 seconds.

John (2026-11-13), moving the party by hand out of the temple: "after I moved up and south,
then tried to move west, the game wasnt responsive for a couple seconds". He asked for the
fix.

## Inputs

What the leader's logs show (2026-11-13, 21:56):

- `u` went at once; its room and prompt came back 0.9 s later.
- 0.04 s after that, another party member's brain sent `frontrank`. The leader's screen showed
  "Lexi just moved to the front rank in your group." with **no prompt after it**. The game
  prints rank changes so.
- The player typed `s` 0.6 s later. The brain held it: a typed line went only at a prompt,
  or after waiting `TYPED_PATIENCE` (5 s) for one. Two empty lines and `w` queued behind it,
  and all four went in a burst 5 s after `s` was typed.
- The party's own rank shuffling after a move makes this likely whenever the player walks
  the party by hand.

## Behaviour

- The brain notes when a prompt has come since the last line went to the game, and when the
  last chunk showed text ending a whole line (`\n`) with no prompt after it.
- With both, a typed line goes once the game has been quiet `TYPED_QUIET` (300 ms) since that
  chunk, as at a prompt: still one line at a time, nothing in flight, no re-sync command
  waiting.
- A chunk ending mid-line, or text after a line of ours the game has not answered with a
  prompt yet, still waits for the prompt or `TYPED_PATIENCE`, as before.

## Done when

- `tests/typing.rs` `a_line_typed_after_a_stray_whole_line_goes_once_the_game_is_quiet`: after
  a stray `frontrank` line, `gossip hi` goes within 500 ms.
- `tests/typing.rs` `a_line_typed_after_half_a_line_waits_for_the_prompt`: after half a line,
  it has not gone a second later.
- `docs/brain.md` "Typing beside the bot" says so.
