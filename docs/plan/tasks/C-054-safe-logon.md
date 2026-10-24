---
id: C-054
title: "Safe logon: the gateway enters the realm only with a brain attached, and entering the realm starts the bot"
tier: B
size: S
wave: 8
depends: [C-005, C-049]
crate: wayfarer-gateway, wayfarer-brain, wayfarer-settings, wayfarer
touches:
  - crates/wayfarer-gateway/src/{gateway.rs,session.rs} (`Options::wait_for_brain`)
  - bin/wayfarer/src/main.rs (`--enter-realm`)
  - crates/wayfarer-brain/src/session.rs (`set_state`); crates/wayfarer-settings (`bot.start_on_logon`)
  - tests/live/src/gateway.rs; docs/gateway.md; docs/brain.md
---

## Goal

A character is never in the realm with nothing playing it. (John, 2026-10-24: "I keep having
deaths because wayfarer and the brain start up and the characters just stand still".)

## Inputs

- Lauren, 2026-10-24: the gateway logged the party on at 14:59:02 into Slum Street, Bend as two
  orc sentries came in; the brain (a debug build under `cargo watch`) attached at 14:59:40;
  every bot was stopped, and she died at 15:00:22 having fought back not once.

## Behaviour

- **The gateway** (`Options::wait_for_brain`; `wayfarer gateway` and `serve` default to it,
  `--enter-realm always` turns it off; `serve` without a world pack never waits): the logon's
  last required step (`E`) is held at the menu until a brain is attached; the session's reason
  says "waiting for a brain to enter the realm"; no step timeout runs meanwhile; a brain
  attached already, or attaching, sends it at once.
- **The brain** (`bot.start_on_logon`, on by default): a session that goes into the realm from
  outside it gets `start` ("Entered the realm: starting the bot"), unless a stored bot is about
  to be resumed, an errand runs or the bot is on already.

## Tests

- Gateway: `E` held a minute at the menu, then sent on `note_brain(true)`; sent at once with a
  brain attached already.
- Brain: entering the realm starts the bot (the old scripts turn it off in `test_settings`).
