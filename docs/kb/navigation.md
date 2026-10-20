# Navigation

## Map model
- Rooms are keyed `(map, room)`. The stock world has 26,720 rooms; from Newhaven, 7,745 are
  reachable on foot without keys, items or spells (an earlier note said 7,369; not reproducible,
  see `changelog.md` 2026-10-14). [C] `mmud_data.World.distances`
- Each room has 10 exits in the order N, S, E, W, NE, NW, SE, SW, Up, Down (commands `n s e w ne
  nw se sw u d`), each with a destination, an exit type, and parameters. [C] `MAJORMUD.H struct
  room`

## Exit types and how to take them
Decoded from `move_user` (`LJNUTIL.C:2755-3996`) for the silvermere T-026 port; `exitparN` is
the exit's Nth parameter in the room record. Every refusal text is in `screen-patterns.md` §5.

| Type | How | Notes |
|---|---|---|
| Normal | direction | Ticket, User Count, Block Guard and Delay exits have no case in `move_user` and pass like Normal [C] `LJNUTIL.C:3729` |
| Door / Gate | direction; `open <dir>` if closed; **`bash <dir>` if locked** | `exitpar1` = lock state (0 open, 1 shut-unlocked, 2 locked; `LOCK_OPEN/CLOSED/LOCKED` `LJNCMDA.C:115-117`). Refusals: state 1 "The door is Closed!" (capital C), state 2 "The door is closed!"; a Gate says "The gate is Closed!" / "The gate is closed!" the same way [C] `LJNUTIL.C:3171-3199`; `open` on state 2 says "The door is locked." OPEN works on 1, LOCK on 1, PICKLOCK on 2, BASH on 1 or 2 [C] `LJNCMDA.C:3346-3400, 4801-4819, 3100-3104, 686-770`. Bash succeeds if genrdn(0,100) < **Str + exitpar2**, 2 ticks, 25% chance of 1-4 self-damage ("You take %d damage for bashing the door!", always "door", rolled for an open door too) [C] `:640-783`; the far room sees "The %s to the %s just flew open." [C] `LJNSUPP.C:1835-1846`. Pick locks: `picklocks > 0` and genrdn(0,100) < **exitpar2 + picklocks**, 2 ticks (+2 on failure); a failed pick of a door in a room with `picktrap` casts that spell [C] `LJNCMDA.C:3100-3186`. Relocks after `exitpar3` steps of 300 fast ticks (5 min), **not** when `exitpar2 > 0` [C] `LJNEXITS.C:75, 406-432`. Both sides of a door change together when the far room's exit leads straight back [C] `LJNCMDA.C:3360-3380`. Start-up locks every door and gate (`exitpar1` = 2) [C] `LJNBUFF.C:1095-1109` |
| Key | direction when unlocked; `use <key>`; `picklock <dir>` | `exitpar2` = lock state (0/1/2), `exitpar4` = relock steps, `exitpar3` = picklock bonus [C] `LJNUTIL.C:2963-2979`, `LJNCMDA.C:3031-3034, 3405-3410`. Shut: "There is a closed door in that direction!" [C] `LJNUTIL.C:2975`. Only a bash-kind move (how 5) passes a shut Key exit and `cmd_bash` never makes one (dead branch `LJNCMDA.C:654`), so keys are turned with USE (`LJNUSE.C`) [C]. Relock text "The door to the %s just locked!" [C] `LJNEXITS.C:392-402` |
| Block Guard | direction | players pass like Normal (`LJNUTIL.C:3729`); the exit only stops Guard-type monsters wandering through it (`LJNMONST.C:1339`, the check at :1320-1341); other monsters take it as a plain exit (`LJNMISC5.C:213`) [C] |
| Text | **type a phrase** from WCCMSG record `exitpar1` (any of its 3 texts) | Newhaven Docks → Small Pier: `borrow skiff`/`go skiff`/`row skiff` (msg 8564) [C] `LJNPEND.C:282`. `exitpar3` = texts message (msgs[1] room left, msgs[2] room entered, msgs[0] player); a plain direction gets "There is no exit in that direction!" [C] `LJNUTIL.C:3201-3295` |
| Action | direction (prints a message) | Small Pier → Newhaven ("You hop in one of the skiffs...") [C] `LJNUTIL.C:3106`. `exitpar1` = message (0 = plain exit; msgs[1] room left, msgs[2] room entered, msgs[0] player); an unseen sneaker passes it as a plain exit; same map only; a refused room entry says "You are not permitted in that room!" [C] `:3106-3170` |
| Remote Action | never walked; **type a phrase** from WCCMSG record `exitpar1` (any of its 3 texts) and an action runs on an exit of the room this exit leads to | walking it: "There is no exit in that direction!" [C] `LJNUTIL.C:3456-3463`. The phrase (whole line, no case) runs `run_room_exit_action` on the target room: `exitpar2` tens = action, units = exit number there; `exitpar3` = message (msgs[1] with the name to your room first, then msgs[0] to you); a non-zero `exitpar4` item must be carried and loses a charge [C] `LJNPEND.C:355-412`. On the target exit: a Door/Gate swaps open/locked (both sides) and swaps back after `exitpar3` exit-delay steps; a Trap/Spell Trap in the hidden state throws every player, then every monster, in that room through; a Hidden exit loses block `action` (0 = all; with `exitpar2` >= 0 only from the highest down, negative = any order) and with no block left becomes a visible passage, the room told (its `exitpar3` message, or "A concealed passage opens to the <dir>."), blocked again after one step (300 fast ticks) [C] `LJNSUPP.C:1134-1410`. Crypt levers and the statue button work this way (`test/play/notes/quests.md` §1). Text, Action and Remote Action [C] |
| Map Change | direction; `exitpar1` is the target map | the add-on activation check is the only gate; an add-on the sysop has not bought is refused with "Your sysop must purchase the <add-on name> before you may move through this exit." [C] `LJNUTIL.C:3606-3626` |
| Toll | direction, pays | `exitpar1` gold crowns, paid by value with the largest coins first and change from a broken coin: "You just paid <list> in toll charges."; paying gives a sneaker away; "You do not have enough to cover the toll of %d %s." [C] `LJNUTIL.C:3076-3105, 8621-8758` |
| Item | direction, carrying item `exitpar1` | message `exitpar3` (msgs[1] room left with name and direction, msgs[2] room entered, msgs[0] player) and one use of the item is spent; without it message `exitpar2` or "You do not have the appropriate item to go that direction!" [C] `LJNUTIL.C:2980-3075` |
| Spell | the exit's spell, or follow a leader through | a plain walk: "You need to cast a spell to go that way!"; only a party march or typed words (move kinds 3, 4) pass [C] `LJNUTIL.C:2942-2961` |
| Class / Race | direction, if allowed | `exitpar1` = the one class (race) allowed, else `exitpar2` = the one barred; "You may not go through this exit!"; refused outright during tournament play ("You may not go through this exit during tournament play!") [C] `LJNUTIL.C:3464-3535` |
| Level | direction, if in range | `exitpar1` min, `exitpar2` max, 0 = no limit; "You have not progressed far enough..." / "You have progressed too far to go through this exit!"; also refused during tournament play [C] `LJNUTIL.C:3536-3571` |
| Alignment | direction, if in range | evil points between `exitpar1` (as an int) and `exitpar2`; "You are too good/evil to go through this exit!" (too good below `exitpar1`, too evil above `exitpar2`); also refused during tournament play [C] `LJNUTIL.C:3572-3605` |
| Timed | direction while open | closed while `exitpar3 != 0`; swaps every `exitpar2` steps of 300 fast ticks; message `exitpar1` (msgs[0] opening, msgs[1] closing, msgs[2] refusal, `%s` direction) or "The exit to the %s just opened!/closed!" and "You may not pass through that exit at this point in time." [C] `LJNUTIL.C:3627-3649`, `LJNEXITS.C:265-316`. The first swap is armed at start-up by `preload_and_generate_buffers` with `exitpar2` steps [C] `LJNBUFF.C:1000-1004` |
| Cast | direction | spell `exitpar1` on the player first (dead, held or stunned he stays); message `exitpar3` msgs[0] player, msgs[1] room left; after any attempt spell `exitpar2` and msgs[2] to the room he is in [C] `LJNUTIL.C:3650-3687, 3958-3977` |
| Ability | direction, if the ability is in range | ability `exitpar1` with value between `exitpar2` and `exitpar3` (top compared unsigned); else message `exitpar4` (msgs[0] player, msgs[1] room) or silence [C] `LJNUTIL.C:3688-3729` |
| Ticket / User Count / Delay | direction | no case in `move_user`, pass like Normal [C] `LJNUTIL.C:3729` |
| Hidden | not listed in "Obvious exits"; must be found (`search <dir>`) | walkable with `EXF_PASSABLE`, `EXF_VISSEARCHED` or `EXF_VISBLOCKED`, else "There is no exit in that direction!" [C] `LJNUTIL.C:3444-3455`. `search <dir>` on an `EXF_NEEDSEARCH` exit: genrdn(0,100) < max(3, perception-15) finds it ("You found an exit to the %s!"), it hides again after one step (300 fast ticks); a Trap exit is reported on genrdn(0,100) < traps; else "You notice nothing different to the %s." [C] `LJNUTIL.C:9975-10055`, `LJNEXITS.C:318-323` |
| Trap / Spell Trap | walkable, may hurt | `exitpar2` kind: 0 and 3 always spring, 2 only when walked into on purpose (else "There is no exit"), 1 and 4 (disarmed) are ordinary exits; Trap damage genrdn(exitpar1/2, exitpar1+1), message `exitpar3` (msgs[0] with `%d` damage); Spell Trap casts spell `exitpar1`, message `exitpar4`, its room texts end with a blank line; the player goes through regardless of the room's entry rules (a Trap runs the entry check and ignores the answer). (sic) a Spell Trap prints the message's first text with the Trap's damage `%d` from an unset variable, and reads it without a NULL check. [C] `LJNUTIL.C:3296-3443`. A disarmed trap re-arms after one step [C] `LJNEXITS.C:246-263` |

