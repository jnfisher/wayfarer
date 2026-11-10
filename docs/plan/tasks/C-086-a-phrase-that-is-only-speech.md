---
id: C-086
title: "The hole is 2/487's: holes that cast a teleport, a MegaMud phrase in the wrong look-alike, and a phrase that is only speech"
tier: B
size: S
wave: 16
depends: [C-085]
crate: wayfarer-world
touches:
  - crates/wayfarer-world/src/nav.rs (`spell_teleport`, `room_commands`, `learn_crossings`)
  - crates/wayfarer-brain/src/position.rs (`you_say`, `spoken`)
  - crates/wayfarer-brain/src/walker.rs (a phrase that was only speech)
  - tests/fixtures/megamud-hole/ (new: `Dthhstt1.mp`)
  - docs/brain.md, docs/kb/navigation.md
---

## Goal

On 2026-11-10 at 17:27, on v0.1.3, Lexi's Go to to Markus reached 2/193 with the position sure
of the right room all the way (`position.log`, C-085). The route then typed `go hole`, and the
game answered `You say "go hole"`. After that:

1. the position took the room after the phrase as the hole's other side. It didn't fit, so it
   threw the sure 2/193 away for 22 look-alikes;
2. the walker planned from 2/193 again, typed `go hole` again, 20 more times in 300 ms, and
   stopped with "lost the way: expected Stone Tunnel, Hole Up".

The hole is real, but it is 2/487's. Its command text casts *fall* (spell 336: Teleport Map 2,
Teleport Room 1306) into Stone Tunnel, Hole Up. 2/193 has no command text. The two rooms are
look-alikes: same name, exits and MegaMud room id, which 22 rooms share. The pack had no way
through at 2/487, because a room command that casts was left out (C-052). It had one at 2/193
instead: MegaMud's `Dthhstt1.mp` starts with `@party go hole`, and its walk placed that step in
the first of the 22.

## Behaviour

- **A spell that only teleports.** A room command's `cast <spell>`, where the spell's only
  abilities are 140 (`ABL_TELEPORTROOM`), 141 (`ABL_TELEPORTMAP`; 0 or missing is the room's own
  map), 144 (`ABL_NONMAGICAL`) and a zero damage slot (1), is the line's teleport. `text` is a
  plain clause. That adds five ways through: the hills holes at 2/487 → 2/1306, 2/630 → 2/1617
  and 2/115 → 2/1123, and the organic chambers 2/2366 ↔ 2/2367.
- **A crossing in the wrong look-alike.** A MegaMud crossing is not added when another room
  with the same MegaMud room id already has a way through to the same room taking the phrase.
- **Only speech.** When the game answers the phrase in flight with `You say "<phrase>"`:
  - the position drops that move, so the room seen next is read as the same room;
  - for a MegaMud crossing (`PathCommand`), the walker bars it for the session
    (`Traveller::blocked`), warns "`go hole` is only speech in Dragon's Teeth Hills: going
    round", and plans again;
  - a room command is the game's own data, so speech there means the position is wrong. The
    `look` after it tells where the character is, as before (C-055, `goto.rs`
    `a_phrase_that_is_only_speech_is_not_taken_for_the_move`);
  - `position.log` gets `speech` and `barred` entries.

## Done when

- `commands.rs`: 2/487 has `go hole` → 2/1306, the tunnel's `climb hole` → 2/487, and 2/193 gets
  no `go hole` once the crossings are learned. (`tests/fixtures/megamud-hole` adds `Dthhstt1.mp`.)
- `nav.rs` unit tests: `spell_teleport` and a `cast` line.
- `walk.rs`: a MegaMud crossing answered as speech (`go arch` at 7/141) is typed once. The
  warning is said, and the belief is never relocated. Without the walker's change the test fails.
  `goto.rs`'s C-055 test (a room command heard as speech: look, and try again) still passes.
- `cargo fmt`, `clippy -D warnings`, `nextest` pass.

## Notes for the agent

- The other 81 path phrases in rooms with shared MegaMud ids and an ambiguous path start are
  left alone. Only a look-alike that has the same way through rules one out. Any other wrong
  placement is caught by "only speech".
