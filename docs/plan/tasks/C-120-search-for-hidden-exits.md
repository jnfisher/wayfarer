---
id: C-120
title: "A Go to searches for a hidden exit when that is the short way"
tier: A
size: S
wave: 17
depends: [C-077, C-118]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/route.rs (Traveller::perception, search_odds, Costs::search_cost, route_or_search)
  - crates/wayfarer-world/src/nav.rs (Edge::needs_search, lever_way)
  - crates/wayfarer-brain/src/position.rs (perception from the status screen)
  - docs/brain.md
---

## Goal

Riley's Go to the dark bishop (7:1227, the Black Fortress) walked some 290 steps west out of
the slums, over the Western Road and down the Dragon's Teeth tunnels, when the way under the
Black House and up the Grand Stair is some 240. The planner took unknown hidden exits only as
a last resort, when no route by known exits existed, and the short way needs one: the Grand
Stair's gate (7:150, `exitpar2` -250, no strength bashes it) is opened by a lever in a Guard
Room (7:152) three steps off behind a hidden exit the character has to search for. John:
"any player can eventually find a hidden exit; searching isn't a big problem and going west is
horrible." A search is a dice roll like a bash at a locked door, which the planner already
prices, so hidden exits are priced the same way instead of excluded.

## Inputs

- `docs/brain.md` "Walking": "Hidden exits", "Locked doors and the character", "Levers
  elsewhere" (C-118).
- `docs/kb/navigation.md` "Hidden": `search` finds an `EXF_NEEDSEARCH` exit on
  `genrdn(0,100) < max(3, perception - 15)`; "Remote Action".
- `crates/wayfarer-world/src/route.rs`: `Costs::door_cost`, `Traveller::can_take`,
  `route_or_search`; `nav.rs`: `find_levers`, `normal_way`.

## Behaviour

- A walk the player asked for (a Go to, an errand, the go box) plans over the pack's Hidden
  exits too. One a search finds costs `hidden` (2) per expected search, `2 * ceil(100 /
  max(3, perception - 15))`: four at perception 65, 68 at the floor, two while the perception
  is not known. One that shows, or is passable, costs nothing extra; one opened by its room's
  phrases costs one search's worth, as before.
- A route through known exits is no longer preferred as such: the cheapest route wins,
  searches included. A loop's legs and the loop list keep to the exits its path knows.
- The way to a door's lever may run through a hidden exit a search finds (or a passable one);
  the walker searches for it on the way as for any hidden step. The lever path's cost counts
  its searches.
- The brain reads `Perception` from the status screen's race line into the traveller.

## Design notes

- `Traveller::perception`, `Traveller::search_odds`, `Costs::search_cost` (mirrors
  `door_cost`), `Traveller::hidden_cost` and `lever_cost`; `route_or_search` plans once with
  `Category::Hidden` allowed instead of falling back to it.
- `Edge::needs_search`; `RoomGraph::lever_way` replaces `normal_way`: Normal exits and ungated
  Hidden exits that are passable or need a search, within `LEVER_STEPS` (3, unchanged).
- No new setting: `movement.search_limit` still bounds one exit's searches.

## Done when

- A Riley-like traveller's `route_or_search` from the Temple (1:2189) to the Elaborate
  Chamber (7:1227) goes through the Black House foyer (1:1313) and the Grand Stair's gate,
  whose step carries a seven-step lever path with a hidden exit from 7:148; `route` (a loop's)
  goes by the Western Road (2:41) and is longer by more than 30 steps.
- The Grand Stair's gate and the Black Fortress gatehouse's gate carry levers (21 doors with
  levers, 17 in their own room).
- `Costs::search_cost` and `Traveller::search_odds` as above.
- `docs/brain.md` says so; CI green.

## Tests

- `crates/wayfarer-world/tests/nav.rs`: `a_go_to_searches_for_a_hidden_exit_when_that_is_the_short_way`,
  the lever count in `a_locked_door_a_lever_opens_carries_the_lever`.
- `route.rs` unit test: `a_hidden_exit_costs_a_search_per_expected_try`.

## Notes for the agent

- The clumsy case (3 %, 68 steps' worth of searches) still goes under the Black House on this
  data: the way round costs more than that too. The test says so rather than asserting a flip.
- The walker already searches for any hidden step not showing and walks lever paths when the
  door refuses (C-077), so no walker change.
