# Wayfarer: support stock 1.11p and Quicksilver 1.20 side by side

This is a work prompt for an agent in the wayfarer repository. Wayfarer plays MajorMUD and runs a
bot. Today it knows stock MajorMUD 1.11p. **Quicksilver 1.20** is a MajorMUD server that plays
1.11p with a set of rule changes, frozen as release 1.20.0. Your job is to make wayfarer support
both: it finds out which rules a server plays, and it turns the 1.20 behaviour on only for a 1.20
server.

Everything you need about 1.20 is in this document: every new line and its pattern, the
detection rule, the spell data, the new commands and the tuning numbers. You have no access to the
server's own repository and don't need it. Where this document says "as 1.11p" or "unchanged",
use what wayfarer already knows.

## 0. Before you start

1. Read wayfarer's own `CLAUDE.md` and everything under `docs/plan/` first. They decide how work is
   planned, where code goes, how tests are written and what a change must include. Where they
   conflict with this document on process, they win; this document only supplies the game facts.
2. **New game facts go in wayfarer's `docs/kb/`**, with the provenance tags wayfarer's KB already
   uses. Tag everything taken from this document as coming from the Quicksilver 1.20.0 handoff
   (the server's frozen 1.20.0 release), and tag anything you then see on a live server as
   observed. Add a line to `docs/kb/changelog.md` for each KB change. Keep 1.20 facts clearly
   apart from 1.11p facts, for example a `1.20` section or page per topic, so that nothing about
   1.11p is overwritten.
3. **Test data goes in `tests/fixtures/<subdir>/`** (for example `tests/fixtures/qs120/`) with a
   `PROVENANCE.md` that says where each file came from: this handoff, or a capture from a server
   (named by placeholder only).
4. **Wayfarer is public.** In code, docs, fixtures, commit messages and PRs, use placeholders for
   anything that identifies a real server or person: `<server-host>`, `<port>`, `<bbs-account>`,
   `<password>`, `<character>`. Never commit a real hostname, IP address, account name or
   password, and scrub captures before they become fixtures.
5. **Bots never `train stats`.** 1.20 changes stat training (section 6.11); that is for the
   documentation and for reading other characters' stats, never for the bot to do.

## 1. The shape of the change

- Give wayfarer a rule set per connection: **1.11p**, **1.20**, or **unknown** until detected.
  A server profile can remember the last detection, but detect again on each login: a server can
  be rebuilt.
- **The 1.20 line patterns can always be registered.** Each 1.20 line has a prefix that no 1.11p
  line has, so a 1.20 pattern never matches a stock line. Register them for every server and let
  the rule set decide what the bot *does*: its spell plan, its rest and meditate logic, `GUARD`,
  its damage models and its "where next?" advice.
- While the rule set is unknown, behave as 1.11p. A wrong guess of 1.11p on a 1.20 server is safe
  (the bot plays a little worse); a wrong guess of 1.20 on a 1.11p server is not (it sends
  commands the server reads as speech, and plans spells that don't exist).

## 2. Detection

### Primary: `HELP 1.20`

After login, at the first prompt and out of combat, send:

```text
help 1.20
```

A 1.20 server answers with this topic. Its first line is the signal; match it exactly, after
stripping ANSI and trailing whitespace:

```text
Quicksilver 1.20: how the rules differ from 1.11p
```

The topic works whether or not the server has the stock help text installed. A 1.11p server has no
such topic and answers with the stock line:

```text
Sorry, no help is available for that topic!
```

(pattern `help_none`, below). Any other answer, or none within a few seconds, leaves the rule set
unknown; try again at the next quiet prompt, at most a few times a session, then fall back.

The whole 1.20 answer, line by line, as the server sends it (each line ends with `\r` on the wire). Wayfarer can show it
to the user, and must not run its normal line patterns over it as game events:

```text
Quicksilver 1.20: how the rules differ from 1.11p

Backstab
A backstab costs 1.5 blows of energy; the rest of the round is normal blows.
A stab can crit (max to 2 x max damage); stealth and level add more to it.
Thieves: 10% of normal hits find an opening and add a stab's bonus.
Spells and mana
Mage, Priest and Druid spells use Spell Mod v2.1. Detect magic is now
protective aura, dark plague is dark binding.
Spell damage +0.5% per point of your casting stat over 50 (a third past 100).
Rest restores mana 3 x faster; meditate 5 x, and heals as rest does.
Each caster family has a line of mana-regeneration spells, sold in shops.
Area spells: a monster may resist, cutting its share by at most 25%.
Witchunters never resist their own or their party's friendly spells.
Races and Str
LOOK at a weapon to see its Str requirement and yours.
Humans, Dwarves, Elves and Gaunt Ones regenerate faster.
Hybrids and class damage
Paladins, Rangers, Clerics, Missionaries, Warlocks, Gypsies and Bards weave:
some melee hits add a free attack spell. Paladins and Rangers weave less.
Each class's damage is tuned to a band against the Warrior's.
Stats
TRAIN STATS goes past your race maximum: 10 CP a point up to 90, then
15, 25 and 40 CP a point, up to 30 points over.
Crit chance past 50, magic resistance past 110 and dodge Agility past 150
count a third. A stat-lowering spell stops at 0.
Groups
Monsters attack the front rank most and the back rank least.
GUARD <player>: take blows aimed at a party member. GUARD alone stops.
Scaling
Backstab, opportunist and weave stop growing at level 60; spells at 65.
Smaller
Below 50 magic resistance, armour-piercing spells hurt more, not less.
Monsters cast their own copies of their spells.
```

### Fallback: any 1.20-only line or fact

If `HELP 1.20` gave no answer, switch to 1.20 the first time any of these is seen. Each is
impossible on 1.11p:

1. any line matching one of the 1.20 patterns in section 3 (`opportunist_*`, `weave_*`,
   `guard_*`, `look_weapon_strreq`);
2. a backstab line with both `critically` and `surprise`, such as
   `You critically surprise slash giant rat for 80 damage!`: in 1.11p a stab never crits;
3. a 1.20 spell name in the character's spell list (`spells`), a shop list or a scroll name:
   `protective aura`, `dark binding`, or any regen-line name of section 4.2 (`lesser attunement`,
   `devotion`, `song of reverie`, ...), or `scroll of protective aura`;
4. `meditate` starting (`You are now meditating.`) when the character's mana is full and its HP
   is not: 1.11p refuses that with `Meditation will not help at this time.`

Signals 1 and 2 are passive and come up in normal play for the classes that have them. Signal 3 is
the reliable passive one for a caster: it costs one `spells` command, which wayfarer sends anyway.

### Not signals

- **The version line is the same on both.** `version` prints `MajorMUD v 1.11p-WG (...)` on a 1.20
  server too, and the login banner is unchanged. Don't use either.
- **Don't probe with `guard`.** On 1.11p the word is not a command: it falls through and is
  spoken aloud to the room. Only send `guard` once the rule set is 1.20.
- The server's patterns export (a JSON registry the server project keeps for its own tests) carries
  a field `"rules": "1.20"`. That is a property of the file, never sent over the wire. If wayfarer
  keeps its own pattern registry as data, mark the 1.20 entries the same way, but it is not a
  detection signal.

## 3. New and changed lines

### How to read the patterns

- Each pattern is a Python-style regex (`(?P<name>...)` groups) to be matched against **one whole
  line** (a full match), after ANSI sequences and the line-clear prefix are stripped and the line
  is trimmed. Rust's `regex` crate takes the same syntax (`(?P<name>...)` is accepted).
- **Priority: lower wins.** Stock static patterns are 50, the generic "other player hits" pattern
  52, patterns built from game data 60, generic fallbacks 90. Every 1.20 pattern is 40 (39 for the
  `_victim` forms), so it is tried before the stock ones. Within a priority, the longer (more
  literal) pattern wins.
- `{HITV}` and `{HITV3}` in the stock hit patterns stand for the alternation of every weapon hit
  verb in the first and third person (`slash`/`slashes`, `stab`/`stabs`, `punch`/`punches`, ...),
  as wayfarer builds them for 1.11p.
- Every line, regex and example below was checked against the server's frozen 1.20.0 patterns
  export. The example damage numbers are examples, nothing more.

### 3.1 Thief opportunist

A Thief's **normal hit** (not the stab) that leaves its target alive has 10% to "find an opening".
The opening prints **one extra line straight after the hit line**, using the same weapon verb, in
the form the hit line used (`slash` for you, `slashes` for others):

```text
You slash giant rat for 7 damage!
You find an opening and slash giant rat for 31 damage!
```

```text
opportunist_you   [combat, priority 40]
  rx:      You find an opening and (?P<verb>\S+) (?P<target>.+) for (?P<dmg>\d+) damage!
  example: You find an opening and slash giant rat for 23 damage!

opportunist_room   [combat, priority 40]
  rx:      (?P<who>[A-Z][A-Za-z]+) finds an opening and (?P<verb>\S+) (?P<target>.+) for (?P<dmg>\d+) damage!
  example: Thornwyn finds an opening and slashes giant rat for 23 damage!

opportunist_victim   [combat, priority 39]
  rx:      (?P<who>[A-Z][A-Za-z]+) finds an opening and (?P<verb>\S+) you for (?P<dmg>\d+) damage!
  example: Thornwyn finds an opening and slashes you for 23 damage!
```

For tracking:

- **Damage:** `dmg` is extra damage on top of the hit line's. Add it to the target's damage taken
  and to the Thief's damage dealt. It comes off the target's HP and counts for the kill like the
  hit.
- **Hit and crit statistics:** it is **not** a blow. Don't count it as a hit, a swing or a crit;
  count it as an "opening" (proc) of the hit before it. A crit shows only on the hit line; the
  opening never crits.
- A hit that kills gets no roll, so the death line follows the hit line directly.
- The room form names the Thief; the victim form is PvP (a player is the target).

### 3.2 Weave (the hybrids)

When a Paladin, Ranger, Cleric, Missionary, Warlock, Gypsy or Bard lands a normal hit (not the
stab, not a killing blow), it may "weave": the best single-target attack spell the character
knows fires as a free volley. **One line per proc, after the hit line**, naming the spell; `dmg`
is the volley's **summed** damage (a spell that would hit several times a round is summed into one
number). There is no cast line, no mana and no energy. A Bard weaves one hit of its best damage
song, and the line names the song.

```text
weave_you   [spell, priority 40]
  rx:      You weave (?P<spell>.+?) into (?P<target>.+) for (?P<dmg>\d+) damage!
  example: You weave meteor swarm into giant rat for 75 damage!

weave_room   [spell, priority 40]
  rx:      (?P<who>[A-Z][A-Za-z]+) weaves (?P<spell>.+?) into (?P<target>.+) for (?P<dmg>\d+) damage!
  example: Thornwyn weaves meteor swarm into giant rat for 75 damage!

weave_victim   [spell, priority 39]
  rx:      (?P<who>[A-Z][A-Za-z]+) weaves (?P<spell>.+?) into you for (?P<dmg>\d+) damage!
  example: Thornwyn weaves meteor swarm into you for 75 damage!

weave_resisted_you   [spell, priority 40]
  rx:      You weave (?P<spell>.+?) into (?P<target>.+), but it is resisted!
  example: You weave meteor swarm into giant rat, but it is resisted!

weave_resisted_room   [spell, priority 40]
  rx:      (?P<who>[A-Z][A-Za-z]+) weaves (?P<spell>.+?) into (?P<target>.+), but it is resisted!
  example: Thornwyn weaves meteor swarm into giant rat, but it is resisted!

weave_resisted_victim   [spell, priority 39]
  rx:      (?P<who>[A-Z][A-Za-z]+) weaves (?P<spell>.+?) into you, but you resist!
  example: Thornwyn weaves meteor swarm into you, but you resist!
```

For tracking:

- **Damage:** add `dmg` to the target's damage taken and the weaver's damage dealt, as **spell**
  damage, not melee. `for 0 damage!` is possible (magic or element resistance zeroed it): it is a
  landed proc that did nothing, not a resist.
- **Resist:** the `weave_resisted_*` forms are one resist roll for the whole proc: a resisted proc
  deals nothing. Count it as a resisted spell for the target (useful for estimating its magic
  resistance), never as a miss or a failed cast.
- **Hit statistics:** the proc is not a blow; the hit line before it is the blow.
- **Kills:** a weave that kills prints its line, then the death line. Credit the kill as a spell
  kill.
- A spell the target is immune to is never picked; the next candidate is used, and with none left
  the hit doesn't weave. So a monster that never takes a weave from a weaver who weaves elsewhere
  is likely immune to its spells.
- Mages, Priests, Druids, Mystics, Warriors, Witchunters, Thieves and Ninjas never weave.
- **Spell names** in the line use `.+?`, so a name with "into" in it still splits correctly on
  the last ` into `; match the spell name against the character's known spells.

### 3.3 Backstab lines

No new text; one new combination. A stab can now crit, and a critical stab prints the stock
`critically` prefix together with the stock `surprise` prefix, before the weapon's own hit verb:

```text
You surprise slash giant rat for 61 damage!
You critically surprise slash giant rat for 140 damage!
Thornwyn critically surprise slashes giant rat for 140 damage!
```

The stock patterns already have both optional groups:

```text
you_hit   [combat, priority 50]
  rx:      You (?:(?P<crit>critically) )?(?:(?P<surprise>surprise) )?(?P<verb>{HITV}) (?P<target>.+) for (?P<dmg>\d+) damage!
  example: You smack giant rat for 4 damage!

other_hits   [combat, priority 52]
  rx:      (?P<who>[A-Z][A-Za-z]+) (?:(?P<crit>critically) )?(?:(?P<surprise>surprise) )?(?P<verb>{HITV3}) (?P<target>.+) for (?P<dmg>\d+) damage!
  example: Thornwyn whaps giant rat for 4 damage!
```

For tracking:

- The `surprise` prefix goes before the weapon's verb in every person (`surprise slash`,
  `surprise slashes`).
