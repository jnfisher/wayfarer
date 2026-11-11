---
id: C-093
title: "Stash what is carried now, and only where the bot is sure it is"
tier: C
size: S
wave: 16
depends: [C-092]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/stash.rs, crates/wayfarer-brain/src/modes.rs, party.rs
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
- **A follower** hides only when it is also certain of its room and that room is the
  leader's first stop (the leader's reported room key). A follower that isn't sure skips
  the round.
- The narration names the room and its key ("Stash: hiding 571 gold in Huge Cavern, Training
  Area (5/123)"), so the player can find the right copy.

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
- `task.rs`: with the belief unsure at the first stop (two look-alike rooms), nothing is
  hidden and the narration says why.
- `party.rs`: a follower that isn't certain of its room hides nothing while the leader does.
- Rust: fmt, clippy `-D warnings`, nextest.
- Live: a give right before the first stop, then check that `i` shows no silver, gold or
  copper after the stash.

## Tests

As in "Done when", next to C-092's tests in `tests/task.rs` and `tests/party.rs`.

## Notes for the agent

- Never stash on a guess. When in doubt, skip the round: the next round tries again.
- If the parser lacks "You don't have %d %s to hide!", add it from the game texts and note it
  in the KB (`docs/kb/economy.md`, changelog).
