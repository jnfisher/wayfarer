# Screen parsing: patterns a client must recognise

All patterns below were observed in play on 1.11p/MBBSEmu [C] unless marked otherwise.
Regexes are as used in `test/play/progress.py`.

## Prompt
- `[HP=48]:` is the normal prompt and marks the end of a response. Regex: `\[HP=(\d+)`.
- `[HP=29 (Resting) ]:` appears while resting, and repeats as HP ticks up (about +2 per tick at
  level 1).
- Players with mana or kai show more fields (not seen yet for a Witchunter). [I]

## Room display (after a move, or when pressing Enter)
```
Slum Street                        <- title (can be overwritten by noise, see below)
You notice large sign, newbie manual here.   <- optional: items
Also here: thin guardsman, Bob.    <- optional: monsters (lower case) and players (Capitalised)
Obvious exits: north, south, east, west     <- may wrap; doors: "closed door north"
The room is dimly lit              <- optional light note
```
- **Press Enter** for brief room info (title, items, who's here, exits). `look`/`l` adds the
  long description. QUIET mode (`quiet`) shortens descriptions. [C, John]
- Titles aren't unique. Identify the room by the **title plus the set of visible exits**
  (hidden exits aren't listed). See `navigation.md`.
- The title line can be **replaced by noise**, e.g. "You hear movement to the east." printed
  where the title would be. A client must still advance its position when "Obvious exits"
  appears without a known title. [C]
- Monster names can carry adjectives ("angry kobold thief", "fat thug", "thin guardsman"):
  strip leading words until the name matches a known monster. [C]

## Darkness
- `The room is very dark - you can't see anything`: there's no room info at all. Light a light
  source (`light torch`), then look again. Other levels are "pitch black", "very dark", "barely
  visible", "dimly lit", "Regular Light", "Daylight", "You are blind!". [C]
  `LJNUTIL.C:8790`

## Movement errors
- `There is no exit in that direction!` [C]
- Closed doors show as `closed door north` in the exit list. `open north` opens one. [C]
- A Text exit is taken by typing its phrase, e.g. `borrow skiff`. The response is the room you
  arrive in. [C]

## Combat
```
*Combat Engaged*
You whap kobold thief for 2 damage!          (verbs vary: whap, beat, smack, ...)
The kobold thief lunges at you with their shortsword!
The fat thug smashes you for 10 damage!
The thin thug swipes at you with their spiked club, but you dodge!
3 silver drop to the ground.
9 copper drop to the ground.
The kobold thief falls to the ground with a shrill cry.   (death text varies by monster)
You gain 13 experience.
*Combat Off*
```
- **Count kills by `You gain (\d+) experience`.** Death messages vary per monster ("dissolves
  into a puddle of bluish goo", "collapses with a grunt"). [C]
- The end of combat is `*Combat Off*`.
- Being attacked: lines with "at you", "you!", "attacks you", "lunges at you". Arrival text: "X
  creeps/crawls/sneaks into the room from nowhere". [C]
- **Strike first on arrival.** Arrival texts are per monster (WCCMSG `arrivemsg`, 236 texts, plus the
  standard `X just arrived from …` / `X moves into the room from …` when a monster has none,
  LJNMONST.C:863-908), so match the whole set from the data rather than a fixed phrase, then send
  Enter to redraw the room (`Also here:` names the target) and attack. Wake on the same regex while
  waiting or resting. The bot got from arrival line to `bash` in about 1 s, where waiting to be
  hit took up to 10 s. [C] `progress.py` `wake_re`
- Blows aimed at you all contain "at you" or "you … for N damage!" (e.g. "whips you with its
  pseudopod for 5 damage!"); a simple "attacks you"/"you!" check misses the second kind. [C]
- Can't attack: `Your weapon has no effect against this monster!` (immune or an NPC) and `To do
  this action, you must turn off your evil warnings.` (a good-aligned target). [C]

## Loot and money
- Coins: `N <coin> drop(s) to the ground.` → `get <coin>` → `You picked up N silver nobles`. [C]
- Coin names: runic, platinum, gold, silver (nobles), copper (farthings). [C]
- Inventory (`i`): `You are carrying 39 silver nobles, ..., padded vest (Torso), quarterstaff
  (Two handed), torch` (equipped items have `(Slot)`), then `You have no keys.`, `Wealth: N
  copper farthings`, `Encumbrance: 545/3792 - None [14%]`. [C]
- Equipped slots seen in 1.11p: `(Torso) (Legs) (Head) (Hands) (Feet) (Finger)`, `(Two handed)`
  for two-handed weapons, `(Weapon Hand)` for one-handed ones, and `(Readied/0)` for a lit light
  source held ready. Other versions may print
  "Two-handed", so normalise. **Parse by splitting the list on commas**: a regex over the whole
  line can swallow "You are carrying" into the first item's name (pitfall 30). [C]

## Shops
- `list`: a table of `Item  Quantity  Price` (Price can be `Free`). [C]
- `buy X` → `You just bought X for 20 silver nobles, 8 copper farthings.` / `You just bought X
  for nothing.` [C]
- Failures: `You cannot buy X here!` (not stocked, or 0 in stock), `You cannot BUY if you are
  not in a shop!`. [C]
- `wear X` → `You are now wearing X.`; `wield X` → `You are now holding X.` [C]

## Status screens
- `st`: `Name: ... Lives/CP: 9/2`, `Race: Kang  Exp: 0  Perception: 33`, `Class: Witchunter
  Level: 1`, `Hits: 48/48  Armour Class: 16/1`, then the six stats as `Strength: 79` and so on.
  [C]
- `exp`: `Exp: 13 Level: 1 Exp needed for next level: 1637 (1650) [0%]`. The figure in
  brackets is the total for the next level. [C]

## Status changes and resting
- `rest` → `You are now resting.` Any action ends resting. [C] The game prints nothing when plain
  resting ends: the only sign is the prompt's `(Resting)` mark going away (a client must watch the
  prompt, not the text). Meditation ends with `You awake from deep meditation feeling stronger!`.
  [C] (wayfarer C-015, 2026-10-19: no static pattern exists for a rest ending)
- `quiet` → `Quiet mode set`. [C] `brief` is the same command and `verbose` undoes it. The mode is saved with the character
  (`LJNPERM.C:113/301`), and new characters start verbose (`LJNUTIL.C:1436`). Room title, items,
  who's here and exits are unchanged, so a client can default to quiet. [C]

## Typing commands: what the server does with a line

A client sends a line and the server tries it in a fixed order (`execute_input`, `LJNPEND.C:100-600`) [C]:

1. A player waiting out the exit delay (`X`) is refused everything but `break`: "You may not perform any
   commands while waiting to exit!" (`:150-172`).
2. A line typed while the command delay (`bash`, `search`, `sneak`, `track`, `picklock`, ... `add_delay`)
   runs, or while older lines wait, is **queued** and nothing is shown for it. The delay also switches the
   input echo off, so a queued line appears as `<line>` plus a new line only when it runs, one queued
   line per fast tick; the echo comes back when the queue is empty. A delay that ends with nothing queued
   shows the prompt again (`:178-195, 695-745`, `LJNUTIL.C:8075-8085`, `LJNBKGND.C:2357-2374`).
   **Flood control**: a new line that finds nine or more lines already waiting (the 10th typed) gets
   `Why don't you slow down for a few seconds?` and is still queued; one that finds thirteen or more (the
   14th) gets that line and `You are typing too quickly - command ignored` and is dropped
   (`PENDWARN` 8 and `PENDMAX` 12 links, `LJNPEND.C:45-46, 776-797`). Neither text has the `ESC[79D ESC[K`
   prefix.
3. An empty line redisplays the room only for statline FULL or FULL CUSTOM, then the prompt (`:197-206`).
   `x` asks to leave (`cmd_quit`; it is queued behind a delay like any line). A confused player may lose the line (`:231-243`).
4. `handle_commands` (`LJNCMDS.C:108-1100`) parses the first word. A command still in its delay is
   answered with nothing, not even a prompt (`:147-149`). At 0 hit points or below, or paralysed, only 24 codes
   run (HELP, SAVE, LOOK, SOUND, STATUS, HEALTH, INVENTORY, KEYS, WHISPER, BROADCAST, SUICIDE, REROLL,
   IDENTITY, TOPTEN, QUIET, VERBOSE, SCAN, BRIEF, STATLINE, PROFILE, SYSOP, EXPERIENCE, FEEDBACK, GLOBAL);
   anything else, JOIN included, gets "You may not do that while you are mortally wounded!" (a paralysed
   player gets the same text) (`:156-209`). A colour reset (`ANSI_NORMAL`, `ESC[0;37;40m`) is written before
   every routine (`:213`).
5. A word that is no command, or whose routine answers 0, falls through: the room's Text and Remote Action
   exits, the room's command text, a spell named in the spellbook (exact short or full name; needs
   `hp > 0`, not paralysed), then "You cannot speak!", the profanity check, social actions, and speech
   (see `navigation.md` "Text blocks"). A line that nothing handles and nobody hears answers "Your command
   had no effect."; with `talk fast` a line starting with `/` is neither said nor yelled yet is answered
   `You say "..."` (sic, `:570-575`).

**Command words** are parsed letter by letter by a decision tree, not by a table (`parse_command`,
`LJNPARSE.C:1-4960`); the complete list of codes with the shortest accepted form is the comment block in
`MAJORMUD.H:858-1011`. Every prefix from the shortest accepted abbreviation up to the full word gives the
command, one letter more or a wrong letter gives none. Words a client is likely to trip over [C]:

| Typed | Means | Notes |
|---|---|---|
| `AA`, `AL`..`ALLOUT`, `FORC`/`FORCE`, `BAS`..`BASH` | BASH | `AU` is ATTACK; `A`, `AT`.. ATTACK |
| `BS`, `BACKS`..`BACKSTAB` | BACKSTAB | |
| `BG`, `GB`, `BROADG`..`BROADGANG` | BROADGANG | |
| `BAL`..`BALANCE`, `BAN`..`BANKBOOK` | BANKBOOK | |
| `ORD`..`ORDER`, `BU`..`BUY` | BUY | |
| `WHO` (only whole), `SC`..`SCAN` | SCAN (the player list) | |
| `REA` | READY | `READ` is READ, `READY` is READY |
| `KIL`..`KILLLBLO` | KILLBLOW | (sic) one L too many: `KILLBLOW` typed in full is no command (`:68-69, 2532-2545`) |
| `BORROW` + 2 more characters | BORROW | (sic) `word[8]` is tested where `word[7]` was meant: +2 is still BORROW, +3 or more gives none, +1 depends on the byte past the end of the word (`:70-73, 694-705`) |
| `VERB`.. anything | VERBOSE; `VERS`.. anything is VERSION | (sic) the check runs after the code is set (`:74-75, 3290-3350`) |
| `N`/`NORTH`, `Q`/`QUIT` | the full word or one letter | `NO`/`NOR`/`NORT` are no command; `QU`/`QUI`/`QUIE` are QUIET (`:76-78`) |
| `PU`, `PUN`.. , `PUR`, `PURG` | PUNCH | (sic) PURGE only in full (`:3405-3413`) |
| `?` | HELP, only alone | `'` or `-` broadcast, `>` direct message, `/` whisper, with any text after |
| `SUICIDE`, `REROLL` | only in full | the minimum forms elsewhere: `STATL` STATLINE, `ST` STATUS, `SOUN` SOUND, `INVI` INVITE, `INVO` INVOKE, `C` CAST, `G` GET, `L` LOOK, `I` INVENTORY, `U` UP, `D` DOWN |
| `I`, `INVE`..`INVENTORY` | INVENTORY | `IN` and `INV` are no command: the 4th letter decides (`INVE` INVENTORY, `INVI` INVITE, `INVO` INVOKE). Likewise `EX` is none (`EXI` EXITS, `EXP` EXPERIENCE) and `GO` is none (`GOS` GOSSIP) [C: silvermere's `command_words.json`, generated from `LJNPARSE.C`] |
| `SAY`, `YELL`, `TELEPATH`, `TIME` | no command | the parser has no such codes: talking in the room is any line it does not take (step 5 above); `/name text` is WHISPER and `>name text` DIRECTMSG [C: as above] |

Codes the parser knows that do nothing: the dispatcher has no routine for SOUND, THROW, FIRE, DONATE, PLAY,
SHOOT, TAKE, TEXT, AUTOCOMBAT, DEFEND, HUNT, DISMISS and RESEARCH (the list is the natural reading of the
compiled jump table, `LJNCMDS.C:1085-1098` [C/I]), and the routines for GUARD, IDENTITY, BORROW and REPAY
return 0 at once (`LJNCMDA.C:2820-2834`, `LJNCMDB.C:5006-5036`). All of them end as "not a command":
the word goes on to the room's text exits and then to speech, which is why `borrow skiff` reaches the
Docks' Text exit.

**Colour reset in front of a command's output.** Because the dispatcher writes `ANSI_NORMAL` into the output
buffer first, the first thing sent carries it: when that first send goes to the other players in the room
(`tell_room`), they get the reset and the actor's own line comes without it (`LJNTELL.C:300-341`);
`tell_user` only rewinds the buffer (`:695`), so text sent again by a later `tell_user` with nothing written
in between is sent again (this is why every party member is told a rank change). A name lookup
(`get_user_number`) empties the buffer, reset included, on a full-name match, on no match and on a single
abbreviation match; only an ambiguous abbreviation keeps it, and then the reset, the "Please be more
specific" list and the prompt arrive in one write (`LJNUTIL.C:7765-7800`, `LJNMULT.C:79-160`). `SEARCH`
shows the reset before `You notice ... here.`, drops it when nothing is found (`Your search revealed
nothing.` is coloured by itself), and a blind searcher gets it before `You are blind.`
(`LJNDISP.C:2196-2214`, `LJNCMDA.C:3220-3262`). A client that reads colour should not rely on a reset
being present.

**The module's own pager.** Long texts (the help topics, the release notes in the menu, the gang house
and long-text displays, `SYSOP ... INFO`) are sent in the kernel's page mode: after 22 lines the server
shows `Hit any key to continue...` and waits (`btuxnf`, `LJNHELP.C:129`, `LJNDISP.C:3696, 5406`,
`LJNGAME.C:3625-3639, 4039`, `LJNCMDB.C:2374`) [C]. Under MBBSEmu these pauses are reported never to appear, because it
ignores `btuxnf` (MBBSEmu v1.0-alpha-090826) [I: the MBBSEmu source is not in this repo]. The `(N)onstop, (Q)uit, or (C)ontinue?` pager is the BBS's own and is not in the MajorMUD source.

## Recommendation: generate the pattern table from source
Every player-visible string is a `prf(...)` literal in `SRC/*.C`, or a message in
`WCCMMUD.MSG`, `WCCTEXT.MSG` or `WCCMMHLP.MSG`. Monster arrival, death and attack texts are
WCCMSG records (the `arrivemsg`, `deathmsg` and `hitmsgs` fields). A client can build its
parser table from those instead of collecting examples. [I, not yet done]
