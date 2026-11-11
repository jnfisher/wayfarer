---
id: C-093
title: "Stash what is carried now, and only where the bot is sure it is; connect to recover in Silvermere"
tier: C
size: M
wave: 16
depends: [C-092]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/stash.rs, crates/wayfarer-brain/src/modes.rs, party.rs, task.rs
  - crates/wayfarer-brain/src/session.rs, brain.rs (`recover_to`)
  - crates/wayfarer-proto/src/lib.rs (`RecoverTo`), ui/src/proto/
  - ui/src/Tile.svelte, ui/src/lib/bot.ts, ui/src/lib/gateway.ts (the connect menu)
  - docs/web-ui.md
  - crates/wayfarer-brain/tests/ (task.rs, party.rs)
  - docs/brain.md "Stash currency"
---

## Goal

A stash hides the coins the character carries when it hides them, not the coins the last `i`
showed. It hides them only in the loop's first stop, and only when the bot is sure that is
the room it is in. Coins hidden in the wrong room of a maze of look-alikes are lost to the
player.

John (2026-11-11): "make a card for the stash fix and also we need it to not stash unless its
sure of its room so we dont accidentally put coins in the wrong place."

John, later the same day, after Rocco and Sasha were hung up at 5% HP: "I need a way to
connect and (1) disable the bot (2) do not hang up again because we're low on health (3)
automatically send "sys goto silvermere" … Right now if I press "connect", the characters will
log in and instantly disconnect again if monsters are in the room … Possibly turn the "Connect"
button into a dropdown that offers the recover to silvermere option."

## Inputs

- v0.1.9 live logs, 2026-11-11 07:12 (Rocco's loop "Orc Barracks", first stop Huge Cavern,
  Training Area):
  - Sasha's player typed `share 1800 gold` one second before the stash ("You gave Rocco 900
    gold crowns"; Rocco saw "Sasha gave you 900 gold crowns").
  - Sasha then sent `hide 5 silver`, `hide 1859 gold` and `hide 8 copper`. The game answered
    the gold with "You don't have 1859 gold crowns to hide!", and she kept her 959 gold.
  - Rocco sent `hide 1 gold` and kept the 900 he had just been given.
  - Both hid from the coins of their last `i`. A give in either direction doesn't change the
    brain's coin counts.
- Huge Cavern, Training Area is one of many rooms with the same name and look (C-084, C-085),
  so the belief can be sure of the wrong copy.
- `stash.rs`, `modes.rs` `View::certain`, `position.rs` `belief.certain()`.

## Behaviour

- **Fresh counts.** A stash starts with `i` and hides from what that `i` shows. Coins that
  arrive or leave in between (gives, shares, pickups) are counted.
- **"You don't have N … to hide!"** That coin is read again with `i` and hidden once more
  with the new count, once. If it fails again, the stash goes on to the next coin and the
  narration says so.
- **Sure of the room.** A stash hides nothing unless all of these hold when it sends its
  first `hide`:
  - the belief is certain (`View::certain`);
  - the believed room is the loop's first stop;
  - the room display that made it certain came after the arrival: the room name and exits
    shown match the stop's.

  If any of them fails, the stash narrates "Stash: not sure this is <room>, not hiding" and
  is dropped for this time round. It isn't put off to a later tick in another room.
- **A follower** borrows the leader's certainty. It hides when the leader is certain it is at
  its first stop (the leader's reported room key) and the follower is in the same room: it
  came in by "-- Following your Party leader … --", the room shows the leader ("Also here:
  Rocco") and the title and exits match. Otherwise it skips the round. Its own belief is not
  required: on the 07:00–07:50 watch Sasha's belief was sure in only 23 of 635 room
  displays, usually 8 or 135 look-alikes. A rule that needed her to be sure would mean she
  never stashed.
- The narration names the room and its key ("Stash: hiding 571 gold in Huge Cavern, Training
  Area (5/123)"), so the player can find the right copy.

- **The leader holds** at its first stop while a member with the round has not finished
  ("Waiting for Lauren to hide their coins", at most `STASH_HOLD`, a minute). The game takes a
  follower along when the leader moves, so a slow follower would hide its coins a room on. The
  round is published while the leader is sure it is in the first stop's room, whatever its loop
  does, so the hold catches it.
- **Recover in Silvermere.** The tile's **[connect]** gets a **▾** menu with **Connect and
  recover in Silvermere** (`recover_to`). The brain asks the gateway to connect, stops the bot
  if it runs, and on entering the realm leaves it stopped (a stopped bot never hangs up) and
  sends `sysop goto silvermere` at the first prompt, ahead of the re-sync and without waiting
  for a fight: the game refuses it under attack, and right after logon nothing has attacked
  yet. Already in the realm, it goes at once. The character alone.

## Design notes

- `Stash` gains a `read: bool` step before hiding: send `i` with `Expect::Prompt`, then
  `next()` uses the counts that `i` refreshed. `outcome`/`on_event` watch for the
  "don't have … to hide" text; add it to the parser registry if it isn't there yet.
- `wants` requires `v.certain`. `begin` records the stop's key. The sure-of-room check runs
  once, before the first `hide`, and a failed check ends the stash.
- No change to `cash.stash_on_loop` or the party round itself.

## Done when

- `task.rs`: a coin count changed by a give just before the first stop is hidden in full
  (the fake game answers `i` with the new count).
- `task.rs`: "You don't have N gold crowns to hide!" leads to `i` and one retry with the new
  count.
- `stash.rs` `coins_are_hidden_only_where_the_belief_is_sure_of_the_stop` (a unit test of the
  check; the harness has no look-alike first stop).
- `party.rs`: a follower that is unsure itself but followed the leader in, and sees it in the
  room, hides its coins. One that is not in the leader's room hides nothing.
- Rust: fmt, clippy `-D warnings`, nextest.
- `party.rs`: the leader says "Waiting for Lauren to hide their coins" and does not move before
  the follower's `i` after hiding.
- `sys_goto.rs` `connecting_to_recover_goes_to_the_town_with_the_bot_stopped`; `bot.test.ts`
  "offers to connect and recover in Silvermere from the connect button's menu (C-093)".
- UI: `npm run check`, `npm test`.
- Live: a give right before the first stop, then check that `i` shows no silver, gold or
  copper after the stash.

## Tests

As in "Done when", next to C-092's tests in `tests/task.rs` and `tests/party.rs`.

## Notes for the agent

- Out of scope, worth its own card: a follower that keeps following the leader stays unsure
  among look-alikes (Sasha: 135 candidates for Huge Cavern, Training Area, while Rocco was
  sure of 15/12). It could take the leader's room each time it follows it in.
- Never stash on a guess. When in doubt, skip the round: the next round tries again.
- If the parser lacks "You don't have %d %s to hide!", add it from the game texts and note it
  in the KB (`docs/kb/economy.md`, changelog).