- `surprise` set: the blow is the stab. `crit` and `surprise` both set: a critical stab, which
  rolls from the stab's maximum to twice its maximum (other crits keep 2× to 4×).
- **The round goes on after the stab** (section 5.4): in the same round, more `You slash ...` hit
  and miss lines follow without `surprise`. They are normal blows with normal damage and normal
  crits. Wayfarer's 1.11p logic may assume that a stab ends the round, or that every blow in a
  backstab round is a stab: on 1.20 neither is true.
- One stab a fight, as in 1.11p.

### 3.4 `GUARD`: intercepts

When a guard takes a monster's melee blow meant for its ward, three viewers each get one line, and
then **the stock monster hit or miss line follows, naming the guard** (not the ward):

```text
You intercept the orc's attack on Mirelle!        (to the guard)
Brannoc intercepts the orc's attack!              (to the ward)
Brannoc intercepts the orc's attack on Mirelle!   (to everyone else in the room)
The orc hits Brannoc for 12 damage!               (stock line, as 1.11p prints it)
```

```text
guard_intercept_you   [combat, priority 40]
  rx:      You intercept (?:the )?(?P<mon>.+?)'s attack on (?P<ward>[A-Z][A-Za-z]+)!
  example: You intercept the orc's attack on Mirelle!

guard_intercept_ward   [combat, priority 40]
  rx:      (?P<guard>[A-Z][A-Za-z]+) intercepts (?:the )?(?P<mon>.+?)'s attack!
  example: Brannoc intercepts the orc's attack!

guard_intercept_room   [combat, priority 40]
  rx:      (?P<guard>[A-Z][A-Za-z]+) intercepts (?:the )?(?P<mon>.+?)'s attack on (?P<ward>[A-Z][A-Za-z]+)!
  example: Brannoc intercepts the orc's attack on Mirelle!
```

For tracking: the intercept line itself carries no damage. Attribute the **next** monster hit or
miss line to the guard, as its own line says. The ward took nothing from that blow. Count an
intercept as "blows taken for the ward" in the guard's statistics; it is not a dodge, a miss or a
block for the ward.

### 3.5 `GUARD`: the command and its refusals

```text
guard_start_you   [party, priority 40]
  rx:      You are now guarding (?P<ward>[A-Z][A-Za-z]+)\.
  example: You are now guarding Mirelle.

guard_start_ward   [party, priority 40]
  rx:      (?P<guard>[A-Z][A-Za-z]+) is now guarding you\.
  example: Brannoc is now guarding you.

guard_stop_you   [party, priority 40]
  rx:      You stop guarding (?P<ward>[A-Z][A-Za-z]+)\.
  example: You stop guarding Mirelle.

guard_stop_ward   [party, priority 40]
  rx:      (?P<guard>[A-Z][A-Za-z]+) stops guarding you\.
  example: Brannoc stops guarding you.

guard_not_guarding   [party, priority 40]
  rx:      You are not guarding anyone\.
  example: You are not guarding anyone.

guard_not_here   [party, priority 40]
  rx:      You don't see that player here\.
  example: You don't see that player here.

guard_not_party   [party, priority 40]
  rx:      (?P<who>[A-Z][A-Za-z]+) is not in your party\.
  example: Mirelle is not in your party.

guard_self   [party, priority 40]
  rx:      You cannot guard yourself\.
  example: You cannot guard yourself.

guard_resting   [party, priority 40]
  rx:      You cannot guard while resting or meditating\.
  example: You cannot guard while resting or meditating.
```

Start and stop come in pairs: the guard sees `You are now guarding Mirelle.` and the ward sees
`Brannoc is now guarding you.`; on stop, `You stop guarding Mirelle.` and `Brannoc stops guarding
you.`. Section 5.3 has the command and when it ends.

### 3.6 `look <weapon>`: the Str requirement

After a weapon's description, when its Str requirement is above 0, wherever the weapon is (in the
room, carried, worn or in a shop):

```text
look_weapon_strreq   [room, priority 40]
  rx:      It requires (?P<str>\d+) strength to wield well \(you have (?P<have>\d+)\)\.
  example: It requires 75 strength to wield well (you have 92).
```

`str` is the requirement and `have` the looker's Str. Wayfarer can record the requirement in its
item knowledge and use it in gear choice: a weapon with `str > have` is wielded badly, as in
1.11p (wielding itself is unchanged).

### 3.7 Rest and meditate: no new lines, new behaviour

The rest and meditate lines are the stock ones:

```text
rest_start   [rest, priority 50]
  rx:      You are now resting\.
  example: You are now resting.

meditate_start   [rest, priority 50]
  rx:      You are now meditating\.
  example: You are now meditating.

meditate_done   [rest, priority 50]
  rx:      You awake from deep meditation feeling stronger!
  example: You awake from deep meditation feeling stronger!

meditate_useless   [rest, priority 50]
  rx:      Meditation will not help at this time\.
  example: Meditation will not help at this time.

rest_other   [rest, priority 50]
  rx:      (?P<who>.+) stops to rest\.
  example: Bob stops to rest.

meditate_other   [rest, priority 50]
  rx:      (?P<who>.+) kneels to meditate\.
  example: Bob kneels to meditate.
```

What changed (details in section 5.1):

- **Rest ends by itself, silently, once HP and mana are both full** (for a class with no mana:
  at full HP). There is **no line**: the `(Resting)` marker just disappears from the next prompt.
  In 1.11p rest never ends by itself; it keeps sending the prompt at full HP.
- **Meditate wakes with the stock line only when HP and mana are both full**, not when mana
  alone is full: `You awake from deep meditation feeling stronger!`
- `Meditation will not help at this time.` now means HP **and** mana are both full. At full mana
  with HP missing, meditate starts.

Read the state from the prompt: the statline's `(Resting)` / `(Meditating)` marker. The server's
standard prompt pattern is:

```text
\[HP=(?P<hp>-?\d+)(?:/(?P<mtype>MA|KAI)=(?P<mana>-?\d+))?(?P<state1> \((?:Resting|Meditating)\) )?\]:(?P<state2> \((?:Resting|Meditating)\) )?(?:\{Energy=(?P<energy>-?\d+) (?P<dbg>[^}]*)\}:)?
```

(unchanged from 1.11p).

### 3.8 The regen-line cast and status texts

The new mana-regeneration spells (section 4.2) use four new message pairs, shared by all tiers of
a family. `%s` is the caster's name in the target and room forms.

| Family | To the caster | To the room |
|---|---|---|
| Mage (attunement) | `You attune your mind to the flow of mana.` | `%s attunes to the flow of mana.` |
| Priest (devotion) | `You bow your head in quiet devotion.` | `%s bows their head in quiet devotion.` |
| Druid (communion) | `You commune with the living world around you.` | `%s communes with the living world.` |
| Bard (songs) | `You hum a slow, dreaming melody.` | `%s hums a slow, dreaming melody.` |

| Family | When it ends | How it shows on the caster |
|---|---|---|
| Mage | `Your attunement to the mana fades.` | `You are attuned to the flow of mana.` |
| Priest | `Your devotion wanes.` | `Your devotion renews your power.` |
| Druid | `Your communion with nature ends.` | `You are in communion with nature.` |
| Bard | `The melody in your mind fades.` | `A dreaming melody fills your mind.` |

These are spell texts, built into the server's spell-message patterns the same way as 1.11p's (no
static pattern of their own). Mana flux, nature tap, serenity and profane link keep their stock
texts, except that mana flux's old start text, `You draw power from the mana surrounding you!`, is
now its cast text, and its old cast text is gone.

Wayfarer should treat each family's "when it ends" line as the buff's expiry, for re-casting.

### 3.9 Lines a bot can ignore

- `The next point of Strength costs 15 CP.` (any stat name, any price): shown in the stat-training
  editor, before each stat prompt. No pattern; bots never train stats.
- Monster casts: monsters now cast their own copies of their spells (section 4.3). The copies keep
  the stock names and texts, so monster cast lines look exactly as in 1.11p.

## 4. Spells, for the spell plan

### 4.1 What changed

- **Spell Mod v2.1.** The Mage, Priest and Druid **player** spells carry Spell Mod v2.1's values:
  132 of the 219 spells in those books differ from 1.11p. The pattern: damage minimums grow with
  level, level caps are raised, the Good/Neutral/Evil alignment nukes are brought level with each
  other, area spells are up about 15-30% at their caps, buffs last longer and scale, a few big
  heals are raised. Mana costs are almost unchanged, so the gain is damage per mana.
- **Renamed and reworked:** spell 24 **detect magic → protective aura** (Mage, level 5, 10 mana:
  magic resistance +4+L/5 to +3+L/2, 20+2L/3 rounds, cap 30; the item that taught detect magic is
  now `scroll of protective aura`). Spell 1056 **dark plague → dark binding** (Druid, level 50).
- **Slow (59) and solid fog (282)** no longer add energy use from their strength (only their
  slowness remains), for players and monsters alike.
- **Level caps at 65.** Every Mage, Priest and Druid player spell that grows with level (strength
  or duration) and whose cap was above 65, or none, is capped at 65. Three spells outside the
  table below get only this change: 825 (a Priest chant), 1379 ice shield and 1380 fire shield
  (Druid), all now cap 65.
- **Songs, kai and every other class's spells are unchanged**, except the four new Bard songs of
  the regen line.
- **Spell damage scales with the casting stat** and with the class's `class_spell_pct`
  (section 6). Heals and buffs don't scale.
- **Mana flux moves from level 16 to level 35** and becomes a fixed +150% regen buff (section
  4.2). A Mage's level-16 plan that relied on flux uses arcane attunement at 20 instead.

How strength grows with level is unchanged from 1.11p: with the caster's level `L` held to the
spell's level cap,

```text
min = minval + mininc × L / minlvls      (no growth when minlvls is 0), held to at most max
max = maxval + maxinc × L / maxlvls      (no growth when maxlvls is 0)
duration = duration + durinc × (L / durlvls)
```

The cast's strength is uniform in `[min, max]`.

### 4.2 The regen line (new spells 6000-6020)

Each caster family gets four fixed mana-regeneration self-buffs at levels 5, 20, 35 and 50. Each
is self-only, lasting, adds a fixed amount to the caster's "Alter Mana Regeneration" (no roll, no
growth with level, never negative) and costs no energy.

| Tier | Mana | Duration (rounds) | Difficulty |
|---|---|---|---|
| L5 | 4 | 120 | 0 |
| L20 | 8 | 150 | -20 |
| L35 | 12 | 180 | -40 |
| L50 | 16 | 200 | -60 |

