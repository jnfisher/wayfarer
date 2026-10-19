---
id: C-018
title: Web UI dev panel: the event stream, unclassified lines and rate, brain build and restarts, re-sync
tier: B
size: M
wave: 2
depends: [C-017]
crate: ui/
touches:
  - ui/src/DevPanel.svelte (new), ui/src/App.svelte, ui/src/lib/state.ts, ui/src/lib/keys.ts
  - docs/web-ui.md
---

## Goal

A panel beside the focused tile that shows what the brain sees: the live event stream for
that character with the output sequence number each event came from, the lines nothing
classified with a running rate, and the brain itself (build, start time, world pack,
attached or not, restarts). It is how John and the cards watch the parser against the real
server (`07-development-and-testing.md`), and where the unclassified-line soak metric is
read.

## Inputs

- `docs/plan/03-engine.md` section 1 ("Every line is accounted for": the unclassified rate
  is a soak metric), `05-ui.md` "Side panels" (the bot panel later; this is the first drawer),
  `07-development-and-testing.md` "The development machine" (the dev panel's contents).
- C-014 (`event`, `notice`, `warning`, `brain_info`, `brain_state`, `brain_lagged`,
  `resync`, `state_request`), C-016 (`parse {lines, unclassified}` in the snapshot; what `notice`
  says), C-017 (`lib/state.ts`).

## Behaviour

- **Open and close.** `Alt+D` (row in the help table) or a `dev` button in the top bar
  toggles it; it opens as a drawer on the right, a third of the width at most, the tiles
  refit. It follows the focused tile. Its open state and tab are remembered in
  `localStorage` (try/catch as always).
- **Events tab.** The last 2,000 events for the focused session, newest at the bottom,
  auto-scrolling until the user scrolls up: `seq`, `name`, `cat`, and the fields compact
  (`hp=28 maxhp=40`); a filter box over name and line text; a pause button that holds the
  view while events keep arriving. Clicking an event shows its full `line`.
- **Unclassified tab.** The `unknown` lines for the session grouped by text with counts,
  most frequent first; the rate from the snapshot's `parse` counts
  (`unclassified / lines`, two decimals, red above 0.2%); a "copy as KB notes" button that
  puts the list on the clipboard as a Markdown list, ready for a PR's "KB notes".
- **Brain tab.** `brain_info` (build, started, world pack), attached or not, a log of
  `brain_state` changes ("brain left 11:02:10", "brain attached 11:02:14"), `brain_lagged`
  (followed by the automatic `state_request`), `notice` and
  `warning` lines with their session and time, and a **Re-sync** button sending `resync`
  for the focused session (disabled with no brain).
- **Memory.** Bounded buffers (2,000 events, 500 notices per session) so a day-long page
  does not grow.

## Design notes

- `DevPanel.svelte` with three tabs; the buffers live in `lib/state.ts` beside the
  snapshots, so the panel is a view and can be unmounted.
- Reuse the VGA look: a box-drawn frame, the same font at 16 px; dense rows.
- No virtual list library: 2,000 short rows render fine; cap and trim.

## Done when

- Against the rig with the brain running: walking a room shows `room_title` and
  `room_exits` with sequence numbers; a line the parser does not know appears under
  Unclassified with its count and the rate updates; restarting the brain logs the two
  `brain_state` changes and the new `started_at`; Re-sync produces the `st`/`exp`/`i`
  sequence in the tile and a `notice "re-synced"` in the Brain tab.
- The panel open with 2,000 events buffered costs no visible terminal lag (check with the
  WebGL renderer on).
- `Alt+D` is in the help overlay and `docs/web-ui.md`; the keys test passes.
- `npm run check`, `npm test`, the UI workflow green.

## Tests

- Buffer trimming and grouping of unclassified lines in `ui/tests/state.test.ts`; the rate
  formatting; the Markdown the copy button produces.

## Notes for the agent

- The clipboard needs a user gesture; the button is one.
- This panel is for development; keep it out of the way of play (closed by default).
