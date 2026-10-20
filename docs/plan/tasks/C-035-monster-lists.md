---
id: C-035
title: Monster lists: never fight, flee from and avoid by name; a stalemate ends a fight; the gigantic black ooze is never fought
tier: B
size: M
wave: 4
depends: [C-028, C-030, C-034]
crate: wayfarer-world, wayfarer-brain, wayfarer-proto, ui/
touches:
  - crates/wayfarer-world/src/monsters.rs (`never_fight` default list, `MonsterRule`)
  - crates/wayfarer-brain/src/{policy.rs,fight.rs,flee.rs,modes.rs,task.rs,session.rs} (rules; stalemate; Flee on sight)
  - crates/wayfarer-proto/src/ (`monster_rules`, `set_monster_rule`), ui/src/ (a rules list in the dev panel or stats tab)
  - docs/brain.md, docs/protocol.md, docs/plan/04-settings.md
---

## Goal

John (2026-10-20, during the M3 soak): Thorgrim's Wooden Passage loop met the **gigantic black
ooze** (monster 63: 2,500 HP, regenerates 500 a round, damage reduction 40, 9,075 exp) in a
Labyrinth Passage and attacked it. Every swing "hits, but glances off its armour"; the ooze
mostly misses and hits for 1 to 12. The fight is a stalemate: the bot stays in Fight for as
long as the ooze stays, HP never reaches the flee line, the loop never continues. The ooze
roams the island (it is not lair-bound), so skipping rooms does not avoid it. In the original
game it was not meant to be fought; MegaMud's manual uses it as the example of its "Flee"
monster class (`docs/kb/megamud-manual.md`, "Flee: run from it. Attack only if you can't move.
Its presence overrides everything else in the room").

Two things, both from `docs/plan/04-settings.md` "Monsters and players" (per name: friend,
avoid, enemy, flee from), brought forward from M4 because the soak needs them:

1. **Rules by monster name**: *never fight* (do not attack it; fight on if it attacks),
   *flee from* (leave the room as soon as it is seen, whatever the HP; fight only if the bot
   cannot leave) and *avoid* (do not walk through rooms where it was last seen, when there is
   another way). Matching as MegaMud: case-insensitive substring, longest name first.
2. **A stalemate ends a fight**: when the bot's own swings have done no damage for a number of
   rounds, the monster is unfightable for this character now; the bot stops attacking, leaves
   the room as Flee does, marks the monster *never fight* for the session and says so.

## Inputs

- `docs/kb/megamud-manual.md` 9.2 "Relationships" (Friend, Avoid, Enemy, Flee, Hangup) and its monster-list matching,
  `docs/kb/combat.md` "Who sees the misses" (a glance: the armour took all the damage;
  the exact line is `Your swing at <name> hits, but glances off its armour.`, confirmed live
  2026-10-20 [C]).
- C-028 (Flee: the exit choice, the followed-hop logic, `Flee::leave`), C-030 (Task mode: the
  loop's stops and `--skip`), C-034 (`monsters_by_name`, boss lairs), C-029 (the store: the
  session's rules persist with the bot's record).
- The recorded soak at `target/live-check/20261020-080407-61354/` on John's laptop (the gateway
  logs under `01-logon/logs/probe/`): the ooze fight from seq 252 on. Copy the ooze's room
  lines into a fixture under `tests/fixtures/` if a replay test wants them (no account names
  in the copy).

## Behaviour

- **Rules** (`wayfarer-world`): `MonsterRule { NeverFight, FleeFrom, Avoid }`; `MonsterRules`
  holds `(pattern, rule)` pairs and answers `rule_for(name) -> Option<MonsterRule>` by the
  longest matching pattern, case-insensitive, substring. The pack ships a **default list**
  in code (not the database): `gigantic black ooze` is *flee from*. The defaults are a
  constant with a comment per entry saying why; settings (M4) will layer over them.
- **Fight** never targets a *never fight* or *flee from* monster (`policy::target` takes the
  rules); a monster that attacks the character is fought back unless it is *flee from*, in
  which case Flee takes over even above the flee line.
