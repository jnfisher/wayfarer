---
id: C-076
title: "An opened Hidden exit with a blank name: the Labyrinth's `river` east exit is read as there"
tier: B
size: S
wave: 15
depends: []
crate: wayfarer-parse, wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-parse/src/helpers.rs (parse_exits: the empty slot)
  - crates/wayfarer-world/src/ (the pack: which Hidden exits print blank)
  - crates/wayfarer-brain/src/position.rs (matching a room's exits line)
  - tests/fixtures/blank-exit/ (new: the transcript and its PROVENANCE.md)
  - docs/kb/navigation.md, docs/kb/screen-patterns.md, docs/kb/changelog.md
---

## Goal

In the Labyrinth Passage with the brass plaque (1/1972), the game opens a hidden exit east when
you say `river`. It then lists the exits as `Obvious exits: north, , west`, with the east exit
printed as a blank. wayfarer should know that blank is the east exit:
- the position belief keeps the room;
- the map shows east as open;
- Go to and the walker take it.

The same fix covers the other four exits in the data with the same fault.

## Inputs

- **The live text** (2026-11-04, the Thrag run). On the server, in
  `~/.local/share/wayfarer/logs/lexi/output.log`, seq 329371 to 329393:
  - before: `Obvious exits: north, west`;
  - `river`, answered "You speak the word "river".";
  - then a `look`: `Obvious exits: nB\borth, , wI\best`. Each direction carries the game's
    letter-backspace slot. The east exit is the empty slot between the two commas.
- **The cause, in the stock data** (`tests/fixtures/world/`, room 1/1972):
  - **East** is type 6 (Hidden) to 1/1983. Its `exitpar1` is 16 (`BLOCKED1`, so no search finds
    it), `exitpar2` -1, `exitpar3` 1 and `exitpar4` 1.
  - **Down** is type 12 (Remote Action) to the room itself, with phrase message 8387 (`river`,
    `say river`, `speak river`). Its `exitpar2` is 2: action 0, exit 2, so it takes east's blocks
    off.
  - **An opened Hidden exit is listed by the name in message `exitpar4`,** with the direction
    last (`parse_exits`'s "custom" case: `cramped passage south`). 0 means the default
    (`secret passage south`, `kb/navigation.md` "Hidden"). Message 1 is `["", "", ""]`, so the
    game prints nothing, not even the direction.
- **Every Hidden exit that can be opened and whose `exitpar4` names a blank message** (a scan of
  `WCCMP001`):

  | Room | Exit |
  |---|---|
  | 1/1972 Labyrinth Passage | east |
  | 1/2100 Labyrinth Passage | west |
  | 1/2103 Labyrinth Passage | east |
  | 12/2383 "Blank" | west |
  | 1/2441 Sewer Tunnel | up |

  "Can be opened" means `exitpar1` other than plain `PASSABLE`. Hidden exits that are always
  passable are never listed, so their blank names don't matter.
- `parse_exits` (`helpers.rs`) drops an empty part (`if part.is_empty() { continue; }`), so the
  line reads as `north, west`.
- C-052 and C-055 (phrases as ways through). C-037 (Hidden exits, searching, the router's
  `found` set).

## Behaviour

- **The parser keeps the slot.** An empty part *between* two commas is an exit with no
  direction: `kind: "custom"`, `state: Some("")`, `dir: None`. A trailing comma at a line wrap is
  not a slot. The registry already joins wrapped `Obvious exits:` lines, so this applies only to
  the joined text.
- **The world pack knows the blank exits.** A Hidden exit whose `exitpar4` message is blank in
  all three texts, and that isn't plain `PASSABLE`, is flagged "lists blank". This is derived
  from the data at pack build, not hand-listed, so the five above and any modded world are
  covered.
- **Position.** When the exits line has an unnamed slot, the room's "lists blank" exits fill it.
  - In 1/1972, `north, , west` matches the room with east open, exactly as `north, east, west`
    would.
  - If there are more unnamed slots than "lists blank" exits, the line counts as a mismatch, as
    any wrong line does today.
- **The way through.**
  - After `river` (said by the player, or by the bot's route), the router counts east as open, as
    it does for a Hidden exit whose opening it has seen.
  - The walker's arrival check, which compares the exits line, accepts the blank slot.
  - A route to 1/1983 through 1/1972 says `river` and then `e`. If the pack already has this way
    through as a Remote Action plus a Hidden exit, check that it is taken; if not, this card adds
    it.
- **The map panel** shows the opened east exit like any other opened Hidden exit.

## Design notes

- Keep the special case in the data, not in a hard-coded room list. One field on the pack's exit
  (`lists_blank: bool`) and a few lines in `parse_exits` and the exits matcher.
- If the pack schema changes, bump its version as earlier cards did, and rebuild
  `world.sqlite` the usual way.

## Done when

- `parse_exits("nB\x08orth, , wI\x08est")`, after the backspace clean-up, gives three exits, the
  middle one unnamed.
- With the transcript above replayed (a fixture copied to `tests/fixtures/blank-exit/` with its
  `PROVENANCE.md`), the belief stays on 1/1972, sure, with east open.
- A Go to from 1/1972 to 1/1983 says `river` and walks east.
- The pack flags exactly the five exits in the table (a test against the fixture world).
- KB:
  - `navigation.md` "Hidden" says an opened Hidden exit is listed by message `exitpar4`, and that
    a blank message prints an empty slot. Tag it [C data] and [C live 2026-11-04].
  - `screen-patterns.md` gains the `north, , west` form.
  - A line goes in `changelog.md`.
- The checks pass. If a live check through the Labyrinth runs, say so in the PR (it is optional;
  the room is far from the test characters).

## Tests

- `helpers.rs`: the empty slot, a trailing comma, `NONE!!!`.
- `wayfarer-world`: the five flagged exits.
- `position.rs`: the replayed transcript, plus a line with two unnamed slots in a room with one
  "lists blank" exit, which is a mismatch.
- Routing: 1/1972 to 1/1983 via `river`.

## Notes for the agent

- Work in a worktree. Writing under `crates/` in John's checkout restarts his live brain.
- Copy only the dozen output lines needed into the fixture, not the whole log. The repository
  is public: the transcript has character names (Lexi Belle). Replace them with placeholders, as
  the other fixtures do.
- This is a fault in the game's data, not silvermere's. Nothing goes to `kb/server-behaviours.md`.
