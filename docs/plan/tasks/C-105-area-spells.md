---
id: C-105
title: "Area spells: open on a room of enough monsters with fireball or swarm, with their own mana line, cast limits and choice"
tier: B
size: M
wave: 16
depends: [C-099, C-100]
crate: wayfarer-brain
touches:
  - crates/wayfarer-world/src/spells.rs (`SpellKind::Area`, `SpellDef::cast_line`)
  - crates/wayfarer-settings/src/lib.rs (`spells.area`, `spells.area_min_monsters`, `spells.area_mana_above`, `spells.area_max_casts`, `spells.boss_area_max_casts`)
  - crates/wayfarer-brain/src/spellcraft.rs, modes.rs, fight.rs, session.rs, brain.rs
  - crates/wayfarer-proto/src/lib.rs (`Bot.area_spells`, `Bot.area_spell`, `set_area_spell`)
  - crates/wayfarer-store/src/lib.rs (migration 15, `area_spell`)
  - ui/src/SettingsPanel.svelte, App.svelte, lib/gateway.ts
  - docs/brain.md ("Spells", "Boss fight mode"), docs/protocol.md, docs/web-ui.md
---

## Goal

A Mage or Druid clears a room of several monsters with its area spell (fireball, dancing
blades, swarm, chaos storm...) instead of one at a time, with the same kind of control the
attack spell has: on or off, a mana line, a spell choice, the most casts in a fight and in boss
fight mode, and the fewest monsters to cast at (John, 2026-11-16). It never hits a monster the
bot would not fight.

## Inputs

- `docs/kb/aoe-grinding.md` §1 (player area spells are Full Attack Area, target 12), §2 (cast
  out of combat it engages everything and auto-combat recasts it every round; neutral and good
  monsters are hit), §5 (never with neutral or good bystanders).
- The pack's spell messages: an area spell's caster line is its own ("The fireball explodes,
  causing 45 damage!"), not "You cast ... at ...".
- C-099 (the attack spell choice), C-100 (boss fight mode's cast limit).

## Behaviour

- The spellbook keeps area attacks (type 0, target 12, instant damage) apart from the attacks.
- Fight opens with the area spell (`fbal`, no target word) when `spells.area` is `auto`, the
  room holds at least `spells.area_min_monsters` (3) monsters, every monster there is one it
  would fight (hostile, not ignored, not skipped), and the mana is above
  `spells.area_mana_above` (30%; not in boss fight mode). The spell is the player's choice
  (`set_area_spell`, kept with the bot) or the strongest it can pay for.
- A good or neutral monster, an NPC or an ignored one in the room holds the area spell back
  (John: "don't cast AOE if a never-fight or good monster is there"; a neutral one hit joins
  the fight, so it holds it back too).
- Casts are counted by the spell's caster line (the pack message up to its first `%`), a failed
  cast too, over the whole fight. After `spells.area_max_casts` (0: no limit;
  `spells.boss_area_max_casts`, 10, in boss fight mode), when the mana comes down to the line,
  or when the room no longer qualifies, the fight changes to one monster at a time (the attack
  spell or the weapon). A room that fills up mid-fight changes it to the area spell.
- The UI lists the area spells under Area spells, as the attack spells are listed.

## Done when

- A Mage in a room of three rats sends `fbal` and nothing more while the game casts it; with
  two rats, or with a neutral kobold slave there too, or with `spells.area` off, `mmis rat`.
- With `spells.area_max_casts = 2`, two fireball lines bring `mmis rat` and the notice "Fight:
  2 casts of fireball, now large giant rat with magic missile".
- `set_area_spell` picks dancing blades over fireball, refuses `mmis`, and survives a restart
  (store round trip).
- The settings panel shows the Area spell list after the Area spells row and sends the choice.