- **Dragging** a downed player is refused by Spell, Key, Item, Toll, Class, Race, Level and
  Alignment exits ("You may not drag anyone through this exit.", eight copies) [C] `LJNUTIL.C:2951-3581`.
- **Room entry** (`user_allowed_in_room`): an arena above its `maxlevel` says "You have progressed
  too far for this room." and `move_user` adds "You are not permitted in that room!" (two lines);
  the protected-room and colliseum refusals pair the same way [C] `LJNUTIL.C:2519-2612, 3745`.
- **Start-up reset.** On a cold start (not a SYSOP restart of the buffers; the reset sits inside `if (!reinit)`, LJNBUFF.C:871-1113), `preload_and_generate_buffers` puts the exits back to
  their start state: Key `exitpar2` = 2 (locked), Door and Gate `exitpar1` = 2 (locked), Trap and Spell
  Trap `exitpar2` 1 -> 0 and 4 -> 3 (a disarmed trap is armed again), a Hidden exit that was found goes back
  to `EXF_NEEDSEARCH`, a visible-but-blocked one gets its blocks back (`exitpar2` +-n sets blocks 1..n;
  0 and +-3 give three), and every Timed exit is put on the timer list. Normal, Spell, Item, Toll, Action,
  Map Change, Text, Remote Action, Class, Race and Level exits (and the newer types) have no state to reset
  [C] `LJNBUFF.C:943-1112`. So a door or key lock you left open is locked again after a restart.
