---
id: C-063
title: "Quicksilver 1.20 lines: the 1.20 patterns as data, typed events, the critical stab; the KB's 1.20 page"
tier: B
size: M
wave: 12
depends: []
crate: wayfarer-parse
touches:
  - crates/wayfarer-parse/src/{registry.rs,table.rs,typed.rs,event.rs}
  - crates/wayfarer-parse/tests/
  - tests/fixtures/qs120/ (new: patterns-1.20.json, lines.txt, PROVENANCE.md)
  - docs/kb/quicksilver-1.20.md (new), docs/kb/README.md, docs/kb/changelog.md
---

## Goal

Every line Quicksilver 1.20 adds is classified by wayfarer's parser, on every server, and turns
into a typed event the brain can use. Stock 1.11p text classifies exactly as before. The KB gets
its 1.20 page, kept apart from the 1.11p facts.

## Inputs

- `docs/plan/quicksilver-1.20/handoff.md` sections 1 ("The 1.20 line patterns can always be
  registered"), 3 (every pattern, priority, example and tracking note) and 8 (what is unchanged).
- `crates/wayfarer-parse/src/registry.rs` (priorities: 50 static, 55-59 game texts, 60 other
  players, 70-90 fallbacks; lower wins; within a priority the more literal wins) and
  `table.rs` (the static table embedded from `tests/fixtures/parse-golden/patterns.json`).
- `crates/wayfarer-parse/src/typed.rs`: the typed events the character model reads.
- `tests/fixtures/parse-golden/`, `tests/fixtures/captures/`, `tests/fixtures/transcripts/`: the
  1.11p corpus that must classify unchanged.
- `docs/kb/README.md` (the provenance tags), `docs/kb/screen-patterns.md`.

## Behaviour

- **The 1.20 patterns** of handoff section 3 are registered for every connection, whatever the
  rule set: `opportunist_{you,room,victim}`, `weave_{you,room,victim}`,
  `weave_resisted_{you,room,victim}`, `guard_intercept_{you,ward,room}`, `guard_start_{you,ward}`,
  `guard_stop_{you,ward}`, `guard_not_guarding`, `guard_not_here`, `guard_not_party`, `guard_self`,
  `guard_resting` and `look_weapon_strreq`. Priority 40, and 39 for the `_victim` forms, so they
  are tried before every stock pattern. Each regex is a full match on the stripped, trimmed line,
  as the existing patterns are.
- **The critical stab.** `You critically surprise slash giant rat for 140 damage!` and
  `Thornwyn critically surprise slashes giant rat for 140 damage!` classify as `you_hit` /
  `other_hits` with both `crit` and `surprise` set. Check the existing hit patterns and the hit
  typed events carry both flags (they may drop one when both occur); fix them if not.
- **Typed events** (names are the builder's call), one per family: an opening (who, verb, target,
  damage), a weave (who, spell, target, damage, resisted), an intercept (guard, monster, ward),
  guard start/stop (guard, ward, from whose side), a guard refusal (which), and a weapon's Str
  requirement (required, have). "You", "room" and "victim" become a field, not three events.
- **The `HELP 1.20` answer** (handoff section 2, its 34 lines) is not classified line by line here;
  C-064 swallows it as a block. This card only makes sure none of those lines matches a 1.20
  pattern as a game event (`GUARD <player>: take blows ...` must not read as a guard line).
- `help_none` (`Sorry, no help is available for that topic!`) exists as a pattern; add it as a
  stock 1.11p pattern if the table lacks it (it is a stock line, so priority 50, KB tag
  [I] from the handoff unless a capture shows it).

## Design notes

- **The 1.20 patterns are data**, not Rust: `tests/fixtures/qs120/patterns-1.20.json`, in the
  shape of the existing `patterns.json` `static` entries (`name`, `cat`, `rx`, `example`, `prio`)
  plus `"rules": "1.20"` on each, as the server's own export marks them. It is embedded with
  `include_str!` and merged into the registry beside the static table. The `rules` field is not a
  detection signal (handoff section 2 "Not signals"); keep it on `Pattern` so tools and the dev
  panel can tell a 1.20 pattern from a stock one.
- Don't edit `tests/fixtures/parse-golden/`; the golden parity with the prototype's export stays
  as it is. The 1.20 entries come from their own fixture.
- `tests/fixtures/qs120/lines.txt`: every example line of handoff section 3, one per line, with
  the pattern name it must classify as (tab-separated), plus the three backstab lines of 3.3 and
  the four-line intercept sequence of 3.4.
- `PROVENANCE.md` there: every file comes from the handoff (`docs/plan/quicksilver-1.20/handoff.md`,
  the server's frozen 1.20.0 patterns export as quoted there); no capture yet.

## Done when

- Every line of `tests/fixtures/qs120/lines.txt` classifies as named, with the named fields.
- The whole existing 1.11p corpus (golden tests, captures, transcripts) classifies exactly as
  before: same names, same fields, same counts.
- A critical stab yields a hit event with both crit and stab set.
- `docs/kb/quicksilver-1.20.md` exists with a "Lines" section (section 3 of the handoff, the
  patterns and their tracking notes) and a short header saying the page holds 1.20 facts only
  and 1.11p facts stay in their own pages; `docs/kb/README.md` lists it and defines the tag
  **[Q]**: "from the Quicksilver 1.20.0 handoff (the server's frozen 1.20.0 release)", with
  "live" added when a fact is then seen on a server; a `changelog.md` line.
- fmt, clippy and nextest pass.

## Tests

- `lines.txt` driven: each line classifies as its named pattern, with its fields.
- The corpus regression: the existing golden and capture tests unchanged, plus one test that no
  line of the 1.11p captures matches a pattern whose `rules` is `1.20`.
- The 34 `HELP 1.20` lines: none classifies as a 1.20 event.
- A weave line with "into" in the spell name splits on the last ` into ` (handoff 3.2).
- Typed events for each family, including the critical stab.

## Notes for the agent

- Priorities: wayfarer's scale has game texts at 55-59, so 40 and 39 sit before all of them, as
  the handoff intends.
- `rest_start`, `meditate_*` and the prompt are unchanged in text (handoff 3.7); nothing to add
  for them here.
- The regen-line cast and fade texts (handoff 3.8) are spell texts. They don't come from the
  stock world pack, so they don't exist for wayfarer until C-066 adds the 1.20 spell data; C-066
  registers them.
