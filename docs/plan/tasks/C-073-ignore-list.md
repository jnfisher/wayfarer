---
id: C-073
title: "Monster rules: \"ignore\" replaces \"flee from\"; every rule, defaults included, can be removed in the page"
tier: B
size: M
wave: 15
depends: []
crate: wayfarer-world, wayfarer-brain, wayfarer-proto, ui
touches:
  - crates/wayfarer-world/src/monsters.rs (MonsterRule, DEFAULT_RULES)
  - crates/wayfarer-brain/src/rules.rs, policy.rs, flee.rs, fight.rs, modes.rs, session.rs
  - crates/wayfarer-proto/src/lib.rs (MonsterRuleKind, set_monster_rule), ui/src/proto/ (generated)
  - crates/wayfarer-store/src/lib.rs (stored rules: the old kind read as the new)
  - ui/src/StatsPanel.svelte (the rules list)
  - docs/brain.md ("Monster rules"), docs/ui.md or wherever the panel is described
---

## Goal

No monster makes the bot flee on sight. *Flee from* becomes **Ignore**: the bot never attacks
the monster, never leaves because of it, and plays on around it. The gigantic black ooze is
ignored by default. Every rule in the list can be removed from the page, including a default or
one the bot learned from a stalemate.

## Inputs

- **The Thrag run's logs** (2026-11-04, `~/.local/share/wayfarer/logs/*/notes.log` on the
  server, ts around 1793824640000 to 1793824810000). The ooze in the Labyrinth set all four
  members fleeing on their own, in four directions:
  - Penny north, Lexi south, Sasha north, Rocco east.
  - Rocco fled the ooze about fifteen times in three minutes, each time chased six rooms by a
    huge tiger beetle, until "cornered: no exit to flee by".
  - The party fell apart.
- C-035 (`C-035-monster-lists.md`) and `docs/brain.md` "Monster rules": the three kinds today.
  - *never fight*: not attacked unless it attacks.
  - *flee from*: leave on sight.
  - *avoid*: the router keeps away from where it was seen.
- `policy::target`, `policy::flee_from`, `Flee::wants`/`step` (`forced`, `to_flee`) and
  `Bot::flee_urgent`.
- `DEFAULT_RULES` in `monsters.rs`: the ooze as `FleeFrom`.
- `StatsPanel.svelte` "monster rules": only `source === "player"` rows have the remove button.

## Behaviour

- **Ignore** (the new kind, wire `ignore`). The bot never starts a fight with an ignored monster
  and never picks it as a target, even while it is in the room. If it attacks the character, the
  bot fights back. (*Needs John* if he wants "never, even when attacked"; the default is to fight
  back, as *never fight* does.)
  - Nothing about an ignored monster starts Flee. The bot flees only by HP.
  - At a loop stop where only ignored monsters are left, the stop counts as cleared, as it does
    today (`bot.stop_quiet_seconds`).
- ***Never fight* and *ignore* become one kind.** They differ today only in Flee, and Flee goes
  away. The list shows "ignore". A stalemate teaches "ignore" ("Stalemate: … ignoring it from
  now on").
- ***Flee from* is gone.**
  - Stored player and session rules of kind `flee_from` or `never_fight` load as `ignore`.
  - The wire's `set_monster_rule` accepts the old names for one release and stores `ignore`.
  - `wayfarer brain --flee-from` becomes `--ignore`, with `--flee-from` kept as a hidden alias.
- ***Avoid* stays as it is.** The router keeps away from where the monster was last seen.
- **Defaults.** The ooze is `ignore` by default, with its reason kept.
- **Removing a rule** in the page, any source:
  - A **player** rule is deleted.
  - A **session** rule (learned from a stalemate) is forgotten, and the store drops it.
  - A **default** rule is switched off for this character: the player layer holds an "off"
    entry for the pattern, which is shown struck through with a button to put it back.
  - The `x` button is on every row. A default or session row says what removing it does in its
    tooltip.
- **The rules list shows the reason** for a default (the ooze's armour and regeneration) as a
  tooltip.

## Design notes

- `MonsterRule` becomes `{ Ignore, Avoid }` plus an "off" marker in the player layer. Keep
  `MonsterRules::rule_for`'s longest-match semantics. "Off" for a longer pattern beats a shorter
  pattern's rule.
- Remove `policy::flee_from`, Flee's `forced` flee-on-sight path, `Flee::sighted` and
  `Bot::flee_urgent`. The stalemate's "leave the room" stays as it is: a stalemate is a fight
  the bot started or was dragged into, not a sighting. (C-074 changes what a party member does
  there.)
- The wire change regenerates `ui/src/proto/`. Bump whatever protocol version C-014 set, if
  the shape changes.

## Done when

- With the ooze in the room, no member of a party leaves. Each fights what else is there, and
  the ooze is never targeted unless it attacks.
- An old store with `flee_from` and `never_fight` rows loads them as `ignore`.
- In the page, John can remove the ooze default, a stalemate-learned rule and a player rule,
  and can put the ooze back.
- `docs/brain.md` "Monster rules" describes ignore, avoid and removal. The `--flee-from` flag
  doc is updated.
- The checks pass, and `npm run check` (or the UI's equivalent) passes.

## Tests

- `policy::target` never picks an ignored monster that isn't attacking, and picks it when it
  attacks.
- Flee does not start for an ignored monster at full HP.
- Store migration: old kinds load as `ignore`.
- Rules layering: a player "off" for the default ooze makes `rule_for` return `None`. Putting
  the default back restores it.
- The UI's existing panel tests (if any) cover the remove button on a default row.

## Notes for the agent

- Work in a worktree. Writing under `crates/` in John's checkout restarts his live brain.
- `docs/kb/` doesn't change: the ooze facts stay as they are.
