# Task card format

One file per card: `tasks/C-NNN-slug.md`, with this front matter:

```
---
id: C-004
title: Gateway sessions: connect, output log, history, direct input
tier: B
size: M
wave: 1
depends: [C-002, C-003]
crate: wayfarer-gateway
touches:
  - crates/wayfarer-gateway/src/session.rs
  - tests/fixtures/captures/ (read only)
---
```

Then these sections, in this order:

1. **Goal.** Two or three sentences: what exists when the card is done.
2. **Inputs.** Plan sections, KB files, prototype code and fixtures to read, with paths.
3. **Behaviour.** Observable behaviour required: messages, orders, timings, refusals.
4. **Design notes.** The Rust (or TypeScript) shape: types, modules, what to leave out.
5. **Done when.** Checkable criteria.
6. **Tests.** The tests to write.
7. **Notes for the agent.** Pitfalls, open questions ("needs John"), links.

It is `server-plan/tasks/CARD-FORMAT.md` with three changes: no C source to port (so no
"behaviour to keep exact" with line citations; the game facts come from the KB), a `wave`
instead of a `phase`, and the conventions in [README.md](README.md) instead of the server's.
