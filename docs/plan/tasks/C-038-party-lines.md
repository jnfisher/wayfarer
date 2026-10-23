---
id: C-038
title: "Party lines: typed events for invite, follow, drop and `par`; each session's in-game party"
tier: B
size: M
wave: 6
depends: []
crate: wayfarer-parse, wayfarer-brain
touches:
  - crates/wayfarer-parse/src/{typed.rs,parser.rs} (the party events)
  - tests/fixtures/patterns/ (read only), tests/fixtures/party/ (new: captured `par` and party lines, with PROVENANCE.md)
  - crates/wayfarer-brain/src/{party_game.rs (new),session.rs} (the session's in-game party)
  - docs/brain.md (a "Parties" section, first paragraph), docs/kb/grouping.md (KB notes)
---

## Goal

The parser turns the game's party lines into typed events, and each session keeps what the
game last said about its party: whom it follows, who follows it, and when `par` last showed
that. This is the ground truth the party coordinator (C-039) reads; nothing acts on it yet.

## Inputs

- `docs/plan/10-party-play.md` §4 ("Tracking").
- `docs/kb/grouping.md` §9 "Screen messages to parse" (exact strings) and §2a (a follower
  refused by an exit drops with no message; `par` always ends with your own row; `par`'s HP
  and mana are `%3.3s` of the percentage, so `-10` for -100%).
- `crates/wayfarer-parse/src/typed.rs` (`GameEvent`), C-015 (game texts from the pack).

## Behaviour

- New `GameEvent` variants, names as given (fields are the `%s` of the KB's strings):
  - `PartyInvited { leader }` (`%s has invited you to follow %s.`),
    `PartyInviteSent { name }` (`You have invited %s to follow you.`),
  - `PartyFollowing { leader }` (`You are now following %s`, with or without the full stop:
    the mudengine lesson in `kb/plan-grouping.md` §1 lesson 5),
    `PartyFollowerJoined { name }` (`%s started to follow you.`),
  - `PartyLeft { leader }` (`You are no longer following %s.`),
    `PartyFollowerRemoved { name }` (`%s has been removed from your followers.`),
    `PartyMustBeInvited` (`You must be invited first!`),
  - `PartyMoved { dir }` (` -- Following your Party leader <dir> --`: the line names no leader),
  - the `par` screen: `ParFollowing { leader }` (`You are following %s.`), `ParHeader`
    (`The following people are in your travel party:`), `ParRow { name, class, mana, hp,
    invited, rank }` (`mana`/`hp`: `Option<i64>` per cent, `None` when the row has none;
    `invited`: the `[Invited]` form; `rank`: front/mid/back), `ParNone` (`You are not in a
    party at the present time.`).
- `party_game.rs`: `GameParty { following: Option<String>, members: Vec<ParMember>, as_of:
  Option<Instant>, stale: bool }` per session, updated from those events: a full `par` screen
  replaces `members` and sets `as_of`; `PartyFollowing`/`PartyLeft`/`PartyFollowerJoined`/
  `PartyFollowerRemoved` update it between screens and set `stale` until the next `par`.
  A `par` screen that started but never reached its own row (the KB: the own row is always
  last) is discarded, not half-applied.
- Unrecognised party text stays an `unknown` line, as today.

## Design notes

- Capture the real lines from the live silvermere (`scripts/live-check.sh` or a gateway
  session): `par` alone, with an invitation pending, with a follower joined, and the drop
  lines, into `tests/fixtures/party/` with a `PROVENANCE.md`. The KB's strings come from the
  C source; any difference silvermere shows goes in the KB notes.
- `GameParty` lives on `Session` and is not on the wire yet (C-039 decides what the UI sees).

## Done when

- Every string in `kb/grouping.md` §9's party rows parses to its event, from the captured
  fixture, and `GameParty` follows a scripted join, `par`, drop and re-join.
- The parse golden tests and the misreads tests still pass.

## Tests

- `typed.rs`: one test per new event from the fixture lines, including `You are now
  following X` without a full stop and a `par` row with `-10` HP.
- `party_game.rs`: a `par` screen fills `members`; a cut-off screen leaves the last good one;
  `PartyLeft` clears `following` and marks stale.

## Notes for the agent

- `par` rows are fixed-width (`<name 30> (<Class>) [M:nn%] [H:nn%]PR - Frontrank`, the KB);
  check the real spacing in the capture, don't trust the KB's.
