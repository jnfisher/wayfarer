# wayfarer-brain

The brain (`docs/brain.md`): attaches to the gateway as its brain client, parses every
session's output, keeps a character model per session, sends the UIs events and snapshots,
and re-syncs a character (`st`, `exp`, `i`, `look`) at an idle prompt. Depends on
`wayfarer-proto`, `wayfarer-parse` and `wayfarer-world`, not on the gateway.

```rust
let world = World::open(path)?;                          // the world pack's game texts
let mut brain = Brain::new(world, Arc::new(SystemClock), token);
let mut link = WsLink::connect("ws://127.0.0.1:7373/ws").await?;
brain.run(&mut link).await?;                             // Ok when the gateway goes; run again
```

- **`Link`**: `send` (never waits: a task of the link's own writes) and `recv`. `WsLink` over
  the WebSocket; `ChannelLink` over a pair of tokio channels (`Hub::connect`'s), for tests and
  the in-process brain of `wayfarer serve`.
- **`Brain::run`**: one task and one loop, `tokio::time` for its timers (tests pause the clock).
  The brain's sessions, parsers and characters live in `Brain` across runs, so a reattach
  carries on from the last `seq` seen. `Err(Refused)` when the gateway says no.
- **`Character`**: `apply(&GameEvent)` and `snapshot() -> CharacterState`; `merge_patch` and
  `apply_patch` (RFC 7386) between snapshots.
- **The re-sync** (`session.rs`): `Off`, `Want {next, idle}`, `Waiting {step, deadline}`; the
  only code that sends to the game.
- `BUILD`: the version and commit (`build.rs`), for `brain_info`.

Tests: `cargo nextest run -p wayfarer-brain`. `tests/brain.rs` runs the brain against a fake
gateway: the fixtures replayed as live output (the snapshot, the patches, the events), history
catch-up, the re-sync's timing on a paused clock, reattach, restart and refusal. The live
scenario: `scripts/live-check.sh --scenario brain`.
