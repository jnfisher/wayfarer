---
id: C-059
title: "The gear plan: the brain answers a planner's request with every weapon scored by attack, bash and smash against a monster set"
tier: B
size: M
wave: 9
depends: [C-057, C-058]
crate: wayfarer-brain, wayfarer-proto
touches:
  - crates/wayfarer-brain/src/{gear_plan.rs (new),session.rs,brain.rs}
  - crates/wayfarer-proto/src/lib.rs (`gear_plan_request`, `gear_plan`, `GearRow`, …)
  - ui/src/proto/ (generated)
  - docs/protocol.md; docs/brain.md "Gear plan"
---

## Goal

A UI can ask the brain for a character's gear plan: every weapon the character could ever use
(and the rest, greyed), each scored for a plain attack, a bash and a smash against a set of
monsters, from the character's live stats or from what-if values the player types in, with the
buffs the player ticks. The answer is fast enough to recompute on every change of an input.
Solo play only (John, 2026-10-26). The planner UI is C-060.

## Inputs

- C-057 (`combat.rs`: `Fighter`, `Defender`, `Fight`), C-058 (`gear.rs`: `Catalogue`,
  monster sets).
- The spell plan as the pattern for a request and its answer: `spell_plan_request` /
  `spell_plan` in `crates/wayfarer-proto/src/lib.rs`, `Session::spell_plan` in
  `crates/wayfarer-brain/src/session.rs`, `ui/src/lib/spells.ts`.
- The character model: level, class, race and the six stats (`character.rs`, from `st`); the
  load from `i` (`Encumbrance:`; the View's `load`); the carried items and which weapon is
  readied, when known.
- `docs/kb/spells.md` (spell strength at a level), `docs/kb/combat.md` "Attack modes".

## Behaviour

### Request

`gear_plan_request { session, monsters?, what_if?, buffs?, smash?, filter? }`:
- `monsters`: `{ loop: <file> } | { area: <name> } | { names: [..] }`. Left out: the loop the
  character runs; with none, the spawn pool of the area around the believed room; with no
  position either, the answer says so and scores nothing.
- `what_if`: any of `level`, `str`, `int`, `wil`, `agi`, `hea`, `cha`, `load_percent`. Left
  out, the character's own (live) values. Without a known value (no `st` yet), the answer
  names what is missing and scores nothing.
- `buffs`: the names of the buffs ticked (below).
- `smash`: score smash as though the character had it. Smash is shown only for the classes
  that may keep it, from the level they may (`combat.md`: Warrior 22, Witchunter 20, Paladin
  25, Cleric 27, Warlock 27, Ranger 22), and only with `smash` set (the planner cannot see
  whether the character has it).
- `filter`: `now`, `later`, `all` (default `all`: never-usable rows are listed with their
  reasons but not scored).

### Buffs (checkboxes, John 2026-10-26)

The brain offers a list of buffs, sent with the answer: every spell in the world pack, of any
class's magery, that lasts (`duration` above 0), targets the caster, a single player or the
party, and raises AV (abilities 22, 105, 106), Alter Damage (4), Alter Critical (58),
Strength (46), Agility (48) or lowers Alter Energy Usage (87). A buff's ability whose value is
0 takes the spell's strength (`minval`/`maxval` at the caster level the spell is learned,
`kb/spells.md`). Each comes with its name, the class that casts it and its effect ("song of
valour: +5 AV"). The Witchunter's Anti-Magic resists friendly buffs some of the time: show a
note on its rows, don't model the roll.

### Load

`load_percent` from `what_if`, else the live load with the readied weapon's weight swapped for
each candidate's (when the readied weapon is not known, the live load as it is). An off-hand
item is not added automatically; the planner can say "with a shield" by a what-if load.

### Answer

`gear_plan { session, inputs, monsters, buffs_offered, rows, missing? }`:
- `inputs`: the values used (live or what-if), so the UI can show them.
- `monsters`: name, HP, AC, DR, dodge, and how many of the set's rooms spawn it.
- `rows`, one per weapon (armour rows are a later card): item number and name, damage, speed,
  Str needed and the shortfall, min level, two-handed or not, status and reasons, the sources
  (from C-058, with prices for this character's Charm), limited, deleted at cleanup. For each
  move scored: energy per blow, blows in rounds 1 to 3, hit and crit per cent (averaged over
  the set), and, **averaged over the monster set**: expected damage in round 1, chance of a
  kill in round 1 and by round 2, expected rounds to kill. Monsters are weighted by how many
  of the set's rooms spawn them.
- The best move of each row and the best row of the answer marked (fewest expected rounds;
  ties to the higher round-1 kill chance).

`gear_detail_request { session, item, … same inputs }` → `gear_detail`: one weapon against each
monster of the set, every number per monster, per move, and the blows-per-round sequence for 8
rounds.

### Speed

Scoring the weapons a class can use or will (about 150) × 3 moves × a 10-monster set must
answer in under 100 ms on CI (a test asserts it; comment why). Cache the catalogue per class
and race; no cache of results is needed at that speed.

## Design notes

- `gear_plan.rs`: builds the `Fighter` from the inputs and buffs, the `Defender`s from the set,
  runs `combat::Fight` per row and move, and assembles the wire rows.
- Proto: `GearPlanRequest`, `GearPlan`, `GearRow`, `GearMoveScore`, `GearSource`,
  `GearBuff`, `GearDetailRequest`, `GearDetail`; regenerate the TypeScript.
- The brain answers on request only; it never pushes a plan.

## Done when

- A request with live values and one with what-if values both answer, in time.
- The fixture case below scores as stated.
- `docs/protocol.md` documents both requests and answers; `docs/brain.md` has a "Gear plan"
  section.

## Tests

- A Kang Witchunter what-if (L15, Str 95, Int 30, Wil 45, Agi 56, Hea 91, Cha 41, load 51%)
  against the set {kobold warrior, bandit leader, smuggler, skeletal warrior, dark goblin, grey
  wolf, minotaur, minotaur champion, guardian golem}: bash beats attack for the bone-handled
  headcutter and for the Magebane sword; at L20 (Str 97, Agi 62, load 54%) the Magebane bash
  needs fewer expected rounds than the headcutter bash (2 bashes every round against 1 or 2).
- A Gnome Druid what-if (L10, Str 60, Int 50, Wil 70, Agi 45, Hea 75, Cha 35, load 42%): the
  maul's best move is bash; the darkwood staff's is attack, and it needs fewer expected rounds
  than the maul's bash.
- Buffs: ticking song of valour adds 5 AV (the row's hit chance rises); a buff list is sent.
- Smash: absent for a Bard; for a Witchunter below L20 only with `smash` and a level what-if.
- Missing inputs: no `st` yet → `missing: ["stats"]`, no rows scored.
- Timing: the full Witchunter request under 100 ms.

## Notes for the agent

- Keep the answer small: rows carry averages; the per-monster numbers come only with
  `gear_detail`.
- The prototype these numbers came from (a Python model written during the 2026-10-26
  conversation with John) is not in any repository; C-057's formulas are the specification.
