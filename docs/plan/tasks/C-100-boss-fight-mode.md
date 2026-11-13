---
id: C-100
title: "Boss fight mode: fight in place, never flee or walk, its own casts and hang-up"
tier: B
size: M
wave: 16
depends: [C-099]
crate: wayfarer-proto, wayfarer-store, wayfarer-settings, wayfarer-brain, ui
touches:
  - crates/wayfarer-settings/src/lib.rs (`spells.boss_max_casts_per_monster`, `health.boss_hangup_below_hp`)
  - crates/wayfarer-proto/src/lib.rs (`bot.boss_mode`, `set_boss_mode`)
  - crates/wayfarer-store/src/lib.rs (`BotRecord::boss_mode`)
  - crates/wayfarer-brain/src/session.rs, modes.rs, recover.rs, brain.rs
  - ui/src/Tile.svelte (the skull toggle), ui/src/lib/bot.ts, ui/src/lib/header.ts, ui/src/lib/gateway.ts
  - docs/brain.md, docs/protocol.md, docs/web-ui.md
---

## Goal

A mode for a boss fight the player runs at the keys: the player walks the party into the
boss's room, and each bot fights there and keeps the party healed. It does not run, and it
never moves on its own. The player moves the leader out by hand, and the game's follow takes
the others with it.

John (2026-11-12), planning the Champion of Blood and the greater hellion: "this is a mode
where I am at the keys and I move the 4 bots into the room, they blast, they don't run, and I
have some control over the automation". Then, on fleeing: "I dont want any automated movement
in this mode. no flees."

## Behaviour

- **The toggle**, per character, on its tile: a skull when on, "–" when off. `set_boss_mode
  {session, on}`; `bot.boss_mode` in the snapshot. Kept with the bot's record, so it stays on
  across a brain restart until turned off.
- **Turning it on** ends any loop (`run_loop`) or walk (`walk_to`, `walk_loop`, a goto) under
  way, and starts the bot where it stands if it was stopped.
- **While on:**
  - No flee of any kind: not for HP, not for a stalemate, not for a follower's flee line, not
    for a log-off.
  - No movement of the bot's own: no loop or walk, no Recover walk to a quiet room or back
    home, no walk back to a party, no errand or trip. The game's party follow is not the bot's
    and goes on.
  - Fights what is in the room, by the monster rules.
  - Casts the attack spell chosen with C-099 (or auto's), up to `spells.boss_max_casts_per_monster`
    (default 10; 0 is no limit) at one monster, with no mana floor
    (`spells.attack_mana_above` does not apply).
  - Heals itself and the party in and out of a fight, as the healing settings say.
  - Rests where it stands between fights.
  - Loots as usual.
  - Casts no blessings, in a fight or out of one.
  - Hangs up below `health.boss_hangup_below_hp` (default 10%) in place of
    `health.hangup_below_hp`.
- **Turning it off** returns the bot to normal play where it stands: it does not take up the
  loop or walk it was on.

## Done when

- Tests: on ends a loop under way and starts a stopped bot; under the flee line in a fight it
  does not flee and hangs up at the boss line; a stalemate does not flee; Recover rests in
  place in a room that spawns with `health.rest_quiet_room` on; no blessing is cast; the boss
  casts limit and the missing mana floor; off resumes normal play without the loop; the mode
  survives a brain restart.
- UI test: the tile shows the skull or "–" and the click sends `set_boss_mode`.
- fmt, clippy `-D warnings`, nextest, vitest, svelte-check.
