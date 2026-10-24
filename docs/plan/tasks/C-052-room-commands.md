---
id: C-052
title: "Room commands and MegaMud's crossings are ways through: `ring chime` takes Go to into the Strange Mansion"
tier: B
size: M
wave: 8
depends: [C-036, C-037]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/{nav.rs,knowledge.rs,pack.rs,route.rs,layout.rs}
  - crates/wayfarer-brain/src/{brain.rs,walker.rs}
  - docs/brain.md "Walking"; docs/kb/navigation.md
---

## Goal

The router knows the ways through that are not exits: a room's command script that teleports
(`ring chime` on Slum Street, 1:1182, puts the character in the Strange Mansion's Entrance,
1:65), and a step of a MegaMud path that crosses where the pack has no exit at all (`pull
bone`, `@party touch crystal`). Go to finds the spectral mage, and the walker types the phrase.
(Added 2026-10-24 at John's request: "you have access to a whole pack of paths from MegaMud;
perhaps the first attempt at finding a route should see if there's a series of those
well-known paths".)

## Inputs

- `docs/kb/navigation.md` "Text blocks: room commands, quest scripts and ASK/GREET": the
  `pattern:clause:...` format, the first matching line runs, phrases are tried only after the
  command parser has said "not a command".
- Room 1:1182's command text 4087: `ring chime:message 3177:teleport 65 1:message 837`.
  MegaMud's `SLMCSSME.mp` ends with `@party ring chime` at 1:1182.
- The stock data (2026-10-24): 140 rooms have teleport lines (552 lines); 120 pass the rule
  below. MegaMud's verified paths cross where the pack has no exit 25 times.
- C-036 (`PathKnowledge`), C-037 (`Traveller::can_take`, doors the character cannot open).

## Behaviour

- **Room commands.** For each room's command text, the first line of each pattern is a way
  through when its pattern is plain words (no `*` or `?`) and every clause is one of `message`,
  `adddelay`, `minlevel`, `maxlevel`, `nomonsters` and exactly one `teleport <room> <map>`.
  Lines with anything else (`cast`, `random`, `testskill`, `roomitem`, `takeitem`, `checkitem`,
  ability and alignment checks, `summon`, ...) are left out: what they do cannot be told
  beforehand. `minlevel` and `maxlevel` gate it like a Level exit. The patterns of the lines
  that teleport to the same room are its phrases.
- **Which phrase.** The game tries a room command only after its command parser has turned the
  line down, so a phrase whose first word is a game command (`use chime`, `look book`) never
  reaches it. The brain picks the first phrase its command table does not claim (`ring chime`);
  with none, the way is not taken.
- **MegaMud's crossings.** A step of a verified path that types a phrase where the pack has no
  Text exit or room command to the room the next step starts in (or the path's end) is a way
  through, with MegaMud's phrase (`@party ` dropped: walking alone, the character types it).
  Its phrase is used as MegaMud typed it, game command or not (`ask darkbane arlysia` works by
  being one).
- **Walking.** They cost one step, are never listed in `Obvious exits` and are not drawn on the
  map. The walker sends the phrase and expects the room; a phrase that gets no room is handled
  as a Text exit's (refused, routed around). Flee never takes them.
- **Party.** The leader's phrase takes only the leader; followers come after it as a separated
  member does (C-041), by the same way.

## Design notes

- `ExitType::RoomCommand` and `ExitType::PathCommand`, after the pack's types (`name()`
  "Room Command", "Path Command"), `Listing::Never`. Their `dir` is 10 and up, after the
  room's real exits, so `(room, dir)` still names one; nothing indexes the ten directions
  with it (layout skips them; the walker names a direction only for doors).
- `WorldPack::room_commands()`: `(map, room, text)` for rooms with a command text.
  `nav::room_command_edges` parses the lines.
- `PathKnowledge` records the crossings; `RoomGraph::learn_crossings` adds them (rebuilding the
  flat edge list), then the brain loads the knowledge again so loop paths see them.
- `RoomGraph::choose_phrases(f)`: a room command's `command` is its first phrase `f` accepts.

## Done when

- From Newhaven, a route to 1:382 exists and goes `... ring chime, n, n, w, move bookcase, e,
  n, e`; Go to "spectral mage" finds it reachable.
- In-process: the walker types `ring chime` at 1:1182 and arrives at 1:65.
- A line with `cast` or `random` is no way through; `minlevel 20` gates a level 10 character.

## Tests

- `nav.rs`: parsing (the chime, a gated vortex, a `cast` line left out, wildcard left out).
- `wayfarer-world` tests on the fixture pack: the route into the mansion; a MegaMud crossing
  added and not duplicated where a room command exists.
- Brain: Go to answers the mansion reachable; the walker sends `ring chime`.

## Notes for the agent

- KB: the room-command rule for routes and the chime are facts the KB lacks; add them to
  `docs/kb/navigation.md` with a changelog line.
