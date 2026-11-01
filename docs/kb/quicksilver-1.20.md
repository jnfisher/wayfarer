# Quicksilver 1.20

Quicksilver 1.20 is a MajorMUD server that plays 1.11p with a set of rule changes, frozen as
release 1.20.0. **This page holds 1.20 facts only.** Every other page of the KB is about stock
1.11p and stays true for it; where 1.20 differs, the difference is written here and not there.

Source: the Quicksilver 1.20.0 handoff, `docs/plan/quicksilver-1.20/handoff.md` (tag **[Q]**).
A fact then seen on a 1.20 server is tagged **[Q live]**. Wayfarer's wave 12 cards (C-063 to
C-069) build on this page; each adds its section.

## Lines

1.20 adds 22 lines. Each has a prefix no 1.11p line has, so their patterns are registered for
every server and match only on a 1.20 one [Q]. Wayfarer's copy is
`tests/fixtures/qs120/patterns-1.20.json` (each entry marked `"rules": "1.20"`; that mark is a
property of the data and is never sent over the wire). Priority 40, and 39 for the `_victim`
forms, so they are tried before every stock pattern (50 and up) [Q].

| Patterns | Lines | Meaning |
|---|---|---|
| `opportunist_you`, `_room`, `_victim` | `You find an opening and slash giant rat for 31 damage!`, `Thornwyn finds an opening and slashes giant rat for 23 damage!`, `... slashes you for 23 damage!` | A Thief's normal hit (not the stab) that left its target alive found an opening: one extra line straight after the hit line, same weapon verb, in the hit line's person. Extra damage on top of the hit; not a blow, never a crit. A killing hit gets no roll [Q] |
| `weave_you`, `_room`, `_victim` | `You weave meteor swarm into giant rat for 75 damage!` | A Paladin, Ranger, Cleric, Missionary, Warlock, Gypsy or Bard's normal hit (not the stab, not a kill) wove its best known single-target attack spell in as a free volley: one line per proc after the hit line, the damage summed, no cast line, mana or energy. A Bard's names its song. `for 0 damage!` is a landed weave that resistance zeroed. Spell damage; a weave that kills is a spell kill [Q] |
| `weave_resisted_you`, `_room`, `_victim` | `You weave meteor swarm into giant rat, but it is resisted!`, `Thornwyn weaves meteor swarm into you, but you resist!` | One resist roll for the whole proc: no damage. Not a miss or a failed cast [Q] |
| `guard_intercept_you`, `_ward`, `_room` | `You intercept the orc's attack on Mirelle!` (the guard), `Brannoc intercepts the orc's attack!` (the ward), `Brannoc intercepts the orc's attack on Mirelle!` (the room) | A guard took a monster's melee blow meant for its ward. No damage on this line: the stock monster hit or miss line that follows names the guard, and the ward took nothing from that blow [Q] |
| `guard_start_you`, `_ward`, `guard_stop_you`, `_ward` | `You are now guarding Mirelle.` / `Brannoc is now guarding you.`; `You stop guarding Mirelle.` / `Brannoc stops guarding you.` | Start and stop come in pairs, guard's side and ward's side [Q] |
| `guard_not_guarding`, `guard_not_here`, `guard_not_party`, `guard_self`, `guard_resting` | `You are not guarding anyone.`, `You don't see that player here.`, `Mirelle is not in your party.`, `You cannot guard yourself.`, `You cannot guard while resting or meditating.` | The refusals of `guard` [Q] |
| `look_weapon_strreq` | `It requires 75 strength to wield well (you have 92).` | After a weapon's description when its Str requirement is above 0, wherever the weapon is: the requirement and the looker's Str. Above it, the weapon is wielded badly, as in 1.11p [Q] |

- **Weave spell names.** The weave patterns' spell group is lazy (`.+?`), so it stops at the
  first ` into `; a spell whose own name has ` into ` in it is split on the last ` into ` instead,
  and the name matched against the character's known spells [Q; the first-` into ` split is how
  the regex reads, checked in wayfarer's tests].
- **Immunity.** A weave never picks a spell the target is immune to (the next candidate is used;
  with none left the hit doesn't weave), so a monster that never takes a weave from a weaver who
  weaves elsewhere is likely immune to its spells [Q].
- **Who never weaves**: Mages, Priests, Druids, Mystics, Warriors, Witchunters, Thieves and
  Ninjas [Q].
- **The critical stab.** No new text, a new combination: a stab can crit on 1.20, and prints the
  stock `critically` prefix with the stock `surprise` prefix before the weapon's verb, `You
  critically surprise slash giant rat for 140 damage!`, `Thornwyn critically surprise slashes
  giant rat for 140 damage!`. The stock `you_hit` and `other_hits` patterns read both [Q]. In
  1.11p a stab never crits, so this line is 1.20-only.
- **The round after the stab** goes on: more hit and miss lines without `surprise` follow in the
  same round, normal blows with normal crits. One stab a fight, as in 1.11p [Q].
- **Rest and meditate** print the stock lines; only when they print changed (the "Rest and
  meditate" section, C-067) [Q].
- **Lines a bot can ignore**: `The next point of Strength costs 15 CP.` (any stat, any price) in
  the stat-training editor; monster casts look exactly as in 1.11p (monsters cast stock copies of
  their spells) [Q].
- **Unchanged**: every stock line not named here, the login, menus, version line and banner, the
  prompt and its `(Resting)` / `(Meditating)` markers [Q].

## Detection

- **`help 1.20`.** A 1.20 server answers the command with a topic whose first line, after ANSI
  and trailing white space are stripped, is exactly `Quicksilver 1.20: how the rules differ from
  1.11p`, whether or not the server has the stock help text installed. A 1.11p server has no
  such topic and answers the stock `Sorry, no help is available for that topic!` [Q]. The whole
  topic is in `tests/fixtures/qs120/help-1.20.txt`: 34 lines, one of them blank, each ending in
  `\r` on the wire [Q].
- **Lines only 1.20 prints**, each impossible on 1.11p: any line of the "Lines" section; a
  backstab with both `critically` and `surprise` (in 1.11p a stab never crits); a 1.20 spell
  name in the spell list, a shop list or a scroll name (`protective aura`, `dark binding`, the
  regen line's names, `scroll of protective aura`); and `You are now meditating.` when mana is
  full and HP is not, which 1.11p refuses with `Meditation will not help at this time.` [Q].
- **Not signals.** `version` prints `MajorMUD v 1.11p-WG (...)` on a 1.20 server too, and the
  login banner is the same. `guard` is not a command on 1.11p: it falls through and is said
  aloud to the room, so it is no probe [Q].
- **Open.** Whether `You don't see that player here.` and `<Name> is not in your party.` are
  also stock 1.11p lines (for other commands) is not known; wayfarer counts them as 1.20 only
  right after a `guard` it sent [I].
