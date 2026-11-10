---
id: C-077
title: "Levers on doors: a locked gate a lever opens is routed through, and the walker pulls the lever when the gate refuses"
tier: B
size: S
wave: 16
depends: []
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/nav.rs (Edge::lever, found at graph load)
  - crates/wayfarer-world/src/route.rs (can_take, the door's cost, Step::lever)
  - crates/wayfarer-brain/src/walker.rs (the lever's detour on a refusal)
  - crates/wayfarer-brain/src/task.rs (why_unreachable)
  - crates/wayfarer-world/tests/nav.rs, crates/wayfarer-brain/tests/walk.rs
  - docs/kb/navigation.md, docs/kb/changelog.md
---

## Goal

Lexi had a rope and grapple, but Go to found no route to Chancellor Annora (1:1333). Every way
in goes through the Monastery's Inner Gate (1:1331), whose gate north to the Courtyard (1:1375)
is locked with `exitpar2` −300. Nobody can bash or pick it, so the router leaves it out. In the
game, a lever opens it: `pull lever` in the room east or west of the gate (1:1339, 1:1345).

When this card is done, a locked Door or Gate that a lever opens is a way through. The router
takes it. When the door refuses, the walker goes to the lever, pulls it, comes back and goes
through.

## Inputs

- `kb/navigation.md` "Remote Action": the phrase runs an action on an exit of the room the
  Remote Action leads to. `exitpar2` units are the exit number, tens the action. A non-zero
  `exitpar4` item must be carried. **On a Door or Gate the action swaps open and locked, both
  sides** (`LJNSUPP.C:1134-1410`).
- `kb/navigation.md` "Door / Gate": the bash and pick odds (`strength + exitpar2`,
  `picklocks + exitpar2`).
- The stock data (`tests/fixtures/world/`), scanned 2026-11-10:
  - 1,768 Doors and Gates; 289 can never be bashed or picked (`exitpar2` ≤ −100).
  - 22 have a lever (a Remote Action on them). In 18 the lever is in the door's own room, though
    one of those (1/1104) has a blank phrase message and can't be typed. In 2 it is one Normal
    exit away (the Inner Gate's north, 8/910's south). In 2 (7/150's southeast, 7/1173's
    east) the lever's room is reached only by a Hidden exit.
  - The Courtyard (1:1375) has its own lever for the gate back south. 1:1339 and 1:1345 each
    have one for the Inner Gate's north.
  - MegaMud's `MOIGMCOP.mp` goes through the Inner Gate this way: `n[e,pull lever,w]`.
- C-037 (Hidden exits, `Edge::unblock`), C-052 and C-055 (path windows: a Remote Action phrase
  as a route step).

## Behaviour

- **The graph knows a door's lever.** At load, each Door or Gate gets a `lever`:
  1. Find a Remote Action whose target is the door's room and exit (`exitpar2` ≥ 0, units =
     the door's direction) and that needs no item (`exitpar4` 0).
  2. The lever is the steps from the door's room to that Remote Action's room over Normal exits
     (at most `LEVER_STEPS`, 3), the phrase, then the steps back.
  3. If the Remote Action is in the door's own room, the lever is just the phrase.
  4. With several levers, the one with the fewest steps wins.

  A door with no such Remote Action has no lever.
- **The router.** A door whose odds are 0 can be taken when it has a lever. Its cost is the
  door's cost plus the lever's steps, which are walked only if it refuses. A door with odds
  above 0, or one that isn't locked, routes as before: the lever is a last resort, never the
  first try.
- **The walker.**
  1. The walker tries the door as it does now: it moves, and opens a door that is shut but
     not locked.
  2. If the door is locked and the character can't open it, but the door has a lever and the
     walker hasn't pulled it this walk, the walker puts the lever's steps into the route
     before the door step. It says "The gate north is opened by a lever: e, pull lever, w",
     walks them, then tries the door again.
  3. If the door refuses a second time, the walker blocks it for the session and plans again,
     as it does today for a door that can't be opened.

  The lever toggles the gate, so pulling it before a refusal could lock a gate someone else
  left open. Pulling only after a refusal avoids that.
- **Why no route.** `why_unreachable` no longer says "cannot be opened" about a door that has a
  lever.

## Design notes

- `Edge::lever: Vec<Edge>`: the detour's edges, the phrase as a Remote Action edge from its room
  to itself, as path windows have it. `Step::lever: Vec<Step>`, filled by `Step::through`.
- The walker's detour reuses what path windows do with a Remote Action step (`Typing ...`,
  `Expect::Prompt`, advance). A `levered: BTreeSet<(Key, u8)>` on `Walk` remembers which doors
  it has pulled the lever for.
- No pack change: the lever is worked out at `RoomGraph::load` from the exits already there.
- Left out: levers reached through a Hidden, Door or gated exit (7/150 and 7/1173), levers
  whose phrase is blank (1/1104), and levers that need an item (none in the stock data).

## Done when

- At load, the fixture world has 19 doors with a lever, 17 of them in the door's own room. The Inner Gate's north
  lever is `e`, `pull lever`, `w`. The Courtyard's south lever is `pull lever`.
- A traveller with strength 50 gets a route from the Inner Gate to the Courtyard, and from town
  to the Large Office (1:1333).
- A walk from the Inner Gate to the Courtyard: `n` is refused locked, then `e`, `pull lever`,
  `w`, `n`.
- If the gate is still locked after the lever, the walk blocks it and stops (there's no other
  way), with no second pull.
- If the gate is open, the walk goes `n`, with no lever.
- KB: `navigation.md` "Remote Action" gets the lever count from the stock data; a line goes in
  `changelog.md`.
- The checks pass.

## Tests

- `wayfarer-world/tests/nav.rs`: the levers at load (count, the Inner Gate's, the Courtyard's);
  routing with and without a lever; a door with odds above 0 routes without its lever.
- `wayfarer-brain/tests/walk.rs`: the three walks above.

## Notes for the agent

- Work in a worktree. Writing under `crates/` in John's checkout restarts his live brain.
- An optional live check (Lexi, the Inner Gate) can follow on the server; it isn't part of this
  card.
