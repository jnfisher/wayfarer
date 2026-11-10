---
id: C-079
title: "Errands beside the running bot: pick up, equip, train, recover the pile and buy spells drop the loop or Go to, and the bot keeps fighting"
tier: B
size: M
wave: 16
depends: []
crate: wayfarer-brain, ui
touches:
  - crates/wayfarer-brain/src/session.rs (errand_control, errand_tick, the walker's arrival)
  - crates/wayfarer-brain/src/errand.rs, trip.rs, shopping.rs (`rewalk`)
  - crates/wayfarer-brain/tests/errand.rs
  - ui/src/lib/bot.ts (errandItems), ui/tests/bot.test.ts
  - docs/brain.md "Errands", docs/web-ui.md
---

## Goal

The errands in a tile's `[▼]` menu (pick up all, equip all, train, recover death pile, and buy
spells) can be started with the bot running. They used to be greyed out ("Stop or pause the bot
first"). Starting one drops the bot's loop or Go to, and the bot keeps running beside the errand:
if something attacks while John is picking things up, the character fights (John, 2026-11-10).

## Behaviour

- **Starting an errand with the bot running**:
  - a walk of the bot's stops after the move in flight;
  - the loop or journey is dropped ("Stopped …: playing here");
  - the bot stays running.

  With nothing to drop, the bot is left as it is (`halt` would stop it).
- **The bot comes first.** While a monster the rules heed attacks, or the character is in
  combat, the errand sends nothing:
  - the bot fights, rests and flees as always;
  - a walk of the errand's is stopped and walked again after the fight (`Active::rewalk`).
  - With the bot stopped or paused, an attack still ends the errand, as before.
- **Where an errand's walk ends** (the trainer, the death pile, a shop), the bot plays from
  there, as after a move by hand (`arrived_by_hand`).
- **Starting the bot while an errand runs** is allowed; the errand goes on.
- **The UI**: the errand items are no longer greyed while the bot runs. Their title says the loop
  or Go to stops and the bot keeps running.
- Unchanged:
  - a follower's errands that walk still take it out of the party first;
  - `walk_to` is still refused while an errand runs.

## Done when

- `errand.rs`:
  - pick up all mid-fight starts at once, sends nothing while engaged, and runs after the
    kill, with the bot still running;
  - pick up all during a Go to stops the walk and the journey, and the bot keeps running.
- `trip.rs`: a walk stopped for a fight is walked again.
- `bot.test.ts`: the items are enabled while the bot runs.
