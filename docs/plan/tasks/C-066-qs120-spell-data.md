---
id: C-066
title: "1.20 spell data: the player-spell table, the regen line with its scrolls and shops, the monster copies left out; the spellbook reads by rule set"
tier: B
size: M
wave: 12
depends: [C-064]
crate: wayfarer-world, wayfarer-brain, wayfarer-parse
touches:
  - crates/wayfarer-world/data/quicksilver-1.20/{spells.csv,regen.csv,texts.csv} (new), PROVENANCE.md there
  - crates/wayfarer-world/src/{spells.rs,spellshop.rs}, crates/wayfarer-world/src/qs120.rs (new)
  - crates/wayfarer-brain/src/{spellcraft.rs,session.rs,shopping.rs}
  - crates/wayfarer-parse/src/registry.rs (the regen-line texts)
  - docs/kb/quicksilver-1.20.md, docs/kb/changelog.md
---

## Goal

On a 1.20 server, wayfarer plans with 1.20's spells: Spell Mod v2.1's values for Mage, Priest and
Druid, protective aura and dark binding, the 65 level caps, mana flux at 35, and the new regen
line (6000-6020) with the scrolls that teach it and the shops that sell them. The monster copies
(5000-5999) never reach a player's plan. On 1.11p and unknown, the stock pack's spells are used,
exactly as today.

## Inputs

- `docs/plan/quicksilver-1.20/handoff.md` section 4 (all of it: 4.1 what changed, 4.2 the regen
  line and its shops, prices, stock and tier rules, 4.3 the monster copies, 4.4 the scrolls, 4.5
  the CSV and its column key) and 3.8 (the regen-line cast, room, fade and status texts).
- `crates/wayfarer-world/src/spells.rs` (`Spells::load` from the pack, `SpellDef`, `BuffDef`,
  ability numbers), `spellshop.rs` (scrolls shops stock, who can read them),
  `crates/wayfarer-brain/src/spellcraft.rs` (the spellbook matched from the `spells` screen).
- C-064's rule set.

## Behaviour

- **The data.** The handoff's 4.5 CSV is copied byte for byte into
  `crates/wayfarer-world/data/quicksilver-1.20/spells.csv` (it is shipped data, not a test
  fixture: the brain reads it at run time, embedded with `include_str!`). The regen line's table
  of 4.2 (number, family, tier, regen %, scroll item, shop number and name, price) goes in
  `regen.csv`; the 3.8 texts in `texts.csv`. A `PROVENANCE.md` beside them names the handoff
  section each file came from.
- **`Spells` by rule set.** `Spells::load(pack)` stays the 1.11p view. A 1.20 view is the pack's
  spells with every row of `spells.csv` replacing the spell of the same number (or added, for
  6000-6020), and every number 5000-5999 removed. 825, 1379 and 1380 get cap 65. The spellbook
  (`spellcraft.rs`) uses the view of the session's current rule set and is rebuilt when the rule
  set changes (the `spells` screen often comes before detection ends).
- **Names.** `detect magic` becomes `protective aura` and `dark plague` becomes `dark binding` in
  the 1.20 view; the 1.11p view is untouched. Item 121's name in the 1.20 view is `scroll of
  protective aura`.
- **The regen line** is in `BuffDef` with its family and tier, so C-067 can keep the highest tier
  up and knows that tiers of a family replace each other (and that an upgrade takes two casts).
  Serenity and profane link are both tier 35 of the Priest family and don't replace each other.
  Paladins and Rangers get no tier.
- **Scrolls and shops.** In the 1.20 view the 24 new scrolls (items 5100-5123) are stocked by the
  shops of 4.2, at the prices and stock given there, with the shop markups as 1.11p applies them.
  Any class can buy one; only "who casts it" can learn it (`spellshop.rs`'s read check). A shop
  that is sold out is a known state: the plan must allow skipping a tier or waiting (the shopping
  code shouldn't loop on a sold-out scroll).
- **Mana regen estimate.** The 1.20 view's mana-per-hour estimate adds the known regen tier's %
  to the Alter Mana Regeneration rate (handoff 4.2 "What the bonus does"); the race bonuses are
  C-069's.
- **The regen-line texts** (3.8) are registered as 1.20 patterns (priority 40, `rules: 1.20`): the
  cast, room, fade and status lines of each family, as typed buff-on / buff-off events naming the
  family. Mana flux's start text becomes its cast text on 1.20.

## Design notes

- Keep the CSV's columns as named in the handoff; parse with the `csv` crate if the workspace
  has it, else a small splitter (the only quoted field is the empty `""`; check that).
- `qs120.rs` holds the overlay and the regen tables; `spells.rs` gets a constructor taking the
  rule set, not a global switch.
- Out of scope: casting behaviour (C-067), damage scaling by the casting stat (C-069).

## Done when

- On 1.20, a level-20 Mage's spellbook lists arcane attunement and protective aura with 1.20's
  numbers; the same character on 1.11p lists detect magic and mana flux at 16, as today.
- No spell numbered 5000-5999 appears in any player view.
- A Gypsy can buy a scroll of minor attunement and learn it; a Paladin can buy one and is not
  offered it in a plan.
- Every row of `spells.csv` loads; the counts match the handoff (219 Mage/Priest/Druid spells, of
  which 132 changed, plus the regen line).
- `docs/kb/quicksilver-1.20.md` has a "Spells" section ([Q]); a changelog line.
- fmt, clippy and nextest pass.

## Tests

- The CSV round trip: every row parsed, the `a/b` and `-` columns read, abilities split.
- The 1.20 view vs the 1.11p view for magic missile, mana flux, detect magic / protective aura,
  serenity and one of 825/1379/1380.
- The monster-copy range excluded.
- Shops: each regen scroll found at its shop with its price and markup.
- The regen-line texts classify as buff events, on a 1.20 rule set and as patterns on any.
- The spellbook rebuilt after a rule-set change.

## Notes for the agent

- The handoff's "changed: ..." column is information only; don't drive behaviour from it.
- The monster copies matter only for the threat model: a monster's spell does what it did in
  1.11p, so monster-side numbers stay the pack's.