| No. | Name | Short | Book (magery/level) | Who casts it | Level | Regen | Scroll (item) | Shop (number) | Price |
|---|---|---|---|---|---|---|---|---|---|
| 6000 | lesser attunement | latt | Mage/3 | Mage | 5 | +50% | 5100 scroll of lesser attunement | Newhaven Spell Shop (48) | 20 gold |
| 6001 | arcane attunement | aatt | Mage/3 | Mage | 20 | +100% | 5101 scroll of arcane attunement | Jorah's Mage Spells (174) | 600 gold |
| 406 | mana flux | flux | Mage/3 | Mage | 35 | +150% | 5121 scroll of mana flux | Nekojin Village Store (111) | 3500 gold |
| 6002 | greater attunement | gatt | Mage/3 | Mage | 50 | +200% | 5102 scroll of greater attunement | Nekojin Village Store (111) | 5000 gold |
| 6009 | minor attunement | mnat | Mage/2 | Gypsy, Warlock | 5 | +25% | 5109 scroll of minor attunement | Newhaven Spell Shop (48) | 20 gold |
| 6010 | modest attunement | mdat | Mage/2 | Gypsy, Warlock | 20 | +50% | 5110 scroll of modest attunement | Jorah's Mage Spells (174) | 600 gold |
| 6011 | steady attunement | stat | Mage/2 | Gypsy, Warlock | 35 | +75% | 5111 scroll of steady attunement | Jorah's Mage Spells (174) | 3500 gold |
| 6012 | deep attunement | dpat | Mage/2 | Gypsy, Warlock | 50 | +100% | 5112 scroll of deep attunement | Jorah's Mage Spells (174) | 5000 gold |
| 6003 | lesser devotion | ldev | Priest/3 | Priest | 5 | +50% | 5103 scroll of lesser devotion | Newhaven Spell Shop (48) | 20 gold |
| 6004 | devotion | devo | Priest/3 | Priest | 20 | +100% | 5104 scroll of devotion | Jorah's Priest Spells (173) | 600 gold |
| 1377 | serenity (good) | nity | Priest/3 | Priest | 35 | +150% | stock scroll, as 1.11p | Very Powerful Spells (75), as 1.11p | 2750 gold, as 1.11p |
| 1378 | profane link (evil) | prfl | Priest/3 | Priest | 35 | +150% | 5123 scroll of profane link | Very Powerful Spells (75) | 2750 gold |
| 6005 | greater devotion | gdev | Priest/3 | Priest | 50 | +200% | 5105 scroll of greater devotion | Nekojin Village Store (111) | 5000 gold |
| 6013 | minor devotion | mndv | Priest/2 | Cleric, Missionary | 5 | +25% | 5113 scroll of minor devotion | Newhaven Spell Shop (48) | 20 gold |
| 6014 | modest devotion | mddv | Priest/2 | Cleric, Missionary | 20 | +50% | 5114 scroll of modest devotion | Jorah's Priest Spells (173) | 600 gold |
| 6015 | steady devotion | stdv | Priest/2 | Cleric, Missionary | 35 | +75% | 5115 scroll of steady devotion | Jorah's Priest Spells (173) | 2750 gold |
| 6016 | deep devotion | dpdv | Priest/2 | Cleric, Missionary | 50 | +100% | 5116 scroll of deep devotion | Jorah's Priest Spells (173) | 5000 gold |
| 6006 | lesser communion | lcom | Druid/3 | Druid | 5 | +50% | 5106 scroll of lesser communion | Newhaven Spell Shop (48) | 20 gold |
| 6007 | communion | comm | Druid/3 | Druid | 20 | +100% | 5107 scroll of communion | Jorah's Tower, Druid Spells (73) | 700 gold |
| 854 | nature tap | ntap | Druid/3 | Druid | 35 | +150% | 5122 scroll of nature tap | Nekojin Village Store (111) | 3500 gold |
| 6008 | greater communion | gcom | Druid/3 | Druid | 50 | +200% | 5108 scroll of greater communion | Nekojin Village Store (111) | 5000 gold |
| 6017 | song of reverie | reve | Bard | Bard | 5 | +25% | 5117 songsheet of reverie | Bard Song Store (37) | 20 gold |
| 6018 | song of rapture | rptr | Bard | Bard | 20 | +50% | 5118 songsheet of rapture | Orfeo's Stuff (93) | 50 platinum |
| 6019 | song of stillness | stil | Bard | Bard | 35 | +75% | 5119 songsheet of stillness | Orfeo's Stuff (93) | 80 platinum |
| 6020 | song of transcendence | tran | Bard | Bard | 50 | +100% | 5120 songsheet of transcendence | Orfeo's Stuff (93) | 120 platinum |

- **Prices** are the scroll's cost. The shop's markup comes on top, then the buyer's Charm, as in
  1.11p: Newhaven Spell Shop and Orfeo's Stuff add none; Bard Song Store, Jorah's shops, Nekojin
  Village Store and Very Powerful Spells add 100% (600 gold sells for 1200).
- **Stock** (quantity, restock time, restock chance): Newhaven 40/240/100%; Bard Song Store
  30/24/100%; Jorah's three shops 5/56/15%; Nekojin 2/120/25%; Very Powerful Spells 1/252/5%;
  Orfeo's 2/640/25%. The high-tier scrolls can be sold out; the plan must allow for waiting or
  skipping a tier.
- The old items that taught mana flux (a black tome), nature tap and profane link (a stained
  papyrus scroll) still teach them.
- **A shop sells a scroll to anyone.** A class outside the "who casts it" column can buy it but
  can't use it (`You may not use that item!`). Paladins and Rangers get no tier.
- **Tiers replace each other, and that takes two casts.** Each tier removes every other tier of its
  family (full and half lines). Casting a new tier while an old one is up removes the old one
  **and ends that cast there**; cast again to land the new tier. Plan a second cast after an
  upgrade. Serenity and profane link don't remove each other; their alignment gates keep them
  apart.
- The rewritten stock spells: mana flux was level 16 with a rolled -115..+140 (growing with level)
  for 70+L rounds at 20 mana; nature tap was level 32, -100..+150 rolled, 100 rounds, 20 mana;
  serenity was level 37 and profane link level 35, -25..+50 rolled, 80 rounds, 30 mana. Serenity
  loses its damage -5, profane link its healing rate -25. All four are now fixed +150% for 180
  rounds at 12 mana.
- **What the bonus does:** the normal (standing) mana a slow tick gives is, as in 1.11p,
  `castval × (level + 20) × (magelvl + 2) / 1650`, then scaled by the total Alter Mana
  Regeneration (`× (100 + rate) / 100`). A regen tier adds its value to that rate, as do the race
  bonuses (section 6.7). `castval` is the casting stat: Int for Mage magery, Wil for Priest, the
  mean of Int and Wil (integer division) for Druid, Cha for Bard. Rest and meditate multiply the
  result (section 5.1).

### 4.3 The monster copies (5000-5056), never learned

Monsters now cast their own copies of the player spells they use, so Spell Mod's changes never
reach monsters. **Players never learn, buy or cast these numbers.** Leave the whole range
5000-5999 out of the spell plan. Each copy is its 1.11p original, whole, under the same name; the
spells and items that remove or negate the original remove or negate the copy too.

```text
5000 boulder (1047); 5001 dark plague (1056); 5002 frozen spike (1003); 5003 inferno (1048);
5004 ember (207); 5005 acid tempest (805); 5006 summon fire elemental (239);
5007 summon ice elemental (240); 5008 summon lightning elemental (241);
5009 summon stone elemental (242); 5010 earthfist (296); 5011 earthquake (295);
5012 plasma bolt (293); 5013 acid jet (95); 5014 chain lightning (1018); 5015 bladed sphere (1041);
5016 hellstorm (1057); 5017 magma blast (802); 5018 sphere of implosion (216); 5019 blizzard (771);
5020 eldritch storm (1058); 5021 forked lightning (706); 5022 colour spray (254);
5023 dragonfire (288); 5024 spirit horde (287); 5025 meteor swarm (285); 5026 eldritch bolt (136);
5027 ice storm (135); 5028 sunbolt (56); 5029 lightning bolt (8); 5030 frost jet (5);
5031 exalted word (1005); 5032 tainted word (1081); 5033 soul rip (211); 5034 soulstrike (721);
5035 divine fury (267); 5036 god's wrath (122); 5037 holy force (110); 5038 spiritual hammer (16);
5039 harm (12); 5040 ember (210); 5041 exalted word (1007); 5042 tainted word (1080);
5043 earthquake hold (927); 5044 drenched in acid (934); 5045 freezing (1015);
5046 mageshield (132); 5047 partial petrification (141); 5048 agony (22); 5049 magic armour (52);
5050 unholy force (111); 5051 damnation (1025); 5052 blur (129); 5053 magic missile (1);
5054 sandstorm (1310); 5055 regeneration (349); 5056 godheal (121)
```

(The number in brackets is the player spell it copies.) For wayfarer's threat model: a monster's
spell does what it did in 1.11p, so keep 1.11p's numbers for monster casts, and use the table in
4.5 only for players.

### 4.4 The scrolls

- Items 5100-5120 teach spells 6000-6020 in order (names in 4.2); 5121 scroll of mana flux,
  5122 scroll of nature tap, 5123 scroll of profane link. Each is read like any stock spell scroll.
- Item 121 is renamed `scroll of protective aura` (it taught detect magic).
- No other item changes.

### 4.5 The full 1.20 player-spell table

Every Mage, Priest and Druid player spell as 1.20.0 has it, plus the regen line (the four songs at
the end). Spells not listed (songs, kai and the spells of the other classes) are as in 1.11p, save
825, 1379 and 1380 (cap 65, section 4.1).

Columns:

- `num`, `name`, `short` (the short name the spell list shows; you cast by it);
- `magery`, `mlvl`: the book and the magery level a class needs (3: the full caster; 2: Gypsy,
  Warlock, Cleric, Missionary; 1: Paladin, Ranger);
- `level`, `mana`, `energy`, `diff` (difficulty), as 1.11p uses them;
- `target`: 0 single, 1 self, 2 self or single, 3 divided area not you, 4 monster, 5 divided
  area including you, 6 any, 7 item, 8 monster or single, 9 divided attack area, 10 divided
  party area, 11 full area, 12 full attack area, 13 full party area;
- `type`: 0 auto-combat, 1 poison, 3 general;
- `resmode` (when magic resistance is rolled): 0 never, 1 only against Anti-Magic, 2 always;
- `element`: 0 cold, 1 hot, 2 stone, 3 lightning, 4 normal, 5 water, 6 poison;
- `min`, `min+/lvls`, `max`, `max+/lvls`: the strength; `a/b` means +a every b levels (`-` none);
- `cap`: the level cap (`-` none);
- `dur`, `dur+/lvls`: the duration in rounds and its growth;
- `abilities`: the record's ability slots as `ability:value`. The ones that occur:
  1 inflict damage ignoring armour; 2 alter defence value; 3 alter cold attack damage (defence);
  4 alter damage (attacking); 5 alter hot attack damage (defence); 6 enslave creature; 7 alter
  damage resistance; 8 steal hit points; 10 protective shield; 13 alter user light; 14 alter room
  light; 17 inflict damage on armour; 18 heal; 19 poison; 20 cure poison; 22 alter AV (bless);
  23 kill dead; 24 defence against evil; 25 defence against good; 27 stealth; 34 dodge; 36 magic
  resistance; 52 evil in combat; 58 alter critical hit chance; 65 resist stone; 66 resist
  lightning; 68 slowness; 70 spellcasting; 71 confusion; 72 damaging shield; 73 dispel magic;
  74 hold person; 76 mute; 77 perception; 80 affects animal; 81 freedom; 84 remove curse; 87 alter
  energy usage; 97 good aligned; 98 evil aligned; 101 confuse message; 105 alter AV (curse);
  106 3rd alter attack value; 107 blind user; 108 affects living; 110 not good aligned; 111 not
  evil aligned; 112 neutral aligned; 115 descriptive message; 116 alter backstab AV; 117 alter
  backstab min damage; 118 alter backstab max damage; 120 starting message; 122 remove spell
  (value: the spell removed); 123 alter healing rate; 137 shockshield message; 145 alter mana
  regeneration; 148 trigger text block; 150 heal mana; 151 cast on ending (value: the spell
  chained); 161 open door; 164 cast-on-ending chance; 178 shadowform;
- `change`: empty when the spell is as 1.11p; `changed:` and the members that differ from 1.11p;
  `new (regen line)`; `rewritten (regen line L35)`; `removes the monster copy too` (only a remove
  slot was added).

`type`, `resmode` and `element` are blank for the four songs: they are as the stock song they were
made from, and don't matter for a self buff.

