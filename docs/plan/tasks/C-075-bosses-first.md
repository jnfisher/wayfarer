---
id: C-075
title: "Bosses first: a boss in the room is the target before its guards, for every member"
tier: B
size: M
wave: 15
depends: [C-073]
crate: wayfarer-brain, wayfarer-world, ui
touches:
  - crates/wayfarer-brain/src/policy.rs (target)
  - crates/wayfarer-brain/src/fight.rs (choose, pick, retargeting)
  - crates/wayfarer-world/src/monsters.rs (MonsterRule: a "first" kind, defaults)
  - crates/wayfarer-brain/src/rules.rs, crates/wayfarer-proto/src/lib.rs, ui/src/proto/ (generated)
  - ui/src/StatsPanel.svelte (the rules list)
  - docs/brain.md ("Fight here", "Monster rules", "Assist")
---

## Goal

When a boss is in the room, the party fights it before its guards. A new monster rule, **attack
first**, marks a monster as the top target. Every boss in the game data has it by default, and
the player can add it to any monster or remove it, as with C-073's rules.

## Inputs

- **The Thrag run** (2026-11-04, `~/.local/share/wayfarer/logs/lexi/notes.log`, ts 1793826226000
  to 1793826299000).
  - Lexi, the leader, walked into Thrag's room. She fought a dire wolf, another dire wolf and a
    large dire wolf before "Fight: Thrag" at 1793826249291.
  - The followers assist the leader's target, so they went for the wolves too.
- `policy::target`: an attacker first, else the first hostile monster listed. The wolves attack,
  so they come first.
- `Fight::choose`: a follower with nothing attacking it takes the leader's target. One attacked
  fights its attacker.
- `Monsters::is_boss` / `boss_named` (`monsters.rs`): `maxalive` 1 and `regenhrs` > 0. Thrag
  (1:1810) is one (`docs/kb/progression.md`, "Boss loot").

## Behaviour

- **Attack first** (wire `first`). The bot picks a monster with this rule over any other target,
  attackers included.
  - If several are here, it picks by the order the room lists them.
  - It is picked even while something else attacks the character.
  - It is not picked if the bot can't hurt it: a target the game refused, or a stalemate.
- **Bosses are *attack first* by default.** This is a default layer built from the game data
  (every `boss_named` monster), shown in the rules list as "attack first · default (boss)".
  Removing it works as C-073's default removal does.
  - Precedence: the player's rule for the pattern wins over a default; the longest match wins as
    today.
  - A boss the player set to *ignore* is ignored.
- **Retargeting.** When an *attack first* monster comes into the room (or is first seen by the
  `look` after a kill) while the character fights something else, the bot switches to it at once:
  `<attack> <name>`, narrated "Fight: Thrag is here: switching from dire wolf".
  - It switches once per arrival, not every round.
- **The party.**
  - The leader's choice follows the rule.
  - A follower goes for an *attack first* monster in the room even while something else attacks
    it. This changes C-042's "attacked by another, it fights that one", for this case only. The
    game shares exp only among those fighting the monster that dies (`kb/grouping.md` §9), and
    the boss is what the party came for.
  - Every member's own rules decide: a follower with the boss ignored doesn't switch.
- **Spells.** An attack spell goes at the *attack first* target too. `spells.max_casts_per_monster`
  counts per monster, as today.

## Design notes

- `MonsterRule` gains `First` beside C-073's `Ignore` and `Avoid`. `policy::target` checks
  `First` among the monsters here before the attacker pass.
- The boss default is a rules layer built once per world pack, from `Monsters`: lower-cased boss
  names as patterns. It goes under the player layer and over nothing else. Keep
  `DEFAULT_RULES` for the hand-written ones (the ooze).
- Retargeting lives in `Fight`: when it is engaged and `target()` would now pick a different
  *first* monster, send the attack line once. Don't retarget for anything else.
- Keep "Never leaves the room itself" as it is.

## Done when

- The Thrag room as a test: two dire wolves attack the leader, and Thrag is listed last. The
  leader's first attack is `bash Thrag` (its attack command). A follower attacked by a wolf goes
  for Thrag too.
- Thrag arriving mid-fight with a wolf: one switch line, then no more.
- The rules list shows the boss defaults for bosses the character has met, or all of them
  behind a "show defaults" toggle (agent's choice, say which in the PR). Removing one works.
- `docs/brain.md` covers the rule, the boss default and the follower exception.
- The checks pass.

## Tests

- `policy::target` with attackers and a *first* monster present picks the *first* one.
- With two *first* monsters it picks the listed order. A refused or stalemated *first* monster
  is skipped.
- Player-layer precedence: a player rule of *ignore* on a boss wins over the default.
- `Fight` retargeting: one switch on arrival, none after.
- Follower assist with an attacker present, and a *first* monster here.

## Notes for the agent

- Work in a worktree. Writing under `crates/` in John's checkout restarts his live brain.
- Runs after C-073. It builds on C-073's rule kinds and its remove-any-rule UI.
- Some bosses are not worth a party's fight at its level. That is the player's call ("the player
  decides where to hunt"), made by removing the default or ignoring the boss. Don't add a
  difficulty check.
