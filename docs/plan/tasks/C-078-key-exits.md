---
id: C-078
title: "Key exits as the game words them: the shut door's refusal, `use <key> <dir>`, then `open`"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/walker.rs (the Key exit branch, `use` with the direction, `open`)
  - crates/wayfarer-brain/tests/keys.rs
  - docs/kb/navigation.md, docs/kb/changelog.md
---

## Goal

A Go to through a Key exit, with the key held, unlocks the door with the key, opens it and walks
through. On 2026-11-10 Lexi's Go to to Markus (2/2518) stopped at Rhudaur's Massive Doors
(2/2519, north, the black serpent key): "`n` refused: a closed door". The exit was then blocked
for the session, so every later Go to said "no way there" until John typed `use key n` and
`open n`.

## Inputs

- The live text (`~/.local/share/wayfarer/logs/lexi/output.log` on the server, seq 1758458 to
  1758468): `n` → "There is a closed door in that direction!"; `use key n` → "You successfully
  unlocked the door."; `open n` → "The door is now open."; then the exits line reads `open door
  north`.
- `kb/navigation.md` "Key": a shut Key exit refuses a move with "There is a closed door in that
  direction!" (`door_closed_drag`, `Refusal::ClosedDoor`), not with "The door is closed!".
- silvermere `commands/use_item.rs`: USE of a key takes the word after the key's name as the
  direction; with no word, nothing is unlocked. A second USE says "The door was not locked."

## Behaviour

- A move refused at a Key exit (either text):
  - **with the key held**: `use <key> <dir>`, then `open <dir>`, then the move. At most two
    tries of the key.
  - **without the key**: `open <dir>`, since the door may be shut but not locked.
- If `open` says "The door is locked." and the key is still untried, the walker tries the key.
  Otherwise it blocks the exit and plans again ("the door north would not unlock").
- A Key exit is never bashed: `bash` can't pass one.

## Done when

- `keys.rs` walks Thrag's door with the game's texts: `e`, `use large iron key e`, `open e`, `e`.
- A key that won't turn is tried twice and then given up, with no bash.
- KB "Key" gains the direction and the `open` step, tagged [C] and [C live]; a line goes in
  `changelog.md`.

## Notes for the agent

- The fake game in the tests answered a Key exit with "The door is closed!" (a Door's text),
  which hid the fault. Use the game's own texts.
