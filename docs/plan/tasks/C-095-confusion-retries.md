---
id: C-095
title: "A command lost to confusion goes again"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-parse, wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-parse/src/game_texts.rs, registry.rs (the generated `confused` patterns)
  - crates/wayfarer-world/src/pack.rs (`game_texts`: the confuse messages)
  - crates/wayfarer-brain/src/executor.rs (`Refused::Lost`, the resend), session.rs, walker.rs
  - docs/brain.md, docs/kb/spells.md, docs/kb/changelog.md
---

## Goal

When the game loses a command to confusion, the bot sends the same command again, whatever it
was. A command lost repeatedly is given up without blaming the target or the exit.

John (2026-11-11): "in the game, confusion randomly stops some actions where you fumble. when
that occurs we should retry whatever command failed - moving, attacking, spellcasting, etc
(check the messages on that in the game data)."

## Inputs

- `LJNUTIL.C` `check_confusion`: the Confusion ability (71) is the percent chance an action
  is lost. The texts are the Confuse Message (ability 101) record's first text, or "You look
  around stupidly and do nothing!". Every typed line is checked (`LJNPEND.C` stage 5), as are
  autocombat rounds and casts.
- Stock data: 85 spells and items have Confusion, with about 22 texts (`kb/spells.md`).

## Behaviour

- The parser generates a `confused` pattern from the first text of each confusing spell's and
  item's Confuse Message, at priority 49, ahead of the static table.
- The executor answers any command with `Refused::Lost` on a `confused` line or `too_afraid`
  ("You are too afraid!"). `too_afraid` is no longer an attack refusal.
- A lost command with nothing pipelined ahead is queued again for the next prompt, up to
  `LOST_TRIES` (5) in a row, narrated "Confused: "<cmd>" was lost, sending it again (n/5)".
  After that, or with a move ahead in flight, the owner gets `Lost`. The walker treats it as
  held: the walk stops and a journey begins again after a moment; the exit is not refused.

## Done when

- `typed.rs` `a_lost_action_is_confused_whatever_the_effect_says`.
- `fight.rs` `an_attack_lost_to_confusion_goes_again`, `a_move_lost_to_confusion_goes_again`,
  `a_move_lost_five_times_is_given_up_as_held`.
- fmt, clippy `-D warnings`, nextest.
