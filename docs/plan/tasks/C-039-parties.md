---
id: C-039
title: "Parties: definitions saved by the brain, the party coordinator, forming up, `par` tracking, followers don't walk, Leave and Join"
tier: B
size: L
wave: 6
depends: [C-038]
crate: wayfarer-proto, wayfarer-store, wayfarer-brain
touches:
  - crates/wayfarer-proto/src/lib.rs (party messages; ui/src/proto regenerated)
  - crates/wayfarer-store/src/lib.rs (a `parties` table, schema version +1)
  - crates/wayfarer-brain/src/{party.rs (new: the coordinator),brain.rs,session.rs,modes.rs}
  - docs/protocol.md, docs/brain.md "Parties"
---

## Goal

The brain holds party definitions (a leader and followers), saves them, and runs a party
coordinator above the sessions. A leader and its follower in one room form up by themselves;
the coordinator keeps the game's real party state with `par`; followers stop their own walking
while in a party; Leave and Join move a character out of and into a party. Separation, rests,
flees and heals come in later cards; this one gives them their seam.

## Inputs

- `docs/plan/10-party-play.md` §3, §4, §5 and §10 (Leave and Join), and §3's "Who counts" and
  "A leader walked by hand".
- `docs/plan/03-engine.md` §8 (the coordinator works by constraints and requests to each
  character's mode stack; no telepaths between our characters).
- C-038 (`GameParty`, the party events), `crates/wayfarer-brain/src/brain.rs` (`Brain`,
  `sessions`), `modes.rs` (`Bot::tick`, `View`), `session.rs` (`bot_tick`, `walk_to`,
  `run_loop`), `crates/wayfarer-store` (schema versions, `custom_loops` as a model).
- `docs/kb/grouping.md` §9 (formation), §10 (re-`follow` works without a new `invite` for a
  follower who dropped at an exit; `invite` of someone already listed still prints the line).

## Behaviour

- **Protocol** (UI to brain, relayed as the other control messages are): `parties_request`,
  `party_save { party: PartyDef }` (create or replace), `party_delete { id }`,
  `party_leave { session }`, `party_join { session, party }`. Brain to UIs: `parties {
  parties: Vec<PartyStatus> }`, sent on attach, on any change, and when a member's status
  changes. `PartyDef { id, leader: SessionId, followers: Vec<SessionId> }`; `PartyStatus {
  party: PartyDef, members: Vec<MemberStatus> }`; `MemberStatus { session, state:
  MemberState, detail: Option<String> }`; `MemberState`: `joined`, `not_joined`, `away`
  (logged off or bot paused), `separated`, `walking` (to the leader), `resting`,
  `unreachable`. The last four are set by C-041 and C-042; this card sends `joined`,
  `not_joined` and `away`.
- **Validation**: one to five followers; the leader is not a follower; a character in at most
  one party (saving moves it out of another, which is then saved too); unknown session ids
  refused with a warning (the brain cannot send `error`).
- **Saved** in the store's `parties` table; loaded at start.
- **Forming**: when a leader and a follower that is not in the leader's game party are in the
  same room, both in the realm with their bots running, the leader sends `invite <name>` and,
  on `PartyInvited`, the follower sends `follow <leader>`; `PartyFollowing` makes it joined. A
  dropped follower is re-joined with `follow` first (the KB: still listed), `invite` only if
  that gives `PartyMustBeInvited`. At most one invite per follower per 10 s.
- **Names**: `invite` and `follow` use each character's **in-game name** from its model (the
  `st` name, first word), never the config's display name or session id; a member whose name
  isn't known yet is not invited until it is (the re-sync's `st` gives it).
- **Restarts**: after a brain restart the coordinator rebuilds each party from a fresh `par`
  before it invites anyone, so a formed party gets no invites and no `follow`s.
- **Typed by John**: `follow`, `leave`, `invite` typed into a tile are read from their game
  lines like the bot's own; the definition is not changed by them (a follower that left by
  hand is re-joined, as after a drop; Leave party is the way out).
- **Tracking**: the leader sends `par` after forming, after every move a follower's
  `PartyMoved` line didn't follow (within one fast cycle; say the timeout in the PR), after a
  text exit, a flee, a death and a re-sync, and every 30 s while the party has members. A
  member the leader's `par` doesn't list is not joined. The coordinator's view of each party
  is the leader's latest `GameParty` plus each follower's own `following`.
- **Followers don't walk**: while a character is a follower of a defined party, its Task,
  Go to, walk-to and errands are refused (a warning: "following Penny: it goes where the party goes"); joining ends the
  one it was running ("Joined Penny's party: the loop stops"). Fight and Recover run as
  today. The leader plays as now.
- **Leave** (`party_leave`): the follower sends `leave`, is removed from the definition (saved),
  and is a solo character again. **Join** (`party_join`): added to the definition; it forms
  when in the leader's room (walking there comes with C-041).
- **The seam**: `party.rs` gives each session, every tick, a `PartyRole` (`None`, `Leader {
  members }`, `Follower { leader, joined }`) that `View` carries (`view.party`). Later cards
  add holds and requests to it.

## Design notes

- The coordinator is a `Parties` struct on `Brain` that runs after the sessions' own steps in
  `Brain::step` and before the next tick: it reads every session's `GameParty`, room and bot
  state and returns lines to send through each session's executor, and the roles. Keep the
  game commands going through the executor (one action in flight), so they don't fight the bot.
- `invite`, `follow`, `leave` and `par` count as the bot's own lines for the typing arbitration
  (C-031).

## Done when

- In-process: with two scripted sessions in one room, a saved party forms (invite, follow,
  joined), a follower's `run_loop` is refused, Leave sends `leave` and frees it, and the
  definition survives a brain restart (in-memory store reopened).
- Live, on John's rig: Penny and Lauren in one room, a party saved through the protocol (a
  test or `websocat`), and they're in a game party within a few seconds.

## Tests

- `party.rs`: validation; forming; re-join with `follow` then `invite` on `PartyMustBeInvited`;
  `par` scheduling (after an unfollowed move, every 30 s); away members skipped; invites use
  the in-game name; a restart with a formed party sends `par` and nothing else.
- `store`: the `parties` table round-trips; the schema version migration.
- `proto`: the new messages round-trip, TypeScript regenerated.

## Notes for the agent

- The UI comes in C-040; until then `parties` is visible in the dev panel's message stream.
