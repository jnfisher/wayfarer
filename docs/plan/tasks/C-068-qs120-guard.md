---
id: C-068
title: "GUARD on 1.20: a party's tank guards the member the player picks, re-guards after moves and rests, and the Parties panel shows it"
tier: B
size: M
wave: 12
depends: [C-063, C-064]
crate: wayfarer-brain, wayfarer-settings, wayfarer-proto, ui
touches:
  - crates/wayfarer-brain/src/{party.rs,party_game.rs,session.rs}, crates/wayfarer-brain/src/guard.rs (new)
  - crates/wayfarer-settings/src/lib.rs
  - crates/wayfarer-proto/src/lib.rs, ui/src/PartiesPanel.svelte
  - docs/brain.md, docs/web-ui.md, docs/kb/quicksilver-1.20.md, docs/kb/changelog.md
---

## Goal

On a 1.20 server a party member can guard another: monster melee blows aimed at the ward may hit
the guard instead. The player chooses who guards whom in the party definition; the bot issues
`guard <ward>`, keeps it on through the moves and rests that end it, and shows it on the Parties
panel. `guard` is never sent unless the rule set is 1.20.

## Inputs

- `docs/plan/quicksilver-1.20/handoff.md` 3.4, 3.5 and 5.3 (the command, its refusals, what it
  covers, when it ends, the chance `25 + 5 × combat`%, several guards in start order).
- `crates/wayfarer-brain/src/party.rs` and `party_game.rs` (C-039 to C-045: definitions, forming
  up, rests, flees), `ui/src/PartiesPanel.svelte` (C-040).
- C-063's guard events, C-064's rule set and its guard check.

## Behaviour

- **Who guards whom.** A party definition gets an optional guard pair per member: "guards
  <member>". The player sets it on the Parties panel; nothing is chosen for them (goal 2: the
  player decides). Shown only when the party's sessions are all on 1.20; otherwise the field is
  greyed with "1.20 servers only".
- **Keeping it on.** When the guard and its ward are both in the room, in the party, and the
  guard isn't resting or meditating, the guard's bot sends `guard <ward first name>` if it doesn't
  believe it is guarding them. It believes it from `You are now guarding <ward>.` and stops
  believing at `You stop guarding <ward>.` (which 1.20 prints when they part, rest, meditate,
  leave the party, log off or die) and at a reconnect (no state survives).
- After each move of the party and each rest, re-guard once both are settled (handoff: both end
  it).
- **Refusals.** `You don't see that player here.`, `<Name> is not in your party.`, `You cannot
  guard yourself.`, `You cannot guard while resting or meditating.`: no retry until something
  changes (a move, the party list, the rest ending). Three refusals in a row hold the guard for
  five minutes and say so in the member's activity (C-046).
- **Stopping.** Removing the pair sends `guard` (alone) once; `You are not guarding anyone.` is
  fine.
- **Never on 1.11p or unknown**: the guard path goes through C-064's check, and a test proves the
  word is never sent there.
- **The ward's side.** `<Guard> is now guarding you.` / `stops guarding you.` set the ward's
  "guarded by" shown on the panel.

## Design notes

- `guard.rs`: the per-session guard state (wanted ward, believed ward, refusals, hold), fed the
  guard events and the party's state, returning `guard`/`guard <name>` actions to the executor at
  a quiet prompt out of combat (in combat too? the handoff doesn't forbid it; start out of combat
  only, and say so in docs).
- The pair is saved with the party definition (store), `#[serde(default)]` so old definitions
  load.

## Done when

- On 1.20, with a pair set, the guard sends `guard <ward>` once both are in the room, re-sends it
  after a move and after a rest, and stops on removal; the panel shows "guarding" / "guarded by".
- On 1.11p and unknown, no `guard` is ever sent, whatever the definitions say.
- Refusals don't loop.
- Docs: `docs/brain.md` "Parties" and `docs/web-ui.md` Parties panel; KB "GUARD" ([Q]); changelog.
- fmt, clippy, nextest, `npm run check` and `npm test` pass.

## Tests

- `guard.rs`: start, the stop lines, a move ending it and the re-guard, rest ending it, each
  refusal, the hold, removal sending `guard`, a reconnect clearing it.
- Never sent on 1.11p or unknown.
- An old party definition without pairs loads.

## Notes for the agent

- **Needs John:** whether the bot should ever pick a guard pair by itself (for example the
  highest-combat member guarding the lowest-HP one). This card leaves it to the player; ask
  through the PR.
