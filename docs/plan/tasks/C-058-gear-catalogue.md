---
id: C-058
title: "The gear catalogue: every weapon and armour piece a character could ever use, why not now, and where it comes from"
tier: B
size: M
wave: 9
depends: []
crate: wayfarer-world
touches:
  - crates/wayfarer-world/src/gear.rs (new)
  - crates/wayfarer-world/src/lib.rs
  - crates/wayfarer-world/README.md
  - docs/kb/economy.md (only if the card confirms a fact the KB lacks)
---

## Goal

For a class and race, a list of every weapon and every piece of armour in the world, each
marked **usable now**, **usable later** (and what is missing: a level, an alignment, Strength)
or **never** (and why), with where it can be had: the shops that sell it and the price this
character pays, the monsters that carry it and how often, the quests that give it, the rooms it
is placed in. Plus the monster sets a planner measures weapons against: the monsters of a loop
or an area. It is the data side of the gear planner (C-059, C-060); `spellshop.rs` is the
model to follow (it does the same job for spells).

## Inputs

- `crates/wayfarer-world/src/spellshop.rs` (shop offers, prices, the nearest shop room),
  `monsters.rs` (spawn pools, bosses, `regen_hours`), `pack.rs` (`items_of_type`, `shop`,
  `text_block`, `classes`, `races`, raw records).
- `docs/kb/economy.md` "Carrying and wearing items" (who may use an item), "Prices", "Shops"
  (a shop buys and sells only what is on its list; `maxqty` 0 slots only resell),
  "Limited items"; `docs/kb/progression.md` "Unique monsters" (a never-killed limited monster
  carries all its items); `docs/kb/navigation.md` "Text blocks" (`giveitem`, `random`).
- The world fixtures (`tests/fixtures/world/`).

## Behaviour

### Who may use an item (`user_can_use`, `LJNUTIL.C` 9418-9740)

For a character of class K and race R, at level L, an item of type 0 (armour) or 1 (weapon):
- **Class list**: if the item names any class, it must name K. **Race list**: the same for
  R. Being named in either list overrides the weapon-type and armour-type checks.
- **Anti-Magic** (a class or race with ability 51): no item with Magical (28).
- **Weapons**: the class's weapon kind allows the item's type (`wpntype` 0 one-handed blunt,
  1 two-handed blunt, 2 one-handed sharp, 3 two-handed sharp): kind 0 → {0}, 1 → {1},
  2 → {2}, 3 → {3}, 4 any one-handed → {0, 2}, 5 any two-handed → {1, 3}, 6 any sharp →
  {2, 3}, 7 any blunt → {0, 1}, 8 all; 9 "Limited" allows only items 100 and 68.
- **Armour**: `armtype` at most the class's `armour`; classes of a one-handed weapon kind
  (0, 2, 4) may not use off-hand items (`worn` 12).
- **Item type 11** only for classes 1, 2, 3, 4 and 9.
- **Level**: Minimum level (ability 135) above L, or Maximum level (136) below it → *later*
  (or *never* for a maximum already passed).
- **Alignment**: Good (97), Evil (98), Not Good (110), Not Evil (111), Neutral (112)
  depend on the character's legal level, which wayfarer does not track: mark the item *later*
  with the alignment it needs, never *never*.
- **Strength**: a weapon above the character's Str is usable (each blow costs more, C-057);
  report the shortfall as a note, not a block.

Status: **now** (usable at L), **later** (only level or alignment stands in the way),
**never** (class, race, type, Magical, off-hand). Every *later* and *never* carries its reasons.

### Where an item comes from

- **Shops**: every shop slot with `maxqty > 0` (a `maxqty` 0 slot only resells what players
  sold). Price for this character: `cost × (100 + markup)/100 × (110 − Cha/5)/100`, each
  `/100` truncated, the markup first (`economy.md` "Prices"); the room nearest the character
  and the steps to it, as `spellshop.rs` does.
