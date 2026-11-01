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
- **The two guard refusals that read like stock lines**, `You don't see that player here.` and
  `<Name> is not in your party.`, are in no file of the stock 1.11p package (the module and its
  message files); the nearest stock texts differ (`... don't see that anywhere`, `... don't see
  your target here`, `... is not in your gang`) [C, the stock package searched 2026-11-01].
  Wayfarer still counts them as 1.20 only right after a `guard` it sent, in case a sysop's own
  text matches.

## Spells

The data is wayfarer's `crates/wayfarer-world/data/quicksilver-1.20/` (its `PROVENANCE.md` says
where each file comes from).

- **Spell Mod v2.1.** The Mage, Priest and Druid player spells carry Spell Mod v2.1's values: 132
  of the 219 spells in those books differ from 1.11p (damage minimums grow with level, caps
  raised, the alignment nukes brought level, area spells up about 15-30% at their caps, buffs
  longer, a few big heals raised; mana costs almost unchanged) [Q].
- **Renamed.** Spell 24 detect magic is **protective aura** (Mage, level 5, 10 mana: magic
  resistance +4+L/5 to +3+L/2 for 20+2L/3 rounds, cap 30); item 121, the scroll that taught it,
  is `scroll of protective aura`. Spell 1056 dark plague is **dark binding** (Druid, level 50) [Q].
- **Slow (59) and solid fog (282)** no longer add energy use from their strength, for players
  and monsters alike [Q].
- **Caps at 65.** Every Mage, Priest and Druid player spell that grows with level and whose cap
  was above 65 or none is capped at 65, 825 (a Priest chant), 1379 ice shield and 1380 fire
  shield among them [Q].
- **Unchanged**: songs, kai and every other class's spells, but for the four regen songs [Q].
- **The regen line** (6000-6020): each caster family gets fixed mana-regeneration self-buffs at
  levels 5, 20, 35 and 50 (mana 4, 8, 12, 16; 120, 150, 180, 200 rounds; difficulty 0, -20, -40,
  -60), adding a fixed amount to Alter Mana Regeneration, no roll, no growth, no energy. The full
  casters get +50/+100/+150/+200%; the Gypsy and Warlock, the Cleric and Missionary, and the Bard
  get +25/+50/+75/+100%; the Paladin and Ranger get none. The level-35 tiers are the stock spells
  rewritten: mana flux (Mage, moved from level 16), nature tap (Druid, from 32), and serenity
  (good) and profane link (evil) for the Priest, each now a fixed +150% for 180 rounds at 12
  mana [Q].
- **Tiers replace each other in two casts.** Each tier removes every other tier of its family
  (full and half lines); a cast while another tier is up removes it and ends there, and a second
  cast lands the new tier. Serenity and profane link don't remove each other: their alignment
  gates keep them apart [Q].
- **Scrolls and shops.** Items 5100-5120 teach 6000-6020 in order; 5121 mana flux, 5122 nature
  tap, 5123 profane link; serenity keeps its stock scroll. The shops and prices are in
  `regen.csv`; markups are the shops' own (Newhaven Spell Shop and Orfeo's Stuff none, the others
  100%); stock is small high up (Nekojin 2, Very Powerful Spells 1, Jorah's 5), so a tier can be
  sold out. A shop sells a scroll to anyone; a class that can't use it is told `You may not use
  that item!` [Q].
- **Texts.** The new tiers share four message sets, one per family (cast to the caster and the
  room, the "when it ends" line, the status line), in `texts.csv`; mana flux, nature tap,
  serenity and profane link keep their stock texts, and mana flux's old start text, `You draw
  power from the mana surrounding you!`, is now its cast text [Q].
- **The monster copies** (5000-5056): monsters cast their own copies of the player spells they
  use, each its 1.11p original under the same name, so Spell Mod never reaches them. No player
  learns, buys or casts one; whatever removes or negates the original removes the copy too [Q].
- **Missing from the handoff's table:** the four regen songs' own rows (6017-6020). Wayfarer's
  `songs.csv` builds them from the tier table: magery Bard, magery level 0, self, type 3; their
  status lines are the song family's [Q, built; the rows asked for].

## Rest and meditate

- **Rest** gives HP as in 1.11p plus mana at three times the standing rate (`rest_mana_mult`),
  for any class with mana, from level 1 (the slow tick's mana plus the rest ticks' share; the
  server keeps the remainder exact). In 1.11p a rest gives no extra mana [Q].
- **Meditate** (still the stock ability from the stock quest; a class without it gets no output
  at all, as in 1.11p) gives mana at five times the standing rate (`meditate_mana_mult`; 1.11p
  about three) **and rest's HP**, and counts the regen tiers. A meditating character never needs
  to rest [Q].
- **Both end by themselves once HP and mana are full**: rest with no line (the `(Resting)` mark
  leaves the next prompt), meditate with `You awake from deep meditation feeling stronger!`. A
  class with no mana counts as full on mana. In 1.11p a rest never ends by itself [Q].
- **Meditate starts at full mana if HP is missing**; `Meditation will not help at this time.`
  only when both bars are full [Q].
- Rest and meditate still end each other, and anything that broke them in 1.11p still does [Q].
- The standing mana a slow tick gives is, as in 1.11p, `castval × (level + 20) × (magelvl + 2) /
  1650`, then scaled by the total Alter Mana Regeneration (`× (100 + rate) / 100`), to which a
  regen tier adds its value; rest and meditate multiply the result [Q].