```csv
num,name,short,magery,mlvl,level,mana,energy,diff,target,type,resmode,element,min,min+/lvls,max,max+/lvls,cap,dur,dur+/lvls,abilities,change
1,magic missile,mmis,Mage,1,1,1,1000,15,8,0,0,4,6,1/1,15,1/1,9,0,-,17:0,changed: min max cap maxlvls mininc minlvls
2,illuminate,illu,Mage,1,2,4,0,5,1,3,0,4,4012,-,4012,-,-,0,-,148:0,""
4,ethereal shield,shld,Mage,1,5,6,0,-5,1,3,0,4,3,1/2,3,1/2,18,60,3/1,10:0 115:8531 122:132 7:10 122:1046 122:1004 122:1041,removes the monster copy too
5,frost jet,fjet,Mage,1,4,2,1000,5,8,0,0,0,8,1/1,17,1/1,13,0,-,17:0,changed: mana max cap mininc minlvls
6,resist cold,rcol,Mage,1,4,6,0,0,2,3,1,0,6,1/1,6,1/2,18,70,-,3:0 115:8526 122:9 122:28 122:29 122:897 122:792,""
7,smite,smit,Mage,1,3,4,0,5,2,3,1,4,1,1/10,1,1/10,30,60,2/1,4:0 115:8532,""
8,lightning bolt,lbol,Mage,1,8,5,1000,0,8,0,0,3,12,1/1,20,2/1,18,0,-,17:0,changed: min max cap mininc minlvls
9,resist fire,rfir,Mage,1,6,8,0,-5,2,3,1,4,4,1/1,4,1/2,22,70,-,5:0 115:8527 122:6 122:28 122:29 122:1013 122:897 122:792,""
12,harm,harm,Priest,0,1,1,1000,15,8,0,2,4,4,1/1,13,1/1,9,0,0/1,17:0 108:0,changed: cap mininc minlvls
13,minor healing,mihe,Priest,0,1,2,0,10,2,3,1,4,2,1/3,8,2/3,10,0,-,18:0 108:0,""
14,bless,bles,Priest,0,2,4,0,5,2,3,1,4,3,-,3,-,20,40,5/1,22:0 115:8539 122:15 122:75 122:23 122:62 122:146 122:668,""
15,curse,curs,Priest,0,2,4,0,5,8,3,2,4,-6,-,-6,-,20,40,5/1,22:0 52:5 115:8540 122:14 122:23,""
16,spiritual hammer,hamm,Priest,0,4,3,1000,5,8,0,0,4,5,1/1,14,1/1,13,0,0/1,17:0,changed: min cap mininc minlvls
17,major healing,mahe,Priest,0,8,6,0,-5,2,3,1,4,6,1/3,10,1/1,30,0,0/1,18:0 108:0,""
18,turn undead,turn,Priest,0,3,4,1000,0,4,0,0,4,12,1/1,15,3/1,21,0,0/1,1:0 23:0,changed: desc cap
19,cure poison,cure,Priest,0,7,8,0,-10,2,3,1,4,8,-,8,-,30,0,0/1,20:0 73:19 122:794 122:798,""
20,holy aura,aura,Priest,3,9,15,0,-10,13,3,1,4,10,-,10,-,42,60,0/1,24:0 97:0 115:8541,changed: cap
21,unholy aura,unho,Priest,3,9,15,0,-10,13,3,1,4,10,-,10,-,37,60,0/1,25:0 98:0 115:8542,changed: cap
22,agony,agon,Priest,0,8,5,1000,0,8,0,0,4,8,1/1,14,2/1,18,0,-,17:0 108:0,changed: min max resmode cap maxinc mininc minlvls durlvls
23,chant,chan,Priest,0,6,10,0,-5,13,3,1,4,3,-,3,-,65,40,5/1,22:0 115:8539 122:75 122:15 122:14,changed: abil cap
24,protective aura,prot,Mage,1,5,10,1000,-15,1,3,0,4,4,1/5,3,1/2,30,20,2/3,36:0 115:158,changed: name short desc msg energy mana min max diff target dur magelvl cap maxinc maxlvls mininc minlvls durinc durlvls abil
25,vine strike,vine,Druid,0,1,1,1000,15,8,0,0,4,5,1/1,14,1/1,9,0,-,17:0,changed: min max cap maxlvls mininc minlvls durlvls
26,starlight,star,Druid,0,1,4,0,0,1,3,0,4,175,-,175,-,65,80,1/1,14:0 115:2092,changed: cap
27,mend,mend,Druid,0,2,4,0,8,2,3,1,4,3,1/3,12,1/2,30,0,0/1,18:0 108:0,""
28,resist fire,rfir,Druid,0,5,8,0,-5,2,3,1,4,0,1/1,0,1/1,20,70,0/1,5:0 115:8527 122:29 122:897 122:32 122:6 122:9,""
29,resist cold,rcol,Druid,0,3,6,0,5,2,3,1,4,0,1/1,0,1/1,20,70,0/1,3:0 115:8526 122:9 122:28 122:6 122:897 122:792,""
30,elemental chaos,chao,Druid,0,4,3,1000,5,8,0,0,4,7,1/1,15,1/1,13,0,0/1,17:0,changed: max cap mininc minlvls
31,antidote,anti,Druid,0,6,8,0,-15,2,3,1,4,8,-,8,-,-,0,0/1,20:0 73:19 122:794 122:798,""
32,resist lightning,rlit,Druid,0,9,10,0,0,2,3,1,4,1,1/1,1,1/1,20,60,0/1,66:0 115:8529 122:1013 122:897 122:792,""
33,swarm,swar,Druid,0,12,28,1000,-10,12,0,0,4,17,1/2,28,3/1,20,0,-,17:0,changed: min max maxinc mininc minlvls durlvls
34,barkskin,skin,Druid,0,7,15,0,-5,1,3,1,4,10,10/10,10,10/10,20,40,1/1,7:0 115:8528 2:5 122:422 122:289 122:1194 122:1381 122:1062 5:-25,""
35,poison bolt,pbol,Druid,0,10,6,999,0,8,0,2,6,10,1/1,17,2/1,18,0,-,17:0 52:0 151:1366 108:0,changed: min max cap minlvls
51,flash,flas,Mage,1,7,10,0,-5,12,3,2,4,9999,-,9999,-,20,5,1/2,107:0 2:-5 22:-8 52:0 115:8534,""
52,magic armour,armr,Mage,3,6,6,0,-5,1,3,1,4,5,1/3,5,1/3,17,60,4/1,7:0 115:8533 2:5,changed: cap durinc
54,vampiric touch,vamp,Mage,1,11,8,1000,-7,8,0,1,4,8,1/3,20,1/3,21,0,-,8:0 108:0,changed: cap maxinc maxlvls mininc minlvls durlvls
55,enslave,ensl,Mage,2,18,12,0,-35,4,3,2,4,0,-,0,-,-,60,0/1,6:0 108:0,""
56,sunbolt,sbol,Mage,1,13,8,1000,-14,8,0,0,1,30,1/2,18,7/2,25,0,0/1,17:0,changed: cap maxinc mininc minlvls
57,speed,sped,Mage,1,12,12,0,-20,2,3,1,4,85,-,85,-,-,20,0/1,87:0 115:8537 122:59 122:271 122:282 122:256 122:396 122:106 122:168 122:183,""
58,deathtouch,dtch,Mage,1,16,25,1000,-5,8,0,0,4,15,1/1,20,2/1,40,0,-,8:0 108:0 120:760,changed: min max cap mininc minlvls durlvls
59,slow,slow,Mage,1,10,12,0,-20,8,3,2,4,125,-,125,-,-,10,0/1,52:0 115:8536 68:200 122:57 122:272 122:283 122:106 122:183 122:168,changed: abil
61,greater curse,gcrs,Priest,0,9,8,0,-10,8,3,2,4,-10,-,-10,-,24,40,0/1,22:0 115:549 122:146 52:0,""
62,divine favour,dfav,Priest,2,19,12,0,-35,2,3,1,4,10,-,10,-,-,30,0/1,22:0 122:67 115:550 122:14 122:15 122:23 122:61,""
63,confusion,conf,Mage,1,14,12,0,-30,8,3,2,4,25,-,25,-,-,10,-,71:0 101:8489 52:0 115:8538,""
64,shockshield,shok,Mage,1,9,12,0,-15,1,3,0,3,1,1/5,5,-,20,40,0/1,72:0 115:8535 137:1446 66:10,""
66,hold person,hold,Priest,0,14,12,0,-30,8,3,2,4,100,-,100,-,-,4,0/1,74:0 52:0 115:8544,""
67,divine disfavour,disf,Priest,2,19,12,0,-40,8,3,2,4,-15,-,-15,-,-,30,0/1,22:0 122:62 115:551 52:0,""
68,mute,mute,Mage,3,10,8,0,-15,8,3,2,4,-40,-,-20,-,36,20,0/1,70:0 76:0 115:78 52:0,changed: cap
70,freedom,free,Druid,0,16,10,0,-40,2,3,1,4,100,-,100,-,-,0,0/1,81:0,""
71,burning aura,burn,Druid,0,13,10,0,-15,8,3,2,1,1,1/5,1,1/5,15,20,0/1,1:0 52:0 2:-5 120:621 115:8596 7:-30 22:-5,""
73,black wind,wind,Priest,2,21,30,0,-40,12,3,2,4,1,-,1,-,-,10,0/1,107:0 52:0 108:0 115:8534 120:552 22:-10 2:-5,""
74,remove curse,rcrs,Priest,3,14,20,0,-40,2,3,0,4,-1,1/7,-1,1/7,65,0,0/1,84:0,changed: cap
75,blight,blit,Priest,0,6,10,0,-5,12,3,2,4,-6,-,-6,-,65,40,5/1,22:0 52:0 115:8540 122:23 122:14,changed: cap
77,blind,blin,Priest,0,12,12,0,-25,8,3,2,4,1,-,1,-,20,10,-,107:0 2:-5 22:-8 52:0 115:8534 108:0,""
87,cure blindness,site,Priest,0,12,12,0,-25,2,3,1,4,9999,-,9999,-,-,0,-,73:107,""
88,control undead,cded,Priest,2,16,12,0,-25,4,3,2,4,100,-,100,-,-,80,-,6:0 23:0 98:0,""
89,greater healing,grhe,Priest,2,15,12,0,-25,2,3,1,4,5,1/1,10,2/1,30,0,-,18:0 108:0,""
91,alertness,aler,Druid,0,2,4,0,5,2,3,1,4,10,-,10,-,-,60,-,77:0 115:8525 122:137,""
92,charm animal,char,Druid,0,5,10,0,-5,4,3,2,4,100,-,100,-,-,60,-,6:0 80:0,""
93,entangle,enta,Druid,0,14,12,0,-30,8,3,2,4,100,-,100,-,-,4,-,74:0 52:0 115:8530,""
94,stonestrike,strk,Druid,0,8,5,1000,0,8,0,0,2,10,1/1,17,2/1,18,0,-,17:0,changed: min max cap mininc minlvls
95,acid jet,acid,Druid,0,13,8,1000,-14,8,0,0,5,26,1/2,18,6/2,25,0,-,17:0,changed: min cap maxinc mininc minlvls
107,protection from evil,prev,Priest,0,5,6,0,0,2,3,1,4,10,-,10,-,-,50,-,24:0 111:0 115:8541,changed: abil
108,protection from good,prgd,Priest,0,5,6,0,0,2,3,1,4,10,-,10,-,-,50,-,25:0 110:0 115:8542,changed: abil
110,holy force,hfor,Priest,0,13,8,1000,-10,8,0,0,4,18,1/2,12,5/2,25,0,-,17:0 111:0,changed: max cap maxinc maxlvls mininc minlvls
111,unholy force,ufor,Priest,0,13,8,1000,-10,8,0,0,4,26,1/2,24,5/2,25,0,-,17:0 110:0,changed: min cap maxinc maxlvls mininc minlvls
120,fireball,fbal,Mage,1,15,34,1000,-20,12,0,0,1,26,1/1,40,4/1,28,0,-,17:0 120:100,changed: min max cap maxinc mininc minlvls
121,godheal,gdhe,Priest,3,22,24,0,-50,2,3,1,4,35,1/2,30,2/1,65,0,-,18:0 108:0,changed: cap
122,god's wrath,gwra,Priest,2,25,14,1000,-50,8,0,0,4,15,2/1,20,5/1,35,0,-,17:0,changed: min max maxinc mininc minlvls
123,greater healing rain,grai,Priest,3,24,30,0,-55,13,3,1,4,15,1/2,35,1/1,46,0,-,18:0,changed: cap
125,paralyze,para,Priest,3,25,30,0,-55,12,3,2,4,100,-,100,-,-,4,-,74:0 115:8544 52:0,""
129,blur,blur,Mage,1,1,4,0,10,2,3,1,4,5,1/5,5,2/5,25,70,-,34:0 115:68,changed: cap maxinc maxlvls mininc minlvls abil
130,shadowform,shad,Mage,1,7,8,0,-10,1,3,0,4,5,-,5,1/3,30,30,1/1,27:0 115:70 178:4157,changed: durinc durlvls
131,stinking cloud,stnk,Mage,1,8,10,0,-5,12,3,2,4,5,-,5,1/2,20,20,-,71:0 105:-5 101:72 115:71 52:0,""
132,mageshield,mshi,Mage,3,9,10,0,-15,1,3,0,4,7,1/2,7,1/2,25,60,2/1,2:0 115:73 122:4 7:10 122:1046 122:1004 122:1041,changed: cap durinc durlvls
133,dancing blades,blad,Mage,1,11,26,1000,-10,12,0,0,4,21,1/2,38,3/1,20,0,-,17:0 120:102,changed: maxinc mininc minlvls
134,vulnerability,vuln,Mage,1,14,6,0,-30,8,3,2,4,-8,-,-8,-,25,1,4/1,2:0 36:-10 52:0 115:81,""
135,ice storm,isto,Mage,1,17,20,0,-25,12,3,1,0,3,-,3,-,22,15,-,1:0 106:-6 52:10 115:84 120:104 5:-10,""
136,eldritch bolt,ebol,Mage,1,20,12,1000,-30,8,0,0,4,30,1/1,10,5/1,30,0,-,17:0,changed: min max cap maxinc mininc minlvls
137,senselessness,sens,Druid,0,4,6,0,0,8,3,2,4,-10,-,-10,-,-,40,-,77:0 108:0 52:0 115:85 122:91,""
138,rapid healing,rhel,Druid,0,11,8,0,-20,2,3,1,4,200,-,200,-,-,60,-,123:100 108:0 115:86 122:674 122:831 122:1056,changed: abil
139,acid rain,rain,Druid,0,15,12,0,-25,12,3,2,5,1,1/5,1,1/5,15,20,-,1:0 2:-5 52:0 115:87 120:106,""
140,chaos storm,csto,Druid,3,15,34,1000,-20,12,0,0,4,15,1/1,20,4/1,29,0,-,17:0 120:108,changed: resist min max magelvl cap maxinc mininc minlvls
141,partial petrification,petr,Druid,3,16,12,0,0,8,3,2,4,150,-,150,-,-,20,-,87:0 2:5 7:10 36:5 52:0 115:88 70:-5 34:-15,changed: resist magelvl
142,poison cloud,pclo,Druid,0,17,14,0,-35,12,3,2,4,3,1/5,3,1/5,20,40,-,19:0 115:8575 52:0 108:0 22:-6 4:-3 108:0,""
143,rotting flesh,rott,Druid,0,18,16,0,-35,8,3,2,4,6,-,6,-,-,10,-,1:0 108:0 115:90 52:0 120:279,""
144,iron faith,irfa,Priest,3,7,6,0,-10,1,3,0,4,2,1/2,2,1/2,35,30,1/2,36:0 115:91,changed: cap durinc durlvls
145,healing rain,rain,Priest,0,10,5,0,-15,13,3,1,4,4,1/3,10,1/2,25,0,-,18:0 108:0,""
146,greater bless,gbls,Priest,0,10,8,0,-15,2,3,1,4,6,-,6,-,30,30,2/1,22:0 115:93 122:61 122:14 122:15 122:23 122:62 122:668,changed: dur cap durinc durlvls
147,glitterdust,glit,Priest,0,11,6,0,-15,0,3,0,4,-10,-,-10,-,-,30,-,27:0 52:0 115:94,""
148,holy armour,armr,Priest,3,11,10,0,-20,1,3,0,4,2,1/2,2,1/2,40,60,-,2:0 111:0 115:95 122:149 122:1383 122:1384 7:0,changed: cap abil
149,unholy armour,uarm,Priest,3,11,10,0,-20,1,3,0,4,2,1/2,2,1/2,40,60,-,2:0 98:0 115:96 122:148 122:1383 122:1384,changed: cap
150,cure disease,cdes,Priest,2,13,12,0,-25,2,3,1,4,143,-,143,-,-,0,-,122:0 122:992 122:294 122:323 122:362 122:445 122:446 122:952 122:1024,""
151,disrupt,disr,Priest,0,16,8,1000,-25,4,0,0,4,30,1/1,52,4/2,52,0,-,17:0 23:0,changed: cap
152,major healing rain,mrai,Priest,2,17,15,0,-40,13,3,1,4,8,1/3,20,1/1,30,0,-,18:0,""
153,godstrike,gstk,Priest,0,20,12,1000,-40,8,0,0,4,15,1/1,10,4/1,30,0,-,17:0,changed: min max cap maxinc mininc minlvls
160,cure paralysis,curp,Priest,2,23,15,0,-50,2,3,0,4,100,-,100,-,-,0,-,81:0,""
207,ember,embr,Druid,3,45,10,333,-90,8,0,0,1,0,2/1,-25,5/1,65,0,-,17:0 120:3088 151:210,changed: max cap maxinc mininc
210,ember,embr,Druid,3,45,10,333,-90,8,0,0,1,0,2/1,-25,5/1,65,0,-,17:0,changed: max cap maxinc mininc
211,soul rip,srip,Priest,3,45,30,1000,-100,8,0,0,4,25,5/1,-50,14/1,65,0,-,17:0 120:3086,changed: min max cap maxinc mininc
216,sphere of implosion,simp,Mage,3,40,25,1000,-70,8,0,0,4,40,5/1,30,12/1,50,0,-,17:0 120:279,changed: min maxinc mininc
239,summon fire elemental,fire,Druid,3,40,25,1000,-85,8,0,0,1,42,4/1,-24,12/1,50,0,-,17:0 120:552,changed: min max cap maxinc mininc
240,summon ice elemental,icel,Druid,3,40,25,1000,-85,8,0,0,0,42,4/1,-24,12/1,50,0,-,17:0 120:552,changed: min max cap maxinc mininc
241,summon lightning elemental,lite,Druid,3,37,25,1000,-80,8,0,0,3,42,4/1,-24,12/1,50,0,-,17:0 120:552,changed: min max cap maxinc mininc
242,summon stone elemental,stne,Druid,3,37,25,1000,-80,8,0,0,2,42,4/1,-24,12/1,50,0,-,17:0 120:552,changed: min max cap maxinc mininc
254,colour spray,colu,Mage,2,32,20,1000,-45,8,0,0,4,106,2/1,50,10/1,42,0,-,17:0,changed: min max cap maxinc
267,divine fury,fury,Priest,2,30,16,1000,-60,8,0,0,4,60,2/1,20,9/1,40,0,-,17:0,changed: min max mininc minlvls
268,divine mercy,dmer,Priest,3,30,50,0,-65,13,3,1,4,25,1/2,65,1/1,52,0,-,18:0,changed: cap maxinc maxlvls minlvls
281,mana storm,msto,Mage,2,23,50,1000,-45,12,0,0,4,14,2/1,60,5/1,33,0,-,17:0 120:564,changed: min max cap mininc minlvls
282,solid fog,sfog,Mage,3,19,30,0,-35,12,3,2,4,125,-,125,-,-,20,-,52:0 115:8536 120:566 122:57 122:272 122:283 122:106 68:200,changed: abil
283,wind of haste,wind,Mage,3,22,30,0,-50,13,3,1,4,80,-,80,-,-,20,-,87:0 115:8537 122:59 122:282 122:271 120:568,""
284,corrosion,corr,Mage,2,24,15,0,-55,12,3,2,4,-8,-,-8,-,40,1,4/1,2:0 36:-10 52:0 115:81 120:570 7:-50,""
285,meteor swarm,mete,Mage,2,25,4,250,-50,8,0,0,4,10,1/2,10,4/3,35,0,-,17:0,changed: min maxinc maxlvls mininc minlvls
286,mystic mist,mist,Mage,3,26,30,0,-55,12,3,2,4,25,-,25,-,40,10,1/4,71:0 101:8489 52:0 120:198 115:8538,""
287,spirit horde,spir,Mage,3,28,60,1000,-60,12,0,0,4,55,2/1,55,9/1,45,0,-,17:0 120:572,changed: min cap maxinc mininc minlvls
288,dragonfire,dfir,Mage,2,30,16,1000,-60,8,0,0,1,100,2/1,50,10/1,40,0,-,17:0 120:578,changed: max mininc minlvls
289,stoneskin,ston,Druid,2,19,20,0,-35,1,3,0,2,60,10/10,60,10/10,40,30,1/1,7:0 2:10 36:5 115:579 122:422 122:34 122:1381 65:10 122:1062,""
290,blaze,blaz,Druid,0,20,12,1000,-40,8,0,0,1,22,1/1,10,9/2,28,0,-,17:0,changed: min max maxinc maxlvls mininc minlvls
291,creeping doom,doom,Druid,2,22,16,0,-50,8,3,2,4,4,-,4,1/10,35,30,3/5,1:0 22:-5 2:-5 115:580 52:0,""
292,maelstrom,mael,Druid,2,24,50,1000,-45,12,0,0,4,0,2/1,60,4/1,33,0,-,17:0 120:552,changed: min max cap maxinc mininc minlvls
293,plasma bolt,plas,Druid,2,23,14,1000,-50,8,0,0,4,24,2/1,30,5/1,35,0,-,17:0,changed: min max maxinc mininc minlvls
294,plague,plag,Druid,3,26,40,0,-55,12,3,2,4,2,1/10,2,1/10,40,40,-,1:0 120:583 115:584 52:0 34:-5 4:-2 2:-5,""
295,earthquake,quak,Druid,2,28,50,999,-50,12,0,0,2,40,2/1,55,7/1,45,0,-,17:0 120:586 151:927,changed: mana min max diff magelvl cap maxinc mininc minlvls
296,earthfist,fist,Druid,3,30,16,1000,-60,8,0,0,2,80,2/1,35,19/2,40,0,-,17:0,changed: min max magelvl maxinc maxlvls mininc minlvls
304,zeal,zeal,Priest,0,25,20,0,-30,1,3,0,4,0,1/10,0,1/10,50,10,1/2,4:0 36:0 58:0 22:0 115:816 122:347 122:816 122:1315,""
345,frenzy,fren,Mage,1,15,18,0,-25,2,3,1,4,0,1/6,0,1/6,30,1,2/1,4:0 58:0 34:-5 2:-5 122:801 122:346 22:0 120:810 115:811 7:50,""
346,mass frenzy,mfre,Mage,3,20,45,0,-40,13,3,1,4,0,1/7,0,1/7,46,10,1/2,4:0 58:0 34:-5 7:50 2:-5 122:345 122:801 115:811 120:814 22:0,changed: cap maxlvls minlvls
347,righteousness,rite,Priest,3,15,24,0,-25,2,3,1,4,1,-,1,1/20,30,40,1/2,22:6 18:0 120:815 115:816 111:0 122:304 122:816 122:1315 122:348,changed: min max cap maxinc maxlvls durinc durlvls abil
348,blood ritual,blud,Priest,3,15,24,0,-25,2,3,1,4,1,-,1,-,30,40,1/2,22:6 4:1 18:0 98:0 120:819 115:820 122:304 122:816 122:1315 122:347,changed: min max cap durinc durlvls abil
349,regeneration,rgen,Druid,3,15,12,0,-25,2,3,1,4,2,1/20,4,1/10,50,7,1/4,18:0 120:822 115:823 122:1012,changed: dur magelvl cap durinc durlvls
374,mass entangle,msen,Druid,3,25,30,0,-55,12,3,2,4,100,-,100,-,-,4,-,74:0 115:8530 52:0 120:570,""
393,stone to flesh,ston,Priest,3,18,30,0,-25,0,3,1,4,100,-,100,-,-,0,-,122:392 122:141 122:289 120:1052 122:769,removes the monster copy too
404,sacrifice,sacr,Priest,3,18,0,0,-25,0,3,1,4,-80,-,-40,-,-,0,-,8:0 120:1106 108:0,changed: mana min max
405,chaos shield,chao,Mage,3,21,16,0,-35,2,3,1,4,-100,2/1,50,-,50,40,1/1,72:10 120:1108 115:1109 2:5 7:0,changed: min dur cap mininc minlvls durinc durlvls
406,mana flux,flux,Mage,3,35,12,0,-40,1,3,1,4,150,-,150,-,75,180,-,145:150 115:1112 122:748 122:6000 122:6001 122:6002 122:6009 122:6010 122:6011 122:6012,rewritten (regen line L35)
706,forked lightning,fork,Mage,3,35,10,500,-75,8,0,0,3,30,2/1,0,6/1,45,0,-,17:0 120:1999,changed: min cap maxinc mininc minlvls
716,elemental fury,fury,Druid,2,35,5,250,-70,8,0,0,2,10,1/1,-5,3/1,45,0,-,17:0 120:2037 151:718,changed: min max cap maxinc mininc minlvls
718,elemental lightning,"",Druid,2,35,5,250,-70,8,0,0,3,10,1/1,-5,3/1,45,0,-,17:0 151:719,changed: min max cap maxinc mininc minlvls
719,elemental fire,"",Druid,2,35,5,250,-70,8,0,0,1,10,1/1,-5,3/1,45,0,-,17:0 151:720,changed: min max cap maxinc mininc minlvls
720,elemental ice,"",Druid,2,35,5,250,-70,8,0,0,0,10,1/1,-5,3/1,45,0,-,17:0,changed: min max cap maxinc mininc minlvls
721,soulstrike,soul,Priest,3,35,20,1000,-70,8,0,0,4,20,4/1,-45,12/1,50,0,-,17:0 120:2042,changed: min max maxinc mininc minlvls
744,annointed hands,anno,Priest,2,10,1,0,-5,0,3,1,4,2,-,0,1/1,15,0,-,18:0 120:234,""
748,chaos surge,chsu,Mage,3,47,0,0,-110,1,3,0,4,-1,1/13,0,1/8,65,80,-,150:0 123:-150 7:-50 36:20 122:406 120:570 115:3430 72:5 137:1446,changed: cap maxlvls minlvls
749,eldritch fury,elfu,Mage,3,47,7,200,-115,8,0,0,4,25,1/1,5,3/1,65,0,-,17:0 120:621,changed: mana min max cap maxinc mininc minlvls
750,wizard knock,knok,Mage,3,17,25,500,0,11,3,0,0,100,-,100,-,-,0,-,161:0 120:104,""
771,blizzard,bliz,Mage,2,40,45,1000,-75,12,0,0,0,80,1/1,50,8/1,55,0,-,17:0 120:631 151:1015,changed: cap maxinc mininc minlvls
774,thunderstun,"",Druid,3,45,0,1,200,12,3,0,4,60,-,50,1/1,60,3,1/15,71:0 52:0 101:56 115:57,""
775,thunderclap,thdr,Druid,3,45,75,999,-125,12,0,0,3,-30,2/1,190,2/1,65,0,-,17:0 120:631 164:25 151:774,changed: min cap mininc minlvls
802,magma blast,magm,Mage,3,45,5,166,-110,8,0,0,1,5,1/1,-7,5/2,55,0,-,17:0,changed: min max maxinc maxlvls minlvls
804,merciful grace,mgra,Priest,3,40,35,0,-90,2,3,0,4,-25,2/1,25,4/1,65,0,-,18:0 20:100 120:2351 73:107,changed: cap maxinc
805,acid tempest,atem,Druid,3,40,65,1000,-80,12,0,2,5,-40,4/1,-10,12/1,65,0,-,17:0 120:2305 151:934,changed: min max cap maxinc mininc minlvls
816,divine courage,dcor,Priest,3,45,45,0,-90,13,3,0,4,3,-,3,-,65,40,2/5,18:0 22:15 4:3 115:2395 122:304 122:347 122:1315 122:348 111:0,changed: cap abil
853,wrathful curse,wcur,Priest,2,40,25,0,-100,8,3,2,4,100,-,100,-,-,7,-,74:0 76:0 13:-999 22:-15 18:-2 115:8540 52:0,""
854,nature tap,ntap,Druid,3,35,12,0,-40,1,3,1,4,150,-,150,-,50,180,-,145:150 120:2511 115:2512 122:6006 122:6007 122:6008,rewritten (regen line L35)
897,globe of the elements,gele,Druid,3,29,25,0,-75,1,3,1,4,25,-,25,-,-,100,-,3:0 115:2604 122:1053 5:0 66:0 65:0 122:28 122:29 122:32,""
927,earthquake hold,"",Druid,3,28,0,1,200,12,3,0,4,100,-,100,-,-,4,-,74:0 115:648 52:0,""
934,drenched in acid,"",Druid,3,40,0,0,200,12,3,0,5,4,-,7,-,-,20,-,1:0 115:87 2:-10 22:-15 34:-5 52:0,""
1003,frozen spike,fspk,Druid,3,50,45,1000,-110,8,0,0,0,-25,6/1,-20,15/1,65,0,-,17:0 120:621 97:0,changed: min max cap maxinc mininc
1004,manashield,mans,Mage,3,50,25,0,-100,1,3,0,4,0,-,0,-,65,5,1/1,36:20 7:50 2:20 115:2804 97:0 34:15 122:132 122:4,changed: cap abil
1005,exalted word,word,Priest,3,50,8,200,-110,8,0,0,4,5,1/1,-20,3/1,65,0,-,17:0 120:1046 151:1007 97:0,changed: min max cap maxinc minlvls
1007,exalted word,"",Priest,3,50,8,200,-110,8,0,0,4,5,1/1,-20,3/1,65,0,-,17:0 97:0,changed: min max cap maxinc minlvls
1008,necromantic bolt,nebo,Mage,3,50,50,999,-100,8,0,0,4,50,6/1,100,13/1,65,0,-,17:0 120:1046 98:0 108:0 151:1246,changed: min max cap maxinc mininc
1012,rejuvinating field,reju,Druid,3,32,40,0,-65,13,3,1,4,1,1/10,0,2/10,40,10,-,18:0 115:823 120:570 122:349,removes the monster copy too
1013,resist lightning,rlit,Mage,3,10,10,0,-15,2,3,0,4,0,1/1,0,1/1,15,70,-,66:0 115:8529 122:897 122:32 122:732,""
1015,freezing,"",Mage,0,40,0,0,200,12,3,1,0,4,-,10,-,20,10,-,1:0 22:-15 115:84 52:0 2:-10,""
1016,natural prowess,prow,Druid,0,25,20,0,-30,1,3,0,4,0,1/10,0,1/10,50,10,1/2,22:0 4:0 117:0 118:0 58:0 115:2513,""
1018,chain lightning,chai,Mage,3,50,75,1000,-120,12,0,0,3,70,5/1,75,12/1,65,0,-,17:0 97:0 120:2802,changed: cap maxinc mininc
1025,damnation,damn,Priest,3,42,30,0,-100,8,3,2,4,-20,-,-10,-,48,10,-,22:0 52:10 36:-11 4:-8 2:-10 58:-9 115:2830 120:2831 98:0,changed: cap
1026,spore cloud,spor,Druid,3,50,60,0,-130,12,3,0,4,0,1/5,10,1/5,50,1,1/5,1:0 71:10 120:2801 115:2803 101:72 58:-5 68:100 4:-5 52:0 98:0,""
1038,prismatic beam,prbe,Mage,3,50,50,900,-110,8,0,0,4,-20,7/1,90,15/1,65,0,-,17:0 120:570 151:1052 112:0,changed: min max cap maxinc mininc
1041,bladed sphere,blsh,Mage,3,50,25,0,-100,1,3,0,4,0,-,0,-,65,5,1/1,36:10 137:2879 7:50 115:2881 72:15 98:0 2:20 122:132 122:4,changed: cap abil
1042,glacial mist,gmst,Druid,3,50,60,0,-130,12,3,0,4,0,1/10,10,1/5,50,1,1/5,1:0 71:10 68:100 115:84 101:2871 58:-5 120:106 5:10 52:0 97:0,""
1043,close wounds,clwo,Druid,3,50,25,0,-75,2,3,1,4,-30,1/1,-50,2/1,65,0,-,18:0 97:0,changed: min max cap maxinc minlvls
1046,protective shell,prsl,Mage,3,50,25,0,-100,1,3,0,4,0,-,0,-,65,5,1/1,7:50 2:20 115:2876 112:0 122:132 36:30 122:4,changed: cap
1047,boulder,boul,Druid,3,50,45,1000,-110,8,0,0,2,25,6/1,40,15/1,65,0,-,17:0 120:621 112:0,changed: min max cap maxinc mininc
1048,inferno,infe,Druid,3,50,45,1000,-110,8,0,0,1,50,6/1,60,15/1,65,0,-,17:0 120:621 98:0,changed: min max cap maxinc mininc
1052,pris effect,"",-,0,0,0,50,200,8,0,0,4,1064,-,1067,-,-,0,-,151:0,changed: max
1056,dark binding,dbin,Druid,3,50,30,0,-90,1,3,0,4,50,-,0,2/1,65,30,1/1,123:-50 145:0 120:822 115:3132 98:0 122:138,changed: name short msg msgstyle mana min diff target dur cap maxinc maxlvls mininc minlvls durlvls abil
1057,hellstorm,hsto,Mage,3,50,75,1000,-120,12,0,0,1,30,6/1,75,13/1,65,0,-,17:0 98:0 120:572,changed: cap maxinc mininc
1058,eldritch storm,esto,Mage,3,40,70,1000,-90,12,0,0,4,-30,5/1,0,13/1,55,0,-,17:0 120:566,changed: maxinc mininc
1059,divine healing,dihe,Priest,3,50,50,0,-140,2,3,0,4,0,3/1,0,5/1,65,0,-,18:0 120:2926 97:0,changed: min max target cap maxinc mininc
1060,priestly benediction,prbe,Priest,3,50,50,0,-140,0,3,0,4,5,1/7,7,1/4,65,10,1/5,18:0 115:2927 112:0,changed: cap maxlvls minlvls durinc durlvls
1061,dark transfusion,datr,Priest,3,50,50,0,-110,0,3,0,4,-200,-,-80,-,-,0,-,8:0 115:2927 98:0 18:200,changed: abil
1062,barbskin,barb,Druid,3,50,35,0,-90,1,3,0,4,2,1/6,2,1/5,65,30,1/1,72:0 7:50 2:0 115:2878 122:422 122:34 122:289 137:2877 112:0 122:1381,changed: min abil
1063,comet,come,Mage,3,50,16,333,-120,8,0,0,4,0,2/1,20,5/1,65,0,-,17:0 97:0 120:595,changed: max cap mininc
1064,yellow beam,"",Mage,0,0,0,50,100,8,0,1,4,200,-,200,-,-,10,0/1,68:0 22:-10 115:8536,""
1065,violet beam,"",Mage,0,0,0,50,100,8,0,1,4,15,-,15,-,-,10,-,71:0 101:8489 52:0 115:8538,""
1066,black beam,"",Mage,0,0,0,50,100,8,0,1,4,-500,-,-100,-,20,10,-,13:0 2:-10 22:-15 52:0 115:8534 108:0,""
1067,red beam,"",Mage,0,0,0,50,100,8,0,1,4,-5,-,-1,-,-,10,0/1,4:0 22:0 115:1172,""
1069,yellow wave,"",Mage,0,50,0,200,100,12,0,1,4,200,-,200,-,-,5,0/1,68:0 22:-10 115:8536,""
1070,violet wave,"",Mage,0,50,0,200,100,12,0,1,4,15,-,15,-,-,5,-,71:0 101:8489 52:0 115:8538,""
1071,black wave,"",Mage,0,50,0,200,100,12,0,1,4,-500,-,-100,-,20,5,-,13:0 2:-10 22:-15 52:0 115:8534 108:0,""
1072,red wave,"",Mage,0,50,0,200,100,12,0,1,4,-5,-,-1,-,-,5,0/1,4:0 22:0 115:1172,""
1074,prismatic blast,pbla,Mage,3,50,75,800,-120,12,0,0,4,75,5/1,0,13/1,65,0,-,17:0 120:570 151:1075 112:0,changed: min max cap maxinc mininc
1075,pris effect,"",-,0,0,0,0,200,12,0,0,4,1069,-,1072,-,-,0,-,151:0,changed: max
1080,tainted word,"",Priest,3,50,8,200,-100,8,0,0,4,15,1/1,-10,3/1,65,0,-,17:0 98:0,changed: min max cap maxinc minlvls
1081,tainted word,word,Priest,3,50,8,200,-100,8,0,0,4,15,1/1,-10,3/1,65,0,-,17:0 151:1080 120:1046 98:0,changed: min max cap maxinc minlvls
1096,balanced word,word,Priest,3,50,8,200,-100,8,0,0,4,10,1/1,-15,3/1,65,0,-,17:0 151:1097 120:1046 112:0,changed: min max cap maxinc minlvls
1097,balanced word,word,Priest,3,50,8,200,-100,8,0,0,4,10,1/1,-15,3/1,65,0,-,17:0 112:0,changed: min max cap maxinc minlvls
1098,angelic halo,halo,Priest,3,50,45,0,-130,2,3,0,4,0,8/1,0,8/1,65,80,1/1,14:0 115:2923 97:0 24:10,changed: desc cap maxinc mininc abil
1099,balanced sight,bals,Priest,3,50,45,0,-130,13,3,0,4,200,-,200,-,65,80,1/1,14:0 77:50 115:2924 112:0 24:5 25:5,changed: cap abil
1102,infernal darkness,dark,Priest,3,50,45,0,-130,12,3,0,0,0,-,0,-,65,80,1/1,13:-9999 98:0 52:0 115:1322 22:-30 24:-10,changed: cap abil
1110,sleep,slep,Mage,3,15,30,0,-50,8,3,2,4,100,-,100,-,30,1,1/30,71:0 115:958 101:960 52:0,""
1112,earthbind,ebin,Druid,3,50,30,0,-90,1,3,0,4,0,1/2,-60,2/1,65,30,1/1,123:0 145:0 120:822 115:3132 112:0,changed: max dur cap maxinc durinc durlvls
1246,nebo secondary,"",Mage,3,50,0,1,200,8,0,0,4,0,1/3,-20,3/1,65,0,-,8:0 108:0,changed: level
1301,barbed tentacles,tent,Mage,3,37,23,1000,-80,8,0,0,4,72,4/1,6,12/1,47,0,-,17:0 120:552,changed: min max cap maxinc mininc
1310,sandstorm,sand,Druid,2,38,70,0,-90,12,3,0,4,4,1/60,1,1/10,65,7,1/3,1:0 13:-9999 2:-5 22:-10 52:0 68:200 115:3406 120:595,changed: abil cap
1311,glacial blades,glac,Mage,3,33,20,1000,-70,8,0,0,0,114,2/1,50,10/1,43,0,-,17:0 120:578,changed: min max cap mininc minlvls
1314,camouflage,camo,Druid,1,7,10,0,-5,1,3,0,4,3,1/1,3,1/1,25,30,1/2,27:0 115:3422 116:10,""
1315,unholy fanaticism,unfa,Priest,3,45,45,0,-90,13,3,0,4,0,-,0,-,65,40,2/5,4:5 58:4 18:1 122:348 122:304 122:347 98:0 115:2171 122:816,changed: cap abil
1316,blessed vision,bevi,Priest,3,45,35,0,-95,2,3,1,4,0,-,0,-,65,0,-,122:440 122:443 120:2808 73:107 122:933 122:1102 122:1113 122:1310,changed: desc level resist msg resmode target cap abil
1317,curing wind,curi,Priest,3,47,65,0,-110,13,3,0,4,-40,2/1,30,2/1,65,0,-,18:0 120:2351 20:100,changed: min max cap maxinc maxlvls mininc minlvls abil
1366,poison bite,"",Druid,0,10,0,1,0,8,0,2,6,4,1/5,4,1/5,15,60,0/1,19:0 115:8575 108:0,""
1367,exorcism,exor,Priest,3,47,35,1000,-110,4,0,0,4,100,5/1,200,10/1,65,0,-,17:0 23:0 120:3443,changed: max cap abil
1377,serenity,nity,Priest,3,35,12,0,-40,1,3,0,0,150,-,150,-,50,180,-,145:150 115:3403 111:0 122:6003 122:6004 122:6005 122:6013 122:6014 122:6015 122:6016,rewritten (regen line L35)
1378,profane link,prfl,Priest,3,35,12,0,-40,1,3,0,0,150,-,150,-,50,180,-,145:150 115:3404 98:0 122:6003 122:6004 122:6005 122:6013 122:6014 122:6015 122:6016,rewritten (regen line L35)
1381,glacial skin,glac,Druid,3,48,35,0,-125,1,3,0,0,30,2/1,30,2/1,60,20,1/1,7:0 2:5 3:20 5:-50 115:3442 122:34 122:422 122:289 122:1062,""
1382,shimmering mirage,mira,Mage,3,38,50,0,-85,13,3,0,4,5,1/4,5,1/4,60,40,1/1,34:0 115:3415,changed: cap maxlvls minlvls durinc durlvls
1383,sacred bastion,bast,Priest,3,37,25,0,-80,1,3,0,4,10,1/2,10,1/2,65,25,1/1,2:0 111:0 120:2808 122:148 122:149 122:1384 115:158,changed: dur cap durinc durlvls abil
1384,hellfire shield,hesh,Priest,3,37,25,0,-80,1,3,0,4,0,1/2,0,1/2,65,25,1/1,2:0 115:3421 98:0 122:148 122:149 122:1383 72:10 137:1145 120:2831,changed: dur cap durinc durlvls
6000,lesser attunement,latt,Mage,3,5,4,0,0,1,3,1,4,50,-,50,-,-,120,-,145:50 115:5101 122:6001 122:406 122:6002 122:6009 122:6010 122:6011 122:6012,new (regen line)
6001,arcane attunement,aatt,Mage,3,20,8,0,-20,1,3,1,4,100,-,100,-,-,150,-,145:100 115:5101 122:6000 122:406 122:6002 122:6009 122:6010 122:6011 122:6012,new (regen line)
6002,greater attunement,gatt,Mage,3,50,16,0,-60,1,3,1,4,200,-,200,-,-,200,-,145:200 115:5101 122:6000 122:6001 122:406 122:6009 122:6010 122:6011 122:6012,new (regen line)
6003,lesser devotion,ldev,Priest,3,5,4,0,0,1,3,0,0,50,-,50,-,-,120,-,145:50 115:5103 122:6004 122:1377 122:1378 122:6005 122:6013 122:6014 122:6015 122:6016,new (regen line)
6004,devotion,devo,Priest,3,20,8,0,-20,1,3,0,0,100,-,100,-,-,150,-,145:100 115:5103 122:6003 122:1377 122:1378 122:6005 122:6013 122:6014 122:6015 122:6016,new (regen line)
6005,greater devotion,gdev,Priest,3,50,16,0,-60,1,3,0,0,200,-,200,-,-,200,-,145:200 115:5103 122:6003 122:6004 122:1377 122:1378 122:6013 122:6014 122:6015 122:6016,new (regen line)
6006,lesser communion,lcom,Druid,3,5,4,0,0,1,3,1,4,50,-,50,-,-,120,-,145:50 115:5105 122:6007 122:854 122:6008,new (regen line)
6007,communion,comm,Druid,3,20,8,0,-20,1,3,1,4,100,-,100,-,-,150,-,145:100 115:5105 122:6006 122:854 122:6008,new (regen line)
6008,greater communion,gcom,Druid,3,50,16,0,-60,1,3,1,4,200,-,200,-,-,200,-,145:200 115:5105 122:6006 122:6007 122:854,new (regen line)
6009,minor attunement,mnat,Mage,2,5,4,0,0,1,3,1,4,25,-,25,-,-,120,-,145:25 115:5101 122:6000 122:6001 122:406 122:6002 122:6010 122:6011 122:6012,new (regen line)
6010,modest attunement,mdat,Mage,2,20,8,0,-20,1,3,1,4,50,-,50,-,-,150,-,145:50 115:5101 122:6000 122:6001 122:406 122:6002 122:6009 122:6011 122:6012,new (regen line)
6011,steady attunement,stat,Mage,2,35,12,0,-40,1,3,1,4,75,-,75,-,-,180,-,145:75 115:5101 122:6000 122:6001 122:406 122:6002 122:6009 122:6010 122:6012,new (regen line)
6012,deep attunement,dpat,Mage,2,50,16,0,-60,1,3,1,4,100,-,100,-,-,200,-,145:100 115:5101 122:6000 122:6001 122:406 122:6002 122:6009 122:6010 122:6011,new (regen line)
6013,minor devotion,mndv,Priest,2,5,4,0,0,1,3,0,0,25,-,25,-,-,120,-,145:25 115:5103 122:6003 122:6004 122:1377 122:1378 122:6005 122:6014 122:6015 122:6016,new (regen line)
6014,modest devotion,mddv,Priest,2,20,8,0,-20,1,3,0,0,50,-,50,-,-,150,-,145:50 115:5103 122:6003 122:6004 122:1377 122:1378 122:6005 122:6013 122:6015 122:6016,new (regen line)
6015,steady devotion,stdv,Priest,2,35,12,0,-40,1,3,0,0,75,-,75,-,-,180,-,145:75 115:5103 122:6003 122:6004 122:1377 122:1378 122:6005 122:6013 122:6014 122:6016,new (regen line)
6016,deep devotion,dpdv,Priest,2,50,16,0,-60,1,3,0,0,100,-,100,-,-,200,-,145:100 115:5103 122:6003 122:6004 122:1377 122:1378 122:6005 122:6013 122:6014 122:6015,new (regen line)
```

