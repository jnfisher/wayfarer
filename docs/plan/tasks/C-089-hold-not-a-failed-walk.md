---
id: C-089
title: "A leader's hold is not a walk that ended elsewhere: a Go to waits for the party and goes on"
tier: C
size: S
wave: 16
depends: []
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/modes.rs (the party gate and the sneak gate on a Task-mode walk)
  - crates/wayfarer-brain/src/journey.rs (`walk_deferred`, `give_up`)
  - crates/wayfarer-brain/src/session.rs (`party_give_up`)
  - crates/wayfarer-settings/src/lib.rs (`party.wait_separated_minutes` help)
  - crates/wayfarer-brain/tests/party.rs
  - docs/brain.md "Parties"
---

## Goal

John (2026-11-10): the party stood still for 90 minutes. Lexi, the leader, was on a Go to
Outpost, Officer's Quarters. Two fights stopped the walk and she went on after each. On the
third leg Rocco fell out of the party, and she held: "Stopped walking: Waiting for Rocco to
catch up". A second later the Go to ended: "Not going to Outpost, Officer's Quarters: 3 walks
to Outpost, Officer's Quarters ended elsewhere; playing here". She waited where she stood,
and the party with her.

The gate that turns a leader's walk into "Waiting for Rocco to catch up" (C-041) acts after
the Go to (or loop) has asked for the leg. Asking counts a try (`Journey::step`, `Task::leg`).
So each tick of the hold counted a walk that ended elsewhere. A Go to gives up at three, which
took about a second. A loop passes the stop by after `bot.leg_tries`, more slowly because it
waits `LEG_PAUSE` between legs. Under the sneak gate, a Go to's legs were also counted, and
the try was taken back from the loop instead.

## Behaviour

- A Task-mode walk held back by the party gate (a leader's hold or a party rest, or a
  follower staying with its leader) or by the sneak gate is not a try. The Go to, or else the
  loop, that asked for it takes the try back (`Bot::walk_deferred`, as C-083 does for a
  character that cannot move).
- A Go to holds for a member walking back like a loop does, and walks on when the member is
  back. `docs/brain.md` said a Go to is not held. It is, and that is right for a party.
- A Go to waits no longer than `party.wait_separated_minutes` (5 by default, 1 to 60), as a
  loop does. Then it ends with a warning: "Not going to Newhaven, Narrow Road: Lauren did not
  catch up in 5 minutes; playing here". Until now the limit stopped only a loop, so with the
  first fix alone a Go to would have waited for ever. The party-rest limit
  (`party.wait_rest_minutes`) gives up the same way, so it ends a Go to too.

## Done when

- `party.rs` `a_go_to_waits_for_a_member_walking_back_and_goes_on`: the leader is sent on a Go
  to while the follower walks back from the village. The leader says "to catch up", never
  "could not get there", and ends in the Go to's room. It fails before the change ("Waiting
  for Lauren to catch up", then "Going: could not get there").
- `party.rs` `a_go_to_ends_when_a_member_does_not_catch_up_in_time`: the follower cannot
  move. After 4 minutes the Go to is still on. After 6 it has ended with that warning, and the
  leader is still in the Arena. It fails without `party_gave_up`.
- `cargo fmt`, `cargo clippy -D warnings` and `cargo nextest run --workspace` pass.
