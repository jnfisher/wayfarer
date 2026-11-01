---
id: C-064
title: "Rule set per connection: 1.11p, 1.20 or unknown; the HELP 1.20 probe and the fallback signals; remembered per server"
tier: B
size: M
wave: 12
depends: [C-063]
crate: wayfarer-world, wayfarer-brain, wayfarer-store, wayfarer-proto, ui
touches:
  - crates/wayfarer-world/src/rules_set.rs (new; the name is the builder's call)
  - crates/wayfarer-brain/src/{session.rs,commands.rs}, crates/wayfarer-brain/src/detect.rs (new)
  - crates/wayfarer-store/src/lib.rs
  - crates/wayfarer-proto/src/lib.rs, ui/src/ (a badge on the tile header and the dev panel)
  - docs/brain.md, docs/kb/quicksilver-1.20.md, docs/kb/changelog.md
---

## Goal

Each connection knows which rules its server plays: **1.11p**, **1.20** or **unknown**. The
brain finds out at login with `help 1.20`, falls back on lines only 1.20 can print, and remembers
the last answer per server. Every later 1.20 behaviour (C-066 to C-069) asks this one value, and
an unknown server plays as 1.11p.

## Inputs

- `docs/plan/quicksilver-1.20/handoff.md` sections 1 and 2 (the probe, its exact answers, the
  fallback signals, the things that are *not* signals).
- `crates/wayfarer-brain/src/session.rs` (`RESYNC_COMMANDS`, how the brain sends commands at a
  quiet prompt out of combat), `commands.rs`, `executor.rs`.
- `crates/wayfarer-store/src/lib.rs`: the brain's SQLite record (bot records per session).
- C-063's typed events and its `help_none` pattern.

## Behaviour

- **The value.** `RuleSet { Stock111p, Qs120, Unknown }` (names the builder's call), per session,
  starting `Unknown` on every login. While it is `Unknown` everything behaves as 1.11p
  (handoff section 1: a wrong 1.11p guess is safe, a wrong 1.20 guess is not).
- **The probe.** At the first quiet prompt in the realm, out of combat and not typing-paused, the
  brain sends `help 1.20` once.
  - A line equal to `Quicksilver 1.20: how the rules differ from 1.11p` (after ANSI stripping and
    trimming) sets 1.20. The lines after it, up to the next prompt, are the topic's text: they are
    kept (the dev panel can show them) and **not** fed to the event stream as game events.
  - `Sorry, no help is available for that topic!` sets 1.11p.
  - Anything else, or no answer within 5 s, leaves it unknown; the brain tries again at a later
    quiet prompt, at most 3 times a session, then stays unknown (plays as 1.11p) until a fallback
    signal says otherwise.
- **The fallback signals** (handoff section 2), each switching an unknown *or 1.11p* session to
  1.20 the first time it is seen, with the signal recorded as the reason:
  1. any C-063 1.20 event (opening, weave, intercept, guard, Str requirement);
  2. a hit with both crit and stab set;
  3. a 1.20 spell name in the `spells` list, a shop list or an item name: `protective aura`,
     `dark binding`, any regen-line name (handoff 4.2), `scroll of protective aura`;
  4. `You are now meditating.` sent when the character's mana was full and its HP was not.
- A 1.20 result is never turned back to 1.11p in a session.
- **Remembered per server.** The last result is stored by server id with the time. On login the
  session starts from it as a *hint* shown in the UI, but the rule set the bot acts on is still
  `Unknown` until this session's probe or a signal decides (handoff: detect again on each login; a
  server can be rebuilt). If the probe fails three times and a signal never comes, the remembered
  value is **not** used to turn 1.20 on.
- **`guard` is never sent unless the rule set is 1.20** (on 1.11p it is speech). One guard
  function in the brain checks this; C-068 uses it.
- **Shown.** The tile header shows a small `1.20` badge when detected, nothing for 1.11p, `?` while
  unknown; the dev panel shows the reason (probe or which signal) and the probe text.

## Design notes

- The enum lives in `wayfarer-world` (the lowest crate the brain and the world data share), so
  C-066's spell data and C-069's models can take it as an argument.
- `detect.rs` is a small state machine fed typed events and prompts, returning commands to send
  and a decision; it holds the attempt count and the 5 s timer from the brain's clock (C-032), so
  replays and tests run on recorded time.
- The store gets one table or column for `(server, rules, decided_at, reason)`; a store schema
  version bump with its migration, as the store's earlier changes did.
- Out of scope: what changes when 1.20 is set (later cards).

## Done when

- On a server answering with the topic, the session becomes 1.20 after its first quiet prompt; on a
  server answering `help_none`, 1.11p; with no answer, unknown after three tries, behaving as
  1.11p.
- Each of the four fallback signals switches an unknown session to 1.20.
- The `HELP 1.20` text never shows up as events in the dev panel's event stream.
- `guard` cannot be sent while the rule set is unknown or 1.11p (a test calls the guard path).
- The rule set and its reason are in the snapshot the UIs get, and the badge shows.
- `docs/brain.md` has a "Rule sets" section; `docs/kb/quicksilver-1.20.md` a "Detection" section
  ([Q]); a changelog line.
- Against the live server through `scripts/live-check.sh` (opt-in step): the probe's result is
  printed. It runs live only, never in CI.
- fmt, clippy, nextest, `npm run check` and `npm test` pass.

## Tests

- `detect.rs`: the 1.20 answer, the stock answer, a timeout (three tries, then unknown), a
  different answer, each fallback signal, no downgrade from 1.20, no probe in combat.
- The probe's text swallowed: a fixture of the 34-line answer (`tests/fixtures/qs120/help-1.20.txt`)
  produces no game events.
- Store: remembered per server, and a remembered 1.20 alone doesn't set the rule set.
- The guard path refuses on unknown and 1.11p.

## Notes for the agent

- Don't use the version line or the banner: they are the same on both (handoff "Not signals").
- **The live server plays 1.11p** (silvermere, the orchestrator, 2026-11-01), and no 1.20 server
  can be reached: the live step proves the 1.11p path is unchanged, and the 1.20 path is covered
  by fixtures built from the handoff. The step prints whichever it finds and passes either way.