## 5. New behaviour to support

### 5.1 Rest and meditate

- **`rest`**: HP as in 1.11p, **plus mana at 3× the standing rate** (`rest_mana_mult`), for any
  class with mana, from level 1. Over time a resting caster gets 3× the mana per minute of a
  standing one (the slow tick's mana plus the rest ticks' share; the server keeps the remainder
  exact).
- **`meditate`** (still the stock Meditate ability, from the stock quest; classes without it get
  no output at all when they try, as in 1.11p): **mana at 5× the standing rate**
  (`meditate_mana_mult`), **and rest's HP**, and it counts the regen buffs. A meditating
  character never needs to switch to `rest`.
- **Both end by themselves once HP and mana are full**: rest silently, meditate with `You awake
  from deep meditation feeling stronger!`. A class with no mana counts as full on mana.
- **Meditate starts at full mana if HP is missing.** `Meditation will not help at this time.`
  only when both bars are full.
- Rest and meditate still end each other, and anything that broke them in 1.11p still does.

For the bot on 1.20:

- A caster with Meditate should meditate rather than rest, whatever is missing.
- A caster without Meditate should rest to refill mana too (in 1.11p rest gave it nothing extra).
- "Rest until full" loops: don't re-send `rest` because the `(Resting)` marker vanished at full
  bars; that is the end of the rest, not an interruption. Check the bars first. Do re-send if the
  marker vanished with bars not full (something interrupted it).