- **Direction commands** queue the move: delay 1 tick (2 dragging), +1 (+2) over 66% load, x2
  slowed, /2 quickened, at least 1, +1 after two moves in a row; the fast tick walks. No exit:
  "There is no exit in that direction!" and the room sees "%s ran into the wall to the %s."
  (message level Medium, sighted only); sneaking ends [C] `LJNCMDB.C:5110-5205`.
  - **Two at once** (a client pipelining a route, as MegaMud runs paths): the second direction
    sent before the first's room has come is taken after it, and each move shows its own room
    and its own prompt, in the order sent (`w`, `w`, `d` from the Village Entrance: the Narrow
    Path, the Narrow Road, the Arena). Both rooms can reach the client in one read, so a client
    must confirm each move by its own room, not by where the last one left it [C, silvermere,
    wayfarer C-030 live check, 2026-10-20].
- **Leaving a room**: one genrdn(0,100) roll for the room; the first monster whose `followpct`
  is at least the roll strikes once if its alignment lets it (good ones and Angels only a player
  they follow and are not friendly with; a Lawful Evil one spares `evil >= 80`); a death cancels
  the move. Each monster following the player rolls genrdn(0,100) < `followpct` to come along
  [C] `LJNMONST.C:4590-4720`.
- **Movement lines**: "%s just left to the %s." has three wordings (genrdn(1,4)) with a
  space-backspace pair inside the word and a different `dirnames` copy each; up/down are "just
  left upwards/downwards"; arrivals "walks into the room from the <reverse>". A noticed sneaker:
  "You notice %s sneaking out to the %s." / "sneak in from the %s.", and on genrdn(0,100) <
  perception "You make a sound as you enter the room!" [C] `LJNUTIL.C:3805-3911`.

