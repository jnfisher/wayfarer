---
id: C-048
title: "Sneaking and backstab: sneak before moving and after a fight, re-sneak when it fails, open a fight with a backstab"
tier: B
size: M
wave: 7
depends: [C-042]
crate: wayfarer-parse, wayfarer-brain, wayfarer-settings
touches:
  - crates/wayfarer-parse/src/typed.rs (sneak events)
  - crates/wayfarer-brain/src/{sneak.rs (new),modes.rs,executor.rs,fight.rs,session.rs}
  - crates/wayfarer-settings/src/lib.rs (`movement.sneak`, `combat.backstab`)
  - docs/brain.md "Sneaking"; docs/kb/ (what a sneak attempt and a sneaking move print)
---

## Goal

A character with *Sneak* on (off by default) keeps itself sneaking: before it moves on its own
(leading, a loop, Go to), it has a sneak attempt that did not fail; after a fight it sneaks
again; a sign that the sneak failed makes it sneak again. A sneaker opens each fight with
`bs <target>`. (Added 2026-10-23 at John's request; "any sneaker can attempt a backstab, only a
1H or unarmed weapon will actually backstab, and that's ok".)

## Inputs

- The C source (mmud-exploration's `WCCMMUD-1.11p-source`): `cmd_sneak` (LJNCMDA.C:2862-2930),
  `move_user`'s sneak roll (LJNUTIL.C:2808-2816, :2909-2926, :3781-3830), `cmd_backstab`
  (LJNCMDA.C:469-523).
- `docs/kb/classes/ninja.md` (sneak chance, minus one per other player and monster in the room;
  backstab), `docs/kb/megamud.md` (no sneaking for about a round after being attacked).

## Behaviour (game facts, from the C)

- `sneak` ends resting, meditating and auto-combat, then prints `Attempting to sneak...`. On
  success the line ends there. On failure it ends there too unless a perception roll catches it,
  then `You don't think you're sneaking.` follows on the same line. No prompt follows (a command
  delay runs). Refused: `You may not sneak right now!` (just attacked) and `You must wait before
  you may do that!`.
- A sneaking player's own move rolls again; a failure ends sneaking silently, or with `You make
  a sound as you enter the room!` when another perception roll catches it. A follower moved by
  its leader stays sneaking and sees `Sneaking...`.
- Attacking, casting, resting and being attacked end sneaking; `You are no longer sneaking.`
  shows for some of them.
- `bs <target>` while sneaking backstabs; with a weapon that cannot backstab the game prints
  `You cannot backstab with this weapon.` and strikes a normal blow. Not sneaking, it is a
  normal attack. It always starts the fight.

## Behaviour (the bot)

- **Settings**: `movement.sneak` *Sneak* (off by default) and `combat.backstab` *Open with a
  backstab* (on by default; it matters only for a sneaker).
- **Believed sneaking**: on after an attempt with no failure text; off on the failure texts,
  `You are no longer sneaking.`, `You make a sound as you enter the room!`, a fight, a cast, a
  rest, a death.
- **Before a move the bot makes itself** (a leader's or solo loop, Recover's walks, a
  follower's walk back to its leader): sneak first unless believed sneaking; a failed attempt
  is tried again; `You may not sneak right now!` waits a round and tries again. Flee never
  waits to sneak. Walks the player starts (Go to, a map click, errands) go as they are.
- **A follower**: after a fight (and after a rest) it sneaks again; a failure is tried again.
- **Backstab**: Fight's opening attack is `bs <target>` while believed sneaking.
- **Not forever**: five failed attempts in a row and the bot moves on unsneaked, saying so,
  until the next fight.

## Done when

- In-process: a sneaker sneaks before a loop's move and after a fight; retries on the failure
  line; does not move until an attempt succeeds; opens with `bs`; a non-sneaker is unchanged.
- Live, if the local server allows: the sneak attempt's lines as the parser sees them.

## Tests

- Typed events for the sneak lines; the executor's answer to `sneak`; the sneak state; Fight's
  opener; the move hold.