- Recovery-time estimates: mana recovery is 3× (rest) or 5× (meditate) the standing rate, before
  any regen buff.

### 5.2 Regen buffs in the spell plan

Keep the family's highest tier the character knows up while hunting and resting (section 4.2).
It costs 4-16 mana and lasts 120-200 rounds. Re-cast on its "when it ends" line (3.8). Upgrading
to a new tier takes two casts.

### 5.3 `GUARD`

```text
guard <player>     start guarding a party member in the room
guard              stop guarding
```

- The ward must be in your party and in the room. The name is matched as other player-target
  commands match it (a prefix of the first name; several matches list the options as usual).
- Guarding someone new stops guarding the old ward first (stop lines, then start lines).
- **Refusals:** `You don't see that player here.` (no such player in the room), `<Name> is not in
  your party.`, `You cannot guard yourself.`, `You cannot guard while resting or meditating.`, and
  for `guard` alone when not guarding, `You are not guarding anyone.`
- **What it does:** each **monster melee blow** aimed at the ward has `25 + 5 × combat`% (at most
  95) to hit the guard instead. `combat` is the guard's class combat rating on the help scale (the
  class's raw combat rating minus 2, at least 0): for example, a Warrior gets 45%. Several guards
  on one ward roll in the order they started guarding. A guard at 0 HP or below rolls nothing but
  keeps the guard.
