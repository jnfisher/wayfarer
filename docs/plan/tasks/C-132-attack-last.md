---
id: C-132
title: "A party member that attacks last: its attack goes again when another member engages, so it acts after them in the round, picked in the Parties panel"
tier: A
size: S
wave: 22
depends: [C-039, C-068]
crate: wayfarer-brain, wayfarer-store, wayfarer-proto, ui
touches:
  - crates/wayfarer-proto/src/lib.rs (`PartyDef.attack_last`)
  - crates/wayfarer-store/src/lib.rs (`parties.attack_last`, migration 18)
  - crates/wayfarer-brain/src/party.rs (`PartyRole::{last, others}`, `attack_last()`; the check in `save`; cleared with the member)
  - crates/wayfarer-brain/src/fight.rs (`mate_engaged`, `last_attack`, `restarted`; `attack_last_now`)
  - crates/wayfarer-brain/tests/party.rs (the Arena's monster; the test)
  - ui/src/lib/parties.ts, ui/src/PartiesPanel.svelte, ui/tests/parties.test.ts, ui/tests/parties-panel.test.ts
  - docs/brain.md "Parties", docs/protocol.md, docs/web-ui.md, docs/kb/combat.md, docs/kb/changelog.md
---

## Goal

John wants Asa to attack after the others (silver-mirror, 2026-11-26). Her meteor swarm lands
up to four times a round at four mana each; cast first it spends all four on a monster the
melee would have finished, cast after them it spends only what the monster has left. When this
card is done the Parties panel has, under the guards, an "attacks last" pick naming one member
or nobody, kept with the party; that member's Fight sends its attack or cast again whenever
another member of the party engages after it, so the game queues it behind them for the round.

## Inputs

- `docs/kb/combat.md` "How a fight runs": quicksilver's auto-combat ring runs the queued players
  front to back each round, each one's attack queuing them again behind the marker, so the order
  they engaged in is the order they act in; an attack or cast typed into a running fight frees
  the player's slot (`kill_autocombat`, `*Combat Off*`) and `engage` pushes it to the back
  (`combat/autocombat.rs`, `commands/combat.rs`, `commands/cast.rs`). A typed attack spell only
  engages: the casts come with the rounds, so the repeat costs nothing.
- The room's lines for an engage: `attacks_other` (`Riley moves to attack orc fanatic.`) and
  `casts_upon` (`Asa moves to cast meteor swarm upon orc fanatic.`), both `GameEvent::Other`
  with `who` and `target`; the parser has both.
- `crates/wayfarer-brain/src/fight.rs`: `switched` and the `*Combat Off*` the weapon switch
  causes, the model for `restarted`; the attack and cast steps (`Expect::Engaged`).
- `crates/wayfarer-brain/src/party.rs`: `PartyDef.guards` through `save` (the check), `leave`
  and the move out of another party (cleared), `from_records`; `PartyRole` and `plan`.
- `ui/src/lib/parties.ts` `Draft`, `setGuard`, `toDef`; `PartiesPanel.svelte`'s guards list.

## Behaviour

- `party_save` takes `attack_last`, a session id that is the leader or a follower of the party,
  else "X attacks last but is not in the party". Left out or null is nobody. A member moved out
  of the party, or leaving it, stops being the one. Kept in the store with the party.
- The member's Fight remembers the attack or cast it sent for the fight (the opening, the
  weapon after the casts, the area spell, the switch to a boss) and when. A party member's
  engage line that comes after it, within 4 s, has the Fight send that line again, once per
  engage line, narrated "Fight: Riley engaged, attacking last". The `*Combat Off*` the game
  prints for the restart does not end the fight for it.
- Nothing goes again for a backstab opening, for a line that came before the Fight's own attack
  (then it is already behind), for a name not among the other members in the realm, or when
  the character is not in a fight.
- The Parties panel: "attacks last" under the guards, one member or nobody, saved with the
  party. The pick is cleared when the member is unticked or becomes the leader of another draft.

## Design notes

- `PartyDef.attack_last: Option<SessionId>` (serde default, skipped when none; ts optional).
  `PartyRecord.attack_last: Option<String>`; column `attack_last TEXT NOT NULL DEFAULT ''`.
- `PartyRole::Leader` and `Follower` get `last: bool` and `others: Vec<String>` (the other
  members in the realm by in-game first name, lower case); `attack_last() -> Option<&[String]>`.
- `Fight`: `mate_engaged: Option<(String, Instant)>` from `attacks_other` / `casts_upon`;
  `last_attack: Option<(String, Instant)>` set at every attack or cast step but the backstab;
  `restarted: bool` swallows the next `*Combat Off*`. `attack_last_now` runs after the heals.
- One such member a party: two would chase each other to the back of the ring.

## Done when

- Asa, marked, is seen sending `mete <word>` again after Riley's `moves to attack` line, and
  her rounds' meteor lines come after Riley's blows.
- The panel saves and shows the pick; the brain refuses a non-member.

## Tests

- `party.rs` (unit): the check, the clearing on leave, the role's `attack_last()`.
- `tests/party.rs`: the Arena with a giant rat; the follower marked attacks, the leader's engage
  line reaches it, and it attacks again (`[FOLLOWER, LEADER, FOLLOWER]`); unmarked, it does not.
- `ui/tests/parties.test.ts`, `parties-panel.test.ts`: the draft helpers and the pick's save.

## Notes for the agent

- The engage line's `who` is the first name as the game shows it; the role's `others` are
  lower case, compared case-insensitively.
- The brain steps right after every output chunk, so the repeat goes out within milliseconds of
  the engage line, well before the next round.
