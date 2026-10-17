---
id: C-010
title: wayfarer-parse: pattern table, generated game-text patterns, incremental parser, golden parity with patterns.py
tier: B
size: L
wave: 2 (early: runs beside wave 1)
depends: [C-001]
crate: wayfarer-parse
touches:
  - crates/wayfarer-parse/ (new)
  - tests/fixtures/parse-golden/ (new: expected events produced by patterns.py, with PROVENANCE.md)
  - tests/fixtures/{captures,transcripts,patterns,world}/ (read only)
---

## Goal

A Rust crate that turns the game's terminal text into a stream of named events with fields,
incrementally as text arrives, and does it at least as well as the prototype's
`test/play/patterns.py`, which reaches 0.2% unclassified lines on our transcripts. It covers
the static patterns from the source, the patterns generated from the game's own message
records (WCCMSG: monster arrivals and deaths, hits and misses, spells), the prompt in its
statline forms, multi-line joins (exits, "Also here") and tables (`par`, equipment). Nothing
uses it yet; the brain (wave 2) will.

## Inputs

- `docs/plan/03-engine.md` section 1 "Parser: text to events".
- `mmud-exploration/test/play/patterns.py`, the reference implementation to port:
  - `raw_to_lines` (:61-75): line cutting (CR, LF, `ESC[79D ESC[K`), ANSI dropped for
    matching, **backspaces applied** (direction names carry a random capital and a backspace,
    `nJ\borth`, rewritten every slow cycle: the captures' `script.json` masks note it).
  - `compile_statline`, `PromptParser.peel` (:113-200): the prompt and its five statline modes;
    titles glued behind a prompt are peeled off.
  - `template_regex` (:837-877): WCCMSG text → regex (`%s` lazy group, `%d` number, first letter
    either case, trailing `%s` optional, `a|b|c` alternatives cut at 5 and 39 characters).
  - `Registry.load_static`, `load_world` (:930-1047): which record fields give which role
    (monster arrive/leave/follow/death texts, weapon hit and miss verbs from items, spell cast
    texts), and the catch-all `wccmsg` family; `build`, `match_line`, `fields`,
    `_fix_monster` (:1048-1121): the first-word index, priorities, monster-name splitting.
  - `classify`, `iter_events`, `_join` (:1122-1229), `parse_exits`, `parse_carrying`,
    `parse_also_here`, `split_monster` (:1230-1328).
  - `selftest` (:1409-): the metrics to reproduce.
- `tests/fixtures/patterns/patterns.json` (C-001): the static patterns, tables, joins, exit
  tails, prompt forms, statline codes, role arguments and categories, as data.
- `tests/fixtures/world/`: `WCCMSG`, `WCCKNMSR`, `WCCITEMS`, `WCCSPELS`, `WCCMP001` (room
  titles, Text-exit phrases) `.jsonl.gz`, for the generated patterns.
- `tests/fixtures/captures/` and `tests/fixtures/transcripts/`: the text to classify.
- `mmud-exploration/kb/screen-patterns.md` (the catalogue: sections 1-5 how text arrives, the
  prompt, room display, generated families, static patterns; section 6 what the bot misread)
  and `kb/screen-parsing.md`.
- `mmud-exploration/kb/plan-patterns-migration.md` (the misreads the event names fix).

## Behaviour

- **Incremental**: `Parser::feed(&mut self, text: &str) -> Vec<Event>`. Text arrives in
  arbitrary chunks (C-004's chunks); a line is classified when it is complete; the prompt,
  which has no newline, is recognised at the end of a chunk. Feeding a session whole or in
  any split gives the same events.
- **Events**: `Event { name, cat, fields, line, seq }`, with `name` and `cat` as in
  `patterns.py` (e.g. `you_hit`, `mon_hits_you`, `prompt`, `room_title`, `exits`,
  `also_here`) and `fields` the named groups. `seq` is the terminal chunk the line ended in,
  so events can point back at the screen. Unclassified lines are events too
  (`unknown`/`unclassified`), never dropped.
- **Generated patterns** are built at load time from a `GameTexts` value (messages, monster
  and item and spell references, room titles, Text-exit phrases), so a customised world gets
  its own patterns. `GameTexts::from_export(dir)` reads the world export; the world pack
  (C-011) will provide the same struct later.
- **Prompt**: the standard form, the custom statline templates (`set statline` codes from
  `patterns.json`), and the generic `[...hp=...]:` form. The generic form's Python regex uses
  a lookahead, the only one of the 416 patterns that does: match the bracket body, then check
  it contains `hp=` (any case). Use the `regex` crate throughout (no `fancy-regex`).
- **Speed**: building the registry from the export under 150 ms (release); classifying under
  5 µs per line on average (the Python is about 11 µs).

## Design notes

- Load `patterns.json` with `include_str!` at compile time (it is data, kept as the
  prototype's export format; a later card may move it into the repo's own source of truth).
- Index patterns by their first literal word, as `Registry.build` does, with an "always try"
  list for patterns that start with a group; keep priorities.
- Keep the event as a generic struct now; typed accessors for the events the brain needs come
  with the brain cards.
- `cargo bench` or a `#[ignore]`d timing test for the speed numbers; not in CI.

## Done when

- **Golden parity**: `tests/fixtures/parse-golden/` holds, for every line of every capture and
  transcript fixture, the event `patterns.py` produces (name and fields), generated by a small
  script the agent runs in a checkout of mmud-exploration (the script and the commit go in
  `PROVENANCE.md`). The Rust parser agrees on at least 99.5% of lines; every disagreement is
  listed in the PR with a reason (a Python misread the Rust fixes is fine and welcome).
- **Generated-pattern parity**: for every WCCMSG record that `patterns.py` turns into a
  pattern, the Rust generator gives the same role and an equivalent regex (tested by matching
  each record's own example text and the golden lines).
- Unclassified rate over the fixtures no higher than `patterns.py`'s on the same fixtures
  (both numbers in the PR).
- Split-feeding test: each capture fed in random chunk sizes gives the same events as fed
  whole.
- Speed numbers (above) measured and reported in the PR.
- The CI test time for this crate stays under ten seconds (the golden test may be the one
  slow test; say so in its comment).

## Tests

- Golden parity, generated-pattern parity, split feeding, prompt forms (each statline mode
  from `patterns.json` examples), the misreads listed in `kb/plan-patterns-migration.md`
  "Why bother" (each as a named test), backspace handling (`nJ\borth` → `north`).

## Notes for the agent

- This card is wide: port faithfully first, then improve. A difference from `patterns.py` is
  only acceptable when the PR shows the Python was wrong.
- Write "KB notes" for any game text the KB's catalogue lacks.
- Do not build the brain, typed events or anything that sends commands.