- **What it doesn't cover:** spells, robbery, PvP and room damage still reach the ward.
- **It ends by itself, with the stop lines**, when the two part: either moves away, leaves the
  party, rests or meditates, logs off, hangs up or dies.
- **No state survives a reconnect**: the guard is session-only.

For the bot: a tank in a party can guard the squishiest member. Re-issue `guard <ward>` after each
move and after each rest (both end it). Watch for the stop lines to know it ended.

### 5.4 Backstab energy and the round after the stab

- **A stab no longer ends the round.** It costs `backstab_energy_pct = 150`% of the energy of the
  same character's normal blow with the same weapon, never more than the round's energy (1000).
  The rest of the round is normal blows, with normal damage, accuracy, energy and crits. So at low
  levels (a normal blow of 667 energy or more) the stab still takes the whole round; later it
  doesn't. Example: a normal blow of 400 energy makes the stab 600, leaving 400 for one more blow.
- This holds for the `backstab` command and for auto-combat alike, and for the quest backstab
  bonus and the Mystic's unarmed stab.
- **The stab's damage:** stealth adds `stealth / 4` (1.11p: `/ 10`), and the level multiplier is
  `× (100 + 2 × L)`% (1.11p: `100 + L`), with `L` held to `scalecap = 60`.
- **A stab can crit** at the normal crit chance (the crit knee applies; no fast-weapon bonus); a
  critical stab rolls from max to `backstab_crit_max_pct = 200`% of max.

For the bot: an opener estimate is the stab plus `(1000 − stab energy) / normal energy` normal
blows in the same round. Don't stop counting damage after the stab line.

### 5.5 Thief opportunist

A Thief's normal hit (not the stab) that leaves the target alive has `opportunist_pct = 10`% to
find an opening, for extra damage

```text
(2 × L + stealth / 4 + a roll in Backstab Min..Max) × (100 + L × 50 / 100)%  −  target DR / 10
```

with `L` held to 60, `Backstab Min..Max` the character's backstab bonus abilities, and the weapon's
damage playing no part. It never crits. For a damage model: expected extra damage per normal hit
on a surviving target is 10% of that.

### 5.6 Weave

Passive: no command, nothing to plan. Support it in display and statistics (section 3.2), and in
damage models:

- Proc chance per normal hit that leaves the target alive: `weave_pct = 20`% for Cleric,
  Missionary, Warlock, Gypsy and Bard; `weave_pct_magery1 = 10`% for Paladin and Ranger.
- The volley's expected damage is held to `weave_budget_per_level × L = 3 × L` (`L` held to 60),
  by rolling the spell at a power between `weave_power_min_pct = 25`% and `weave_power_max_pct =
  100`% of its normal strength; then `weave_stat_bonus_pct = 50`% of the casting-stat bonus
  (section 6.3); scaled by the class's `class_spell_pct` and `class_weave_pct` (section 7); and
  each hit goes through the target's element and magic resistance as a cast does, with one resist
  roll per proc.
- A Bard's proc is one hit of `3 × L` (its budget), held to its `class_weave_pct`.
- Typical picks, as the server's own balance tables found them (the pick is the known
  single-target attack spell with the best volley; ties go to the higher level): Gypsy and
  Warlock eldritch bolt at L20, meteor swarm at L25, dragonfire at L30, colour spray from L35;
  Cleric and Missionary godstrike at L20, god's wrath at L25, divine fury from L30 (agony at L10);
  Paladin godstrike from L20; Ranger blaze; Bard song of pain at L20, song of sonic blast from
  L25.

### 5.7 Area spells: monsters resist

Every **area damage** spell now rolls a resist for each monster it hits, whatever the spell's
`resmode`. A monster that resists has its share cut by `min(25, MR / 4)`%, never to zero. **There
is no resist line**: the per-target damage line just shows the smaller number. For damage models,
an area spell's expected damage on a monster of magic resistance `MR` is the share × (1 − P(resist)
× min(25, MR/4)/100); the resist chance is the stock spell-resist chance for that MR (capped at
98%). Players hit by monster area spells are unchanged.

### 5.8 Crit knee

Past `crit_knee = 50` a crit point counts a third: `effective = (crit − 50) / 3 + 50` (integer),
for every blow. 1.11p had the same curve with the knee at 40. Crit damage (2× to 4× max for normal
moves) is unchanged.

### 5.9 Other knees (players only)

- Magic resistance: the stat part `(Int + 3 × Wil) / 4` counts a third above `mr_stat_knee =
  110`; gear and spells are added after. Monsters unchanged.
