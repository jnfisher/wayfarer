---
id: C-099
title: "Choose the attack spell a caster fights with, in every mode"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-proto, wayfarer-store, wayfarer-brain, ui
touches:
  - crates/wayfarer-proto/src/lib.rs (`AttackSpellOption`, `bot.attack_spells`, `bot.attack_spell`, `set_attack_spell`)
  - crates/wayfarer-store/src/lib.rs (migration 12, `BotRecord::attack_spell`)
  - crates/wayfarer-brain/src/spellcraft.rs, modes.rs, session.rs, brain.rs
  - ui/src/SettingsPanel.svelte, ui/src/App.svelte, ui/src/lib/gateway.ts
  - docs/brain.md, docs/protocol.md, docs/web-ui.md
---

## Goal

The player picks the attack spell each caster fights with, from the attack spells it knows,
or leaves it to the bot (auto, as now). The choice holds in every mode: a loop, a goto, a fight
in place, and boss fight mode (C-100).

John (2026-11-12): "I'd rather be able to pick an attack spell *in general* that applies to all
modes. then for the hellion id pick something that it deosnt resist, and switch it back to
blaze after. this could also be useful when grinding."

## Inputs

- The greater hellion resists fire 100% (ability 5, value 100) and takes 25% more from cold
  (ability 3, value -25), in silver-mirror's world pack. Auto picks the spell that does the
  most damage on paper, which may be a fire spell.
- A setting's value is a number, a flag or a fixed word list (`wayfarer-settings` `Kind`), and
  a character's known spells are none of these. So the choice is kept with the bot's record,
  as the monster rules are (C-035), not in the settings files.

## Behaviour

- `bot.attack_spells` lists the attack spells the character knows (the spellbook the `spells`
  screen gave: short name, name, mana), and `bot.attack_spell` the short name chosen, absent
  for auto.
- `set_attack_spell {session, spell}` sets it: a short name from that list, or null for auto. A
  name it does not know is refused with a warning. The choice is saved with the bot's record
  (store migration 12, `bot_state.attack_spell`), so it survives a brain restart.
- With a spell chosen, Fight casts only that spell. When the mana is short for it, the weapon
  fights, as when auto has nothing it can pay for. Everything else stays: `spells.attack`
  off still means weapons only, `spells.attack_mana_above` and
  `spells.max_casts_per_monster` still apply. A chosen spell the character no longer knows
  falls back to auto.
- The settings tab shows, in the Spells group, an "Attack spell" list: "auto (the strongest it
  can pay for)" and each known attack spell with its mana. A caster with none shows nothing.

## Done when

- `spellcraft.rs` `a_chosen_attack_is_cast_while_the_mana_lasts_and_auto_otherwise`.
- `store.rs` the attack spell round-trips with the bot record.
- `tests/spells.rs` (or the brain's fight tests): with a spell chosen, the fight casts it
  rather than the stronger one auto would; set back to auto, the stronger one.
- `messages.rs` `set_attack_spell` both ways, null included.
- `settings-panel.test.ts`: the list shows the known spells, and choosing one sends
  `set_attack_spell`.
- fmt, clippy `-D warnings`, nextest, vitest, svelte-check.
