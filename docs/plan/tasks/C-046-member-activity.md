---
id: C-046
title: "What each member is doing: a short activity on the bot's state, on each line of the Parties panel"
tier: B
size: S
wave: 7
depends: [C-040]
crate: wayfarer-brain, wayfarer-proto, ui
touches:
  - crates/wayfarer-brain/src/{modes.rs,fight.rs,recover.rs,flee.rs,loot.rs,task.rs}
  - crates/wayfarer-proto/src/lib.rs (`Bot::doing`)
  - ui/src/{PartiesPanel.svelte,App.svelte,lib/parties.ts}
  - docs/protocol.md, docs/web-ui.md "Parties"
---

## Goal

Each member's line in the Parties panel says what it is doing now: *attacking large rat*,
*healing Lauren*, *casting healing rain*, *resting*, *fleeing*, *walking to Penny*, *waiting*.
(Added 2026-10-23 at John's request.)

## Inputs

- `docs/plan/10-party-play.md` §10; C-040 (the panel), `modes.rs` (`Bot::tick`, `Step`, the
  narration), `ui/src/lib/parties.ts` (`memberLine`).

## Behaviour

- The bot's wire state gets `doing`: a few words, lower case, without the mode's name. Set by
  each step: Fight *attacking <target>* (or *looking for a fight*), a heal *healing <name>* /
  *healing itself* / *casting <party heal>*, Recover *resting* (or *resting with the party*),
  Flee *fleeing*, Loot *picking up*, Task *walking to <stop>* or *walking back to <leader>*,
  Idle *waiting*; a follower held by the party *following <leader>*. Paused or stopped, the UI
  shows *paused* / *stopped*.
- The Parties panel shows, for the leader and each follower, the membership state (as now)
  and then the activity with the member's HP: `joined · attacking large rat · 72%`.

## Done when

- In-process: the bot's `doing` for a fight, a heal on another member, a rest and a flee.
- UI test: the panel's member line with an activity and HP.

## Tests

- `modes.rs`: `doing` per mode, the heal's override.
- `ui/tests/parties.test.ts`: the member line.
