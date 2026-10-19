---
id: C-020
title: M1 follow-ups: `sent` ordered with output, more events applied to the model, inventory on the wire
tier: B
size: S
wave: 3
depends: [C-016, C-018]
crate: wayfarer-gateway, wayfarer-brain, wayfarer-proto
touches:
  - crates/wayfarer-gateway/src/session.rs (the `sent` stream)
  - crates/wayfarer-brain/src/{character.rs,session.rs}
  - crates/wayfarer-proto/src/ (optional `CharacterState` fields), ui/src/proto/, ui/src/lib/state.ts
  - docs/protocol.md
---

## Goal

The loose ends the wave 2 reviews left (PR #25, #27, #28 comments), fixed before the walker
and the bot build on the brain: a typed line's echo is never mistaken for game text, the
model applies every event the parser already types, and what the brain knows about coins and
carrying reaches the UIs.

## Inputs

- PR #25 "Follow-ups" comment; PR #27 and #28 comments.
- `crates/wayfarer-gateway/src/session.rs` (where `sent` is produced, after the write) and
  `src/server.rs` (how `sent` and `output` are forwarded on separate paths).
- `crates/wayfarer-brain/src/session.rs` (`sent`, the `own` queue, `silent_until`; `attach()`
  resets the parser and `fed` today but not `silent_until`), `src/character.rs` (`apply`,
  `last_line` and the counters), `crates/wayfarer-parse/src/typed.rs`.
- `docs/protocol.md` "The brain".

## Behaviour

- **Order.** The brain sees a `sent` line before the output chunk that echoes it. Make `sent`
  part of the session's output stream order: the session task emits it on the same channel
  as chunks (a `Chunk`-or-`Sent` item), or stamps it with the `seq` it precedes and the server
  forwards both through one task per subscriber. Either way a brain receiving `sent {seq: n}`
  has received every chunk up to `n` and none after. The wire message is unchanged.
- **`own` queue.** The brain's queue of lines it sent itself shrinks when the gateway reports
  `sent` for them or after a bounded time, so a lagged `sent` broadcast cannot make a later
  player-typed `st` look like the brain's own.
- **Log restart.** When a session's log restarts (`last_seq` below what was fed), the
  session's `silent_until` and `last_line` reset with the parser, so events and counts resume.
- **Model.** `Character::apply` handles `Healed`, `TookDamage` (HP adjusted until the next
  prompt confirms), `Unconscious { who }` for the character's own name (HP known to be at or
  below 0, `in_combat` false), `MonsterDeath` (clears the current target if one is tracked;
  a `kills` counter in the snapshot's `parse`-style counters), `Poisoned`/`PoisonTick`
  (status on) with `Healed`-by-cure or death clearing it, `Blind` likewise.
- **Wire.** `CharacterState` gains optional `coins` (total in copper, and the raw list),
  `carrying` (item names), `encumbrance { current, max, level }`, `kills`. The UI state
  merges them; nothing displays them yet beyond the dev panel's snapshot view, if it has one.
- **UI.** `brain.characters` is pruned for ids absent from a `welcome`'s session list.

## Design notes

- Prefer the "one channel per session" ordering (item enum) over seq-stamping: it is the
  gateway's invariant, not the client's arithmetic.
- Adding optional fields bumps nothing; regenerate the TypeScript and the README examples.

## Done when

- Gateway test: a UI line typed while output streams is delivered to the brain as `sent`
  strictly before the chunk holding its echo, over 1,000 random interleavings.
- Brain tests: the echo of a typed `st` parses as `echo`, never `unknown`; the `own` queue
  is empty after each re-sync; a log restart resumes events and counts; each new typed event
  changes the snapshot as described, from fixture lines.
- The new fields round-trip and appear in `docs/protocol.md`'s `CharacterState` table.
- `scripts/live-check.sh --scenario brain` still passes; paste the summary.

## Tests

- As above, under a second each.

## Notes for the agent

- Keep the hard rules: the brain still sends only the re-sync commands.
- Do not start on the executor or the walker; C-022 does.