- **Flee on sight**: a *flee from* monster seen in the room (`Presence::here`, whether or not
  it has attacked) makes Flee urgent: the bot leaves by Flee's usual exit choice at once,
  narrating "Fleeing: gigantic black ooze is here (flee from)". Mid-walk, the walker yields to
  Flee as it does for the HP line. After the flee, the loop goes on from the next stop; the
  room is not marked cleared. If no exit is usable (Flee's give-up), the bot fights only when
  attacked, as MegaMud does.
- **Avoid**: the router (`route`) treats a room where an *avoid* or *flee from* monster was last
  seen (this session, from the belief's `Presence` history) as costly (not impassable: add a
  large cost so another way wins when there is one); a loop stop in such a room is walked to,
  looked at from the neighbour if the walker can, else skipped once with a narration. Keep this
  simple: the cost, and the stop skip. The memory of sightings clears after
  `AVOID_MEMORY` (30 minutes).
- **Stalemate**: Fight counts the character's own damaging hits and glancing blows per target
  (new `GameEvent` variants from the texts: `SwingGlanced {target}` and the existing hit
  events). After `STALEMATE_ROUNDS` (4) rounds of only glancing blows and misses, with no
  damage dealt, Fight gives the target up: narrates "Stalemate: my swings do nothing to
  gigantic black ooze; leaving", adds it to the session's *never fight* list (persisted in
  the bot's record, with the time) and hands over to Flee's `leave` (as the log-off path
  does). A stalemate on a monster already in the list is not counted again.
- **Protocol**: `bot` snapshot gains `monster_rules: [{pattern, rule, source: default |
  session | player}]`; `set_monster_rule {session, pattern, rule | null}` adds, changes or
  removes a *player* rule for the session (stored with the bot's record, C-029). The UI gets a
  small list in the stats tab (C-029) with add and remove: enough for John to add a name
  during a run; the settings page (M4) replaces it.
- **Live check**: `--scenario soak` gains `--flee-from <name>` (repeatable), passed to the brain
  as `--flee-from`, so a soak can declare a monster without a UI.
- Docs: `docs/brain.md` (Fight and Flee sections), `docs/protocol.md`, `04-settings.md` (the
  defaults list is noted as the seed of the Monsters and players group), KB notes in
  `docs/kb/combat.md` for the glancing line and the ooze's numbers.

## Design notes

- The rules are a policy input, not a mode: Fight and the walker consult them; Flee already
  knows how to leave. Keep the stalemate counter in Fight (it owns the target).
- "Flee from" overrides the flee line; "never fight" does not (an attacker that is *never
  fight* is fought back, the HP line still applies).
- The default list is deliberately one entry. A data rule ("regenerates more than a tenth of
  its HP a round") does not separate the ooze from the pack: nearly every monster regenerates
  a tenth a round. What makes the ooze unfightable is its damage reduction against this
  character's damage, which is why the stalemate rule exists: it is the data-free general case.
- Substring matching means `ooze` also matches `glittering ooze` (monster 172, fightable):
  the default uses the full name. Document the pitfall in the help text.

## Done when

- World tests: `rule_for("gigantic black ooze")` is *flee from* by default; `rule_for("ooze")`
  is none; the longest-pattern rule wins when two match; case-insensitive.
- Brain tests (fake game): a *flee from* monster seen in the room makes the bot leave at full
  HP and the loop continues from the next stop; a *never fight* monster present is not
  attacked and an ordinary one beside it is; a *never fight* monster that attacks is fought;
  four rounds of `glances off its armour` with no damage end the fight with the narration, the
  monster is in the session list and the bot leaves; a replay of the ooze fixture (if copied)
  shows Fight ending by the fourth round; `set_monster_rule` round-trips through the snapshot
  and survives a brain restart (store).
- `route` prefers a longer way around an *avoid* room when one exists and still routes through
  it when it is the only way.
- **Live** (pool character, Newhaven, `--flee-from` naming a Newhaven monster that is present,
  e.g. the one the fight-here scenario attacks): the bot leaves the room on sight and does not
  attack it; summary in the PR.
- `docs/brain.md`, `docs/protocol.md`, `docs/web-ui.md` updated; checks green.

## Tests

- As above; the glancing-blow text goes into the typed-event tests from the game texts.

## Notes for the agent

- John's own characters (Waydeva, Waydevb, Thorgrim) are not yours to log in; the live check
  uses the pool character only. Never start, stop or reset the silvermere server.
- The ooze is on the island (map 1); the pool character's live check stays in Newhaven. Test
  the ooze path in-process from the recording or with the fake game.
