---
id: C-107
title: "Back in the realm, the bot goes back on as it was"
tier: A
size: S
wave: 16
depends: []
crate: wayfarer-brain, wayfarer-store
touches:
  - crates/wayfarer-store/src/lib.rs (migration 16: `bot_state.on_entry`)
  - crates/wayfarer-brain/src/session.rs (kept on leaving, put back on entry)
  - crates/wayfarer-brain/src/modes.rs (the record's new field)
  - crates/wayfarer-settings/src/lib.rs (`bot.start_on_logon` help)
  - crates/wayfarer-brain/tests/store.rs
  - docs/brain.md ("Bot states", "Restarts", the store's tables)
---

## Goal

A server restart or a wayfarer update no longer leaves the characters standing there: each
bot that was running a loop when the connection dropped, or when the gateway logged it off
for an update, goes back to that loop at the stop it had once the character is back in the
realm (John, 2026-11-16). Before, leaving the realm stopped the bot and dropped its loop, and
`bot.start_on_logon` started it afresh with no loop ("Waiting here").

## Behaviour

- The session leaving the realm or `logging_off` with the bot running or paused keeps the
  bot's record (home, loop, stop, skips, why paused) to put back at the next entry; the bot
  still stops and its run still ends.
- The store keeps it too: the bot saved `stopped` with `on_entry` set to the state it goes
  back to, so a new wayfarer (an update logs every character off first) puts it back.
- At the next entry into the realm, with `bot.start_on_logon` on, it is put back as a brain
  restart puts one back, the re-sync first: "Entered the realm: putting the bot back as it
  was", "Resumed: running loop ... at stop 53/104 (back in the realm)". A new run starts.
- A brain that starts with a running or paused bot stored while the character is out of the
  realm keeps it for the next entry the same way.
- Not kept: a bot stopped before the session left (by hand, a death, a hang-up). ■ while out
  of the realm forgets the kept one. `bot.start_on_logon` off: it stays off.

## Done when

- A dropped connection mid-loop, then an entry: the loop is back on at its stop.
- A log-off with the bot paused on a loop, a new brain, an entry: back paused on the loop.
- Stopped by hand while out of the realm: nothing kept.
