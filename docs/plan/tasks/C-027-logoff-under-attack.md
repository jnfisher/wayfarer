---
id: C-027
title: Log-off under attack, and the M2 brain follow-ups (Recover's fallbacks narrated, stuns, players in the room)
tier: B
size: M
wave: 4
depends: [C-025]
crate: wayfarer-gateway, wayfarer-brain, wayfarer-proto
touches:
  - crates/wayfarer-gateway/src/{logon.rs,session.rs,server.rs,gateway.rs} (the log-off runner; the shutdown order), src/presets/*.toml
  - crates/wayfarer-proto/src/ (`logging_off`, `ready_to_leave`)
  - crates/wayfarer-brain/src/{recover.rs,session.rs,fight.rs}
  - tests/live/ (the log-off step under attack), docs/gateway.md, docs/brain.md
---

## Goal

Stopping the gateway never leaves a character dropped in the realm. Today the log-off sends
`x` once and waits; `x` is accepted and ends auto-combat, but a monster's blow during the
six-tick meditation interrupts it (`Your meditation has been interrupted - you may not exit
now!`), the wait times out after 90 s and the gateway hangs up, which the game punishes (seen
twice on the rig in wave 3). Worse, the hub closes every client, the brain included, before
the log-off starts, so the bot cannot get the character out of the fight first. After this card the log-off keeps
trying until the character is out, and the small brain follow-ups from the wave 3 reviews are
closed.

## Inputs

- PR #38's live run and the "Follow-ups" comments on PRs #36 and #38.
- `docs/kb/connection.md` (log off cleanly; the punishment), `docs/kb/recovery.md` "Leaving
  the realm" (`x` is accepted in combat and ends auto-combat; the meditation is six ticks; a
  blow interrupts it: `exit_interrupted`), `docs/kb/pitfalls.md` 20, 24, 31.
- `crates/wayfarer-gateway/src/logon.rs` (`Logoff`: steps with `at`, `send`, `timeout`; the
  `Window`), `src/session.rs` (where the log-off runs and times out), `src/presets/silvermere.toml`
  and `mbbsemu.toml` (`[[logoff]]`), `README.md` "Stopping" (`Hub::stop` before
  `Gateway::shutdown` today).
- `crates/wayfarer-brain/src/recover.rs` (`WALKS`, the two fallbacks), `src/session.rs`
  (`combat()`), `src/policy.rs` (`COMBAT_QUIET`), `src/modes.rs` (`Presence.attackers`),
  `src/fight.rs`.

## Behaviour

**Gateway.**

- **Order of shutdown.** The brain's connection stays open until every log-off has finished
  (or given up); only then does the hub close the clients. Before the first `x`, the gateway
  sends the brain `logging_off {session}` for each session it is about to log off and waits
  up to `leave_patience` (30 s) for `ready_to_leave {session}`, or for the brain to go away;
  then it logs off. In this card the brain answers at once after stopping the session's bot
  (nothing more is sent); C-028 makes it flee first when attacked.
- **Retries.** A log-off step can name texts that mean "not yet" (`retry_on` in the preset:
  `meditation has been interrupted`). On one of them the runner waits for the next prompt and
  sends the step again, up to `retries` (20) and within the patience (raise the realm step's
  `timeout` and `logoff_patience` so twenty tries fit: a try is the six-tick meditation plus
  the prompt). No `break` step: `x` already ends auto-combat (KB); only a blow during the
  meditation interrupts, so retries succeed when the attacker misses or dies.
- When the bound or the patience runs out the gateway still has to close; it logs a
  `warning` naming the character and "dropped in the realm", so the punishment is never a
  surprise.
- `sent` carries the retried `x`s like any line; UIs see them as before.

**Brain.**

- `logging_off {session}`: stop that session's bot (no new commands), answer
  `ready_to_leave {session}`. (C-028 inserts the flee before the answer.)
- Recover: after two failed walks home it narrates "Rested; could not walk home, waiting here"
  (`notice`) rather than a plain "Waiting here"; after two interrupted walks out it narrates
  that it is resting here against the quiet-room rule (`warning`). Both stay bounded.
- The 12 s quiet rule (`COMBAT_QUIET`): a `smashed_you` (knocked down) up to `rises` is not
  quiet time; the clock for the rule starts again at `rises`.
- A player in `Also here` is never an attacker unless a `pvp` line says so; a player
  standing in the room does not block Recover from resting.
- `in_combat` in the snapshot goes `false` when the quiet rule declares the fight lapsed.

## Done when

- Gateway tests (the in-memory connector, paused clock): a log-off whose `x` is answered by
  the interruption text three times then succeeds; one answered forever is retried `retries`
  times and then dropped with the warning; with a brain attached, `logging_off` goes out
  first and the `x` waits for `ready_to_leave` (or 30 s, or the brain leaving); the brain's
  connection is still open while the log-off runs and closes after; the non-combat path is
  unchanged (one `x`, the meditation, the menus).
- Brain tests: each follow-up above, from fixture lines.
- **Live**: `scripts/live-check.sh --scenario fight` gains a step: with the character still in
  the Arena with a monster attacking, stop the gateway; the log-off succeeds (the log shows
  retries and `Closed`, never a hang-up), and the next log-on shows no punishment notice.
- `docs/gateway.md` "Reconnecting and logging off" and the preset format document `retry_on`
  and `retries`; `docs/protocol.md` the two messages.

## Tests

- As above, under a second each.

## Notes for the agent

- Getting out of the fight is the bot's job (C-028, through `logging_off`); the gateway's
  part is to ask the brain first, keep trying, and never drop silently.
- Keep the presets' MBBSEmu wording too (`docs/kb/connection.md`).