## Text blocks: room commands, quest scripts and ASK/GREET

Rooms, spells, monsters, deaths and the sysop point at a `WCCTEXT` text block whose lines are small
scripts (`LJNSPEC.C`). A client meets them as typed phrases that do something in a room ("sit throne",
"pull lever", "destroy apparatus") and as a monster's answer to `ask`.

- **Where a typed phrase is looked at.** After the command parser has said "not a command" (it answers
  0 for a word it does not know and for the words that have no routine, so `borrow skiff` gets here),
  a living player's line is tried against the room's Text and Remote Action exits, then against the
  room's command text, then as the name of a spell in his spellbook, and only then becomes speech
  [C] `LJNPEND.C:245-458`. Room scripts and text exits work for `hp > 0` only [C] `:271`.
- **Command text format.** `pattern:clause:clause...` per line; the pattern is a wildcard match against
  the whole typed line (no abbreviations: "sit throne" and "sit on throne" are two lines). Matching lines
  run in order until one runs through to the end; a line that stops (a failed condition) lets the next
  matching line try. A block may run on over up to 50 records [C] `LJNSPEC.C:1805-1930`. A script with no
  pattern (spell, death, monster question, sysop) runs its lines the same way [C] `:2040-2113`.
- **Clauses.** 42 keywords, each matched as a case-free prefix of the clause text in the order the
  engine tests them (so `nomonsters` is seen before `monsters` and every `teleport_<town>` form falls under
  `teleport`); an unknown clause is skipped and the line goes on [C] `LJNSPEC.C:849-1790`. A condition
  that fails shows its optional WCCMSG message (text 0 to the player, text 1 with his first name to
  the room) and stops the line. Worth knowing: `price` takes the coins silently; with the sysop's
  KID_GLOVES option on, items a line already took with `takeitem` are given back when a later `price`
  or `takeitem` fails [C] `:896-908, 1137-1170`; `giveitem` puts an item that does not fit on the
  floor, `hideitem` hides one in the room and ends the line [C] `:1186-1232`; `addexp` gives quest
  experience with no "You gain" line and has no cap (the script prints its own message), and does
  nothing for a character without the new-exp flag [C] `:1173`, `LJNMISC5.C:719-760`; `random <block>`
  rolls once, 0-99, and runs the first line whose percentage is above the roll [C] `LJNSPEC.C:1932-2030`;
  `remoteaction <room> <message> <action> <exit>` runs a Remote Action on another room of the map
  [C] `:1639-1664`.
- **(sic) quirks.** `flag <1-64> set|check|test|clear` does nothing but check its range (it reads the flag
  number where it meant the verb) [C] `:570-640`; `nomonsters` as the last clause of a line always passes
  [C] `:1046-1071`.
