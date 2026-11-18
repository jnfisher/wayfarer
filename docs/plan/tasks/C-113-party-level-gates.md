---
id: C-113
title: "A leader's routes take only the gated exits every follower may take"
tier: A
size: S
wave: 16
depends: [C-111]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/route.rs (Passer; Traveller::party; gates checked for each)
  - crates/wayfarer-brain/src/party.rs (MemberInfo::passer; the leader's role carries its followers')
  - crates/wayfarer-brain/src/session.rs (the leader's traveller and loop key take them)
  - crates/wayfarer-brain/src/task.rs (the reason names the party)
  - docs/brain.md
---

## Goal

Walking a party of six to the ancient willow (Dying Grove, Small Pool), the leader, level 30,
routed through the Ironwood vortexes (`go vortex`, a room command gated `minlevel 20`). The
level 12 and 13 followers cannot go through, and were left on the near side. A way on foot
(up the Silver River and the Silvery Stream) exists.

## Behaviour

- A leader's routes, and the loops generated for it, take a level, class, race or alignment
  gate only if it lets the leader and each follower in the realm through.
- A follower out of the realm gates nothing.
- With no route, the reason says when the leader would pass and a follower would not.

## Done when

- A level 2 leader with a level 5 follower does not reach the Newhaven Healer (levels 1 to 3);
  with a level 3 follower it does.
- The party plan gives the leader the gates of its followers in the realm, and only those.
