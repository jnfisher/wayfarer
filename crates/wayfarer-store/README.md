# wayfarer-store

The brain's own SQLite file, `store.sqlite` in the data directory
(`docs/plan/02-architecture.md` "Storage", `docs/brain.md` "What it keeps"): each session's bot
state, so a restarted brain resumes what the character was doing, and measured statistics:
runs, their events (kills with their exp, deaths, flees, rests, level-ups) and a sample of exp
and HP a minute. A leaf: it depends on `rusqlite` only and knows nothing of the wire or the
game. Times are Unix seconds from the brain's clock.

```rust
let (store, note) = Store::open(path)?;          // made if missing, made afresh if unreadable
store.save_bot("aldric", &BotRecord::new(BotState::Running), now)?;
let run = store.start_run("aldric", now, None)?;
store.record(run.id, &Event { at: now, kind: EventKind::Kill, value: 9, detail: None, room: None })?;
let totals = store.totals(run.id)?;               // kills, deaths, flees, rests, exp gained
```

- **`Store::open`**: WAL, the schema brought up to date (`PRAGMA user_version`, one step per
  version in `MIGRATIONS`). `Note::Created` for a new file; `Note::Recreated` when the file
  could not be read as a store and was moved to `<path>.corrupt`; `Error::Newer` for a file
  from a newer wayfarer, left alone. `Store::in_memory` for tests and replay.
- **`bot_state`**: `bot`/`save_bot` with a `BotRecord` (state, home, loop id, the loop's stops
  as they were, the stop index, the stops to skip, why it is paused).
- **`runs`, `events`, `samples`**: `start_run`, `end_run`, `open_run`, `end_other_runs`;
  `record` and `add_value` (the exp that follows a kill); `sample`; `events`, `samples`,
  `totals`, and `runs` (the last N of a session, newest first, with their totals).

Tests: `cargo nextest run -p wayfarer-store`: round trips, the totals, newest-first listing,
a missing, a corrupt and a table-less file, an unversioned file migrated and a newer one left
alone.
