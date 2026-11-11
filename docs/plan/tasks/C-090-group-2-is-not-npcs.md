---
id: C-090
title: "Montype 2 is a spawn group, not NPCs: the ice sorceress and the other unique evil monsters are fought"
tier: C
size: S
wave: 16
depends: []
crate: wayfarer-world
touches:
  - crates/wayfarer-world/src/monsters.rs (`npc`, `boss`, `huntable`)
  - docs/brain.md "Worth fighting"
  - docs/kb/combat.md, docs/kb/changelog.md
---

## Goal

John (2026-11-10): the party would not attack the ice sorceress in the Tower's Open Chamber.
She blasted Sasha while Penny rested and the others watched, and the party fled at Sasha's
flee line. When John walked Lexi back in, nobody attacked.

The bot skipped every `montype` 2 monster as an NPC (`kb/combat.md`). `montype` is the spawn
group (`kb/progression.md`: a room spawns `montype == mongroup`). Group 2 is the one no group
spawn makes. It holds the shopkeepers and healers, and also the unique evil monsters, which a
room's `specmon` or `genmonster` places. The game data has 15 evil group-2 monsters that come
back after their kill: the ice sorceress (9,500 exp, back in an hour), the adult red and
enormous blue dragons, Lord Feyr, Nahr, the beautiful woman and handsome man, and the chess
pieces. None of them was fought, and none counted as a boss.

## Behaviour

- An NPC is a `montype` 2 monster, except an evil one that comes back after its kill
  (`maxalive` 1, `regenhrs` > 0). Those are monsters worth fighting, and bosses (attack first,
  a lair to stop short of).
- Group-2 monsters that are not evil (healers, shopkeepers) or never come back (Cygani and
  Aderem, Chaotic Evil, 9 exp, `regenhrs` 0) stay NPCs.

## Done when

- `monsters.rs` `an_evil_montype_2_monster_that_comes_back_is_a_boss_to_hunt_not_an_npc`:
  the ice sorceress is huntable and a boss; Cygani and a healer are neither.
- The KB says what group 2 is, with a changelog line.
- `cargo fmt`, `cargo clippy -D warnings` and `cargo nextest run --workspace` pass.
