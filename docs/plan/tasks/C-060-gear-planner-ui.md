---
id: C-060
title: "The gear planner: a full-screen view from the top bar, every weapon a character could use, scored by attack, bash and smash"
tier: B
size: M
wave: 9
depends: [C-059]
crate: ui
touches:
  - ui/src/{GearPlanner.svelte (new),App.svelte,lib/gear.ts (new),lib/keys.ts,lib/gateway.ts}
  - ui/tests/gear.test.ts (new)
  - docs/web-ui.md "Gear planner"
---

## Goal

A **Gear** button on the top bar (and **Alt+E**) opens a full-screen planner for the focused
character: one row per weapon it could ever use (and the others, greyed), scored for attack,
bash and smash against the monsters of its loop or a set the player picks, from its live stats
or from what-if values, with buffs as checkboxes. The best weapon and the best move stand out.
Esc or the button closes it. (John, 2026-10-26: "it's pretty nuanced what would be the best
weapon … we ONLY need to take solo play into account … we kill most things in 1 round,
possibly 2.")

## Inputs

- C-059: `gear_plan_request` / `gear_plan`, `gear_detail_request` / `gear_detail`.
- `ui/src/App.svelte` (the top bar's tabs and the drawer), `ui/src/lib/keys.ts` (one keys
  table for the docs and the `?` overlay), `ui/src/lib/spells.ts` and where the spell plan is
  shown (the pattern for a request and its answer), `docs/web-ui.md`.
- John's sketch: green rows for what the character can use, grey for what it can't.

## Behaviour

- **Opening**: the Gear button sits on the top bar beside the panel tabs; Alt+E toggles it;
  Esc closes it. It covers the tiles (full screen), keeps the top bar, and opens on the focused
  character; a picker in its header switches character. It sends `gear_plan_request` on open
  and on every change of an input (debounced about 200 ms).
- **Header**:
  - **Monsters**: "Its loop" (default), an area (a text field with the area names as
    suggestions), or names. The set is listed under the header: name, HP, AC, DR, dodge, and
    how many of the rooms spawn it.
  - **What if**: level and the six stats, pre-filled from the character and marked when
    changed, with a "live" button that clears them; load % with the live value shown beside it.
  - **Buffs**: one checkbox per buff the answer offers, labelled with its effect ("song of
    valour +5 AV"); the ticks are kept per character in `localStorage`.
  - **Smash**: a checkbox "score smash (as if it had it)", shown only for the classes that
    may keep it.
  - **Filters**: usable now / usable later / everything; "has a source" (hides items nothing
    sells, drops or gives); a maximum price.
- **Table**: one row per weapon, sortable by any column:
  - Name, damage, speed, Str (red when short, with the shortfall), min level, 1H/2H.
  - Status: rows the character can use **now** are green; **later** amber, with the reason
    ("level 15", "Evil only"); **never** grey, with the reason, and not scored.
  - Source: the cheapest shop and its price, else the best drop ("25% minotaur champion, boss,
    first kill"), else "quest" or "placed"; a small marker for limited and for deleted at
    cleanup.
  - For each move (Attack, Bash, Smash when shown): **kill in 1 round %**, **kill by 2 %**,
    **expected rounds**, and round-1 damage; the row's best move highlighted.
  - The best row pinned at the top.
- **Detail**: clicking a row opens it below itself with `gear_detail`: per monster and move,
  hit %, crit %, dodge %, damage range, the bashes per round for 8 rounds ("1 2 1 2 1 2 2 1"),
  and the kill chances.
- **States**: no stats yet ("type `st` in the tile"), no position or loop for the default set
  (pick a set), and the brain not attached, each said plainly in the view.

## Design notes

- `GearPlanner.svelte` and `lib/gear.ts` (request building, sorting, filtering, formatting:
  pure functions, tested). The planner state lives in the component; nothing new on the tiles.
- The keys table gets Alt+E; the `?` overlay and `docs/web-ui.md` follow from it.
- Rows: a plain table, with no virtualisation needed at a few hundred rows.

## Done when

- On John's rig the Gear button opens the planner for the focused character, the rows fill in,
  changing a stat or a buff re-scores within a fraction of a second, and Esc closes it.
- Green, amber and grey rows as above; the best weapon and moves marked; the detail opens.
- `docs/web-ui.md` has a "Gear planner" section.

## Tests

- `ui/tests/gear.test.ts`: request building from the inputs (what-if only for changed values),
  sorting by each column, the filters, the status colours, the best-move marking, the
  formatting of the blows-per-round sequence and of percentages.

## Notes for the agent

- The armour side (pieces scored by AC, DR and weight, and the damage taken from the set) is a
  later card; leave room for a second tab but don't build it.
- Live check on John's rig only; cloud sessions can't reach silvermere.
