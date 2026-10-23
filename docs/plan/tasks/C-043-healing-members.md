---
id: C-043
title: "Healing party members: a heal-a-member line in a fight and while resting, party heals chosen by how many are hurt"
tier: B
size: M
wave: 6
depends: [C-042]
crate: wayfarer-brain, wayfarer-settings, wayfarer-world
touches:
  - crates/wayfarer-brain/src/{spellcraft.rs,fight.rs,recover.rs,party.rs,session.rs}
  - crates/wayfarer-world/src/ (party heal spells from the pack: type 3, ability 18, target 10 or 13)
  - crates/wayfarer-settings/src/lib.rs (three settings)
  - docs/brain.md "Spells" and "Parties"
---

## Goal

A character with a heal spell heals other party members the way it heals itself, by a second
pair of lines, and uses its party heal when several members are hurt.

## Inputs

- `docs/plan/10-party-play.md` §8.
- `docs/brain.md` "Spells" (heals in a fight, heals while recovering: the cheapest heal that
  covers the missing HP, one per round, re-engage after a cast), `spellcraft.rs`
  (`Spellbook`), `docs/kb/spells.md` "Spell record" (target 10 Divided Party Area, 13 Full
  Party Area; ability 18 heal), `docs/kb/classes/{priest,paladin,druid,cleric,missionary}.md`.
- `docs/kb/grouping.md` "Witchunters in groups" (Anti-Magic resists friendly heals).

## Behaviour

- **Settings** (the Party group, `docs/plan/10-party-play.md` §10; their help points to the
  self-heal pair in Spells):
  - `party.heal_member_in_combat_below_hp` "Heal a party member in a fight below" (default
    50, 0 off),
  - `party.heal_member_resting_below_hp` "Heal a party member while resting below" (default
    60, 0 off),
  - `party.heals` "Use party heals" (`auto`/`off`, default `auto`; help says it only
    matters for a character that knows one).
- **Who**: every joined member in the healer's room, itself included (its own pair of lines
  for itself). HP from each member's own prompt (our characters), not `par`. The member
  furthest below its line first; a member below its flee line before anyone.
- **Single heal**: the spellbook's single-target heals that can target another player
  (target 0, 2 or 6), cast as `<short> <name>`; the cheapest that covers the missing HP, else
  the strongest affordable, as for self-heals. In a fight, once per round and re-engage the
  target after (as today); while resting, between rests (as today).
- **Party heal**: a heal with target 10 or 13 from the pack, found by its record, **never by
  the command word** (`rain` is also the Druid's acid rain, a room attack). With
  `party.heals` `auto`, cast it when two or more members are below their heal line,
  or when its average heal summed over every hurt member (each capped at their missing HP)
  beats the single heal's on the worst-hurt member; a member below its flee line still gets
  the single heal first.
- **Witchunters**: healed like anyone (the casts may be resisted); say so in `docs/brain.md`.
- No mana reserve (John: "not for now").

## Done when

- In-process: a Priest with healing rain in a party of three heals the one member at 40% with
  a single heal, two members at 40% with the rain; a Druid's acid rain is never chosen as a
  heal; a healer resting heals a resting member below 60%.
- Live: on John's rig a party rest is visibly shorter with a healer casting on the others.

## Tests

- `spellcraft.rs`: party heals found by record (target 10/13, ability 18); acid rain excluded;
  single heals that can target others.
- The choice rule: one hurt, two hurt, one badly hurt and one slightly, a member below flee.
