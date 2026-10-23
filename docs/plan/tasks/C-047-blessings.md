---
id: C-047
title: "Blessings: each character keeps up the self-buffs the player ticks, picked in a Blessings panel"
tier: B
size: M
wave: 7
depends: [C-043]
crate: wayfarer-world, wayfarer-brain, wayfarer-store, wayfarer-proto, ui
touches:
  - crates/wayfarer-world/src/spells.rs (buff spells and their wear-off texts)
  - crates/wayfarer-brain/src/{bless.rs (new),modes.rs,session.rs,brain.rs}
  - crates/wayfarer-store/src/lib.rs (migration: chosen blessings per session)
  - crates/wayfarer-proto/src/lib.rs (`blessings_request`, `blessings_save`, `blessings`)
  - ui/src/{BlessingsPanel.svelte (new),App.svelte,lib/blessings.ts (new)}
  - docs/brain.md "Spells", docs/protocol.md, docs/web-ui.md
---

## Goal

The player picks, per character, one or more lasting spells it knows (bless, barkskin, holy
aura...) and the bot keeps them on: it casts each one that is not on between fights, and again
when it wears off. Picked in a drawer panel like Parties: one box per character, its lasting
spells ticked. (Added 2026-10-23 at John's request: "the party UI was convenient and something
like that per-character, picking a list of 1-N bless spells a character will keep up on
themselves".)

## Inputs

- `docs/kb/spells.md` "Casting" (lasting spells: rounds = `duration + durinc*(L/durlvls)`, 5 s
  rounds; a copy already on is renewed only when the engine allows, otherwise "You attempt to
  cast %s, but fail."; a spell ending prints its Descriptive Message line 0; death ends every
  spell) and "Spell record".
- The pack's spell records: type 3, a `duration` above 0; ability 115 (Descriptive Message) has
  the message number in the same slot of `abilvals`, whose texts are [wear-off line, "", the
  line the target sees] (bless: "The effects of bless wear off!", "You feel lucky!").
- `docs/kb/megamud-manual.md` §6.1 (MegaMud's Bless Spells: up to 10, not in combat, only above
  a mana line; bless while resting off by default because it ends the rest).

## Behaviour

- **Which spells**: the character's known spells (its `spells` screen) that are lasting general
  spells (type 3, duration above 0) it can cast on itself (target 1, 2, 6) or on the party (10,
  13). The panel lists them per character; the player ticks any number.
- **When it casts**: out of combat, nothing attacking, not resting below its rest line (it rests
  first), mana above the attack mana line (`spells.attack_mana_above`) after the cast, and the
  spell not believed on. A party member in a party rest does not bless (a cast ends the rest).
- **On or off**: on from the cast's confirmation (the target line of its message, or the
  generic cast line) for its duration at the character's level; off at its wear-off line, at
  death, and when the duration has run out. A cast that fails ("You attempt to cast ..., but
  fail.") is tried again after a round; three failures in a row hold that spell for five
  minutes (it may already be on and not renewable).
- **Storage**: the chosen spells are kept in the brain's store per session and survive
  restarts.
- **Narration**: "Blessing: casting bless".

## Done when

- In-process: a character with bless ticked casts it between fights, not again until the wear-off
  line, then again; not in a fight; not while resting below its line; a failure retries.
- The panel lists the lasting spells per character and saves the ticks.

## Tests

- `spells.rs`: buff spells and their wear-off and target lines from the fixture pack.
- `bless.rs`: the cast rules and the on/off state.
- Store round trip; protocol messages; the UI's lib.
