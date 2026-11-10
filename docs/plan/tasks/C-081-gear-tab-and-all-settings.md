---
id: C-081
title: "Gear is a tab like the others, and the settings form shows all settings by default"
tier: C
size: S
wave: 16
depends: []
crate: ui
touches:
  - ui/src/App.svelte, ui/src/lib/state.ts (`pressTab`), ui/tests/state.test.ts
  - ui/src/SettingsPanel.svelte, ui/tests/settings-panel.test.ts
  - docs/web-ui.md
---

## Goal

- **Gear is a tab.** With the gear planner open, pressing another tab on the bar (map, parties,
  stats …) or its `Alt` key closes the planner and shows that panel. Today the planner stays over
  everything until its `[x]`, and the other tabs light up underneath it. John reported that
  pressing `[x]` sometimes "breaks the interface altogether"; that wasn't reproduced, and with
  this change the `[x]` is no longer needed.
- **Settings**: the `all` box is ticked by default, so the standard and advanced settings show
  (John, 2026-11-10).

## Behaviour

- `pressTab(prefs, gearOpen, tab)`:
  - the gear tab toggles the planner and leaves the drawer as it is;
  - any other tab closes the planner and opens that panel; a panel that was open under the
    planner stays open rather than toggling shut;
  - without the planner, a tab toggles as before.
- While the planner is open, no drawer tab shows as pressed.
- `Alt+G` (the go box) closes the planner too.

## Done when

- `state.test.ts` covers `pressTab`.
- The settings panel tests expect the advanced rows by default, and hidden with `all` unticked.
- `npm run check` and `npm test` pass.