- **Worked example: the Ice Sorceress throne** (room 10:245, "Tower, Open Chamber", command text 4700;
  `sit throne` and `sit on throne`). First line: `nomonsters` (else "You can't do that right now!"),
  `failability 125` (a character who already has the Icy Throne flag, ability 125, is stopped without a
  word), `minlevel 15` (else "A strange force stops you from sitting on the throne!" and the room hears
  "<name> can't seem to fit in the throne's seat!"), `addexp 750000`, `addability 2 1`, `giveability 125 2`,
  then "As you sit on the throne a shimmering blue light surrounds you." and "<For capturing the ice throne
  you gain 750,000 experience points!>". The second line serves characters whose flag is exactly 1 (an
  older version gave the armour but not the experience): it pays the 750,000 and raises the flag to 2. The
  +1 is ability 2 "Alter Defence Value", which `st` counts as ten times its value in tenths of Armour
  Class, so it shows as +1 Armour Class. Everything acts on the character who typed `sit throne`: **each
  party member has to sit on the throne himself, with the room clear, at level 15 or more** [C, WCCTEXT 4700 and
  WCCMSG 233/1093/3383/3384 from the stock export; `LJNSPEC.C:937-1004, 1233-1262, 1575-1635`;
  `LJNUTIL.C:8863-8971`]. (The crypt's `sit throne` at 6:994, text 985, is a different script: with the room
  clear it runs `remoteaction 994 1538 0 8`, which opens that room's exit 8.)
- **ASK and GREET** (`ask_monster_a_question`, `LJNUTIL.C:10088-10256`). The question is upper-cased; the
  monster's talk block is read (first record only) as `KEYWORDS:text number` lines and the first line whose
  keywords occur anywhere in the question (substring, not word) wins; its answer text is shown with the
  monster's name and the answer's own link runs as a whole script. `greet` shows the talk block's link, or
  `<monster> doesn't understand you.` when it links nowhere. No talk block, or no line matching, gives
  `<monster> has nothing to tell you!` [C].

## Room spells and trapped locks

A room can cast a spell on you ("The room" is the caster) [C] `LJNROOMC.C:1079-1178`:

- **When.** Every *other* medium pass (every 6 s; the 3-s pass between casts nothing) on every
  player standing in a room with a `roomspell` [C] `LJNBKGND.C:1015-1035, 1913-1917`. On
  entering a room flagged `RF_ENTRYSPELL` (0x20), but only when the room you left has a
  different `roomspell`, and not on a no-history move [C] `LJNUTIL.C:1852-1857`; the flag adds
  no other cast. The entry cast happens while you still stand in the old room (a summon or a
  Scatter acts there) [C] `LJNUTIL.C:1852-1870`. Also: a Spell Trap exit casts `exitpar1`, a
  Cast exit `exitpar1` on the way and `exitpar2` after any attempt (see the table above), a
  failed PICKLOCK on a trapped lock casts the room's `picktrap` instead of "Your skill fails
  you this time." [C] `LJNCMDA.C:3153-3157`, a failed DISARM the trap's `exitpar1`
  [C] `LJNCMDB.C:5368`.
- **Resisting.** No energy, mana or cast roll. You resist when a worn item negates the spell,
  or (resist mode "always", or "anti-magic" with Anti-Magic) `1d100 <= min(MR/2, 97)`; the
  strength (min..max, no level growth) is cut by your element resistance for attack-type
  spells [C] `LJNROOMC.C:1103-1137`.
- **What it can do.** Damage, heal, poison, energy, hunger/thirst, summon (the monster follows
  you, hostile), freedom, dispel, cure poison act only for spells aimed at one player (Single,
  Self or Single, Any, Monster or Single); an area-kind room spell does nothing at all. There is
  **no death check**: room damage only knocks you out (from above 0); a later kill check does
  the rest. Shatter always destroys the weapon ("Your <item> has been shattered!"). Scatter
  moves the floor's coins and gettable items to room `amount` (the "cleanup" room spells);
  Clear Item erases an item; a Teleport Room + Teleport Map pair moves you after the last slot,
  dropping your followers [C] `LJNROOMC.C:241-1009, 1171-1177; LJNSUPP.C:733-815`.
- **What you see.** Only you see it (the room hears nothing): the spell's WCCMSG target line
  with "The room" as the caster ("The room casts smite upon you!"), or for alternate-message
  spells just the amount ("You are seared by the flames for 42 damage!"), then the
  descriptive message's third line. A lasting spell is added and shown once per cast; the
  stat alters say nothing; hunger and thirst say nothing [C] `LJNROOMC.C:85-153, 1023-1040`.
  Pattern hint: a damage line with no caster name and no `*Combat Engaged*`.
- Stock examples [C WCCMP001/WCCSPELS]: Lava Pool (map 7, rooms 991+) magma heat 30-60 Hot
  every 6 s; Rancid Underwater Passage (map 8, 631-632) black water 5-15 on entry from the
  Musty Room; the 22 stock Spell Trap exits cast poison darts (poison 12-20, "You feel ill.").
- Nothing in the game casts a room spell on a monster (`room_cast_on_monster` has no caller; a
  monster crossing a Spell Trap takes plain damage) [C] `LJNMONST.C:1365-1373`.

## Position tracking (essential)
- Keep a **belief set** of candidate rooms. After a move with command `c`, the new set is the
  destinations of `c` from every candidate, filtered by the observed title and the visible exit
  set. If it comes out empty, look near the last belief (within about 12 steps), then globally.
  [C] `progress.py Bot.update`
- If no title is seen but "Obvious exits" is, and the move wasn't refused, still advance the
  belief. [C]
- A walk counts as arrived when the destination is in the belief set, even if ambiguous:
  identical rooms can't be told apart. [C]
- Pitfall: matching by title alone picked "Town Square" on map 17 instead of map 1. Always
  prefer candidates close to the previous belief. [C]

## Routing
- BFS over walkable exit types, giving commands that include Text-exit phrases. [C]
  `World.route`
- Restrict routes to an area (`within=`) for patrols. [C]
- Newhaven → Silvermere goes through the Docks' Text exit (`borrow skiff`); the way back is
  the Small Pier's Action exit north. [C]

## Published paths and loops
- MegaMud `.mp` files carry a step list and room checksums. `test/play/routecheck.py` replays
  them through the data. Six published loops verified; see `test/play/notes/routes.md`. [C]
  All 1,957 paths of MegaMud v2.0's set walked the same way: room ids exact for 99.2% of steps,
  175 of 177 loops close; format, exit-type bits and step flags in `megamud-formats.md`. [C]

## SYSOP GOTO (operator shortcut)
- `sysop goto <town>` teleports to a named town: `newhaven` → 1:2150 (Newhaven Arena),
  `silvermere` → `reincroom` 1:2189 (Temple, Halls of the Dead, which also has a healer),
  `rhudaur` 2:2519, `khazarad` 6:1249, `lostcity` 16:426. The add-on towns check the add-on
  activation. Refused in autocombat, while being attacked, or during a PvP retaliation period.
  [C] `LJNCMDB.C ~1853`; tested 2026-10-13.
- **Rhudaur (2:2519) is an evil-aligned city.** Its dark monks attack a character who isn't evil
  enough, so a GOTO there can land a fresh or good character straight in a fight (seen in silvermere
  m4 run 39039053836: three dark monks attacked the sysop on arrival). An evil character is left
  alone, and a strong one simply kills them. [F: John, 2026-10-17; the attack confirmed by capture]
  The reverse holds in Silvermere town: its guardsmen attack outlaws (40+ evil points; see
  `combat.md` "What attacks you" and `grouping.md` §4), so walking across Silvermere with an evil
  character is dangerous [F: John, 2026-10-17; C for the guard rule]. A bot or test that goes to
  Rhudaur should arrive evil or strong, and should be neutral again before it reaches Silvermere.
- It needs the account to hold the module's `SYSOPKEY` (`wccsysop`) or GAMEOP key, plus
  `SYS_GOTO_KEY` if one is set (empty here). On MBBSEmu, keys live in `mbbsemu.db
  AccountKeys` and are **read at BBS login**: after granting one, log off with `X`, then `R`
  to re-logon. [C]
- Response: no message, just the prompt. Without the key the line is spoken instead ("You say
  ..."). [C]
- Room-number teleport (`SYSOP EXPRESS_TRAIN <room> [map]`) needs a developer code. **By
  policy, bots use only town GOTO** (John: arbitrary teleports would make the game trivial).
- Bot use: before a long walk, take GOTO when the walk from the town is at least 5 steps
  shorter. [C] `progress.py try_goto`
- Exiting the realm (`x`) is a short "silent meditation" that monsters can interrupt, so step
  out of spawn rooms first. [C]

## Level-restricted exits and danger-aware routing
- "Level" exits carry a level range in `exitpar1` (min) / `exitpar2` (max). The Silvermere Docks
  1:33 → Newhaven crossing is 0–3: "You have progressed too far to go through this exit!" [C]
- Routes skip Level exits outside the character's level, and any exit the game has refused
  (learned at run time). [C] `mmud_data.World.exit_ok`
- **Danger-aware routing:** a room is dangerous for a character if its spawns hit harder than a
  quarter of max HP (after DR) **and** have more HP than about 3 rounds of our damage
  (wilderness dark monks, brigands, gnolls; not slum half-ogres). Routes add a cost of 40 per
  dangerous room, so they go around them, and a ground whose safest route still crosses one isn't
  chosen. [C] `mmud_data.World.path` (Dijkstra), `progress.Bot.update_danger`
- Monster damage per round needs swings per round [C]: each blow picks the first attack slot
  whose `atkpcts` is above a 0-99 roll (cumulative percentages; none above: the last slot used
  again, LJNCOMBT.C:3398-3405) and costs that slot's `atkenergies`; a blow the energy left cannot
  pay ends the round, at most 6 blows (LJNCOMBT.C:3387, :3530-3540). The leftover carries over:
  each energy round adds one round of `energy` to a monster below it and cuts one above it back
  to it (`energy_update_monster`, LJNBKGND.C:470). So a round has `floor((leftover + energy) /
  atkenergies)` blows, **`energy / atkenergies` on average**: the kobold thief (1000, 666)
  swings 1, 2, 1, 2... Stock giant rat `atkpcts` is 100, 20, 30, so slot 0 always wins and its
  other slots are never used. Even with that, the dark monk's buffs make it far deadlier than
  the table suggests, so the danger rule leans on "can't kill it in ~3 rounds". [C, silvermere
  T-036 `monster_blows_per_round`]
- Monster hit chance [C LJNCOMBT.C:1254-1347]: a monster's blow is a plain attack (`atkmove`
  MOVE_NORMAL, :3380), so `hitmod` is 0 plus the defender's Defence Modifier (`def->defmod`,
  :1254-1258, :1293). `atkpower = (av + hitmod)^2 / 140`; `hitchance = 100 - (dv +
  defalign)^2 / atkpower`, or 5 when `atkpower` is 0; then clamped to 10-99. A defender with a
  negative dodge rating is first caught off guard when `genrdn(0,100) > dodge + 100` (about
  `-dodge`% of the time): then `hitchance` is 99 (:1316-1318). The blow lands when
  `genrdn(1,100) < hitchance`, and that roll is 1-99, so **the real odds are
  `(hitchance - 1) / 99`** (:1304, :1347). So the giant rat (accuracy 10, `atkpower` 0,
  `hitchance` 10) hits 9/99 ≈ 9% of the time against anyone not caught off guard, while the
  kobold thief (15, `atkpower` 1) has `hitchance` 99, hitting 98/99 of the time against
  DV 0 and no Defence Modifier.
- **Newhaven is for levels 1-3 only** [C]: the Silvermere Docks exit north (1:33 → 1:296) is a Level exit, and it is the only way back to the Newhaven island; `route()` finds no path at level 4+. Party play: see `plan-grouping.md` §7c (`@solo`).
