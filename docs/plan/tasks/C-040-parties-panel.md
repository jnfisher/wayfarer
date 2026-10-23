---
id: C-040
title: "Web UI: the Parties panel, party chips on the tiles, followers' walking controls greyed"
tier: B
size: M
wave: 6
depends: [C-039]
crate: ui
touches:
  - ui/src/PartiesPanel.svelte (new), ui/src/{App.svelte,Tile.svelte}, ui/src/lib/{gateway.ts,keys.ts,parties.ts (new)}
  - docs/web-ui.md
---

## Goal

John defines parties and watches them in the page: a Parties panel beside Map, Settings and
Stats, a chip on each tile's header saying who leads and who follows, and a follower's own
walking controls greyed while it is in a party.

## Inputs

- `docs/plan/10-party-play.md` §10 (approved by John as written; follow it).
- C-039's messages (`parties`, `party_save`, `party_delete`, `party_leave`, `party_join`,
  `MemberState`), `ui/src/lib/gateway.ts`, the panels in `App.svelte` (Map, Settings, Stats),
  the tile header in `Tile.svelte` (bot button, errands menu, minimize).

## Behaviour

- **Parties panel**: one box per party with the leader picked from a dropdown of the
  characters and the followers ticked from the rest; **+ New party**; delete (with a confirm);
  saving sends `party_save`. A character already in another party is shown with that party's
  name and moves when ticked. Each member has its live status from `parties`: *joined*, *not
  joined yet*, *away*, *separated*, *walking to Penny (4 rooms)*, *resting 62%*, *can't reach
  Penny: <reason>*.
- **Chip** on each tile's header next to the bot button: leader **★ Leading: Lauren, Faye,
  Manuel**; follower **Following Penny**, or **Separated** in the warning colour; nothing for a
  character outside any party. Click: a follower gets **Leave party** (`party_leave`); a
  character outside a party gets **Join <leader>'s party** for each party (`party_join`).
- **Follower tiles**: loop, Go to and the errands menu disabled, with the title "Following
  Penny"; play/pause untouched.
- A key opens the Parties panel (pick one free in `keys.ts`, add it to the help overlay).

## Done when

- John forms *Penny: Lauren, Faye, Manuel* in the panel, sees the chips change as they join,
  uses Leave party on Manuel and Join again, and the panel's statuses follow.
- `npm run check` and `npm test` pass.

## Tests

- `parties.ts`: the chip text for leader, follower, separated and none; which party a
  character is in; the follower's disabled controls.
- A component test of the panel: create, tick, save sends `party_save` with the right
  `PartyDef`.

## Notes for the agent

- The panel only edits definitions; it never sends game commands itself.
