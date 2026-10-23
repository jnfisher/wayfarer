# Spells: learning, casting, mana

## Learning [C, observed 2026-10-13]
- Spells come from **scrolls** (item type 9) with ability 42 "Link to Spell" = spell number.
  Buy one in a spell shop, then `use <scroll>` (or `read <scroll>`, the same handler) → "You
  read scroll of X and learn the spell X." The scroll is used up. [C] `LJNUSE.C:157-158`,
  `LJNCMDB.C:313-322`
- Reading prints exactly one of three lines, each after an ANSI clear-line [C] `LJNUSE.C:143-165`:
  - "You read %s and learn the spell %s." (learned; one charge is taken: a stock scroll has 1);
  - "You realize that you already know this scroll!" when the spell's name finds that same spell
    in the spellbook (`find_spell_in_spellbook(..., partial 1)`); nothing is used up, the scroll
    stays;
  - "You don't know what to do with this!" when the spell is not usable, or the spellbook has no
    free slot (`add_spell_to_spellbook` fails). Nothing is used up.
- A scroll can only be used if its spell is usable (`user_can_use_spell`): the spell's magery
  type is the class's, its `magelvl` is at or below the class's magery level, its level is at
  or below the character's, and its alignment abilities suit the legal level. [C]
  `LJNUTIL.C:9758-9824` (the function), applied to scrolls by `user_can_use` at
  `LJNUTIL.C:9733-9741`. So an unusable scroll is normally refused before it is read, with
  "You may not use that item!" (`LJNCMDB.C:186-189`).
- **One Time Cast** items (ability 43: potions, food, drink, wands, Auto Use weapons) cast their
  spell in "item" mode (`cast_no_target(spl, unum, 1)` and `manual` 1 for the targeted
  engines): no success roll, a magery test of its own (a spell of no magery works for anyone
  with no target or a player target), but the spell's **energy, mana and level are still
  required and spent**; short of them nothing happens and the item keeps its charge. [C]
  `LJNUSE.C:166-186`, `LJNSPELL.C:2052-2085`, `:2134-2237`
- (sic) Cast from an item **at a monster**, a spell that has any magery at all is always
  refused, even for a caster of the matching magery and level (`return(0)` sits outside the
  inner class test); only magery-free spells work there. A sysop in debug mode is told
  "DEBUG : INCORRECT MAGERY GROUP/LEVEL. FAIL CAST." only when the class really does not
  match. On a player or with no target the matching class is enough. [C] `LJNSPELL.C:7052-7075`
  against `:2054-2060`, `:5106-5111`, `:8566-8571`
- **Auto Use Abilities** (ability 114) on a weapon: after the weapon's blow lands (not for
  punch, kick or jumpkick) the weapon's other abilities fire at the victim. The slot's value is
  the percent chance, rolled `genrdn(0,100) < value` (0 means always); a failed roll skips the
  light, heal and cast abilities that come after it. Used by hand (`use`), the first Auto Use
  slot ends the ability list. [C] `LJNUSE.C:355-364, :497-506`, `LJNCOMBT.C:2120-2130, :3030-3042`
- **Heal** (18) and **Heal Mana** on an item: `You receive %d hit points.` / `You receive %d
  mana.` with the item's **listed** value, not the amount actually gained; HP is held to max
  HP, mana to 0..max mana (a negative value drains). Used on another player, that player gets
  the same line with a prompt. [C] `LJNUSE.C:187-207, :414-444`
- Worn or wielded items with "Give Spell" (160) put the spell in the spellbook while on and
  purge it when taken off; "Alter HP" (88) adds to max and current HP while on. [C]
  `LJNUSE.C:651-709`
- Shop `list` marks other classes' scrolls **"(You can't use)"** and scrolls above your level
  **"(Too powerful)"**. [C]
- Newhaven Spell Shop 1:2144 (no markup): magic missile, blur, cause harm, minor healing, vine
  strike and starlight are free; illuminate and bless 4 gold; smite and mend 8 gold;
  songsheets of valour, misfortune and discord 4–8 gold. [C]
- `spells` lists the spellbook:
  ```
  Level Mana Short Spell Name
    1   4    star  starlight
    1   1    vine  vine strike
  ```
  [C]

