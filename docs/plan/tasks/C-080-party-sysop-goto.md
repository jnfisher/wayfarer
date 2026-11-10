---
id: C-080
title: "Sysop goto for the whole party: a town list in the Parties panel and the tile menu, usable with the bot on"
tier: B
size: M
wave: 16
depends: [C-079]
crate: wayfarer-proto, wayfarer-brain, ui
touches:
  - crates/wayfarer-proto/src/lib.rs (`sys_goto`), ui/src/proto/ (regenerated), docs/protocol.md
  - crates/wayfarer-brain/src/sysop.rs (new: the towns), brain.rs, session.rs, party.rs, position.rs
  - crates/wayfarer-brain/tests/sys_goto.rs (new)
  - ui/src/PartiesPanel.svelte, ui/src/Tile.svelte, ui/src/lib/bot.ts, ui/src/lib/gateway.ts
  - docs/brain.md "Sysop goto", docs/web-ui.md
---

## Goal

John teleports the whole party to a town from a list, with the bots running, and nobody gets
lost. On 2026-11-10 at 13:44 he typed `sys goto rhud` into every member at once (the Parties
panel's send box). The members landed in Rhudaur, but two followers still believed they were in
the Large Office (the game shows no room after a sysop goto). They tried to walk back to the
leader: "Can't reach Lexi: no way to Lexi's room is known", "Not walking to Rhudaur, Massive
Doors: no way there from Large Office". The game had broken the follows too. John chose a town
list (sysop goto) over a walking Go to.

## Inputs

- `kb/navigation.md` "SYSOP GOTO":
  - the five towns and the rooms they land in: `newhaven` 1/2150, `silvermere` 1/2189,
    `rhudaur` 2/2519, `khazarad` 6/1249, `lostcity` 16/426;
  - the game answers with only the prompt;
  - it is refused in combat;
  - by policy, never a room number.
- The server's logs for 13:44:07 to 13:45 (the four characters' `sent.log` and `notes.log`).

## Behaviour

- **Protocol**: `sys_goto {session, town}` (UI → brain). An unknown town is refused with a
  warning.
- **Who goes**: the character's party (leader and followers), or the character alone if it is in
  no party (`Parties::members_of`).
- **Each member**:
  1. Cancels its errand.
  2. Drops its loop or Go to; a running bot keeps running.
  3. Stops any walk.
  4. If it is in combat, waits for the fight to end ("Sysop goto Rhudaur: after the fight").
  5. Sends `sysop goto <town>` before the bot's next action.
- **Landing**:
  - the position is set to the town's room (`Position::teleported`), since the game shows none;
  - a running bot plays there (`arrived_by_hand`);
  - a notice says "Sysop goto: in Rhudaur, Massive Doors";
  - a re-sync's `look` checks the position.

  The party coordinator re-forms the party in the game as it does after any separation.
- **The UI**:
  - the Parties panel: a **sysop goto** list (`town…`, then the five towns) in each party's box,
    which sends `sys_goto` for the leader;
  - each tile's `[▼]` menu: **Sysop goto ▸**, which opens the towns under it;
  - both work with the bot on.

## Done when

- `sys_goto.rs`:
  - a character with no party lands in the Newhaven Arena with the position there;
  - an unknown town is refused;
  - mid-fight, the teleport waits for the kill and goes before the bot's next `look`, with the
    bot still running.
- `party.rs`: `members_of`.
- `messages.rs`: the example is in `docs/protocol.md` and the proto README; `ui/src/proto/` is
  regenerated.
- The UI tests cover the panel's list and the tile menu's towns.

## Notes for the agent

- Typing `sys goto` by hand is not recognised as a teleport (documented in brain.md). The list is
  the supported way.
- Whether the account may sysop goto is not checked. Without the key the game speaks the line
  instead, and the re-sync's `look` corrects the position.
