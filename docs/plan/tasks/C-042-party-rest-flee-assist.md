---
id: C-042
title: "Party rests, party flees and assisting the leader: anyone's rest stops the party, anyone's flee runs the party out behind the leader, followers attack the leader's target"
tier: B
size: L
wave: 6
depends: [C-041]
crate: wayfarer-brain, wayfarer-settings
touches:
  - crates/wayfarer-brain/src/{party.rs,modes.rs,recover.rs,flee.rs,fight.rs,session.rs}
  - crates/wayfarer-settings/src/lib.rs (`party.wait_rest_minutes`)
  - docs/brain.md "Parties"
---

## Goal

The party rests together and runs together. When any counted member needs a rest, the leader
stops and everyone rests until all are back to their own lines; when any member reaches its
flee line, the leader flees and the followers go with it by the game's follow. Followers fight
the monster the leader fights, so they share the exp.

## Inputs

- `docs/plan/10-party-play.md` §5 (Assist), §7 and §9.
- `docs/brain.md` "Flee" and "Fight here" (Recover: rest lines, quiet rooms, heals while
  resting), C-028 (Flee: confirmed exits, a quiet room, `flee_rooms`), C-041 (holds,
  separation after a flee).
- `docs/kb/grouping.md` §9 "Target calling", "Mana/rest", "Retreat" (each member leaving
  takes a free-attack roll; retreat the leader first).

## Behaviour

- **Party rest**: a counted member below its own `health.rest_below_hp` (or, for a caster,
  `health.rest_mana_below`) puts the party in a rest: the leader's moves are held (as C-041's
  hold), each member's Recover rests where it stands, out of combat. The party is rested when
  every counted member is at or above its own `health.rest_until_hp` (and mana line); then the
  hold lifts. The leader's own rest does the same. A monster arriving is fought by all
  (Fight outranks Recover), then the rest goes on. The quiet-room rule applies to where the
  leader rests (Recover's own rule, run by the leader only; followers rest where the leader
  is). State `resting` with the member's HP per cent in `detail`.
- **Time limit**: new setting `party.wait_rest_minutes` (Party group; default 10, range 1 to
  60). At the limit the leader stops its Task with a notice naming who has not recovered.
- **Party flee**: any counted member's Flee becoming urgent makes the **leader** flee (its own
  Flee, its own `flee_rooms`); a follower's own Flee does not run while it is joined and in
  the leader's room, it stays and is moved by the game's follow. A follower not in the
  leader's room (separated) flees on its own as a solo character. After the flee the leader
  sends `par` (C-039); anyone missing is separated (C-041). Then the party rests (above)
  before the leader's Task goes on.
- **Assist**: a joined follower in the leader's room, with nothing attacking it, attacks the
  monster the leader is fighting (the leader's Fight target, by name and the room's index, as
  the leader sent it). Attacked by something else, it fights that.

## Done when

- In-process: a follower at 40% (rest below 50) holds the leader and the party rests until all
  are at their rest-until lines; a follower at its flee line makes the leader flee and the
  follower doesn't send its own flee move; a follower in the room attacks the leader's target.
- Live: a party of two on John's rig rests together and flees together at least once in a
  30-minute loop.

## Tests

- `party.rs`: rest start and end across members' own lines; mana rests for casters; the rest
  time limit; flee by a follower's line; a separated follower flees alone.
- `fight.rs`: assist picks the leader's target; an attacked follower fights its attacker.

## Notes for the agent

- Careful with ordering in `Bot::tick`: Flee still outranks everything for the leader; for a
  joined follower, its Flee's urgency becomes a request to the coordinator, not a move.