- Dodge: Agility counts a third above `dodge_agi_knee = 150` in the `(Agi − 50) / 3` dodge term.
- Spell damage: the casting stat counts a third above `spell_stat_knee = 100` (section 6.3).

### 5.10 The magic-resistance fix

A player under 50 magic resistance now takes **more** from "inflict damage on armour" spells
(ability 17): `amount + amount × (50 − MR) / 100` (MR 0: 150%, MR 30: 120%), instead of the full
50% cut 1.11p gave by mistake. At MR 50 and up, and with Anti-Magic, it is as 1.11p. In an area
spell each player's cut is their own. For the bot: a low-MR character takes noticeably more from
monster nukes than 1.11p's numbers say; raise its danger estimate for caster monsters.

## 6. Other 1.20 rules wayfarer's models may need

### 6.1 Damage band

Every class's steady-state damage a round (a target that doesn't die, the opener left out) is
tuned to a band against the same race's Warrior at the same level; section 7 has the knobs and the
results. The casting-only classes are measured by sustained casting.

### 6.2 Class damage knobs

`class_melee_pct` scales the **minimum and maximum damage** of a class's normal blows (every move
but the stab, which keeps its own figures). `class_spell_pct` scales a class's damage spells
(after the casting-stat bonus; the weave included). `class_weave_pct` scales a weaving hybrid's
weave on top of `class_spell_pct`. Values are per level band: L1-29, L30-39, L40-49, L50 up.

### 6.3 Casting stat

Player damage spells (single-target, area and steal hit points; not heals, buffs, monster casts,
item casts or lasting drains) deal

```text
amount × (1000 + max(0, stat' − 50) × 5) / 1000        (integer, rounded down)
stat' = stat                              if stat <= 100
stat' = (stat − 100) / 3 + 100            if stat > 100
```

with `stat` the casting stat of 4.2 (Int, Wil, the Int/Wil mean, or Cha). That is +25% at 100,
+30% at 130, +50.5% at 255. A weave uses half of the part over 50.

### 6.4 Rank-weighted targeting

When a monster picks which player to attack, each candidate's threshold is
`(50 − 5 × attackers) × weight / 2`, with the weight from the party rank: front 3, mid 2, back 1,
solo 2 (a player with no party member travelling with them). Mid and solo roll as in 1.11p. A
front/mid/back party takes about 53/31/16% of first attacks. For the bot: put the tank in front
and the casters at the back; the party rank command is as in 1.11p.

### 6.5 Witchunters and friendly spells

A Witchunter (Anti-Magic) does not roll to resist a resistable spell when the caster is the
Witchunter or in its party (decided at cast time). A potion or scroll it uses counts as its own.
For a party bot: a Witchunter's party can buff and heal it normally.

### 6.6 Scaling past 60

`scalecap = 60`: the backstab (its flat 2L and its level multiplier), the opportunist (both level
terms) and the weave budget stop growing with level at 60. Normal blows, hit chance, energy, HP and
casting still use the real level. Player spells cap at 65 (section 4.1).

### 6.7 Race regeneration

Human: healing rate +15 and mana regeneration +15. Dwarf: healing rate +15. Elf: mana regeneration
+15. Gaunt One: mana regeneration +25. (Each race's stock abilities are kept.)

### 6.8 Stat-lowering spells

A stat-lowering spell stops at 0 instead of wrapping to about 65,500, and the stat comes back
exactly when it ends. Energy is clamped at 0 too. Maximum HP no longer overflows at very high
Health and level. If wayfarer had special handling for a wrapped stat, it can stay for 1.11p only.

### 6.9 Death

Death plays as 1.11p by default. A sysop can turn off coin drops, item drops, the life cost or the
deletion at zero lives; wayfarer can't see those settings, so it keeps 1.11p's assumptions unless
it observes otherwise.

### 6.10 Mana regeneration, summed

A caster's Alter Mana Regeneration is the sum of its race bonus (6.7), its regen tier (4.2) and
anything else that gives it, as in 1.11p.

### 6.11 Stat training (documentation only; bots never `train stats`)

`train stats` lets a stat go 30 points past its soft cap `S = max(race maximum, 90)`: the stock
price up to the race maximum, then 10 CP a point up to 90, then 15, 25 and 40 CP a point for
S+1..10, S+11..20 and S+21..30. The MAX column shows the training maximum. Character creation is
unchanged. For wayfarer this only means: another character's stat above its race maximum is
normal on 1.20, and a stat model must not cap at the race maximum.

## 7. The numbers (`[balance]`, as frozen in 1.20.0)

A sysop can override these; a server that does is not running 1.20.0, and wayfarer can't tell.
Use these values.

| Key | 1.20.0 | 1.11p equivalent | Section |
|---|---|---|---|
| `backstab_energy_pct` | 150 | the whole round | 5.4 |
| `backstab_stealth_div` | 4 | 10 | 5.4 |
| `backstab_level_pct` | 200 (×(100 + 2L)%) | 100 | 5.4 |
| `backstab_crit_max_pct` | 200 | no stab crit | 5.4 |
| `opportunist_pct` | 10 | none | 5.5 |
| `opportunist_level_pct` | 50 (×(100 + L/2)%) | none | 5.5 |
| `weave_pct` | 20 | none | 5.6 |
| `weave_pct_magery1` | 10 | none | 5.6 |
| `weave_budget_per_level` | 3 | none | 5.6 |
| `weave_power_min_pct` | 25 | none | 5.6 |
| `weave_power_max_pct` | 100 | none | 5.6 |
| `weave_stat_bonus_pct` | 50 | none | 5.6 |
| `spell_stat_permille` | 5 | 0 | 6.3 |
| `spell_stat_knee` | 100 | none | 6.3 |
| `rest_mana_mult` | 3 | 1 (rest gave no extra mana) | 5.1 |
| `meditate_mana_mult` | 5 | about 3 | 5.1 |
| `crit_knee` | 50 | 40 | 5.8 |
| `mr_stat_knee` | 110 | none | 5.9 |
| `dodge_agi_knee` | 150 | none | 5.9 |
| `softcap_floor` | 90 | none | 6.11 |
| `softcap_floor_price` | 10 | none | 6.11 |
| `softcap_band_prices` | [15, 25, 40] | none | 6.11 |
| `aoe_resist_cut_max_pct` | 25 | no resist roll | 5.7 |
| `aoe_resist_cut_div` | 4 | no resist roll | 5.7 |
| `rank_weights` | front 3, mid 2, back 1, solo 2 | all equal | 6.4 |
| `guard_chance_base` | 25 | no `GUARD` | 5.3 |
| `guard_chance_per_combat` | 5 (cap 95) | no `GUARD` | 5.3 |
| `scalecap` | 60 | none | 6.6 |

### 7.1 Per-class tables

One value applies at every level; four values are for L1-29, L30-39, L40-49 and L50 up. A class
not listed is 100.

| Class | `class_melee_pct` | `class_spell_pct` | `class_weave_pct` |
|---|---|---|---|
| Warrior | 100 | 100 | 100 |
| Witchunter | 109 | 100 | 100 |
| Paladin | 95, 95, 71, 88 | 100 | 100 |
| Cleric | 70, 70, 70, 60 | 100 | 87, 87, 49, 26 |
| Priest | 100 | 78, 78, 55, 59 | 100 |
| Missionary | 70 | 100 | 68, 68, 76, 73 |
| Ninja | 78, 78, 69, 94 | 100 | 100 |
| Thief | 100, 100, 77, 94 | 100 | 100 |
| Bard | 70, 70, 60, 70 | 100 | 38, 38, 42, 38 |
| Gypsy | 60, 60, 60, 68 | 100 | 42, 42, 38, 100 |
| Warlock | 70, 70, 65, 65 | 100 | 70, 70, 50, 100 |
| Mage | 100 | 93, 93, 76, 92 | 100 |
| Druid | 100 | 107, 107, 89, 93 | 100 |
| Ranger | 70, 70, 60, 51 | 100 | 50, 50, 40, 100 |
| Mystic | 50, 50, 46, 77 | 100 | 100 |

### 7.2 Band targets and where 1.20.0 landed

Steady-state damage a round as a percentage of the same race's Warrior at the same level. "Landed"
is the median over the races of the server's balance model at L30 / L40 / L60 (the gate: within
about 5 points of the band); L20 is reported only.

| Class | Group | Band, % of Warrior | L20 | L30 | L40 | L60 |
|---|---|---|---:|---:|---:|---:|
| Warrior | benchmark | 100 | 100 | 100 | 100 | 100 |
| Witchunter | pure fighter | 115-125 (aim 120) | 93 | 116 | 121 | 122 |
| Mystic | pure fighter | 115-125 (aim 120) | 135 | 118 | 120 | 120 |
| Paladin | weave fighter | 100-110 | 105 | 105 | 103 | 105 |
| Ranger | weave fighter | 100-110 | 100 | 103 | 107 | 105 |
| Warlock | damage hybrid | 125-130 | 151 | 127 | 126 | 129 |
| Gypsy | damage hybrid | 125-130 | 105 | 123 | 125 | 127 |
| Thief | stealth melee | 105-120 (the opener is extra) | 191 | 113 | 110 | 112 |
| Ninja | stealth melee | 105-120 (the opener is extra) | 152 | 112 | 111 | 110 |
| Missionary | squishy healer hybrid | 100-110 | 78 | 104 | 106 | 101 |
| Bard | squishy healer hybrid | 100-110 | 121 | 104 | 108 | 105 |
| Cleric | defensive hybrid | 80-95 | 88 | 88 | 86 | 89 |
| Mage | arcane caster | 130-150 | 82 | 140 | 138 | 139 |
| Druid | nature caster | 130-150 | 79 | 139 | 139 | 140 |
| Priest | divine caster | 75-90 | 71 | 81 | 82 | 82 |

For "where next?" advice: a 1.20 character of class C at level L deals roughly band(C) % of what a
same-race Warrior of that level deals (the "landed" column at the nearest level), where 1.11p's
ratios differed. The casting classes' figure assumes they sustain by casting and resting for mana.
The band is about damage dealt only: the casters, Thieves, Bards and Missionaries still survive a
fight much worse than a Warrior.

## 8. Unchanged in 1.20

So you don't hunt for them:

- the login, the menus, the version line and the login banner;
- the prompt and statline formats, the `(Resting)` / `(Meditating)` markers;
- every stock line not named in section 3, including every combat, room, movement, shop, party,
  loot, death and spell-cast line, the miss and dodge lines, the death lines and the experience
  lines;
- commands other than `guard`, and their syntax; `rest`, `meditate`, `backstab`, `cast`, `party`
  and the party rank commands work as in 1.11p (only their effects changed, as above);
- the world: rooms, maps, exits, doors, monsters and their stats and spawns, items (other than the
  24 new scrolls and the renamed scroll), shops (other than the new slots in 4.2), quests and
  their rewards;
- classes, races (other than the regen bonuses), experience tables, levels, the hit and dodge
  formulas (other than the knees), weapon damage, armour, energy (other than the stab);
- songs, kai, and the spells of every class other than Mage, Priest and Druid (save the four regen
  songs and the three cap-65 spells);
- monster spells (they cast stock copies);
- smash plays as 1.11p; item upgrade ladders and new high-level zones are not in 1.20.0;
- death plays as 1.11p unless the sysop changed it (6.9).

## 9. Suggested work and done criteria

Fit this into wayfarer's own plan format (step 0). A reasonable order:

1. **Rule set and detection** (section 2): the `HELP 1.20` probe, the fallback signals, the
   per-connection rule set, persisted per server profile. Tests: the two answers, the timeout, each
   fallback signal, and that `guard` is never sent while unknown or 1.11p.
2. **Patterns** (section 3): add the 1.20 patterns with their priorities, and the critical-stab
   handling. Tests: every example line in this document classifies as named, and a corpus of
   wayfarer's existing 1.11p captures classifies exactly as before.
3. **Tracking**: opportunist and weave damage and resist accounting, the intercept attribution,
   the round after a stab, area damage without a resist line.
4. **Spell data** (section 4): the 1.20 spell table as a data file in wayfarer (from the CSV
   above), the regen line with its shops and scrolls, the monster-copy range excluded, the
   two-cast tier upgrade.
5. **Behaviour**: rest and meditate (5.1), the regen buff upkeep (5.2), `GUARD` for a party tank
   (5.3), the backstab opener (5.4).
6. **Models**: the `[balance]` numbers and per-class tables (section 7) where wayfarer's damage
   estimates and advice use them.
7. **KB**: the facts above in `docs/kb/`, tagged, with changelog lines; fixtures in
   `tests/fixtures/<subdir>/` with `PROVENANCE.md`.

Done when wayfarer, against a 1.11p server, behaves exactly as before; against a 1.20 server, it
detects 1.20 at login, classifies every line in section 3, plans with the 1.20 spells, rests and
meditates by the 1.20 rules, and can guard; and its tests cover each of these with fixtures
built from this document.
