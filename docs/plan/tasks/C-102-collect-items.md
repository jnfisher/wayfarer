---
id: C-102
title: "Collect items: pick up listed items until the character carries a count of each"
tier: B
size: M
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/loot.rs (items before coins)
  - crates/wayfarer-brain/src/modes.rs (`View.items`, `View.collect`, `items_carried`)
  - crates/wayfarer-brain/src/character.rs (`You took` counts as carried)
  - crates/wayfarer-brain/src/session.rs (`set_collect_items`, snapshot, persist)
  - crates/wayfarer-brain/src/brain.rs (`Atlas.gettable`, dispatch)
  - crates/wayfarer-world/src/pack.rs (`gettable_items`)
  - crates/wayfarer-proto/src/lib.rs (`CollectItem`, `Bot.collect_items`, `set_collect_items`)
  - crates/wayfarer-store/src/lib.rs (migration 14)
  - ui/src/SettingsPanel.svelte, ui/src/App.svelte, ui/src/lib/gateway.ts
  - docs/brain.md ("Collecting items"), docs/web-ui.md, docs/protocol.md
---

## Goal

A character picks up the items the player listed for it, off the floor where it stands, until
it carries a count of each: quest items, and spares to hand out.

John (2026-11-13): "I'd like Rocco to collect up to 4 darkwood ring items to be able to equip
everyone to get to the high druid. Need this in settings on a per-character basis. other
examples of quest items to collect are like.. black diamonds for the stealth quest." He agreed
the design below, and that worn ones do not count.

## Inputs

- Loot picked up only coins. The room view's other items were already parsed
  (`Presence.floor_items`), `You took <item>.` was an event (`ItemTaken`), and `i` gave the
  carried and worn items with counts (`Character.items`).
- The world pack has every item with `gettable` and `weight` in its record: 1,497 of 1,950 can
  be picked up (darkwood ring weighs 10, black diamond 20).
- Per-character lists do not fit the settings' kinds (Int, Bool, Choice), so the list is kept
  with the bot's record, as the attack spell is (C-099).

## Behaviour

- `set_collect_items {session, items: [{name, count}]}` sets the whole list. Each name is
  matched, any case, to an item a character can pick up and kept by the game's name; one that
  is none is refused with a warning naming up to three items that hold its words; a count of
  0 drops it. A notice says the list ("Collect: darkwood ring (up to 4)").
- Stored with the bot's record (store migration 14), shown as `bot.collect_items`.
- Loot, out of a fight with nothing attacking, takes a listed item on the floor of the last
  room view before coins: `get <name>`, one at a time, while the character carries fewer than
  the count ("Loot: picking up darkwood ring (2/4)").
- Carried counts what `i` showed with no slot, plus each `You took` since; worn or held ones
  do not count.
- One more must fit under `cash.max_encumbrance` by the item's weight (any weight when the load
  is unknown). `You cannot carry that much!` ends every try, items and coins, for the room
  view; a `get` is tried twice at most per view.
- It never walks for an item and does not search.
- The settings tab shows **Collect items** under "Carry coins up to": each item with a count
  box and ×, and a name and count to add. Names are typed and checked by the brain (no
  catalogue goes to the UI).

## Done when

- `tests/loot.rs`: `a_listed_item_is_picked_up_after_a_kill`,
  `it_takes_items_up_to_the_count_not_counting_the_one_it_wears` (3 carried and 1 worn of 4:
  one of two taken, then the coins), `a_name_that_is_no_item_is_refused_and_the_rest_kept`.
- The store keeps the list; the proto round-trips `set_collect_items` and `bot.collect_items`.
- `ui/tests/settings-panel.test.ts`: the row sits after "Carry coins up to" and sends the whole
  list on add, a new count and ×.
- `docs/brain.md`, `docs/web-ui.md` and `docs/protocol.md` say so.
