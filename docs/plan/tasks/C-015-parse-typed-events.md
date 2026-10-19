---
id: C-015
title: `wayfarer-parse`: game texts from the world pack; typed events for the character model
tier: B
size: S
wave: 2
depends: [C-010, C-011]
crate: wayfarer-parse, wayfarer-world
touches:
  - crates/wayfarer-parse/src/game_texts.rs (a constructor from plain records)
  - crates/wayfarer-parse/src/typed.rs (new)
  - crates/wayfarer-world/src/ (`WorldPack::game_texts`)
---

## Goal

The brain can build its parser from the world pack it already ships with, instead of the
stock-world export directory the tests use, and it reads the events it needs as typed values
rather than string fields. No new patterns; no change to what `Parser::feed` returns.

## Inputs

- `crates/wayfarer-parse/README.md`, `src/game_texts.rs` (`GameTexts::from_export`: which
  tables and fields it reads: WCCMSG messages, monster names and attacks, item and spell
  names), `src/event.rs`, `src/prompt.rs` (the prompt fields: `hp`, `maxhp`, `mana`,
  `maxmana`, `state`, `form`, ...), `tests/fixtures/parse-golden/patterns.json` (the static
  names: `st_*`, `exp`, `room_title`, `room_exits`, `combat_engaged`, `combat_off`,
  `you_killed`, `exp_gain`, `lives_left`, `drops_unconscious`, `inv_*`, `rest_start`,
  `poison_tick`, `blind`, `stunned`, ...).
- `crates/wayfarer-world/README.md` "Reading a pack" (`messages`, `monsters`, `items`,
  `spells` with their `raw` columns).
- `docs/plan/03-engine.md` section 2 (the character fields).
- `kb/screen-parsing.md` "Prompt", "Status screens".

## Behaviour

- **Texts from the pack.** `GameTexts::from_records(...)` takes iterators of the plain
  records `from_export` reads (the same structs), and `WorldPack::game_texts()` in
  `wayfarer-world` feeds it from the pack's tables (`raw` JSON where a typed column is
  missing). Dependency direction stays `parse ← world`: `world` depends on `parse`, never
  the reverse.
- **Equivalence.** A pack built from `tests/fixtures/world` gives a `Registry` with the same
  patterns (count, names, regex sources) as `from_export` on that directory, and the golden
  test passes with either.
- **Typed events.** `wayfarer_parse::typed::GameEvent` and `GameEvent::from_event(&Event) ->
  GameEvent`: the variants the character model needs, with typed fields, and `Other` for
  everything else (keeping the `Event`):
  - `Prompt { hp, hp_max, mana, mana_max, resting, form }` (every value optional; `form` std,
    custom, generic or off). The standard prompt carries no maxima (`prompt.std` captures
    `hp`, `mana`, `mtype` and the state); they come from `st` or a custom statline, so the
    model has none until the first re-sync. That is expected.
  - `StatusName { name, lives, cp }`, `StatusRace { race, exp }`, `StatusClass { class,
    level }`, `StatusHits { hp, hp_max }`, `StatusMana`, `StatusStat { stat, value }` (from
    `st_*`), `ExpScreen { exp, level, needed, next_total, percent }` (from `exp`).
  - `RoomTitle { title }`, `RoomExits { exits }` (the list `helpers` splits), `RoomDark`.
  - `CombatEngaged`, `CombatOff`, `YouKilled { monster }`, `ExpGain { amount }`,
    `TookDamage`, `Healed` (whichever static names carry them), `Resting`, `RestEnd`.
  - `Unconscious`, `Dead { lives_left }`, `Poisoned`, `PoisonTick`, `Blind`, `Stunned` (as
    `Status { kind, on }` if that reads better).
  - `Inventory { carrying }`, `Wealth { coins }`, `Encumbrance { level }`.
  - `Unknown { line }` for `cat == unclassified`.
- Numbers parse from the field text (`"48/48"` splits; thousands separators, if the game
  prints any, are handled where the golden fixtures show them).

## Design notes

- `typed.rs` is a thin layer over `Event`; no change to `Parser`. Keep `From<&Event>`
  total: it never fails, it falls back to `Other`.
- **Fields that the patterns don't name.** `st_str`, `st_int`, `st_wil` and some other
  static patterns use unnamed groups, so `Event.fields` is empty for them (`Registry::fields`
  reports named groups only, and the golden events show `{}`). For those, `typed.rs` reads
  the values from `event.line` with its own small named regex, keyed by the event name.
  That is allowed: classification is unchanged, the fixture is untouched, golden parity
  holds. Say in the PR which events needed it.
- Name the status-screen variants after the pattern names so the mapping is obvious.
- `WorldPack::game_texts()` reads a few thousand rows; keep it under 200 ms in release and
  say the number in the PR.

## Done when

- Registry equivalence (pack versus export) passes and the golden test runs on the pack-built
  registry too.
- Every typed variant has a test from a fixture line (`tests/fixtures/captures/status-screens`,
  `fight-arena`, `room-walk`, `death-rebirth`, the transcripts).
- Unknown lines map to `Unknown`; everything else to `Other` with the event kept.
- `cargo nextest run -p wayfarer-parse -p wayfarer-world` under the CI budget.

## Tests

- As above; a split-feeding check that typed events are the same whichever way the text
  was chunked (reuse C-010's harness).

## Notes for the agent

- Do not add patterns or change matching; a line that is `unknown` today stays `unknown`.
  Note game texts the KB lacks in the PR's "KB notes".
- Statline **custom** prompts carry `exp` and `wealth` fields; map them when present.
