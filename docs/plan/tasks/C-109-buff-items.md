---
id: C-109
title: "Buff items: a carried item's lasting spell is a blessing, swapped in to use"
tier: A
size: M
wave: 16
depends: [C-047]
crate: wayfarer-brain
touches:
  - crates/wayfarer-world/src/spells.rs (buff items from the pack: `BuffDef::item`)
  - crates/wayfarer-brain/src/bless.rs (the swap: look, wield or wear, use, put back, look)
  - crates/wayfarer-brain/src/session.rs (carried buff items offered as blessings)
  - crates/wayfarer-brain/tests/spells.rs, crates/wayfarer-world/tests/spells.rs
  - docs/brain.md, docs/kb/spells.md, docs/kb/changelog.md
---

## Goal

Riley carries a shimmering greatsword: `use` casts sword major bless, +10 AV on the whole
party for 60 rounds, for 8 mana, as often as she likes. The game lets a weapon be used only
while wielded ("You must have that item readied to use it!") and a worn item only while worn
("You must be wearing that item to use it!", LJNCMDB.C:191-214), so the player wields it,
uses it and wields the headcutter again by hand. John: "it would be great if wayfarer detected
if a player has such an item and would allow it as a buff option ... if the buff is enabled in
settings but I'm not wielding the weapon, it should wear the weapon, use the weapon, and then
wield what I was using before (eg so I can carry it around as a buff stick)" (2026-11-17).

## Behaviour

- **Detection** (from the pack): an item of unlimited charges (`charges` < 0), a weapon or a
  worn item, whose One Time Cast (ability 43) used by hand casts a lasting spell on its user or
  its party: the spell is general (type 3), has a duration and targets 1, 2, 6, 10 or 13. Used
  by hand, the item's first Auto Use (114) ends its ability list (`kb/spells.md`), so a cast
  after one is the weapon's on-hit spell, not its use. In the stock data: shimmering
  greatsword (sword major bless), large silvery cross (cross of vengeance), emerald-tipped
  crozier (divine intervention), cosmic staff (cosmos spell).
- An item the character carries, whose class list (if any) has the character's class, whose
  Minimum Level (135) it has reached, and whose spell's magery, if any, is the class's at a
  high enough magery level, shows in the Blessings panel next to the spells, as "sword major
  bless (shimmering greatsword)" with the spell's mana. Its key is the item's name.
- Ticked, Bless keeps it on as it keeps a spell: with the mana to spare, between fights only
  (never in a fight, even with `spells.bless_in_combat`). It is on from a use that was not
  refused, for the spell's rounds; off at its wear-off line or a death. It is not taken as on
  from the line its target sees, nor checked on the status screen: sword major bless shares
  bless's lines with chant and bless, which the party may cast.
- **The swap**: the bot looks at its inventory (`i`); if the item is not wielded (a weapon) or
  worn (anything else), it notes what is in that place (the weapon in hand, or what the wear
  takes off: "You have removed copper cross."), wields or wears the item, uses it, wields or
  wears the noted item again, and looks at its inventory again. With the item already in
  place it only uses it. A two-handed buff weapon with something worn in the off hand, or an
  off-hand buff item with a two-handed weapon in hand, is not tried: held for five minutes,
  saying why. A refused wield, wear or use (class, level, alignment, mana) counts as a failed
  cast: tried again after a round, held after three. The put-back goes even when the use
  failed. A fight that starts during the swap goes first; the swap goes on after it.
- A brain restart or a death in the middle of the swap does not put the old item back (the
  note is in memory only).

## Done when

- A Paladin carrying a shimmering greatsword sees "sword major bless (shimmering greatsword)"
  among its blessings; one not carrying it does not; a Warrior carrying it does not.
- Ticked, the bot sends `i`, `wield shimmering greatsword`, `use shimmering greatsword`, `wield
  bone-handled headcutter`, `i`, and does not use it again until it wears off.
- Wielded already, it only uses it.
- A wear that takes off the copper cross puts the copper cross back on.
- A two-handed buff weapon with an off-hand item worn is not tried.
- A refused use puts the weapon back and tries again later.