## Casting [C, observed]
- Type the short name (or `cast <short>`) and a target: `vine kobold`. Without CAST a spell
  is matched by its **exact short name**; only a spell with no short name matches an
  abbreviation of its full name. A full name of a spell that has a short name does **not**
  work without CAST. Needs HP > 0 and not paralysed; a Kai class gets INVOKE. [C]
  `LJNPEND.C:438-458`, `LJNFIND.C:1124-1253`
- After CAST/INVOKE the name may also be any abbreviation of the full name (word-prefix);
  several matches list "Please be more specific." [C] `LJNCMDB.C:504-519`
- Precondition order: afraid ("You are too afraid!"), kai/non-kai and Magebind/Kai Bind, the
  spell known ("You do not know how to cast %s."), usable ("You may not cast this spell.",
  prefixed "This spell is too powerful for you - " when the level is the reason), the target
  ("You do not see %s here!", "You are not carrying %s.", "Why would you want to cast a
  spell on that?"), then the engine's own checks. [C] `LJNCMDB.C:381-672`, `LJNUTIL.C:9785`
- **Every cast ends the caster's auto-combat first** (`*Combat Off*` when it ran), and ends
  resting, meditating and sneaking. [C] `LJNCMDB.C:413-415, :482-644`
- An instant attack spell cast by hand out of combat **does not cast**: it prints `*Combat
  Engaged*` (and "%s moves to cast %s upon %s." to the room for a monster, "%s moves to
  attack everyone in the room." for an area spell), empties the energy, and auto-combat then
  **casts it every round** until the target dies: "You cast vine strike at short kobold
  thief for 5 damage!", with mana going down 1 per cast. [C] `LJNSPELL.C:2086-2124,
  :5338-5377, :7359-7455`
- Success roll: `genrdn(0,100) < min(spellcast + diff, 98)`; `diff >= 200` never fails; Kai
  powers never fail. A failed roll costs the full energy and half the mana ("You attempt to
  cast %s, but fail."). So even Spellcasting 200 fails 2% of casts. A cast from an item or a
  Kai power skips the roll. [C] `LJNSPELL.C:81, :2146-2232`
- Strength of an instant spell: uniform from `minval + mininc*L/minlvls` to `maxval +
  maxinc*L/maxlvls` (a minimum above the maximum is lowered to it), `L` = caster level held to
  `levelcap`. [C] `LJNSPELL.C:2240-2267`
- **General** spells once per 5-s round (`PS_CANCAST`, set at the end of `background_energy`
  LJNBKGND.C:398): "You have already cast a spell this round!" ("invoked a power" for Kai).
  [C] `LJNSPELL.C:2182-2196`
- Single-target combat spells **loop while the energy lasts** in auto-combat (at most 20
  tries): a 250-energy spell fires 4 times a round, a 500-energy one twice. Each try rolls,
  resists and prints afresh. [C] `LJNSPELL.C:7276-8445`
- Shortage checks run in the order energy ("You have already cast a spell this round!"), mana
  ("You do not have enough mana to cast that spell.", "kai to invoke that power" for Kai), then
  level ("This spell is too powerful for you to control."), each line in light red; nothing is
  charged. In auto-combat there is **no text**: an area spell spends its energy when it has
  enough and empties it when the mana is short, a single-target spell on a monster only
  spends the energy (the loop ends), and auto-combat tries again next round. [C]
  `LJNSPELL.C:4774-4811, :8394-8430`. The bot falls back to melee.
- Messages: the spell's WCCMSG record (caster/target/room) or "You cast %s on %s." / "%s
  cast %s on you." / "%s cast %s on %s."; area kinds print an empty target name; a
  "Starting Message" record opens each line; `SPF_ALTMSG` passes (target, amount) instead of
  (spell, target, amount). Caster and room lines once per cast; General spells in the
  COL_SPELLGEN colour, others COL_SPELLOTH. Heal shows the **rolled** amount, not what was
  healed. [C] `LJNSPELL.C:269-461, :3895-3903, :6262`
- **Lasting spells on a character** (player caster). Rounds = `duration + durinc*(L/durlvls)`
  (`L` held to `levelcap`); if that is below `durlvl*L` it is re-rolled from the old value up to
  `durlvl*L` inclusive; then multiplied by `(100 + "Alter spell duration")%`. A copy already on
  the target is renewed (new value and rounds, the cast shown) only when the engine allows it;
  otherwise, and when all ten slots are full, the caster gets "You attempt to cast %s, but
  fail." (the same line as a failed roll) and nothing is added. Stat alters (Alter HP and its
  kin) and Give Spell never renew, so a second cast of the same one fails. A lasting spell on a
  **monster** is always renewed and takes every free one of its five slots (sic). [C]
  `LJNSPELL.C:562-680, :4455, :6409`
- A spell ending prints the spell's "Descriptive Message" record line 0 (with the spell name)
  in brown; there is no built-in "wears off" text. Line 2 of the same record is the extra
  line the target sees when it is cast. [C] `LJNSPELL.C:9221-9230, :406-410`
- `purge <spell>` forgets a spell: "Purging spell %s from your spellbook." (PURGE must be
  typed in full). [C] `LJNCMDB.C:5851-5876`
- Kai classes learn their powers into the normal spellbook at TRAIN: "You learn the following
  Kai abilities:" then one name per line. [C] `LJNUTIL.C:8155-8185`
- The prompt shows mana for casters: `[HP=32/MA=22]:`. `st` shows `Mana: * 22/22
  Spellcasting: 59`. [C]
- Kai classes cast with `invoke`. [C]
- **Confusion** (status flag, ability 71 Confusion with a "Confuse Message" naming a WCCMSG
  record): each action is lost with the ability's percent chance (`genrdn(0,100) < chance`),
  giving "You look around stupidly and do nothing!" and the room "%s looks around stupidly
  and foams at the mouth!" (or the message's first two texts), plus one round of delay. [C]
  `LJNUTIL.C:10382-10440`
- **Death ends every spell on the dead character** without the "cast on ending" follow-up
  spells, and cures poison; a suicide does the same. [C] `LJNUTIL.C:4375-4392`,
  `LJNMISC5.C:500-515`

## Spell record (WCCSPELS, struct spell)
- `type` 0 Auto-Combat (attack), 1 Poison, 3 General (heals, buffs, light).
- `target` (spelltargets): 0 Single, 1 Self, 2 Self or Single, 3 Divided Area Not You,
  4 Monster, 5 Divided Area Incl. You, 6 Any, 7 Item, 8 Monster or Single, 9 Divided Attack
  Area, 10 Divided Party Area, 11 Full Area, 12 Full Attack Area, 13 Full Party Area.
  - Single-target attack: type 0 with target 0, 4 or 8. Area attack: type 0 with target 3, 5,
    9, 11 or 12.
- Abilities: 17 damage, 18 heal, 14 room light (starlight +175 is a light spell, an alternative
  to a torch).
- Ability 115, Descriptive Message: its value (the same slot of `abilvals`) is a WCCMSG number
  whose texts are [the line shown when the spell ends, "", the line its target sees when it is
  cast]: bless (14) → 8539 ["The effects of bless wear off!", "", "You feel lucky!"], barkskin
  (34) → 8528 ["The effects of barkskin wear off!", "", "You feel tough!"], holy aura (20) →
  8541 ["You no longer feel safe from evil!", "", "You feel safe from evil!"]. The spell's own
  `msgnum` is the cast message (bless 16: "You cast %s on %s!", "%s casts a spell on you!",
  "%s casts %s on %s!"). [F: the stock-world export in `tests/fixtures/world/`]
- Damage at caster level L = min/max + inc × L / lvls (`mininc`/`minlvls`,
  `maxinc`/`maxlvls`), up to `levelcap`. [C] `test/play/mmud_data.py Spell.damage`
- Area spells are never resisted by monsters (a coding bug), but MR still scales the damage.
  Anti-Magic classes resist friendly spells. See `aoe-grinding.md`, `grouping.md`.

## Resistance [C]

Read from the casting engines in `LJNSPELL.C` (`cast_no_target` :1864, `cast_user_target`
:4933, `cast_monster_target` :6859), `LJNMONST.C` (`monster_cast` :3235, `monster_cast_area`
:2125) and `LJNROOMC.C` (`room_cast_on_user` :1079, `room_cast_on_monster` :1218) by the
silvermere T-049 port; every row is [C] with its line unless tagged otherwise.

### The resist roll

| Rule | Detail | Source |
|---|---|---|
| Resist mode | `spell.resmode`: 0 never rolls, 1 rolls only when the target has Anti-Magic, 2 always rolls. Heals and buffs are usually mode 1, which is why Anti-Magic classes resist friendly spells. | LJNSPELL.C:1130-1133, 5402-5404, 7527-7529 |
| Item negation | A worn item whose `negates[10]` list holds the spell number makes the target resist outright, before any roll. Monster and room casts check it too. | LJNSPELL.C:209-240, 1127, 5399; LJNMONST.C:3398; LJNROOMC.C:1119 |
| Player roll, player caster | `genrdn(1,100) <= min(mr/2, 98)` resists (`MAXCHANCE` 98): a roll of 99 always gets through. `mr = (int + 3*wil)/4 + "Magic Resistance" abilities`. A single-target cast rolls only when its casting roll succeeded (a failed cast is never also "resisted"). | LJNSPELL.C:70, 1134-1137, 5406-5408, 7526-7532; LJNUTIL.C:4956-4958 |
| Player roll, monster or room caster | The same roll with the cap at **97**. A room rolls before the item check and refuses silently; a room cast never fails its casting roll. | LJNMONST.C:1849, 3405; LJNROOMC.C:1109-1121 |
| Cost of a resisted cast | The caster pays the full energy and half the mana, as for a failed casting roll. | LJNSPELL.C:5433-5434, 7547-7548 |
| Monster magic resistance | `"Magic Resistance" ability + monster.magicres`, at least 1. A spell carrying Enslave Creature uses `monster.charmres` instead, unless that is 0. | LJNSPELL.C:3001-3002, 7164-7166, 7262-7266 |
| Monster roll, single target | The same `min(mr/2, 98)` roll as for players (mode 2, or mode 1 against an Anti-Magic monster). | LJNSPELL.C:7527-7532 |
| Monster roll, area spell | The roll and chance are computed, then tested as `if (resroll > 999)`: **a monster never resists an area spell** (kept as is). The one exception is area Steal Hit Points, which really rolls, and after one monster resists every later monster in that cast takes 0 (sic). | LJNSPELL.C:3005-3013, 3543, 3557 |
| Spell Immunity | A target whose "Spell Immunity" total is **above** the spell's level is not affected by a hand cast; casts from auto-combat and "Non Magical Spell" spells skip the check. Checked for monster targets, for player targets of player casts, by monsters casting on players, and by rooms on monsters (an unsigned compare there). The comment says "at or above"; the code says strictly above. The refusal reads "Your spell has no effect on %s." A monster's cast on a player compares as a signed int. | LJNSPELL.C:7240-7254, 5253-5261; LJNMONST.C:3353-3356; LJNROOMC.C:1291-1294 |
| `resstat` | The record's "stat that resists" is never read by any engine; only the map dump prints it. Every roll uses `player.mr` or the monster's magic resistance. | LJNMAP.C:1012 (the only reader) |
| Poison Immunity | Targets with the ability are skipped by the Poison ability (no level change, no message), and a lasting Auto-Combat spell of the Poison element counts as resisted against them. | LJNSPELL.C:4112, 5229-5233, 6325, 8206 |
| Freedom vs Hold/Paralyze | A player with the Freedom ability is immune to any spell carrying Hold person or Paralyze (every attempt counts as resisted, energy and half mana paid). | LJNSPELL.C:5201-5205, 5270 |
| Required to hit | A monster with "Required to hit" is affected only by a spell whose own "Required to hit" value equals it. | LJNMONST.C:5151-5163 |

### Element resistance

| Rule | Detail | Source |
|---|---|---|
| Which spells | Only the attack types (Auto-Combat 0, Poison 1, "Change me" 2) have an element; General spells and the Normal element take nothing off. | LJNSPELL.C:1217-1248 |
| Which ability | Cold: Alter Cold Attack Damage (Defence); Hot: Alter Hot Attack Damage (Defence); Stone: Resist Stone; Lightning: Resist Lightning; Water: Resist Water; Poison: Poison Immunity. The value is a percentage; "random modifier" in the function name is a misnomer, nothing is rolled. | LJNSPELL.C:1217-1248, 7483-7513 |
| Where it applies | Taken off the roll once for a single target (player or monster); per player in the area loops of damage, Steal Hit Points and Energy (not heals, mana, poison or cure); **never off area damage to monsters**; off the dose of a lasting area spell for both players and monsters. | LJNSPELL.C:5496, 7597, 2914-2916, 3024, 1276-1312 |

### The magic-resistance damage multiplier

Only "Inflict Damage on Armour" (ability 17) goes through it; "Inflict Damage Ignoring Armour"
(ability 1) does not. Constants `MR_NEUTRAL` 50, `MRCUT_MAX` 50, `MRCUT_ANTIMAG` 75
(LJNSPELL.C:72-74). The amount has already had the caster's "Alter spell damage" percentage
added (`pct*amount/100 + amount`).

**Players and monsters differ.** A player's `mr` is an `unsigned` (MAJORMUD.H:1164), so the test
`mr - 50 > 0` is true for every `mr` under 50 (the subtraction wraps to a huge number) and the
cut becomes the maximum 50 (sic); a monster's resistance (`mres`, `magres`, `magicres`) is a
signed `int` and works as written. Every player-side site has the unsigned test: self cast
LJNSPELL.C:3109, area loop :3205, single target :6025, monster-cast area LJNMONST.C:2647 and
single :3878.

| Target | Cut | Damage | Source |
|---|---|---|---|
| **Player** without Anti-Magic, mr > 51 | `(mr-50)/2`, at most 50% | `amount - amount*cut/100` | LJNSPELL.C:3108-3115, 3131 |
| Player without Anti-Magic, **mr < 50** | wraps: the full **50%** | `amount - amount/2`: every player under 50 MR gets half damage | LJNSPELL.C:3109-3115 |
| Player without Anti-Magic, mr 50 | 0 | `amount*(50-50)/100 + amount`: full damage | LJNSPELL.C:3120-3127 |
| Player without Anti-Magic, mr 51 | `(51-50)/2` = 0 | `amount*(50-51)/100 + amount` = `amount - amount/100` (integer arithmetic): 99% | LJNSPELL.C:3120-3127 |
| Player with Anti-Magic | `mr/2`, held to 0..75% | `amount - amount*cut/100`; a 0 cut (mr 0 or 1) takes the full amount | LJNSPELL.C:3099-3106, 3121-3123 |
| **Monster** without Anti-Magic, mr > 50 | `(mr-50)/2`, at most 50% | `amount - amount*cut/100` | LJNSPELL.C:3323-3330, 7835-7842 |
| Monster without Anti-Magic, mr <= 50 | 0 | `amount*(50-mr)/100 + amount`: **below 50 takes extra**, up to +49% at mr 1 (monster mr is held to at least 1) | LJNSPELL.C:3333-3338, 7845-7850 |
| Monster with Anti-Magic | `mr/2`, held to 0..75% | as for a player with Anti-Magic | LJNSPELL.C:3312-3320, 7820-7827 |
| Examples (100 damage) | Player: mr 30: **50**; mr 50: 100; mr 51: 99; mr 95: 78; mr 150: 50. Monster: mr 30: 120; mr 50: 100; mr 95: 78; mr 150: 50. Either with Anti-Magic: mr 30: 85; mr 50: 75; mr 95: 53; mr 150: 25. | arithmetic of the above |
| Area quirk | In `cast_no_target`'s player loop the cut is not cleared between targets. With the wrap this matters only for a target without Anti-Magic at **exactly mr 50**, who inherits the previous player's cut (any target under 50 overwrites it with 50, mr 51 with 0; an Anti-Magic target sets its own). In the monster loop the caster's bonus is re-added for every monster, compounding. | LJNSPELL.C:3190-3222, 3305 |
| Room on player | `amount * max(150 - mr, 25) / 100`: linear, no Anti-Magic branch, no caps. | LJNROOMC.C:520-526 |
| Room on monster | The same linear form with `charmres` as the resistance (0 without an Enslave slot), capped at the spell's `maxval`. | LJNROOMC.C:1491-1506 |
| Monster on player | The player rows above (unsigned wrap included) without the caster's bonus (monsters have none). The cast message shows the **unsoftened** amount; the knock-out test uses the softened one. | LJNMONST.C:3869-3910 |

### Anti-Magic and friendly spells

A target with Anti-Magic rolls against mode-1 spells (heals, buffs) at `mr/2 %` **capped at
98**, exactly as against hostile mode-2 spells; the 75% figure is the damage-cut cap, not a
resist cap. A Witchunter with mr 150 therefore resists 75% of a Priest's heals, and takes 25% of
"Inflict Damage on Armour" damage. [C] LJNSPELL.C:1130-1137, 3099-3106.

## What a monster's spell does to a player [C LJNMONST.C `monster_cast` 3235-4560]

How a monster decides to cast, the roll, the energy and the resist texts are in `combat.md`
("Monster spell attacks"); area spells are in `aoe-grinding.md` §2a. These are the effects of a
**single-target** cast (Single, Self or Single, Monster or Single, Any) on the player it fights.
A single-target cast wakes the player (clears resting and meditating) and cancels a typed exit
(:3331-3346); an area cast does neither. Every effect needs a target kind that reaches one
player; an instant spell is the "no duration" case, and a spell with a duration is put on the
player once per cast, however many slots ask for it. If the add is refused (the same spell is
already on him and not stronger, or all ten slots are full) the monster gets its energy back and
the cast returns 0, which lets the round fall back to a normal blow.

| Ability | Effect | Source |
|---|---|---|
| Damage Ignoring Armour (1), Slay, Steal Hit Points | Instant: HP off, cast shown, then a death returns 2 (the death is credited to nobody, 0 experience: `exp` is never set) or a knock-out when the HP just fell to 0 or below. Slay takes the player to one point below the death HP. Steal Hit Points heals the monster by the same amount, capped at the template's `maxhp`. Only Damage Ignoring Armour has a lasting form when the spell has a duration (Slay and Steal Hit Points then do nothing). | :3747-3855, :3918-3955 |
| Damage on Armour (17) | Instant even with a duration, never added as lasting; MR cut as in the table above; the message names the unsoftened value. | :3844-3917 |
| Heal, Heal Mana | Instant: held to max HP / mana (a negative Heal Mana drains to 0); the message shows the amount **actually** gained, unlike a player's Heal, which shows the roll. | :4078-4155 |
| Poison | Skipped if the player has Poison Immunity; otherwise his poison is raised to the value (never lowered), cast shown. | :4156-4192 |
| Cure Poison | Ends his poison spell, then `poison -= value` with **no floor** (poison can go negative); cast shown after the poison spell ends. | :4193-4222 |
| Alter Energy | `energy += value` now, cast shown. | :3956-3983 |
| Alter Hunger, Alter Thirst | Instant: changes hunger or thirst, **silent**. | :4026-4077 |
| Freedom, Dispel Magic | Cast shown first, then spells end. Freedom ends only spells carrying Hold person or Paralyze. Dispel Magic ends **every** spell on him (the ability value is ignored). | :3648-3746 |
| Remove Curse | Only for Self, Self or Single and Any kinds, instant: takes off each worn item with the Cursed ability set. The Major Curse-versus-caster-level clause exists only when a **player** casts it. | :3611-3634; LJNSPELL.C:5765-5777 |
| Summon | Instant: the monster (item value = known monster number) is generated in the caster's room, forced, and ordered to follow the target player; everyone in the room sees the cast, but only for the spell's first effect. | :3984-4025 |
| Stat alters (Alter HP etc.) | Added with no renew (a second cast fails), then the stat changes. | :4252-4523 |

Level and duration: a spell cast from an attack slot uses the slot's "max damage" field as the
caster level, with no `levelcap` (strength `min/max + inc*level/lvls`); a cast from no slot (hit
spell, after-round spell, create/death spell) gets no level bonus. Lasting rounds on the player
are `duration + level/durlvls*durinc` for a slot cast. [C] `LJNMONST.C:3329, :3509-3542`

Self casts by number (`monster_cast_self_spell`, the create/death/self spells): the room sees
"itself" for the target, a lasting spell goes into the monster's own slots, each Poison slot
adds to the monster's poison with no immunity check, each Summon slot generates a monster (any
type, any level, no owner, forced). `createspell` fires at level 1 at the end of generation and
`deathspell` at level 1 before the death message, both with no player, so a Single-kind create
or death spell does nothing. [C] `LJNMONST.C:917-923, :1090-1091, :4950-5010`

Room spells on a **monster** (`room_cast_on_monster`) have no caller in the stock game (see
`navigation.md`); the function rolls with `charmres` only (0 without an Enslave Creature slot,
roll cap 98), Spell Immunity as an unsigned compare, caps each value at the spell's `maxval`,
never heals at once (Heal and every unlisted ability is stored as a lasting spell, into every
free slot, after which the slot loop resumes at 6), does nothing for Summon and Cure Poison,
drops a lasting Energy, takes a lasting Poison's amount off the HP at once with no kill check,
and lets Slay work with any duration. Its "Inflict Damage on Armour" is
`amount*max(150-charmres,25)/100`. [C] `LJNROOMC.C:1218-1640`

## Ability totals [C LJNABIL.C]

How a character's ability total is read (`get_user_ability_value` :270-550), because spells feed
it. The stored ability names are "Anti Magic" (51, no hyphen, :91) and "Poison Immunity" (21,
:61); notes elsewhere write "Anti-Magic".

- **Sum or best.** Most abilities add up over the character's own slots, active spells, race,
  class, worn items, the wielded weapon and non-wearable, non-weapon pack items. Seven keep the
  **best value** instead: Alter AV (Bless), Alter AV (Curse), the third attack value, Confusion,
  Damaging Shield, Alter Energy Usage and Confuse Message (:283-300). A best-value total that found
  nothing is 0.
- A worn, wielded or counted pack item that lacks a best-value ability still offers 0, so a
  negative value (a Curse from a spell or race) reads as 0 while **any** such item is on
  (:452-457, :484-487, :512-515). `update_dynamic_stats` (:1054-1058) keeps the negative.
- An item's value is its **first** slot with the ability (:905). With the ability twice, `value`
  reads the first, `update_dynamic_stats` adds every slot (:1322-1327): the stat bonus is the sum.
- A spell slot whose own value is 0 uses the value rolled at cast time (`spellvals[]`).
- **Negate ability** (124) counts only on a spell: it makes the total 0 whatever the source. A
  Negate slot on a race, class or item changes nothing (:1125-1127). In `update_dynamic_stats`
  the negation clears the **whole** bonus field (a Negate Alter Defence Value removes item and
  race defence too, :1139-1213, :1360-1377).
- `user_has_ability` (the yes/no test used by casting, BASH, SNEAK, Anti Magic checks) ignores
  Negate and (sic) sees a spell's abilities only while the spell's own ability slot *i* is
  non-empty, *i* being the spell's index among the character's active spells: a spell with its
  one ability in slot 0 is seen only while it sits in cast slot 0 (:589-597), though `value()`
  counts it anywhere.
- **Alter Energy Usage** in `update_dynamic_stats` is a running average `(old+value)/2` over the
  sources in order (:1076-1081), while the total from `get_user_ability_value` is the best value
  (:288-299).
- **Monsters** keep the best value for eleven abilities: the seven above plus Cold Defence, Hot
  Defence, Resist Stone and Resist Lightning (:709-722). Every item a monster carries counts
  (:775-800), and a carried item number with no record still adds 0.

## Bot policy (`progress.py`)
- Open with the best affordable single-target attack spell (keeping 2× its cost in mana), else
  melee/BASH. Self-heal with a known heal spell out of combat when below 85% HP. Rest until
  both HP and mana are back. Learn every newly usable spell at the nearest shop when affordable.
- Casters have low HP: use `--hit-frac 0.34` (largest monster hit ≤ a third of max HP).
