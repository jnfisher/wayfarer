---
id: C-074
title: "In a party only the leader flees, and it takes the party: a follower never runs on its own"
tier: B
size: M
wave: 15
depends: [C-073]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/flee.rs (Flee::wants, step)
  - crates/wayfarer-brain/src/party.rs (PartyRole, with_leader, the coordinator's plan)
  - crates/wayfarer-brain/src/session.rs (the follower's ask for a party flee)
  - crates/wayfarer-brain/src/modes.rs (stalemate, party_flee)
  - docs/brain.md ("Party flees"), docs/plan/10-party-play.md if it says otherwise
---

## Goal

A party member never flees on its own while it is in a party. The only flee in a party is the
leader's party flee, which the game's follow takes everyone through. A follower under its flee
line asks for that flee and fights on until the leader runs. On the Thrag run, Sasha's own flee
split the group.

## Inputs

- **The Thrag run's logs** (2026-11-04, on the server, `~/.local/share/wayfarer/logs/`). The
  key moments:
  - **`lexi/notes.log` 1793825829281:** Lexi, the leader, starts the party flee: "Fleeing with
    the party: Sasha at 28%: west to Dark Tunnel". This is right.
  - **`sasha/output.log` 1793825830269:** the game takes Sasha west behind Lexi ("-- Following
    your Party leader west --", Dark Tunnel, with Rocco and Lexi there).
  - **`sasha/notes.log` 1793825830275, 6 ms later:** Sasha's own Flee fires, "Fleeing: HP 28%:
    south to Dire Wolf Lair". She sends `s`, leaves the party behind, and runs six rooms
    ("Fleeing on: … spawns" back and forth between Dire Wolf Lair, Dark Tunnel and Dark
    Chamber).
  - **Why her Flee could run:** `Flee::wants` stands down only while `party.with_leader()`, and
    `with_leader` is `joined && same_room(follower, leader)` in the coordinator's plan. Around a
    move, the two room beliefs differ for a moment (the leader's room has changed, the
    follower's not yet, or the other way round). For that moment she is "not with the leader",
    and a follower under its line "away from the leader flees on its own" (`docs/brain.md`
    "Party flees").
- **The ooze sightings** (C-073) and **stalemates** (`Bot::stalemate` runs Flee) are two more
  ways a member runs alone. C-073 removes the first. This card covers the stalemate.
- `docs/brain.md` "Party flees" and "Separation". The C-041 separation logic covers a follower
  the game dropped from the party.

## Behaviour

- **A joined follower** (in the leader's in-game party, by `par`) never starts Flee: not by HP,
  not by a stalemate, wherever it believes the leader is.
  - **Under its own flee line:** it asks the leader to flee, as today ("Fleeing with the party:
    Sasha at 28%"). It goes on fighting and healing until the follow takes it out.
  - **A stalemate:** it gives the target up (ignored, C-073) and asks nothing. It fights what
    else attacks.
- **The leader** flees as today:
  - under its own line;
  - for a follower under its line;
  - for a stalemate of its own, as a party flee.

  When a follower asks for a flee while the leader is in another room, the leader does not run.
  The follower's separation (C-041) brings it back.
- **A follower the game has dropped from the party** (not joined: separated, or never joined)
  is solo for this rule. It flees on its own as any character does, since nothing would take it
  out otherwise. **Needs John:** if he wants "never, while it belongs to a defined party", the
  card switches this to: it fights on, and the hang-up line (`health.hangup_below_hp`) is the
  last resort.
- **The hang-up safety** (`health.hangup`) is unchanged for everyone. It is not a flee.
- **Narration:** a follower under its line says "Under my flee line: asking Lexi to run", once
  for each time it goes under.

## Design notes

- Gate Flee on "joined to a party as a follower", not on "with the leader". `PartyRole::Follower`
  already carries `joined`. Add `PartyRole::in_party()` (joined follower) and use it in
  `Flee::wants` and wherever a stalemate starts Flee. Keep `with_leader` for assist and for the
  door bashing, where the room does matter.
- The session's ask (`session.rs`, near `!self.party_role.with_leader() && self.bot.hp(...)`)
  should fire for any joined follower. The coordinator turns it into the leader's party flee
  only when the two are together (same room by either belief). Otherwise it is dropped with a
  note on the leader: "Sasha asks to run, but is not here".
- Don't fix the momentary room mismatch itself. With the gate above it no longer matters for
  Flee. If a cheap fix falls out (re-plan after both members' prompts), say so in the PR.

## Done when

- The Sasha sequence, as a test: the follower is under its line, the leader's party flee goes
  out, the follow moves the follower, and for one plan the coordinator says "not with the
  leader". The follower sends no move of its own.
- A follower's stalemate sends no move. A leader's stalemate is a party flee.
- A separated follower under its line flees on its own (or fights on, if John chose that).
- `docs/brain.md` "Party flees" says it: one flee per party, the leader's.
- The checks pass.

## Tests

- `flee.rs` and `party.rs` unit tests for the four cases above.
- The party fixtures from C-042 still pass. Update the one whose expectation was "a follower away
  from the leader flees on its own", if one exists, and say so in the PR.

## Notes for the agent

- Work in a worktree. Writing under `crates/` in John's checkout restarts his live brain.
- Runs after C-073, which removes Flee's on-sight path. Rebase on it, don't duplicate it.
