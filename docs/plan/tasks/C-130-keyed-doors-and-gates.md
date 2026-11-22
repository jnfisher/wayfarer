---
id: C-130
title: "A locked door or gate with a key in `exitpar4` is routed through and opened with that key"
tier: A
size: S
wave: 20
depends: [C-091, C-118]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/nav.rs (a Door or Gate's `exitpar4` is a key: `Gate::Key`)
  - crates/wayfarer-world/src/route.rs (`lock_odds`, `can_take`: a held key is certain)
  - crates/wayfarer-brain/src/walker.rs (`use <key> <dir>` at a keyed door, then `open`)
  - crates/wayfarer-brain/src/task.rs (`why_unreachable` names the key)
  - crates/wayfarer-world/tests/, crates/wayfarer-brain/tests/walk.rs
  - docs/brain.md "Walking" (Locked doors and the character, Keys and items), docs/kb/navigation.md, docs/kb/changelog.md
---

## Goal

Lexi (Druid, level 22+) holds a skeleton key (her `i`: "skeleton key, chrome key, dragon key,
black serpent key, black star key"), yet Go to from the Rhudaur Thieves' Guild (2/2561) to the
Shadow Fist Temple's Meditation Chamber (9/930, the dread mystic) says "no route"
(silver-mirror, 2026-11-22). The way is twenty rooms: the guild's hidden exit north, the
Smelly Dump's `go manhole`, the Rancid Sewer, and at 9/873 a **Gate** east with parameters
`(2, -200, 0, 976)`: locked, a bash at Str − 200, no relock delay, and item 976, the skeleton
key, in `exitpar4`. The MegaMud path Rhucshad types `use skeleton key e` there.

The router only knows a key on a **Key** exit (type 2, `exitpar1` the item). A Door or Gate
gets `Gate::None`, its lock state from `exitpar1`, and odds from `Str + exitpar2` and
`picklocks + exitpar2`: nil here for anyone, so `can_take` refuses it before any gate is
looked at, and "why" says "the gate east of Rancid Sewer cannot be opened". The stock data has
93 such exits (74 doors, 19 gates): the slums' Western Door (rusty iron key), the jail cells,
the Lava Fields' door to the Lake of Fire (basalt key), the Shadow Fist's Carved Doorway
(wire key), the Frozen Cavern (glass key) and this gate.

When this card is done a locked door or gate whose `exitpar4` names a key is a keyed door: in a
route whenever the character holds the key, opened with it, and named by its key when not.

## Inputs

- The game's rule, quicksilver `crates/silvermere-engine/src/items/use_item.rs` `use_key`
  and `movement/doors.rs` (mirroring `LJNCMDA.C`): `use <key> <dir>` fits a Door or Gate when
  `exitpar4 == key` (a Key exit when `exitpar1 == key`); a key that fits a door that is not
  locked says so and counts as used; a fitting key unlocks this side **and the far side when
  its exit leads straight back**, both to state 1 (shut, unlocked), and queues the relock after
  `exitpar3` steps, at least 1 (a Key exit relocks after its `exitpar4`). The door is then shut:
  `open <dir>` before the move, as for any shut door.
- `docs/kb/navigation.md` "Door / Gate" (no `exitpar4` yet) and "Key" rows; `kb/pitfalls.md`.
- `crates/wayfarer-world/src/nav.rs` around `let gate = match kind` (Key → `Gate::Key`, Door
  and Gate → `Gate::None`) and `let door = match kind` (`LockState::from_param(p[0])`).
- `crates/wayfarer-world/src/route.rs`: `lock_odds` (Door or Gate only; bash and pick),
  `door_odds`, `can_take` (odds 0 with no lever → false, before the gate match;
  `Gate::Key { item }` → `allows(Category::Key) || keys.contains(&item)`), `holds`, `holding`
  and the shop purchase of a key an exit asks for (brain.md "Items a shop sells").
- `crates/wayfarer-brain/src/walker.rs`: `key_for` (`Gate::Key` and the key held), the
  `(Doing::Move | Doing::Open, Refused::Locked) if step.exit_type == ExitType::Key` arm
  (`Doing::UseKey`, `MAX_KEY_TRIES` 2, then `Doing::Open`), the bash arm for doors
  (`bash_limit`, `hopeless`: "The gate east is beyond my strength (needs more than 200)").
- `crates/wayfarer-brain/src/task.rs` `why_unreachable`: "cannot be opened" / "would not
  open" / "needs the <key>" texts.
- `crates/wayfarer-brain/src/session.rs`: `traveller.keys` from `G::Keys` (the `You have the
  following keys:` line of `i`), `traveller.items` from the inventory.
- The route, for the test graph: 2/2561 n(Hidden, needs search) 2/2560, w 2/2538, s 2/2539,
  `go manhole` (Text) 2/2602, s (Map Change) 9/896, w w w 9/871, s s 9/873, **e (Gate, key
  976)** 9/909, e e 9/911, e (Door, state 2, bash +0) 9/912, u 9/913, s 9/914, w (Door, state
  2, bash −10) 9/915, s 9/918, w (Door, state 2, bash −10) 9/929, w 9/930.

## Behaviour

- A Door or Gate with `exitpar4` not 0 is a **keyed door**. Locked, it is in a route when the
  character holds the key (certain: one extra step, as a shut door), or, without it, when a
  bash or a pick has odds above 0 as today. With neither it is left out and the reason names the
  key: "the gate east of Rancid Sewer needs the skeleton key" (the Key exit's wording), not
  "cannot be opened".
- At a keyed door the game refuses as locked (`The gate is closed!`, or `The gate is locked.`
  after `open`), the walker holding the key sends `use <key name> <dir>` (`use skeleton key e`,
  the MegaMud form; the game takes the direction), at most twice, then `open <dir>`, then the
  move; `You unlock the gate.` (or the not-locked line) confirms. Without the key it bashes as
  today when the odds allow, and gives up at once with the "beyond my strength" line when they
  do not.
- The far side unlocks with this side, so the belief's door states (`traveller.doors`) mark
  both as shut-unlocked; the relock after `exitpar3` steps (1 for this gate) means the state
  goes back to locked as soon as the character has moved on, so the pack's state is kept for
  planning the way back and the key is used again there.
- A keyed door whose key a shop sells (the skeleton key: Rhudaur Thieves' Guild, shop 82, 25 in
  stock) is routed through by way of that shop as a Key exit's key already is ("Items a shop
  sells", brain.md: the walk plans through the nearest shop selling it and sends `buy <item>`
  there); the same rule with no new setting. The skeleton key keeps its charge, so one buy
  serves every later trip.
- The Go to preview, the loop generator and the party's "can it get there" all go through the
  same `can_take`, so Lexi's Go to shows the route and its length.

## Design notes

- `nav.rs`: for `ExitType::Door | ExitType::Gate` with `p[3] != 0`, `gate = Gate::Key { item:
  p[3] }`; `door` stays `LockState::from_param(p[0])`. The existing `Gate::Key` paths (routing,
  `holds`, the shop purchase, `why_unreachable`'s "needs the" text, `key_for`) then apply.
- `route.rs`: `lock_odds` returns `Some(100)` when `e.gate` is `Gate::Key { item }` and
  `self.keys.contains(&item)` (before the bash and pick arithmetic); keep the arithmetic for a
  key not held so a strong character still bashes the Western Door. `can_take`'s early
  `door_odds == Some(0)` test is then right as it stands.
- `walker.rs`: the `UseKey` arm's guard becomes `key_for(..).is_some()` (a Key exit or a keyed
  door with the key held) and the command gains the direction for a Door or Gate (the Key exit
  form `use <key>` stays as it is, it works today); the bash arm is not entered while a key is
  held. After `You unlock`, `Doing::Open` as now.
- `task.rs`: no change if the "needs the <key>" branch keys off `Gate::Key`; check the order of
  the branches so a keyed door without the key says "needs", not "cannot be opened", unless the
  bash odds are also nil, in which case say both: "needs the skeleton key (the gate cannot be
  bashed)".
- Not in this card: reading `You unlock the gate.` into the door state table for the far side
  (the pack state is right again after one step); the Key exit command form.

## Done when

- A traveller holding item 976 routes from 2/2561 to 9/930 through 9/873 east in the test
  graph; without it and with strength 100 the route is refused and the reason is "the gate east
  of Rancid Sewer needs the skeleton key"; with `Category::Key` allowed the preview counts it.
- A walk test at the gate: `The gate is closed!` → `use skeleton key e` → `You unlock the
  gate.` → `open e` → `e`; a second refusal after two uses replans with "would not unlock".
- The Western Door (1/1098, `exitpar2` −60) is still bashed by a character at Str 130 without
  the key, and used with the key when held.
- `docs/brain.md` "Walking" says so in "Locked doors and the character" and "Keys and items";
  `docs/kb/navigation.md` "Door / Gate" gains `exitpar4` = the key item (0 none), `use <key>
  <dir>` unlocks both sides to state 1 and the relock after `exitpar3` steps (at least 1),
  tagged [C] with the quicksilver files, and the 93-exit count [C data]; `docs/kb/changelog.md`
  has the line; CI green.

## Tests

- `wayfarer-world` route tests: `a_keyed_gate_is_routed_through_with_its_key`,
  `a_keyed_gate_without_its_key_names_the_key`, `a_keyed_door_a_strong_character_bashes_without_the_key`.
- `tests/walk.rs`: `a_keyed_gate_is_unlocked_with_use_then_opened`,
  `two_uses_refused_at_a_keyed_gate_replan`.

## Notes for the agent

- Lexi has no picklocks and the gate's bash is Str − 200, so this card is the only way her Go
  to reaches the dread mystic; the three temple doors beyond it are plain locked doors at
  Str − 10 and Str + 0, bashed as today.
- The guild's hidden exit north (`exitpar1` 2, needs search) and the `go manhole` Text exit are
  on the route and already handled; if the test graph is hand-built, keep them so the whole
  twenty-step route is exercised once.
- The host's logs are not fixtures; Lexi's `i` line above is enough to hand-write.
- KB facts about silvermere's own behaviour go to mmud-exploration's `kb/server-behaviours.md`;
  the key rule here is the stock game's (quicksilver mirrors `LJNCMDA.C`), so it goes in
  `navigation.md` with its tag.
