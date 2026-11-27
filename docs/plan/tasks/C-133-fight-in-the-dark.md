---
id: C-133
title: "Never stand still and die in the dark: the bot attacks what hits it by name in a globe of darkness, runs when the attacker is out of reach, and treats a damaging monster spell as an attack"
tier: A
size: S
wave: 23
depends: [C-108, C-091]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/fight.rs (`cursed`, `blind_check`, `blind_said`, `cornered`; attack by name in the dark; `take_cornered`)
  - crates/wayfarer-brain/src/modes.rs (`Presence::event` reads a damaging `spell_cast_on_you` as an attack; `attacker_name`; `Bot::cornered`)
  - crates/wayfarer-brain/tests/fight.rs (three tests)
  - docs/kb/recovery.md "Light and darkness", docs/kb/changelog.md, docs/brain.md "Fight here"
---

## Goal

John (silver-mirror, 2026-11-27): "the bot never just stands still and gets killed in
darkness." Rocco and Sasha were hunting the nanati, whose shadows cast a globe of darkness
(spell 440: Alter User Light −9999 for about a minute, the room goes "pitch black"). In the
dark the two bots only sent `look` over and over while a nanati sorceress bolted them from the
same room; each took a minute of 40-80 damage hits and died. Two faults: (1) in the dark the
bot waited and looked instead of attacking what it could still name, and (2) the sorceress,
which only cast and never clawed, was never an attacker, because her spell lines were not read
as attacks, so even a working attack-in-the-dark would not have gone for her.

## Inputs

- `docs/kb/recovery.md` "Light and darkness": the globe of darkness is a cast light penalty,
  not a curse; nothing dispels it; a monster that is in the room can still be attacked by name
  in the dark (`bash shadow` engages). The caster line `Nanati shadow fires a darkbolt at you
  for 6 damage!` parses as `spell_cast_on_you` with `caster` and `dmg`.
- `crates/wayfarer-brain/src/fight.rs`: the blind handling for a curse (C-108), the model to
  split from; `choose`/`pick`/`target`; the stalemate flee (`run_from`).
- `crates/wayfarer-brain/src/modes.rs`: `Presence::event`, `seen_fighting`, the attacker list;
  `Bot::stalemate` as the model for `Bot::cornered` (flee whatever the HP; party and boss
  exceptions).

## Behaviour

- **In the dark** (not a curse): the bot attacks a monster it can name (one that is `here` or
  an attacker), even while a `look` still shows only the darkness. An attack that comes back as
  speech (`You say "bash shadow"`: nothing of that name is reachable) is counted; after two
  such the name is left alone until it sees again.
- **Cornered**: blind and still hit while every attacker's name has been refused twice (nothing
  it can name is here), the bot leaves as a stalemate does, whatever the HP. A follower in a
  party stays with it; a boss fight is never left.
- **A curse** ("You are blind.") keeps its old behaviour (C-108): it sees nothing at all, waits
  and looks until it lifts, and does not attack by name or flee on this account.
- A monster's damaging spell (`spell_cast_on_you` with `dmg` > 0) makes its caster an attacker,
  by the monsters' own name for it, so a caster that never lands a blow is fought and fled from.

## Design notes

- `Fight`: `cursed` (a "You are blind." curse, the old wait-and-look path), `blind_check` (look
  once after the first speech to learn curse vs dark), `blind_said: BTreeMap<String, u32>` (the
  names refused as speech), `cornered`/`cornered_told`. `pick` adds a name refused
  `BLIND_SAID_TRIES` (2) times to the skip; `take_cornered` hands the name to the bot.
- `Presence::event`: a `spell_cast_on_you` with non-zero `dmg` pushes the caster onto
  `attackers`. `attacker_name` gives the caster as the monsters' data names it (the line
  capitalises it).
- `Bot::cornered` runs after `fight.step` in the Fight arm, like the stalemate.

## Done when

- In a globe of darkness the bot sends `bash <monster>` and engages, instead of only looking.
- Hit by a caster it cannot reach, it leaves the room whatever the HP.
- A sorceress that only bolts the character is fought.

## Tests

- `tests/fight.rs`: in the dark it attacks what hits it by name; blind and hit by what it
  cannot reach it leaves; a monster's damaging spell makes the caster an attacker.
