---
id: C-045
title: "Shorter party rests: every member rests, and healers heal up to the rest-until lines while the mana keeps up"
tier: B
size: M
wave: 7
depends: [C-043]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/{party.rs,recover.rs,modes.rs,spellcraft.rs,session.rs}
  - docs/brain.md "Parties"
---

## Goal

A party rest takes as little time as the party's HP and mana allow. Every member below its own
rest-until line rests while the party does, not only the leader. A healer heals members up to
those rest-until lines, not only below the 60% heal line, as long as its own mana would not then
be what the party waits for. (Added 2026-10-23 at John's request: "is a healer aggressively
healing hurt party members enough when the group is stuck resting; the name of the game is
minimizing downtime".)

## Inputs

- `docs/plan/10-party-play.md` §7 and §8, C-042 (the party rest), C-043 (healing members).
- `docs/kb/recovery.md` "Resting" and "Background cycles": resting adds three regeneration
  units every 21 s on top of the one per 30 s slow tick, so a character that stands instead of
  resting regains HP several times more slowly; mana comes back on the slow tick whether or
  not the character rests; a cast ends resting.
- `party.rs` (`Parties::plan`, `PartyRole`, `Mate`), `recover.rs` (`Recover::wants`,
  `Recover::step`), `modes.rs` (`View::party_heal`), `session.rs` (`party_health`).

## Behaviour

Found by reading the code, before any change:

1. Only the leader carries the party rest (`PartyRole::Leader::rest`). A follower below its
   rest-until line but above its rest line holds the party up, and stands instead of resting.
2. During a rest a healer heals a member only below `party.heal_member_resting_below_hp` (60%),
   and itself only below `spells.heal_resting_below_hp` (60%). The party rests until everyone
   is at its rest-until line (85%), so the HP between those lines comes back by regeneration
   alone, while the healer has mana to spare.

What this card makes it do:

- **Every member rests for a party rest.** While the coordinator holds a party rest, each member
  with the leader is told so (followers too). A member below its own rest-until lines (HP, or
  mana for a caster) rests in place, as if its own Recover wanted it; one that is rested waits
  (no `rest`, which would be wasted). A fight still comes first.
- **Healing up to the rest-until lines during a party rest.** While the party rests, a member's
  heal line is its own *Rest until* HP line (`health.rest_until_hp`), the healer's included; out
  of a party rest the lines are as before. The choice of who and which spell is C-043's (the one
  furthest below its line first, a party heal for two or more).
- **The mana check.** A cast during a party rest goes only when, after it, the healer's mana
  would still be back at its *Rest until* mana line no later than the slowest member's HP. The
  regeneration rates come from what the session has seen: HP and mana gained per minute while
  resting, measured from the prompts (a rise bigger than a regeneration tick, a heal, is left
  out). Until a rate is known the check falls back to "the cast does not take the healer below
  its *Rest below* mana line".
- **Narration and status**: the healer's line says who it heals and why ("Recover: casting
  minor healing on lauren (party rest)"); a member resting for the party says "Recover: resting
  with the party".

## Design notes

- `PartyRole::Follower` gets the party's rest (`Option<String>`) like the leader;
  `PartyRole::party_rest()` returns it for both.
- `Mate` gets `until: i32` (its rest-until HP line) so the healer can use it.
- `Regen` in the session: HP and mana per minute while resting, a running average over the
  last few minutes, fed from prompts; read by `View`.
- `Recover::wants`: a party rest with the member not rested is a reason to rest.

## Done when

- In-process: in a party rest a follower at 70% (rest below 60, until 85) rests; a healer heals
  a member at 70% during a party rest and not outside one; it holds a cast that would leave its
  mana slower to recover than the slowest member's HP.
- `docs/brain.md` "Parties" says how party rests and heals work now.

## Tests

- `party.rs`: followers carry the rest; mates carry their rest-until line.
- `recover.rs`: a follower between its lines rests in a party rest; a rested one does not.
- `modes.rs`/`spellcraft.rs`: the heal line during a party rest; the mana check with and
  without measured rates.
- `session.rs`: the regeneration rate from prompts leaves a heal out.

## Notes for the agent

- A cast ends the healer's own rest; Recover rests again after it, as now.
