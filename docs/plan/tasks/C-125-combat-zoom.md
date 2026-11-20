---
id: C-125
title: "The combat zoom: histograms, each monster, the party side by side, the last hour or day"
tier: A
size: M
wave: 18
depends: [C-124]
crate: wayfarer-brain
touches:
  - crates/wayfarer-store/src/lib.rs (combat_snapshots, migration 17)
  - crates/wayfarer-brain/src/combat_stats.rs (Tally::since, Tally::hists)
  - crates/wayfarer-brain/src/stats.rs (snapshots, combat_detail)
  - crates/wayfarer-brain/src/session.rs, brain.rs (combat_detail_request)
  - crates/wayfarer-proto/src/lib.rs (CombatWindow, CombatDetailRequest, CombatDetail, HistRow)
  - ui/src/CombatZoom.svelte, ui/src/lib/combat-zoom.ts, ui/src/App.svelte, ui/src/StatsPanel.svelte
  - docs/web-ui.md "Combat zoom", docs/brain.md, docs/protocol.md
---

## Goal

John asked for a zoom-in view of the combat stats (2026-11-20). He wants histograms of round,
backstab and all damage behind a button, and agreed to more:

- each monster fought;
- each spell's damage per mana;
- time windows;
- the party side by side ("yes");
- rates on few tries marked.

The comparison with and without a blessing is out: he checks those himself.

## Behaviour

- **Snapshots.** Before the combat numbers change, the brain keeps them as they stand, at most
  every ten minutes, for 25 hours. They are stamped with the time of their last change. A
  reset forgets them.
- **The request.** `combat_detail_request {session, window}` answers `combat_detail` with the
  numbers over `reset`, `hour` or `day`.
- **How a window is worked out.** It is the numbers now less those kept at or before its
  start, or the oldest kept after it. `since` gives the true start. With the numbers go the
  histograms of the round, every hit, the blows taken and each attack.
- **The view.** The stats tab's **zoom** opens a full-screen view (Esc closes it) with:
  - window buttons;
  - the party side by side;
  - a histogram picked from a list;
  - the by-attack table and the blows taken;
  - each monster's kills, rounds a kill, damage each way a round, hit rate and exp a kill.
  It asks for every party member's numbers, again every minute. Lines under 30 rounds are dim.

## Done when

- Store test for the snapshots.
- Brain test: the last hour is the numbers now less those kept an hour ago, and the day
  reaching past the reset is all of it.
- UI tests for the helpers and the view.
- Docs say so, CI green.
