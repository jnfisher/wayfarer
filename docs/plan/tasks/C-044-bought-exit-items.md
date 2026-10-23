---
id: C-044
title: "Exits that need a bought item: the walker buys the room ticket (or rope, skiff, key) on the way"
tier: B
size: M
wave: 6
depends: [C-037]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/{exit_items.rs (new),route.rs,lib.rs}
  - crates/wayfarer-brain/src/{brain.rs,walker.rs,session.rs,shopping.rs}
  - docs/brain.md "Walking", "Buying spells"; docs/kb/navigation.md; docs/kb/changelog.md
---

## Goal

An Item or Key exit whose item a shop sells is no longer a wall. A walk whose best way goes
through one goes to the nearest shop that sells the item first, buys one, and carries on. A
room ticket is used up at the stairs, so each trip up buys a new one. The spell plan counts the
Bard Song Store upstairs at the Homely Hearth as reachable. The shopping errand skips the bank
when the cash on hand covers everything it can reach. (Added 2026-10-23 at John's request: "room
ticket is like a key, that should be something the brain can find how to acquire and access".)

## Inputs

- `docs/kb/navigation.md` "Item" (exit type 8: `exitpar1` the item, `exitpar2` the message
  without it, `exitpar3` the message with it; one use of the item is spent per pass) and "Key".
- The pack: `items.raw` `charges` (1 for a room ticket, -1 unlimited for a rope and grapple,
  a wooden skiff, a mine pass); `shop_stock` and the rooms of each shop (room type 1,
  `shop_num`); the item's `cost` in `currency`.
- C-037 and #98 (`Traveller::items`, `Traveller::keys`, `Gate::Item`, `Gate::Key`, the walker's
  `use <key>`), `wayfarer_world::spellshop` (shop rooms and prices), `shopping.rs`.

## Behaviour

- **Which items**: the items an Item or Key exit asks for that some shop stocks (`max_qty`
  above 0) in a shop room of the pack. Loaded once with the atlas: number, name, charges,
  base price in copper, the shop rooms.
- **Routing**: a walk (`walk_to`, Go to, errands, a loop's legs, a rejoin) plans as now. When a
  way through a sold item's exit exists that the character can't take yet, it also plans the
  way through the shop: from here to each shop room selling the item, then on from there
  holding it. It takes the shop way when it is the only way or shorter than the plain route.
  One bought item per plan; a second one is bought when the walk replans after the first.
- **Narration**: "Walking to Bard's Room: 23 steps, buying a room ticket at The Homely Hearth,
  Common Room".
- **Buying**: in the shop room the walker sends `buy <item name>`. "You just bought ..."
  (`Bought`) counts the item as carried; a refusal (`BuyRefused`) or no answer stops the walk:
  "Stopped walking: could not buy a room ticket: cannot afford it".
- **Used up**: after a confirmed move through an Item exit whose item has one charge, the item
  is no longer counted as carried (the game took it). The next `i` puts it right either way.
- **Moves ahead** (pipelined legs) are not sent past the buy.
- **Spell plan steps**: a shop beyond a sold item's exit counts as reachable, in steps as if the
  item were held (the detour to buy it is not counted).
- **Shopping errand**: scrolls at a shop with no way there are left at once ("song of lore (no
  way to Bard Song Store)") without a walk, and the bank is visited only when the cash on hand
  is short of what the reachable scrolls cost.

## Design notes

- `wayfarer_world::exit_items`: `ExitItem { number, name, charges, price, shops: Vec<Key> }`
  and `load(pack, wanted) -> BTreeMap<i64, ExitItem>` (only items with a shop room).
- `wayfarer_world::route`: `route_buying(graph, from, to, t, items, within) ->
  Option<(Route, Option<Purchase>)>`, `Purchase { at, item, name, shop }` (`at`: the step index
  before which to buy, the route's step at `at` leaves the shop room), and
  `distances_buying(graph, from, radius, t, items)`. The world crate keeps the routing; the
  walker only carries out the purchase.
- The walker: `Doing::Buy`, `Walk::purchase`, `Walker::heard(&GameEvent)` (the session feeds
  it each event, like the errand's `on_event`).
- No price check before the walk: the shop says if the money is short.

## Done when

- In-process: a walk to a room behind a room-ticket exit goes to the shop, buys a ticket, takes
  the stairs, and arrives; a second walk up buys another; a refused buy stops the walk; a
  rope-and-grapple exit uses a rope already carried and doesn't buy a second one.
- Live: Manuel's **buy spells** goes to the Homely Hearth, buys a room ticket, goes up to the
  Bard Song Store, buys the songs the money covers and reads them.

## Tests

- `route.rs`: the shop way when it is the only way; the plain way when it is shorter; nothing
  bought when the item is held; `distances_buying` reaches beyond the exit.
- `walker.rs` (in `tests/exit_items.rs`): buy, then the stairs; a refusal stops the walk; a
  one-charge item is dropped from the carried items after its exit, so the next walk up buys
  another; a carried ticket is used and none bought. No move goes ahead past the buy (a guard in
  `propose_ahead`; only a loop's legs pipeline, and no test loop climbs the stairs).
- `shopping.rs`: an unreachable shop's scrolls are left without a walk and don't send the
  errand to the bank.

## Notes for the agent

- Key exits use the same path: a sold key is bought and then `use`d as now.
- Selling the ticket back, or keeping a spare, is out of scope.
