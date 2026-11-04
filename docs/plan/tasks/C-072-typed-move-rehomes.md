---
id: C-072
title: "A typed move re-homes the bot where the character arrives, not where it was"
tier: B
size: S
wave: 15
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/modes.rs (rehome, Recover's home)
  - crates/wayfarer-brain/src/session.rs (hand_move, user_input, keys_sent)
  - crates/wayfarer-brain/src/recover.rs, if the fix belongs there
  - docs/brain.md ("The bot's states")
---

## Goal

When the player moves a running character by hand, the bot plays where the character *arrives*,
and after a rest it walks back there, not to the room it was in before the move. On the Thrag
run (2026-11-04), Lexi, the party leader, was walked east into Thrag's room by hand and twice
walked herself back west after a party rest, taking the party with her.

## Inputs

- **The run's logs** on the server: `~/.local/share/wayfarer/logs/lexi/{notes,sent}.log`, and
  `brain.log` for the typed lines. The window is ts 1793826180000 to 1793826380000:
  - 1793826189842: the bot is started in "Stone Passageway, Guardroom", which becomes Recover's
    home.
  - 1793826225865: John types `e`, into Thrag's room. The bot fights there (wolves, then Thrag).
  - 1793826314324: "Recover: rested; walking back to Stone Passageway, Guardroom", then `w`.
  - John types `e` again at 1793826320349. At 1793826334292 it walks west again, the same way.
  - Later, from Thrag's room: "Rested; could not walk home, waiting here".
- **Code:**
  - `Bot::hand_move` (`modes.rs`) sets `self.rehome = true`.
  - `Bot::observe` (`modes.rs`, "Moved by hand") takes `rehome` on the next observe and calls
    `recover.set_home(v.room)`.
  - `Session::user_input` and `Session::keys_sent` (`session.rs`) call `hand_move` when the line
    is *typed*. The typed line is queued (`self.typed`) and sent later, between the bot's
    actions.
- `docs/brain.md` "The bot's states": "only a typed move changes what the bot does".

## Behaviour

- **The likely cause.** `rehome` is set when the line is typed. The next observe runs before the
  move has gone out or landed, takes the flag, and sets home to the room the character is still
  in. Confirm this with a test first. If the cause is something else, fix that and say so in the
  PR.
- **After a typed move** (a line, or keys) by a running bot's player, home becomes the room the
  character arrives in. This works with a move typed in a fight or in a rest, and with the move
  queued behind the bot's own action.
  - A move the game refuses (no exit, a closed door) leaves home where it was.
  - A follow move ("Following your Party leader") is not a typed move. It does not re-home a
    follower.
- **Several typed moves in a row** re-home to the last room reached.
- **The narration says so:** "You moved: playing in <room>" once the character is there. It
  replaces the bare re-home, not the "stopped loop" line, which stays.

## Design notes

- Re-home on arrival, not on intent. One shape: `rehome` becomes "pending until the typed move's
  room lands". The session already knows when a typed line is sent (`sent`, `position.sent`) and
  when the room changes. Clear it on the first room change after the move goes out, or on its
  refusal.
- Keep the change inside the bot and session. The coordinator, Flee and Task don't change.

## Done when

- The Lexi sequence, as a test: started in room A, a typed `e` queued behind a fight action, the
  fight, arrival in B, a rest. The rest ends with no walk, and the bot plays in B.
- A refused typed move leaves home in A.
- A follower moved by its leader's follow keeps its own behaviour (it has no home walk).
- `docs/brain.md` says when home moves.
- The checks pass.

## Tests

- `modes.rs` or `session.rs` unit tests for the three cases above, driven by the session's own
  inputs (typed line, `sent`, room text). No sleeps.

## Notes for the agent

- Work in a worktree. Writing under `crates/` in John's checkout restarts his live brain.
- Don't touch Recover's walk-out-to-rest (`health.rest_quiet_room`). Its home logic is the
  same field. Make sure a walk out to rest and back still ends at home.