- **Monsters**: every monster whose `items[]` holds it, with `itempcts[]`; the monster's HP,
  largest hit, whether it is a boss (`maxalive` 1) and its `regenhrs`, and where it is (its
  `genmonster` or `specmon` rooms, else the rooms whose spawn pool holds it). A limited
  monster (`maxalive` > 0) that has never been killed carries every item on its list: flag
  "first kill drops it" (the pack has the stock `killdate`, 0 for all; say "on a server where
  it has never died").
- **Quests**: text-block lines with `giveitem <n>`. Skip lines of a `random` table (they start
  `NN:`; chests and gambling rewards), and report the room or monster the block hangs from
  when one can be found (a room's `cmdtext`, a monster's `talktext`, or the `link` chain to
  them).
- **Placed**: rooms whose `placed[]` holds it (back on a cold start).
- **Selling**: the shops that buy it (any slot, `maxqty` 0 included) and what the character
  gets: `cost` in copper `× (Cha/2 + 25)/100`.
- Flags: limited N (`limit`), deleted at cleanup (ability 119), Magical (28).

### Monster sets

- **A loop**: the monsters of the spawn pools of its stop rooms (`Monsters::spawn_pool`),
  without duplicates, bosses kept, each with how many stops can spawn it.
- **An area**: the rooms of one name (or a name prefix) and their spawn pools.
- **By name**: a list of monster names; unknown names reported.

## Design notes

- `gear.rs`: `Usability { status, reasons: Vec<Reason> }`, `Reason` an enum (`Class`, `Race`,
  `WeaponType`, `ArmourType`, `Magical`, `OffHand`, `Type11`, `Level(n)`, `MaxLevel(n)`,
  `Alignment(kind)`), `Source` an enum (`Shop { … price, steps }`, `Drop { monster, percent,
  boss, regen_hours, first_kill, rooms }`, `Quest { block, at }`, `Placed { room }`),
  `Sale { shop, price }`, `GearItem { number, name, kind (weapon/armour), slot, usability,
  sources, sales, limited, deleted_at_cleanup, magical, str_short }`.
  `Catalogue::build(pack, class, race)` once per class and race (it reads every item, about
  2,000); `catalogue.for_level(L, cha, from)` fills in the level checks, prices and steps.
- Build it once per (class, race) and keep it: the planner asks often. Building it must stay
  under 200 ms on CI (assert it, with a comment saying why).
- Leave out: alignment from evil points, item `level` (never read by the game), keys and other
  item types.

## Done when

- The fixtures below come out as stated.
- A whole catalogue builds within the time limit.

## Tests

From the fixtures (all [C] in this conversation, 2026-10-26):
- Kang Witchunter: **mithril cutlass** now at L10 (named class, placed in Large Tomb 1:2236),
  **Magebane sword** later (Level 15) with a quest source (block 399), **bone-handled
  headcutter** now with a drop source (minotaur champion 25%, boss, first kill), **golden
  broadsword** never (Magical, the Witchunter's Anti-Magic).
- Dwarf Cleric: **black flail** (placed 1:2258); **huge darkwood club** now at L10 (Thrag,
  5%), Str short 20 at Str 80; **katana** never (sharp weapon, any-blunt class).
- Half-Elf Bard: **jeweled main-gauche** now at L10 (off-hand, but the class list names the
  Bard); **curved bone dagger** later (Alignment: Evil).
- Gnome Druid: **woodland robes** later (Level 20), placed in 3:214.
- **gilded robes** sell only at Jorah's Robes (1:3303) and Harkul's Haberdashery (6:1342);
  at Charm 75 for 11,160 copper. **gold ring** sells only at the Recycler Shop
  (1:168, 1:2337), 62,000 copper at Charm 75.
- A shop price: scalemail tunic at Skali's (1:306) for a Charm-75 buyer: 4,750 copper.
- A monster set for the fixture loop `NARNLOO2.mp` (the Newhaven Arena) holds the Arena's
  spawn pool and no monster twice; the area set "Slum Street" holds the half-ogre bodyguard
  and the orc lieutenant.

## Notes for the agent

- The alignment-locked items are many; don't hide them: *later*, with the alignment named.
- KB notes: confirmed facts about `user_can_use` or item sources that the KB lacks go to
  `docs/kb/economy.md` with [C], and a line in `docs/kb/changelog.md`.
