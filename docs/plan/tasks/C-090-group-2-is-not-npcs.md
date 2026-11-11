---
id: C-090
title: "Monsters the bot missed: montype 2 is a spawn group, not NPCs; movement texts that name their monster; monsters seen fighting the party"
tier: C
size: S
wave: 16
depends: []
crate: wayfarer-world, wayfarer-parse, wayfarer-brain
touches:
  - crates/wayfarer-world/src/monsters.rs (`npc`, `boss`, `huntable`)
  - crates/wayfarer-parse/src/registry.rs (`named_moves`)
  - crates/wayfarer-brain/src/modes.rs (`Presence::seen_fighting`)
  - docs/brain.md "Fight here"
  - docs/kb/combat.md, docs/kb/screen-patterns.md, docs/kb/changelog.md
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

John, later the same evening: three ice golems walked in and fought the party, and Lexi
said "Waiting here" and only healed. "The ice golem stomps in from the the eP\bast!" was
parsed as a monster called "east" arriving. Its text, "The ice golem stomps in from the
%s!", names the golem and has one `%s`, the direction, which the generated pattern took for
the monster. 86 monsters' movement texts are like it. John asked too that a character that
sees a party member fighting, while it is not, joins in.

## Behaviour

- An NPC is a `montype` 2 monster, except an evil one that comes back after its kill
  (`maxalive` 1, `regenhrs` > 0). Those are monsters worth fighting, and bosses (attack first,
  a lair to stop short of).
- Group-2 monsters that are not evil (healers, shopkeepers) or never come back (Cygani and
  Aderem, Chaotic Evil, 9 exp, `regenhrs` 0) stay NPCs.

- A movement text (arrives, leaves, follows) that names its monster and has one `%s`: the
  `%s` is the direction (`dir`), and `mon` is the monster whose record the text is.
- A monster seen fighting in the room is here, if no monster here is called that already:
  `mon_hits_other`, `mon_miss_other`, `spell_cast_room` (its caster), `attacks_other`,
  `other_hits`, `other_miss` (their target), an intercept (its monster). Fight then attacks
  it, without waiting for a look.

## Done when

- `monsters.rs` `an_evil_montype_2_monster_that_comes_back_is_a_boss_to_hunt_not_an_npc`:
  the ice sorceress is huntable and a boss; Cygani and a healer are neither.
- `typed.rs` `a_movement_text_that_names_its_monster_is_that_monster`: the golem's arrival,
  with the doubled "the" and the backspace, and its leaving, are the ice golem's.
- `fight.rs` `a_monster_seen_fighting_someone_else_in_the_room_is_fought`: an idle bot that
  sees "Rocco moves to attack fat giant rat." attacks the rat. It fails before the change.
- The KB says what group 2 is and how such movement texts read, with changelog lines.
- `cargo fmt`, `cargo clippy -D warnings` and `cargo nextest run --workspace` pass.
