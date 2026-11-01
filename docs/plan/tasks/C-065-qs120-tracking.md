---
id: C-065
title: "1.20 tracking: openings, weaves and their resists, intercepts, the round after a stab, in the character model and the combat statistics"
tier: B
size: M
wave: 12
depends: [C-063]
crate: wayfarer-brain, wayfarer-proto, ui
touches:
  - crates/wayfarer-brain/src/{combat_stats.rs,fight.rs,character.rs,party.rs}
  - crates/wayfarer-proto/src/lib.rs, ui/src/ (the stats panel)
  - docs/brain.md, docs/web-ui.md
---

## Goal

The brain counts 1.20's extra damage correctly: a Thief's openings and a hybrid's weaves add to
the damage dealt and to the target's damage taken, intercepts move a blow from the ward to the
guard, and the blows after a stab are counted as normal blows. The stats panel shows openings,
weaves and weave resists beside the hits and crits. Nothing changes on 1.11p, where these lines
never come.

## Inputs

- `docs/plan/quicksilver-1.20/handoff.md` section 3, each subsection's "For tracking" notes, and
  5.7 (area damage with no resist line).
- `crates/wayfarer-brain/src/combat_stats.rs` (C-050: hits, misses, crits, backstabs, casts,
  rounds), `fight.rs` (the target's damage taken, the kill), `party.rs` (members' lines).
- C-063's typed events.

## Behaviour

- **Opening** (3.1): its damage is added to the Thief's damage dealt and the target's damage
  taken and counts toward the kill like the hit. It is not a hit, a swing or a crit: a new
  counter, *openings*, and the hit before it keeps its own crit flag.
- **Weave** (3.2): damage added as **spell** damage, not melee. `for 0 damage!` is a landed weave
  that did nothing. A `weave_resisted_*` line is one resisted weave: no damage, counted as
  *weaves resisted* on the target (useful later as evidence of its magic resistance), never a miss
  or a failed cast. A weave followed by the death line is a spell kill. Counters: *weaves*,
  *weave damage*, *weaves resisted*.
- **Immunity hint**: a monster that never takes a weave from a weaver who weaves on other monsters
  is likely immune to its spells. Record per monster name how many hits the weaver landed on it
  and how many weaves; nothing acts on it yet (shown in the dev panel only).
- **Intercept** (3.4): the next monster hit or miss line after an intercept line is the guard's,
  as that line says. The ward's tally is untouched by it. The guard's statistics count *blows
  taken for the ward*; the ward's don't count a dodge, miss or block.
- **The round after a stab** (3.3, 5.4): after a `surprise` hit, further hit and miss lines in the
  same round are normal blows with normal crits. Check the brain doesn't assume a stab ends the
  round or count every blow of the round as a stab; fix it if it does (it may be 1.11p-safe
  either way, so keep 1.11p's counts identical on the 1.11p fixtures).
- **Area spells** (5.7): no resist line; each target's damage line is read as given. Make sure no
  code infers a resist from a smaller-than-expected area hit.
- **Party lines**: openings and weaves by a party member count for that member, as hits do.

## Design notes

- Extend the combat tally with the new counters; the store keeps the tally as JSON (C-050), so add
  fields with `#[serde(default)]` and an old tally still loads.
- The intercept's "next line" is a one-slot pending attribution in the fight state, cleared by any
  monster hit/miss line, the round's end or a new intercept.

## Done when

- Fed the handoff's sequences (`tests/fixtures/qs120/`, from C-063), the tallies come out as the
  handoff says for each case.
- The stats panel shows openings, weaves, weave damage and weaves resisted when non-zero.
- The 1.11p fixtures and replays give the same tallies as before.
- `docs/brain.md` and `docs/web-ui.md` say what the new counters mean.
- fmt, clippy, nextest, `npm run check` and `npm test` pass.

## Tests

- Opening after a hit: damage dealt and taken, kill credit, not a hit or crit.
- Weave, weave for 0, resisted weave, weave then death (a spell kill).
- Intercept then `The orc hits Brannoc for 12 damage!`: the guard took it, the ward didn't.
- A stab then two normal hits in the same round: one stab, two hits, a crit on the second counted
  as a normal crit.
- An old tally JSON without the new fields loads.
