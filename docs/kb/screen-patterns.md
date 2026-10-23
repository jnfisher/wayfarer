# Screen patterns: the full catalogue

Every text pattern a MajorMUD 1.11p client must recognise, for stock Realm of Legends on
MBBSEmu. The executable form is `test/play/patterns.py`, which loads one registry from this
table plus the game data. `screen-parsing.md` is the short introduction; this file is the
reference. The plan for moving the bot onto the registry is in `plan-patterns-migration.md`.

**Sources**
- The recovered source: every `prf()` literal in `SRC/*.C` (3,144 strings were extracted and
  reviewed), plus `WCCTEXT.MSG`, `WCCMMUD.MSG` and `WCCMSG` records.
- Our transcripts: `test/play/logs/*/transcript.txt` (59 logs, about 40k lines) and `raw.bin`.
- The mudinfo.net captures forum (https://www.mudinfo.net/viewforum.php?f=6): all 27 topics on
  2 listing pages. The forum has no attachments, so none were skipped. No post gives a game
  version.

**Seen column**
- **[C]**: confirmed in the source (file:line) or the transcripts.
- **[F]**: forum capture only.
- **[I]**: inferred, not yet observed.
- **T*n***: matched *n* times in our transcripts (self-test run of 2026-10-13).
- **F**: also matched in the forum captures.

**Size**
- 446 static patterns (438 line patterns plus 8 table-row patterns) in 17 categories, plus the
  prompt parser. (412 at the 2026-10-13 count; 34 rows added by the T-065 write-back, see
  `changelog.md` 2026-10-16.)
- About 6,440 patterns generated from the game data in 18 roles (see "Patterns generated from
  the game data").
- 1,754 room titles taken from the room data.

**Self-test**
- `python3 test/play/patterns.py` classifies 99.8% of transcript lines, at 11 µs per line. The
  rest are character-creation form fragments and type-ahead garbage.

## 1. How the text arrives, and how to cut it into lines

- **Nearly every message starts with `ESC[79D ESC[K`** (`ANSI_CLRLINE`, MAJORMUD.H:1080):
  cursor 79 left, erase line. The prompt is printed without a newline, so the next message
  overwrites it on a terminal. A parser has to treat CLRLINE as a line break. [C]
  - **Printed without the prefix:** `There is no exit in that direction!` (LJNUTIL.C:3292),
    `Command interrupted.` (LJNGAME.C:4916), the two flood-control lines (LJNPEND.C:788, :792) and
    `The gods have punished you appropriately.` (LJNUTIL.C:753; it and the `Last time you were on ...` line before it come in bright white after a bare `\r`). [C]
  - **Colour after the prefix:** `*Combat Engaged*` and `*Combat Off*` brown (LJNCOMBT.C:1740-1742,
    LJNACMB.C:151-153); `X drops to the ground!` grey on red with the bell, then the background goes
    back to black (LJNDISP.C:6006-6010); `X just disconnected!!!` bright white (LJNGAME.C:5444-5445);
    `You hear movement ...` magenta (LJNDISP.C:2260); `X just entered the Realm.` and
    `X just left the Realm.` have the prefix and no colour (LJNGAME.C:1581-1582, LJNCMDA.C:4443-4445). [C]
  - **Labels take the palette.** `Obvious exits:` is `clrtbl[COL_EXITS]` (green on palettes 0, 2 and 3,
    brown on 1; LJNDISP.C:1260-1261, LJNGLOB2.C:75). `Also here:` and its closing `.` are
    `clrtbl[COL_ALSOHERE]` (magenta, green, light magenta, magenta), then `ESC[0m` (LJNDISP.C:2564-2566,
    :2747-2749). A new character starts on palette 0, a stored palette of 4 or more is reset to 0 at realm
    entry, and `SET PALETTE 0-3` answers `You will now use colour palette number N.` (otherwise
    `Valid palettes are: 0, 1, 2, 3`) (LJNUTIL.C:1394, :1237-1239, LJNCMDA.C:6262-6286). [C]
  - `bridge.plain()` only deletes ANSI codes, so the prompt and the next message end up glued on
    one line, for example `[HP=48]:Slum Street` or
    `[HP=7/MA=19]: (Resting) The acid slime whips you ... for 5 damage!`.
  - About 5,300 such glued lines occur in our transcripts, 842 of them behind a mana prompt.
  - `patterns.raw_to_lines()` splits on CLRLINE.
  - `PromptParser.peel()` removes any number of prompts from the front of a plain-text line.
- **Every prompt ends with a `^A` (0x01) byte** in the source (LJNPRMPT.C:702). MBBSEmu doesn't
  pass it through: there are 0 occurrences in raw.bin. Treat it as a line break if it ever
  appears. [C]
- **The echo of typed commands** follows the prompt on the same line (`[HP=48]:exp`). It is only
  recognisable when we know what we typed: pass the sent commands
  (`patterns.sent_marker(cmd)`), or the echo is classified as text. [C]
  - Type-ahead echo can also land *inside* other output. For example, `Name: Kasimir st` is the
    status screen with the echo of a second `st` spliced into the name field. Don't send a
    command while a multi-line display is still arriving. [C]
- **Word wrap at 79 columns** (`btutsw(WRAPCOLS)`) breaks these displays across lines, which
  must be joined back [C]:
  - `Obvious exits:` (a line that ends in "," or in "door"/"gate"/"passage" continues)
  - `You notice ... here.`
  - `Also here: ... .`
  - `You are carrying ...` (up to `You have no keys.` / `You have the following keys:` /
    `Wealth:`)
  - the key list
- **Direction names carry a random letter and a backspace** [C]: every direction the game
  prints (exit lists, doors, movement messages) comes from `dirnames[]`, whose entries are
  `"n \borth"`, `"s \bouth"`, ... with a slot that the slow background cycle fills with a
  random capital A-Z (LJNTABLE.C:65-103, LJNBKGND.C `background_slow`). The bytes are
  `Obvious exits: nJ\borth, sU\bouth, wK\best, sO\boutheast`: a terminal shows `north`, but
  the raw text changes every cycle. Apply backspaces before matching (`raw_to_lines()` does);
  a byte comparison must mask `[A-Z ]\x08`. `dirnames2`/`dirnames3` put the slot at
  positions 2 and 3 for the "just left to the ..." message. Seen in every 2026-10-14 capture.
- **Flood control works on a queue** [C]: a line typed while a command delay runs (`search`,
  `bash`, `sneak`, `track`, `picklock`, ... `add_delay()`), or while older lines still wait, is
  queued; queued lines run one per tick, each echoed as it runs (the delay switched the echo off).
  A new line that finds nine lines already waiting (the 10th typed) gets
  `Why don't you slow down for a few seconds?` and is still queued; one that finds thirteen waiting
  (the 14th) gets that line **and** `You are typing too quickly - command ignored` and is dropped
  (`PENDWARN` 8 and `PENDMAX` 12 links walked, LJNPEND.C:45-46, :776-797). Neither text has the
  `ESC[79D ESC[K` prefix. Commands with no delay never queue: eight `look`s typed at once all just
  run. Capture `flood-and-refusals`. [C]
- **The bell** (`\a`) follows `X drops to the ground!` (LJNDISP.C:6008). Strip it before
  matching. [C]
- **Colours carry information** that plain text loses.
  - The `Also here:` monster names are coloured by alignment (LJNDISP.C:2680-2720): cyan for
    good and neutral, grey for lawful good to non-outlaws, the palette colour for evil.
  - Hit points in the prompt are coloured by health (`get_hp_colour`, LJNPRMPT.C:155-171): the
    normal colour above ½, "hurt" from ½ to ¼, "low" at ¼ or less, in integer division: at exactly half
    (28 of 56) the figure is already "hurt", at exactly a quarter (14 of 56) already "low".
  - The forum's Blood2Html captures give the colour of each line class. Titles are bright cyan
    (#00FFFF), exits green (#008000), "Also here" names magenta (#FF00FF), hits bright red,
    glances dark red, spells blue, death and experience text grey.

## 2. The prompt (status line) is configurable

`prf_prompt()` (LJNPRMPT.C:615) prints one of five forms. It depends on `player.statline`,
which the player sets with `SET STATLINE ...` (LJNCMDA.C:6328, 6393) or `STATLINE ON|OFF|FULL`
(LJNCMDS.C:686).

| Mode | Set with | Prompt printed | Enter on an empty line |
|---|---|---|---|
| 0 OFF | `set statline off` | `>` | prompt only |
| 1 ON | `set statline on` | standard (below) | prompt only |
| 2 FULL (**default**, LJNUTIL.C:1438) | `set statline full` | standard | **room redisplayed** (LJNPEND.C:204), then prompt |
| 3 CUSTOM | `set statline custom <text>` | the player's template | prompt only |
| 4 FULL CUSTOM | `set statline full custom <text>` | the player's template | **room redisplayed**, then prompt |

The standard forms are below. The resting mark goes in different places with and without mana.

| Character | Prompt | Source |
|---|---|---|
| no mana | `[HP=48]:` / `[HP=29 (Resting) ]:` / `[HP=29 (Meditating) ]:` | LJNPRMPT.C:670 [C] |
| mana | `[HP=32/MA=22]:` / `[HP=7/MA=19]: (Resting) ` (the mark comes **after** `]:`) | LJNPRMPT.C:655 [C] |
| kai (Mystic, Ninja) | `[HP=40/KAI=5]:`, same layout | LJNPRMPT.C:659 [C], never seen yet |
| sysop with DEBUG | the above followed by `{Energy=1234 Snk JstEnt Hdn }:` | LJNPRMPT.C:680 [C] |
| hit points below 0 | `[HP=-1/MA=19]:` (unconscious) | [C] T |

**Custom template codes** (`substitute_string()`, LJNPRMPT.C:214; :172 is the end of `get_hp_colour`):

- Values:
  - `%h` hit points, `%H` max HP
  - `%m` mana or kai, `%M` max mana
  - `%x` exp (32-bit total), `%X` exp still needed
  - `%c` wealth in copper
  - `%i` first name
  - `%w` evil warnings `On`/`Off`
  - `%r` ` (Resting) ` / ` (Meditating) ` / nothing
  - `%t` prints "nothing"
  - `%p` / `%P` pronouns
- Layout and colour:
  - `%n` new line, so the prompt spans lines
  - `%%` prints **two** percent signs
  - `%d` prompt colour, and colours `%h` by health
  - `%B` `%N` `%U` `%L` `%R` attributes
  - `%f0`..`%f7` and `%b0`..`%b7` colours (sic: `%b1` gives green, LJNPRMPT.C:279)
- Anything else is copied as typed. `%5` gives `5`, `%z` stays `%z`.
- The template is cut to 60 characters (`STATLINELEN`, LJNCMDA.C:132; the cut itself is at
  LJNCMDA.C:6404-6405 and :6433-6434). Expansion stops once 196 characters have been written
  (`SUBSTMAX`), but the code being expanded at that moment still finishes. [C]
- Details of the expander [C LJNPRMPT.C:214-570]:
  - `%d` stays in force for every later `%h`; only a `%f` or `%b` digit switches the health colour
    off again (`%B %N %U %L %R` do not). `%b1` and `%b2` both give `ESC[42m` (green), as noted above.
  - `%%`, `%f%` and `%b%` all print `%%`. A template ending in an unfinished `%` or `%f` drops it.
  - The `%`, `%f` or `%b` written back for a non-code does not count toward the 196; the
    character itself does.
  - `%X` is the exp still needed, never below 0; with no race or class record it prints a bare `X`
    (not `%X`), because that path clears the pending state before falling into the plain-copy case.
  - The sysop debug suffix `{Energy=.. Snk JstEnt Hdn }:` belongs to the standard line only, never to OFF
    or a custom line; it is still printed for a mana user with no class record, where the HP part is
    missing (LJNPRMPT.C:642-693).
- The template is stored only in the permanent record. `SET` shows it on the `Statusline:` line
  (LJNDISP.C:4829), where On, Full or Off would otherwise appear.

**Custom prompts in the forum captures [F]:**
- `[HP=395/395,MA=392/433]:`, probably `set statline custom [HP=%h/%H,MA=%m/%M]:`
- `[hp=xxx/XXX,MA=8/42,CASH=180750,WARNS=Off]:` (t=1266)

**What the bot assumes today:**
- `HP_RE = \[HP=(-?\d+)` and `MA_RE` assume the standard form.
- Under OFF there is no HP at all.
- Under a custom line, HP is lost unless the template starts with `[HP=`.
- Under ON or CUSTOM, Enter no longer shows the room, and `where()` / `update()` silently stop
  locating us.
- **Recommendation:** at login, read `set` (or just send `set statline full`) before anything
  else, and keep FULL.
- A client that wants max HP and mana on every prompt can use `set statline full custom
  [HP=%h/%H/MA=%m/%M]:%r` and parse it with `PromptParser("custom", template)`. The registry's
  `auto` mode also accepts any `[...hp=...]:` prompt as key=value pairs.

## 3. Room display

Order (display_room_desc, LJNDISP.C:2440-2775):

1. title
2. the colliseum warning, when it applies
3. (`look` only) long description, first line indented 4 spaces
4. `You notice ... here.` (coins first, then items, counted)
5. `Also here: ...` (players first, then monsters)
6. `Obvious exits: ...`
7. `The room is <light>` when the light level is below 0 [C] LJNDISP.C:2756-2771

All of it is [C]. The details:

- **Title.** Titles are matched exactly against the room data (`Registry.room_names`, 1,754
  names).
  - A room whose first description line is `FILE DESCRIPTION` prints a text file instead of
    the title (LJNDISP.C:2515).
  - A colliseum adds the two-line "NOTE: This room is *not* a death-free zone right now!"
    warning, in light red. It appears only for room type `ROOM_COLLISEUM` (5), not `ROOM_ARENA`
    (2), and only while the sysop's `arenasafe` switch is off. `arenasafe` starts at 1, so a
    stock server never shows it. [C] LJNDISP.C:2533-2540, LJNGAME.C:230
  - `FILE DESCRIPTION` rooms (the gang houses, map 15 rooms 850 and up) name a file in
    description line 2 (`WCCnnnnn.HSE`); line 3 is a line prefix, empty in stock. The title is
    the file's first line. `look` prints every line of the file instead of the paragraph.
    [C] LJNDISP.C:2520-2522, :2836-2840, :3682-3730
- **Dark rooms.**
  - `The room is pitch black - you can't see anything` / `The room is very dark - you can't
    see anything` (LJNSUPP.C:480) replace the whole display.
  - `You are blind.` replaces it too (LJNSUPP.C:472).
  - Light above 900 prints `You are blind!` alone (the brightest entry of lightdescs[],
    LJNSUPP.C:486). Ordinary light is capped at 900 and only the Blinding Light ability adds
    past the cap, so a player sees this only under Blinding Light. Plain blindness prints
    `You are blind.` with a full stop. [C] LJNUTIL.C:8834-8837
  - In a visible but dark room (light -150 to -1) the display ends with `The room is barely
    visible` or `The room is dimly lit` in grey (no full stop). [C] LJNDISP.C:2756-2771
- **Exits** (display_visible_exits, LJNDISP.C:1241):
  - Directions are written out (`north` .. `southwest`, `up`, `down`).
  - Doors: `closed door north` / `open door north`. Key exits use the same words.
  - Gates: `closed gate west` / `open gate west`.
  - **Up and down doors are `closed trap door above` / `open trap door below`**, so the word
    is above/below, not up/down.
  - A found hidden exit shows its own WCCMSG text when it has one, otherwise `Secret passage
    north` (capital S only for north; `secret passage East` has a capital E, sic).
  - No exits: `Obvious exits: NONE!!!`.
  - Remote Action, unfound Hidden and Text exits are never listed. A **Trap** exit is listed
    by its direction name while its second parameter is not 2, and a **Timed** exit while its
    third parameter is 0 (timer not running). A Hidden exit shows once searched or blocked
    (`EXF_VISSEARCHED`/`EXF_VISBLOCKED`). [C] LJNDISP.C:1175-1211
  - Plain direction names are `dirnames[]`, whose second byte the slow cycle replaces with a
    random capital letter followed by a backspace (`nJ\borth`). Strip `X\b` pairs before
    matching. [C] LJNBKGND.C:640-644
  - `patterns.parse_exits()` returns `(dir, kind, state)` for each exit.
- **You notice.** Coins are listed first: runic under the sysop's name, then platinum, gold,
  silver and copper. Then come items with counts (`2 rope`). Distinct items appear in the
  **reverse** order of their first slot in the room: a new item takes the last free tally
  entry, and the tally is listed from entry 0. Stock room 1/2140 holds slot 0 = newbie
  manual, slot 1 = large sign, and the capture shows `You notice large sign, newbie manual
  here.` [C] LJNDISP.C:2116-2183
- **Also here.**
  - Players come first as Capitalised first names. They may have a `*` (outlaw star) and/or
    `(Hidden)` with no space before it.
  - Monsters follow as their instance names, lower case with an optional adjective
    (`fierce kobold thief`). They may have ` (Charmed)`, shown only to the player the charmed
    monster follows. [C] LJNDISP.C:2721-2724
  - Monster name colour gives its alignment. Good and Neutral are cyan (`ESC[0;36m`). Lawful
    Good is grey (`ESC[0;37m`) for viewers under 40 evil and the palette's COL_HERENAMES
    otherwise. Lawful Evil is the reverse. Evil, Chaotic Evil and Neutral Evil use
    COL_HERENAMES (`ESC[1;35m` in palette 0). Each name ends with `ESC[0m`. [C]
    LJNDISP.C:2683-2725
  - Named NPCs are capitalised too (`Champion Gudruk`, `Pharaoh Rastep` [F]), so use
    the monster list, not capitalisation, to tell them apart.
  - Plain-text forum captures show two spaces after the colon, so accept `\s+`.
  - `patterns.parse_also_here()` returns `(name, is_player, flags)` for each entry.
- **Noise.** `You hear movement to the <dir>.` / `above you!` / `below you!` (magenta) reaches
  the rooms beyond the entered room's other Normal, Item, Toll, Door, Trap and Gate exits.
  It comes from display_entry_movement; display_movement never prints, because its test
  `!vplr->status&ST_SNEAKING` is always 0. [C] LJNDISP.C:2250-2287, :2340
- **LOOK** with no argument runs `can_see` first. A hidden looker is not announced; otherwise
  the room gets `<name> is looking around the room.` [C] LJNCMDA.C:988-1001
- **Enter on an empty line** redraws the short room only for status line FULL or mode 4
  (FULLCUSTOM). [C] LJNPEND.C:203
- **The title can be overwritten.** A noise line ("You hear movement to the north.") printed
  between CLRLINE and the title is not the problem here. The problem is the glued prompt
  (section 1). Peel prompts before looking for the title.

## 4. Patterns generated from the game data (WCCMSG)

These texts are not in the C source. They are WCCMSG records that the C code formats with
`%s`/`%d`. `Registry.load_world()` turns every record into a regex.
- `%s` becomes a lazy group and `%d` a number.
- The groups are named after the arguments the game passes.
- The first letter matches either case, because the game upper-cases combat lines,
  LJNCOMBT.C:3666.
- A trailing `%s` is optional.

| Role (event) | Count | Example | Fields | Formatted at |
|---|---|---|---|---|
| `monster_arrives` | 236 | `A thin giant rat creeps into the room from nowhere.` | mon, frm | LJNMONST.C:863 |
| `monster_leaves` | 218 | `The giant rat creeps out of the room to the north.` | mon, to | arrivemsg text 2 |
| `monster_follows` | 211 | `The giant rat creeps in after you!` | (none) | arrivemsg text 3 |
| `monster_death` | 605 | `The giant rat falls to the ground with a tortured squeak.` | monsters (all names sharing the text) | LJNMONST.C:1095 (text 3 of deathmsg) |
| `mon_hits_you` | 238 | `The fierce kobold thief stabs you for 7 damage!` | mon, dmg | LJNCOMBT.C:3653 (a spellcasting monster's slot can name a spell message: the dark priest, priestess and gypsies name 17, whose text 1 "You cast %s on %s!" is the caster's own line, so "You cast bless on Aldric!" fits this role and is no blow [F]) |
| `mon_hits_other` | 239 | `The giant rat bites Thornwyn for 5 damage!` | mon, victim, dmg | same, text 2 |
| `mon_deflect_you` | 45 | `The kobold thief lunges at you, but your armour deflects the blow!` | mon, verb | LJNCOMBT.C:3746 |
| `mon_deflect_other` | 44 | `... lunges at Thornwyn, but her armour deflects the blow!` | mon, verb, victim, pron | |
| `mon_dodge_you` | 50 | `The lashworm lunges at you, but you dodge out of the way!` | mon, verb, weapon | LJNCOMBT.C:3793 (defmsg text 3) |
| `mon_dodge_other` | 47 | `The giant rat lunges at Thornwyn, but she dodges out of the way!` | mon, verb, victim, weapon, pron | (missmsg text 1) |
| `mon_miss_you` | 38 | `The kobold thief lunges at you with their shortsword!` | mon, verb, weapon | LJNCOMBT.C:3850 |
| `mon_miss_other` | 35 | `The acid slime flails at Thornwyn!` | mon, verb, victim, weapon | |
| `spell_cast_you` | 209 | `You cast vine strike at acid slime for 7 damage!` | spell, target, dmg | LJNSPELL.C:297 |
| `spell_cast_on_you` | 332 | `Acid burns you for 3 damage!` (a monster's hit spell) | caster, spell, dmg | |
| `spell_cast_room` | 561 | `Thornwyn casts vine strike on short kobold thief for 5 damage!` | caster, spell, target, dmg | |
| `item_destroyed` (+ `_other`) | 36 | `Your torch flickers and goes out.` | | LJNBKGND.C:1950 |
| `wccmsg` (everything else) | 3,299 | quest texts, room actions, potions (`You quaff a %s, and feel MUCH better!`), spell start/end, status effects (`You are entangled!`, `You are flat on your back!`, `You are bleeding all over the place!`), breath attacks | s1.., n1.., msgnum | various |

- **`a|b|c` alternatives in data texts.** A text that offers variants (weapon hit and miss verbs,
  many combat messages) is cut at `|`; at most **5** alternatives count (text after the fifth `|` can
  never be shown) and each shown alternative is cut at 39 characters (`MAXALTS` 5, `COMPSIZ` 40,
  `get_component_string`, LJNSUPP.C:30-31, :1743-1748). Build patterns from the first five parts, each
  at most 39 characters. [C]
- **Monster names.** `mon` is the instance name, with any adjective. `Registry._fix_monster()`
  re-splits name and verb on the known monster names, because `The %s %slunges at` leaves the
  boundary ambiguous. It also returns `base` and `adj`.
- **Death texts.** These are literal: they name the monster *kind* (from the record), not the
  instance. Many records share one text, so `monster_death.fields["monsters"]` lists every
  candidate.
- **Counting kills.** Count `You gain N experience.` instead. [C]
- **Monster criticals** are their own WCCMSG texts (`The Champion Gudruk all-out chops you for
  92 damage!`, 68 records). They are not "critically". [C, F]
- **Player weapon verbs** come from each item's `hitmsg`/`missmsg` records: a `|` list in
  first-person, third-person and room forms. That is 41 hit verbs (`smack`, `whap`, `beat`,
  `hack`, `chop`, `cut`, ... `hurl your chakram and strike`) and 18 miss verbs (`swipe at`,
  `swing at`, `lunge at`, ...).
  - Built-in verbs: punch, kick, jumpkick (Martial Arts) and attack / swing at (no messages).
  - A backstab gives `surprise <verb>` and a critical hit gives `critically <verb>`
    (LJNCOMBT.C:598, 2918).
  - **`You swipe at giant rat!` is the miss message itself** (set_hit_msg_formats,
    LJNCOMBT.C:1547). It is not a "swing announce" before a hit, as the forum write-up
    suggests.
- **Monster attack messages with no WCCMSG record** fall back to the built-in texts:
  - `X <verb> you for N damage!`
  - `X's <verb> hits you, but your armour deflects.`
  - `X <verb> you with <weapon>, but you dodge!`
  - `X <verb> you with <weapon>.`

### Social actions (WCCACTS, `action_*`)

Social actions are the other data-driven family: 64 verbs in the stock `WCCACTS`, each with a target mask and eleven texts (LJNACTS.C:34-44). Patterns are generated per verb from the texts the mask uses. [C]

| Role | Text index | Example | Fields | Formatted at |
|---|---|---|---|---|
| `action_self_solo` | 0 | `You smile happily!` | (none) | LJNACTS.C:535 |
| `action_room_solo` | 1 | `Alice smiles happily!` | actor | :541-544 (args: actor, his/her) |
| `action_self_user` | 2 | `You smile happily at Bob!` | target | :396 |
| `action_to_target` | 3 | `Alice smiles happily at you!` | actor | :407-411 (sent to the target only, after a `ESC[79D ESC[K`) |
| `action_room_user` | 4 | `Alice smiles happily at Bob!` | actor, target | :420-424 |
| `action_self_mon` / `action_room_mon` | 5, 6 | `You hug giant rat!` | (actor,) monster name | :440, :447 |
| `action_self_inv` / `action_room_inv` | 7, 8 | `You kiss your torch!` / `Alice kisses her torch!` | (actor, his/her,) item | :496, :503 |
| `action_self_floor` / `action_room_floor` | 9, 10 | `You kiss the stone!` | (actor,) item | :467, :475 |

- Matching rules. [C]
  - The verb is the whole first word, any case (`find_mud_action`, :209-224); there are no abbreviations.
  - A line no command, exit, room script or spell name claimed reaches the verbs (LJNPEND.C), and speech comes after them. A verb that is refused (target not allowed, several matches, hidden or sneaking target) is spoken instead: with FASTTALK off that is `Your command had no effect.`
  - The name after the verb is searched only for the kinds the mask allows (monsters with trailing words dropped, the first word for players, items carried, items on the floor). Several matches discard the list and refuse the action silently (:344-348); no `Please be more specific` is shown.
  - A player's own name counts as a target (:351-364); a hidden player is not found, a sneaking one is found and then refused (:377-384).
  - Acting clears `hidden` and `SNEAKING` on the actor.
- Text fill. [C]
  - `%p` is expanded first (his/her/its by `plr->sex`), then each `%s` takes the next name; a surplus `%s` prints nothing.
  - Names supplied: solo room (actor, his/her); user self (target); to target (actor); user room (actor, target); monster room (actor, monster); inventory room (actor, his/her, item); floor room (actor, item).
  - Stock quirk: `shake` text 8 reads `%s shakes %p head at %s.`, so the item never prints and the second `%s` takes the pronoun: `Alice shakes her head at her.`
- Colour. The actor's line has `clrtbl[COL_ACTION][palette]` and no `ESC[79D ESC[K`; the target's line has the clear first and then the colour; the room's lines have the colour first (from `tell_room`) and then the clear. Row `COL_ACTION` is the same green in all four palettes (LJNGLOB2.C:109). Every line is followed by the listener's prompt.
- `ACTION` and `help actions` print every verb followed by one space, in WCCACTS physical record order (`slobtv`/`snxbtv` are Btrieve Step First/Next, BTVSTF.H:90-91, LJNACTS.C:115-150; stock `hug kiss wave grin bow bleed nod laugh shake ...`, not alphabetical), word-wrapped at 79 columns (LISTWIDTH, LJNACTS.C:177-189).
- `set actions off` / `ACTION OFF` (`PS_NOACTIONS`) makes the whole stage return 0 (:307). Players on the original who hear the room's line are those not blind (`TRB_SIGHT`).

## 5. Static patterns by category

Regexes are matched against a whole line (stripped of trailing blanks). `<hit verbs>` and
`<miss verbs>` are filled in from the item data at load time. Table rows (marked "row after
...") are only tried on the lines that follow their opener.

### System, BBS, menus, settings

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `bbs_banner` | Enter Username or enter "NEW" to create a new Account | `Enter Username or enter \"NEW\" to create a new Account` | MBBSEmu | [C] T4 |
| `bbs_username` | Username: | `Username:` | MBBSEmu | [C] T4 |
| `bbs_password` | Password: | `Password:` | MBBSEmu | [C] T2 |
| `bbs_masked` | ********* | `\*+` | MBBSEmu (password echo) | [C] T6 |
| `login_greeting` | Fantasy awaits you in MajorMUD v1.11p-WG! | `Fantasy awaits you in MajorMUD v(?P<ver>\S+)!` | WCCMMUD.MSG USRLOGIN | [C] T4 |
| `bbs_menu_intro` | Please select one of the following: | `Please select one of the following:` | MBBSEmu | [C] |
| `bbs_menu_item` | M ... MajorMUD | `(?P<key>\w) \.\.\. (?P<item>.+)` | MBBSEmu | [C] T5 |
| `bbs_menu_title` | Main Menu | `Main Menu` | MBBSEmu | [C] T5 |
| `bbs_menu_prompt` | Make your selection (X to exit): | `Make your selection \(X to exit\):` | MBBSEmu | [C] T5 |
| `bbs_more` | (N)onstop, (Q)uit, or (C)ontinue? | `.*\((?:N)\)onstop, \((?:Q)\)uit, or \((?:C)\)ontinue\?.*` | MajorBBS paging (the BBS kernel's file viewer; not in the MajorMUD source, which pages with `page_pause` below. `WCCMMUD.RLN:2383` says a viewer can press N for nonstop viewing, which fits but does not give the prompt text) | [I] |
| `page_pause` | Hit any key to continue... | `Hit any key to continue\.\.\.` | LJNHELP.C:129, LJNDISP.C:3696, :5406, LJNGAME.C:3625-3639, :4039, LJNCMDB.C:2374 (kernel page mode, 22-line pages; MBBSEmu is reported to ignore `btuxnf`, so it never appears there; not checked against MBBSEmu's source) | [C] source, [I] under MBBSEmu |
| `mud_title` | M A J O R  M U D v1.11p-WG (Dec 30 2005 14:20:05) | `M A J O R  M U D v(?P<ver>\S+) \((?P<built>.+)\)` | WCCTEXT.MSG MENUTEXT | [C] T4 |
| `mud_world` | {Realm Of Legends} | `\{(?P<world>[^}]+)\}` | WCCTEXT.MSG MENUTEXT | [C] T4 |
| `mud_ansi_note` | *ANSI RECOMMENDED* | `\*ANSI RECOMMENDED\*` | WCCTEXT.MSG MENUTEXT | [C] T4 |
| `mud_menu_item` | [E] . Enter the Realm | `\[(?P<key>[A-Z]+)\] \. (?P<item>.+?)(?: \[(?P<left>\d+) game minutes left\]\| \[Last chance\])?` | WCCTEXT.MSG MENUTEXT; LJNPRMPT.C:110 | [C] T41 |
| `mud_menu_prompt` | [MAJORMUD]: | `\[MAJORMUD\]:` | WCCTEXT.MSG MENUPMPT | [C] T6 |
| `tournament_notice` | *** MajorMUD Tournament Play enabled! *** | `\*\*\* MajorMUD Tournament Play enabled! \*\*\*` | LJNPRMPT.C:137; LJNGAME.C:1620, :3982 (logon and entering the realm) | [C] |
| `enter_hangup_note` | Last time you were on, you disconnected while playing. | `Last time you were on, you disconnected while playing\.` | LJNUTIL.C:752 | [C] T1 |
| `enter_hangup_punished` | The gods have punished you appropriately. | `The gods have punished you appropriately\.` | LJNUTIL.C:753 | [C] T1 |
| `enter_banner` | MajorMUD - ROL | `MajorMUD - ROL` | WCCMMUD.MSG USRFOLED | [C] T3 |
| `enter_rule` | =-=-=-=-=-... | `(?:=-)+=?` | WCCMMUD.MSG USRFOLED | [C] T3 |
| `suicide_pw_notice` | SET SUICIDE command. | `(?:To prevent accidental suicide or reroll, these commands\|have been password protected\.  You have not yet entered\|your suicide password, so please do so soon using the\|SET SUICIDE command\.)` | LJNGAME.C:2313 | [C] T24 |
| `bbs_logoff_confirm` | Are you sure (Y/N, or R to re-logon)? | `Are you sure \(Y/N, or R to re-logon\)\?` | MBBSEmu (X from the BBS menu) | [C] T1 |
| `name_validating` | Please wait - Validating your name. | `(?:Please wait - Validating your name\|Validating your name, please wait)\.` | LJNGAME.C:2401,3186 | [C] T4 |
| `game_full` | We're sorry, but the game is currently full. | `We're sorry, but the game is currently full\.` | LJNGAME.C:1562 | [C] |
| `out_of_credits` | You have run out of credits for Major Mud. | `You have run out of credits for Major Mud\.` | LJNBKGND.C:1734 | [C] |
| `player_entered` | Bob just entered the Realm. | `(?P<who>.+) just entered the Realm\.` | LJNGAME.C:1582 | [C] T1 |
| `player_left_realm` | Bob just left the Realm. | `(?P<who>.+) just left the Realm\.` | LJNCMDA.C:4444 | [C] |
| `player_disconnected` | Bob just disconnected!!! | `(?P<who>.+) just disconnected!!!` | LJNGAME.C:5444-5446 | [C] F |
| `player_hungup` | Bob just hung up!!! | `(?P<who>.+) just hung up!!!` | not in 1.11p: the hang-up line is `player_disconnected` (`%s just disconnected!!!`, bright white, to every player without `PF_NOENTRYMSG`, LJNGAME.C:5443-5446) | [C] absent |
| `bbs_logoff_broadcast` | >> Witch1 has just logged off, 28 users on-line. | `>> (?P<who>\S+) has just logged off, (?P<n>\d+) users on-line\.` | MajorBBS (BBS kernel text; no `logged off` string in the MajorMUD source or its `.MSG` files, so it stays a forum capture) | [F] F |
| `bbs_logoff_self` | [Bob] logs OFF - Bye bye now! | `\[(?P<who>[^\]]+)\] logs OFF - Bye bye now!` | MajorBBS (BBS kernel text; not in the MajorMUD source or `.MSG` files) | [F] F |
| `exit_saved` | Your character has been saved. ... | `Your character has been saved\. If you have any comments or suggestions, please` | WCCMMUD.MSG USREXIT | [C] T2 |
| `exit_saved2` | leave them in E-mail to Sysop. Thanks. | `leave them in E-mail to Sysop\. Thanks\.` | WCCMMUD.MSG USREXIT | [C] T2 |
| `inactive_hangup` | You have been inactive for too long.   Bye! | `You have been inactive for too long\.\s+Bye!` | WCCMMUD.MSG INACHUP | [C] |
| `exit_meditate` | You will exit after a period of silent meditation. | `You will exit after a period of silent meditation\.\.?` | LJNCMDB.C:714 | [C] T3 |
| `exit_others` | Bob sits down and meditates. | `(?P<who>.+) sits down and meditates\.` | LJNCMDB.C:735 | [C] F |
| `exit_interrupted` | Your meditation has been interrupted - you may not exit now! | `\.*Your meditation has been interrupted - you may not exit now!` | LJNUTIL.C:9935 | [C] T1 |
| `exit_wounded` | You may not exit MajorMud while you are mortally wounded! | `You may not exit MajorMud while you are mortally wounded!` | LJNCMDB.C:700 | [C] |
| `exit_waiting` | You may not perform any commands while waiting to exit! | `You may not perform any commands while waiting to exit!` | LJNPEND.C:160 | [C] |
| `too_fast` | You are typing too quickly - command ignored | `You are typing too quickly - command ignored` | LJNPEND.C:792 (no clear-line prefix; sent for the 14th line typed, after `slow_down`) | [C] |
| `slow_down` | Why don't you slow down for a few seconds? | `Why don't you slow down for a few seconds\?` | LJNPEND.C:788 (no clear-line prefix; for the 10th line typed and each one after) | [C] |
| `command_interrupted` | Command interrupted. | `Command interrupted\.` | LJNGAME.C:4916 (`X` typed during a paged list such as TOPTEN or the gang list; no clear-line prefix, no prompt; the old cite :4897 was stale) | [C] |
| `quiet_set` | Quiet mode set | `Quiet mode set` | LJNCMDS.C:589 | [C] T2 |
| `verbose_set` | Verbose mode set | `Verbose mode set` | LJNCMDS.C:600 | [C] |
| `version` | MajorMUD v 1.11p | `MajorMUD v (?P<ver>\S+)` | LJNCMDS.C:611 | [C] |
| `ambiguous` | Please be more specific.  You could have meant any of these: | `Please be more specific\.  You could have meant any of these:` | LJNMULT.C:85 | [C] F |
| `ambiguous_item` | -- short sword | `-- (?P<name>.+)` | LJNMULT.C:91 | [C] F |
| `settings_statline` | Statusline:         Full | `Statusline:\s+(?P<mode>.+)` | LJNDISP.C:4829 | [C] |
| `settings_line` | Display Mode:       Verbose | `(?P<key>Life for this CHAR\|Display Mode\|Broadcast Channel\|Talking speed\|Follow Mode\|Attack Interrupts\|Receive Items\|Warn on Evil\|Allow Telepaths\|Block Entrance Msg\|Allow Gossip\|Allow Auction\|View gang members\|Keep some exp\|Messages\|Paging\|Talking Responses\|Style\|Colour Palette):?\s+(?P<val>.+)` | LJNDISP.C:4824 | [C] |
| `statline_options` | Valid statline options: ... | `Valid statline options: ON, OFF, FULL, CUSTOM xxx, FULL CUSTOM xxx` | LJNCMDA.C:6341 | [C] |
| `statline_syntax` | Syntax: STATLINE {ON/OFF/FULL} | `Syntax: STATLINE \{ON/OFF/FULL\}` | LJNCMDS.C:689 | [C] |
| `set_list` | Valid SET options: SUICIDE, MAXPAGES, ... | `Valid SET options: .+` | LJNCMDA.C:6453 (`SET` alone; the list omits STYLE, GOSSIP, AUCTION, RESPONSES, GANG, MESSAGES, GENDER and STATLINE, which the command also takes) | [C] |
| `set_messages` | Only important messages will be displayed now. | `Only important messages will be displayed now\.\|All messages will now be displayed\.\|Only some messages will be displayed now\.` | LJNCMDA.C:5929-5940, :6167-6177 (`SET MESSAGES` alone cycles High, Low, Medium, High) | [C] |
| `set_talk` | Anything you type which is not recognized will be sent to the room. | `Anything you type which is not recognized will be sent to the room\.\|To talk to the room, you must begin your line with a (?P<c>.)` | LJNCMDA.C:6370-6377 (`SET TALK SLOW` names the sysop's talk character) | [C] |
| `set_paging` | Paging is now set to Ok. | `Paging is now set to (?P<state>Ok\|Off)\.?` | LJNCMDA.C:5897, :5902, :6314, :6319 (`SET PAGE`, with a full stop, does not save); LJNCMDS.C:869 (the `PAGE` command, no full stop, saves the character) | [C] |
| `set_max_other` | Max gossips set to 3 | `Max (?P<what>gossips\|auctions) set to (?P<n>\d+)` | LJNCMDA.C:5975, :5985 (`Maxpages set to N` is `maxpages_set`; values are `atoi`, capped at 255, stored in an unsigned byte, so `-1` stores 255) | [C] |
| `gender_free` | You are now female! | `You are now (?P<sex>male\|female)!` | LJNCMDA.C:6011, :6026 (the paid form is `hand_over_generic`; a negative sysop GENDERGO gives `You may not do that here!`; the price is checked before the word) | [C] |
| `set_confirm` | You will now see game broadcasts such as user entrances and exits. | `You will (?:now\|no longer) .+\.` | LJNCMDA.C:5738ff | [C] F |
| `set_options` | Valid follow options: BLIND, NORMAL | `Valid (?:\w+ )?(?:options\|palettes)(?: are)?:? .+` | LJNCMDA.C:6031ff | [C] |
| `syntax` | Syntax: AID {user name} | `Syntax: (?P<syntax>.+)` | many | [C] F |
| `help_topics` | The following help topics are currently available: | `The following help topics are currently available:` | LJNHELP.C:154 | [C] |
| `help_none` | Sorry, no help is available for that topic! | `Sorry, no help is available for that topic!` | LJNHELP.C:346 | [C] |
| `chargen_cp` | You have 12 CP to distribute. | `You have (?P<cp>\d+) CP to distribute\.` | LJNGAME.C:1776 (line-mode form only. On an ANSI terminal of at least 80x23 (`ANSON && scnfse >= 23 && scnwid >= 80`, LJNGAME.C:1768, :2474) the form instead writes `ESC[20;33f` and `%4d` of the CP left, redrawn only after the family name and after an accepted stat, not after the given name or the looks, :1772-1773, :2086. The CP cap is checked against the lifetime `totalcp`, not `cp`, :1946-2071. `TRAIN STATS` locks the first name, so its form starts at the family name, :2593-2595. Total stat bounds 10 and 250, :305. `Your family name can only be 18 characters long!` cannot be reached on the form: blanks are stripped first and the field is 18 wide, :1904-1905) | [C] |
| `chargen_limit` | Health may not be lower than 40. | `(?P<stat>Strength\|Intellect\|Willpower\|Agility\|Health\|Charm) may not be (?P<dir>lower\|higher) than (?P<n>\d+)\.(?:\s+.*)?` | LJNGAME.C:1938 | [C] T8 |
| `chargen_toomuch` | You may not assign that much to Strength | `You may not assign that much to (?P<stat>\w+)` | LJNGAME.C:1948 | [C] |
| `chargen_name` | You must enter a last name! | `(?:You must enter a last name!\|You have entered an invalid name!\|Your given name can only be 10 characters long!\|Only ALPHA characters are allowed in a name!\|Your family name can only be 18 characters long!)` | LJNGAME.C:1851 | [C] T6 |
| `chargen_values` | 40   50   none   black | `(?:\d+\|none\|black\|white\|silver\|red\|brown\|blonde\|green\|blue\|grey\|SAVE\|EXIT\|short\|long\|[\s_\\/\|.])+` | WCCTEXT.MSG EDITCHA1 (form fields) | [C] T52 F |
| `chargen_box` | │ » Strength ($$$$ to $$$$) $$$$ « │ | `.*[\u2500-\u257f\u2580-\u259f].*` | WCCTEXT.MSG EDITCHA1 | [C] T192 |
| `chargen_list_row` | [1]  Human | `\[(?P<n>\d+)\]\s+(?P<name>.+)` | LJNDISP.C:710 | [C] T56 |
| `chargen_choose` | Please choose your race [ ? for help ] : | `(?:Please choose (?:a\|your) (?P<what>race\|class)(?: from the following list:\| \[ \? for help \] :)\|You must choose a (?:valid )?(?:race\|class)\. \[ \? for help \])` | WCCTEXT.MSG CHSRACE/CHSCLASS; LJNGAME.C:3886,4573 | [C] T8 |
| `chargen_lawful` | Do you want to be Lawful?  [Yes/No] | `Do you want to be Lawful\?\s+\[Yes/No\]` | WCCTEXT.MSG GOODPMPT | [C] T2 |
| `menu_invalid_option` | Invalid Option. | `Invalid Option\.` | LJNGAME.C:4425 (main menu, unknown word; then the `[MAJORMUD]: ` prompt) | [C] |
| `lawful_reply_bad` | You must respond with a Y or an N. | `You must respond with a Y or an N\.` | LJNGAME.C:4748 (character creation) | [C] |
| `name_taken` | \bYou may not use Aldric as your name. | `You may not use (?P<name>\S+) as your name\.` | LJNGAME.C:1473, :1518 (leading backspace; then `Please enter a new name: `) | [C] |
| `name_invalid_menu` | The name you have chosen is invalid! | `The name you have chosen is invalid!` | LJNGAME.C:2244, :3255 (light red; the editor close adds `You will have to start over again.` / `Press Enter.`, the new-name prompt adds `Please try another one: `) | [C] |
| `name_is_command` | The name you have chosen is already in use! | `The name you have chosen is already in use!` | LJNGAME.C:2264 (light red, then `You will have to start over again.` / `Press Enter.`; a first name that is a game command) | [C] |
| `name_not_allowed` | You may not use that name. | `You may not use that name\.` | LJNGAME.C:3240 (at the new-name prompt) | [C] |
| `suicide_pw_current` | Enter the current password: | `Enter the current password:` | LJNCMDA.C:5702 (`SET SUICIDE` with a password set) | [C] |
| `suicide_pw_new` | Enter New Password: | `Enter New Password:` | LJNCMDA.C:5709, LJNGAME.C:4857 | [C] |
| `suicide_pw_unchanged` | Password NOT changed | `Password NOT changed` | LJNGAME.C:4849, :4877 (an empty answer) | [C] |
| `suicide_pw_wrong` | Invalid password! | `Invalid password!` | LJNGAME.C:4864 (wrong current password; `Invalid password specified.` is the SUICIDE one) | [C] |
| `suicide_pw_toolong` | The password may not be longer than 8 characters. | `The password may not be longer than 8 characters\.` | LJNGAME.C:4883 | [C] |
| `suicide_pw_changed` | Password changed | `Password changed` | LJNGAME.C:4902 | [C] |
| `lightning_you` | A bolt of lightning from the heavens strikes you for 31 points damage! | `A bolt of lightning from the heavens strikes you for (?P<n>\d+) points damage!` | LJNCMDB.C:1954 (yellow; sysop `LIGHTNING`) | [C] |
| `lightning_room` | A bolt of lightning just struck Bob! | `A bolt of lightning just struck (?P<who>.+)!` | LJNCMDB.C:1960 | [C] |
| `sysop_lightning_refused` | You may not lightning that user. | `You may not lightning that user\.` | LJNCMDB.C:1987 | [C] |
| `sysop_goto_list` | Current GOTO locations: NEWHAVEN, SILVERMERE, SUPPORT, RHUDAUR, KHAZARAD, LOSTCITY | `Current GOTO locations: .+` | LJNCMDB.C:1895 (sysop `GOTO` with an unknown place word; wraps over two lines) | [C] |
| `sysop_report_stopped` | Sysop user report interrupted | `Sysop user report interrupted` | LJNGAME.C:4926 | [C] |
| `sysop_lance_reg` | Reg# 12345678 / Lcon 5 / Death Setting: -200 | `Reg# (?P<reg>\S+)\|Lcon (?P<n>\d+)\|Death Setting: (?P<d>-?\d+)` | LJNCMDB.C:1460ff (sysop `LANCE`, answered to any player) | [C] |
| `god_retrain_done` | The next time Bob logs into MajorMUD they will have to retrain their stats. | `The next time (?P<who>.+) logs into MajorMUD they will have to retrain their stats\.` | LJNCMDB.C:2398 | [C] |
| `limited_removing` | Removing limited items from bob | `Removing limited items from (?P<id>\S+)` | LJNUPDT.C:2959 (sysop cross-reference only) | [C] |
| `bbs_signup` | Please enter a unique Username (Max. 29 Characters): | `Please (?:enter\|re-enter\|select) .+\|NOTE: Passwords are stored .+\|The MajorBBS and Worldgroup have a maximum password size.+` | MBBSEmu (new account) | [C] T19 |
| `who_header` | Current Adventurers | `Current Adventurers` | LJNDISP.C:4342 | [C] F |
| `who_rule` | =================== | `=+` | LJNDISP.C:4344 | [C] F |
| `who_count` | There are 3 users in the game. | `There are (?P<n>\d+) users in the game\.` | LJNDISP.C:4460 (the sysop scan only, `mode` 1 or 2; a player's WHO ends with a blank line) | [C] |
| `who_none` | There are no users in the game at the moment. | `There are no users in the game at the moment\.` | LJNDISP.C:4455 | [C] |
| `topten_header` | Top Heroes of the Realm | `(?:Top )?Heroes of (?:T\|t)he Realm\|Top Gangs of the Realm` | LJNDISP.C:182 | [C] |
| `who_row` (row after `who_rule`) | Good Nester TheDupe   -  Monk  of Old Guard | `(?:(?P<align>[A-Z][A-Za-z]+) )?(?P<name>[A-Z]\w*)(?: (?P<last>\S+))?\s+-\s+(?P<title>.+?)(?:\s+of (?P<gang>.+))?` | LJNDISP.C:4358,4383 | [C] F |

**`SET` quirks** [C LJNCMDA.C:5676-6460]:
- A one-word `SET <something>` that is no option returns 0, so the line goes on to the room's exits and
  then to speech; two or more words with an unknown option are ignored without a word (:5941, :5946).
  `SET P OK` is a bad palette, not PAGE, because PALETTE is tested first (:6262, :6310).
- `SET` never saves the character (only a custom statline does, through `save_permanent_info`, :6423, :6444);
  the separate `PAGE` command saves (see `set_paging`).
- Wrong-value texts keep the original's slips: `SET RECEIVE <bad>` and `SET RESPONSES <bad>` both print
  `Valid interrupt options: ...` (:6202, :6159), and `SET STYLE TECHNICAL` answers
  `You will now receive techincal style messages.` (:6051; the one-word toggle spells it right, :5743).
- `SET STATLINE ON|OFF|FULL` prints nothing (:6328); `SET STATLINE FULL CUSTOM <text>` needs five words and
  with fewer is ignored silently (:6393); `STATLINE` on its own is `statline_syntax`.
- PROFILE: `Keep some exp:      No(Disabled on this system)` has no space before the note, and
  `Life for this CHAR` is `sesstime/2 + lifemins` (LJNDISP.C:4862-4867, :4824).

### Room display, looking, searching

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `room_exits` | Obvious exits: closed door north, up | `Obvious exits: (?P<exits>.+)` | LJNDISP.C:1261 | [C] T3409 F |
| `room_items` | You notice 9 copper farthings, torch here. | `You notice (?P<items>.+) here\.` | LJNDISP.C:1866,2192 | [C] T323 F |
| `room_also_here` | Also here: Thornwyn, giant rat. | `Also here:\s+(?P<who>.+)\.` | LJNDISP.C:2565,2671 | [C] T934 F |
| `room_light` | The room is dimly lit | `The room is (?P<light>barely visible\|dimly lit)` | LJNDISP.C:2760; LJNUTIL.C:8790 | [C] T222 F |
| `room_dark` | The room is very dark - you can't see anything | `The room is (?P<light>pitch black\|very dark) - you can't see anything` | LJNSUPP.C:480 | [C] T23 |
| `blind` | You are blind. | `You are blind[.!]` | LJNSUPP.C:472, LJNUTIL.C:8791 | [C] |
| `arena_warning` | NOTE: This room is *not* a death-free zone right now! | `(?:NOTE: This room is \*not\* a death-free zone right now!\|Treat this room as you would any other combat zone\.)` | LJNDISP.C:2536 | [C] |
| `noise` | You hear movement to the north. | `You hear movement (?:to the (?P<dir>north\|south\|east\|west\|northeast\|northwest\|southeast\|southwest\|up\|down)\|above you\|below you\|above\|below)[.!]` | LJNDISP.C:2262 | [C] T171 F |
| `hiding_seen` | You see Bob hiding in the shadows. | `You see (?P<who>.+) hiding in the shadows\.` | LJNDISP.C:2401 | [C] |
| `search_nothing` | Your search revealed nothing. | `Your search revealed nothing\.` | LJNDISP.C:2206 | [C] |
| `search_found_exit` | You found an exit to the west! | `You found an exit (?:upwards\|downwards\|to the (?P<dir>\w+))!` | LJNUTIL.C:9984 | [C] |
| `search_found_trap` | You found a trap to the north! | `You found a trap (?:above you\|below you\|to the (?P<dir>\w+))!` | LJNUTIL.C:10025 | [C] |
| `search_nothing_dir` | You notice nothing different to the north. | `You notice nothing different (?:above you\|below you\|to the (?P<dir>\w+))\.` | LJNUTIL.C:9997 | [C] |
| `search_others` | Bob is searching the area. | `(?P<who>.+) is searching (?:the area\|for exits)\.` | LJNCMDA.C:3239 | [C] |
| `concealed_opens` | A concealed passage opens to the west. | `A concealed passage opens to the (?P<dir>\w+)\.` | LJNSUPP.C:1399 | [C] |
| `look_others` | Bob is looking around the room. | `(?P<who>.+) is looking (?:around the room\|up\|down\|to the (?P<dir>\w+)\|at you)\.` | LJNCMDA.C:999 | [C] F |
| `peek_in` | Bob peeks in from the north! | `(?P<who>.+) peeks in from (?:above\|below\|the (?P<dir>\w+))!` | LJNCMDA.C:1161 | [C] F |
| `look_nothing` | Why would you want to look at that? | `Why would you want to look at that\?` | LJNCMDA.C:1318 | [C] |
| `look_no_exit` | There are no exits to the north! | `There are no exits (?:upwards\|downwards\|to the (?P<dir>\w+))!` | LJNCMDA.C:1067 | [C] |
| `look_door_closed` | The door is closed in that direction! | `The door is closed in that direction!` | LJNCMDA.C:1048 | [C] |
| `look_monster` | The giant rat appears to be unwounded. | `(?P<who>.+) appears to be (?P<state>.+)\.` | LJNDISP.C:3639 | [C] F |
| `look_poisoned` | Bob is poisoned! | `(?P<who>.+) is poisoned!` | LJNDISP.C:3641 | [C] |
| `look_equipped` | He is equipped with: | `(?P<who>He\|She\|It) is equipped with:` | LJNDISP.C:3408 | [C] F |
| `look_player` | Bob is a Human Warrior, ... | `(?P<who>[A-Z][A-Za-z]+) is a (?P<desc>.+?, .+)` | LJNDISP.C:3370 | [C] F |
| `look_player_header` | [ Nester TheDupe ] (Old Guard) | `\[ (?P<name>.+?) \](?: \((?P<gang>.+)\))?` | LJNDISP.C:3338-3347 | [C] F |
| `look_up_down` | Bob looks Ann up and down. | `(?P<who>.+) looks (?P<target>.+) up and down\.` | LJNCMDA.C:1278 | [C] |
| `coin_desc` | The silver nobles glitter with use. | `The (?:copper farthings look like they've been around forever\|silver nobles glitter with use\|gold crowns are rustic and used\|platinum pieces shine as though they were new)\.` | LJNTABLE.C:284 | [C] |
| `currency_rates` | 10 copper farthings == 1 silver noble | `(?:The currency conversion rates are:\|(?P<n>\d+) (?P<small>.+) == 1 (?P<big>.+))` | LJNDISP.C:2790 | [C] |
| `equip_row` (row after `look_equipped`) | padded vest                    (Torso) | `(?P<item>.+?)\s+\((?P<slot>[^)]+)\)` | LJNDISP.C:3424-3476 | [C] F |

### Movement results, refusals, arrivals

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `no_exit` | There is no exit in that direction! | `There is no exit in that direction!` | LJNUTIL.C:3292 | [C] T18 |
| `door_closed` | The gate is closed! | `The (?P<what>door\|gate) is (?:C\|c)losed!` | LJNUTIL.C:3175-3196 | [C] T148; capital C for lock state 1 (shut, unlocked), lower case for state 2 (locked, the stock state) |
| `door_closed_drag` | There is a closed door in that direction! | `There is a closed door in that direction!` | LJNUTIL.C:2975 | [C] |
| `need_spell` | You need to cast a spell to go that way! | `You need to cast a spell to go that way!` | LJNUTIL.C:2957 | [C] |
| `need_item` | You do not have the appropriate item to go that direction! | `You do not have the appropriate item to go that direction!` | LJNUTIL.C:3072 | [C] |
| `toll_paid` | You just paid 5 gold crowns in toll charges. | `You just paid (?P<paid>.+) in toll charges\.` | LJNUTIL.C:3094 | [C] |
| `toll_short` | You do not have enough to cover the toll of 5 gold crowns. | `You do not have enough to cover the toll of (?P<n>\d+) (?P<cur>.+)\.` | LJNUTIL.C:3101 | [C] |
| `not_permitted` | You are not permitted in that room! | `You are not permitted in that room!` | LJNUTIL.C:3117 | [C] |
| `exit_forbidden` | You may not go through this exit! | `You may not go through this exit(?: during tournament play)?!` | LJNUTIL.C:3479-3531 | [C] |
| `exit_level_low` | You have not progressed far enough to go through this exit! | `You have not progressed far enough to go through this exit!` | LJNUTIL.C:3558 | [C] |
| `exit_level_high` | You have progressed too far for this room. | `You have progressed too far to go through this exit!\|You have progressed too far for this room\.` | LJNUTIL.C:3566,2520 | [C] |
| `exit_alignment` | You are too evil to go through this exit! | `You are too (?P<align>good\|evil) to go through this exit!` | LJNUTIL.C:3595 | [C] |
| `exit_combat` | You may not enter that room while in combat. | `You may not enter that room (?:while in combat\|during a retaliation time-period)\.` | LJNUTIL.C:2578 | [C] |
| `exit_unhealthy` | You are not healthy enough to enter that room. | `You are not healthy enough to enter that room\.` | LJNUTIL.C:2606 | [C] |
| `exit_timed` | You may not pass through that exit at this point in time. | `You may not pass through that exit at this point in time\.` | LJNUTIL.C:3644 | [C] |
| `exit_purchase` | Your sysop must purchase the ... before you may move through this exit. | `(?:Your sysop must purchase\|You must purchase) the .+ before you may move through this exit\.` | LJNUTIL.C:3622 | [C] |
| `cant_move` | You can't seem to move anywhere! | `You can't seem to move anywhere(?: to hide)?!` | LJNUTIL.C:2792 | [C] |
| `stunned` | You are too stunned to move anywhere! | `You are too stunned to move anywhere(?: to hide)?!` | LJNUTIL.C:2800 | [C] |
| `too_heavy` | You are too heavy to move! | `You are too heavy to (?:move(?: anywhere)?\|be dragged)!` | LJNUTIL.C:2846 | [C] |
| `ran_into_wall` | Bob ran into the wall to the north. | `(?P<who>.+) ran into the wall to the (?P<dir>\w+)\.` | LJNCMDB.C:5254 | [C] |
| `sneaking` | Sneaking... | `Sneaking\.\.\.` | LJNUTIL.C:2815 | [C] F |
| `sneak_attempt` | Attempting to sneak... | `Attempting to sneak\.\.\.(?P<rest>.*)` | LJNCMDA.C:2900; a failure the character notices prints `You don't think you're sneaking.` on the same line (:2921), an unnoticed one nothing more. Seen on silvermere: the line ends CR LF, then the prompt about 0.4 to 1 s later, when the command delay ends [C silvermere live, 2026-10-23] | [C] F |
| `sneak_fail` | You don't think you're sneaking. | `You don't think you're sneaking\.` | LJNCMDA.C:2921 | [C] |
| `sneak_not_now` | You may not sneak right now! | `You may not sneak right now!` | LJNCMDA.C:2880 | [C] |
| `sneak_off` | You are no longer sneaking. | `You are no longer sneaking\.` | LJNCMDB.C:5260 | [C] |
| `made_sound` | You make a sound as you enter the room! | `You make a sound as you enter the room!` | LJNUTIL.C:3806 | [C] |
| `sneak_seen_in` | You notice Bob sneak in from the west. | `You notice (?P<who>.+) sneak in from (?:above\|below\|the (?P<dir>\w+))(?P<x>.*)\.` | LJNUTIL.C:3829 | [C] F |
| `sneak_seen_out` | You notice Bob sneaking out to the north. | `You notice (?P<who>.+) sneaking out (?:upwards\|downwards\|to the (?P<dir>\w+))(?P<x>.*)\.` | LJNUTIL.C:3812 | [C] F |
| `player_arrives` | Bob walks into the room from the south. | `(?P<who>[A-Z][A-Za-z]+) walks into the room from (?:above\|below\|the (?P<dir>\w+))(?P<x>.*)\.` | LJNUTIL.C:3900 | [C] T1 F |
| `player_leaves` | Bob just left to the north. | `(?P<who>[A-Z][A-Za-z]+) just left (?:upwards\|downwards\|to the (?P<dir>\w+))(?P<x>.*)\.` | LJNUTIL.C:3863 | [C] F |
| `monster_arrives_generic` | goblin moves into the room from the east. | `(?P<mon>.+) (?:moves into the room\|just arrived\|moves into the) from (?:nowhere\|above\|below\|the (?P<dir>\w+))\.` | LJNMONST.C:894-908,1456-1486 | [C] T40 |
| `monster_leaves_generic` | happy guardsman just left to the east. | `(?P<mon>.+?) just left (?:upwards\|downwards\|to the (?P<dir>\w+))\.` | LJNMONST.C:1431-1439 | [C] T1 |
| `follow_leader` | -- Following your Party leader north -- | `-- Following your Party leader (?P<dir>.+) --` | LJNUTIL.C:3789 | [C] F; the line starts with a space (`" -- Following..."`), so match unanchored |
| `dragging` | You are now dragging Bob. | `(?:You are (?:now )?dragging (?P<who>.+)\.\|(?P<who2>.+) is dragging you around\.\|You are no longer dragging (?P<who3>.+?)\.?)` | LJNUTIL.C:3922 | [C] |
| `teleported` | You find yourself...elsewhere. | `You find yourself\.\.\. ?elsewhere\.` | WCCMSG 837 | [C] |

### Doors, gates, locks, traps

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `door_opened` | The door is now open. | `The (?P<what>.+) is now open\.` | LJNCMDA.C:3392,3466 | [C] |
| `door_already_open` | The door was already open. | `The (?P<what>.+) (?:was\|is) already open\.` | LJNCMDA.C:3348,689 | [C] |
| `door_locked` | The door is locked. | `The (?P<what>.+) is locked\.` | LJNCMDA.C:3398,3470 | [C] T7 |
| `not_a_door` | That is not a door or a gate! | `That is not a door or a gate!` | LJNCMDA.C:3476 | [C] |
| `door_closed_now` | The gate is now closed. | `The (?P<what>.+) is now closed\.` | LJNCMDA.C:3668 | [C] |
| `door_not_open` | That door is not open. Closing it will do nothing! | `That (?P<what>\w+) is not open\. Closing it will do nothing!` | LJNCMDA.C:3674 | [C] |
| `door_no_close_combat` | You may not close doors or gates while attacking or being attacked! | `You may not (?:close\|lock) doors or gates while attacking or being attacked!` | LJNCMDA.C:3611 | [C] |
| `door_moved_remote` | The door to the north just opened. | `The (?P<what>.+?) (?:to the (?P<dir>\w+)\|above you\|below you) just (?P<how>opened\|closed\|flew open\|locked)[.!]` | LJNCMDA.C:3375; LJNSUPP.C:1837; LJNEXITS.C:402 | [C] |
| `exit_moved_remote` | The exit to the north just opened! | `The exit to the (?P<dir>\w+) just (?P<how>opened\|closed)!` | LJNEXITS.C:283 | [C] |
| `door_others` | You see Bob bash the door to the north. | `You see (?P<who>.+) (?P<act>open\|close\|bash\|attempt to bash\|pick the lock on) the (?P<what>.+?) (?:to the (?P<dir>\w+)\|above you\|below you)\.` | LJNCMDA.C:704-757,3058,3384 | [C] |
| `door_locked_now` | The door is now locked. | `The (?P<what>door\|gate) is now locked\.` | LJNCMDA.C:4812 | [C] |
| `lock_need_close` | You must close the door before you may lock it. | `You must close the (?P<what>door\|gate) before you may lock it\.` | LJNCMDA.C:4818 | [C] |
| `lock_no_benefit` | There is no benefit to locking in that direction. | `There is no benefit to locking in that direction\.` | LJNCMDA.C:4873 | [C] |
| `bash_through` | You bash the door open and walk through | `You bash the door open and walk through` | LJNCMDA.C:660 | [C] |
| `door_slams` | The door slams shut behind you! | `The door slams shut behind you!` | LJNCMDA.C:666 | [C] |
| `bash_fail` | Your attempts to bash through fail! | `Your attempts to bash through fail!` | LJNCMDA.C:673 | [C] T10 |
| `must_wait` | You must wait before you may do that! | `You must wait before you may do that!` | LJNCMDA.C:680 | [C] |
| `bash_ok` | You bashed the gate open. | `You bashed the (?P<what>.+) open\.` | LJNCMDA.C:732 | [C] T6 |
| `bash_hurt` | You take 3 damage for bashing the door! | `You take (?P<dmg>\d+) damage for bashing the door!` | LJNCMDA.C:782 | [C] T2 |
| `bash_no_weapon` | You need a weapon to bash with! | `You need a weapon to bash with!` | LJNCMDA.C:797 | [C] |
| `cant_bash_smash` | You don't know the first thing about bashing! | `You don't know the first thing about (?P<what>bashing\|smashing)!` | LJNCMDA.C:565,803 | [C] |
| `unlocked` | You successfully unlocked the door. | `You successfully unlocked the (?P<what>.+)\.` | LJNCMDA.C:3079; LJNUSE.C:248 | [C] |
| `picklock_fail` | Your skill fails you this time. | `Your skill fails you this time\.` | LJNCMDA.C:3088 | [C] |
| `key_not_locked` | The door was not locked. | `The (?P<what>.+) was not locked\.` | LJNUSE.C:251 | [C] |
| `key_wrong` | The brass key doesn't seem to fit that lock. | `The (?P<what>.+) doesn't seem to fit that lock\.` | LJNUSE.C:295 | [C] |
| `trap_disarmed` | You successfully disarmed the trap to the north. | `You successfully disarmed the trap to the (?P<dir>\w+)\.` | LJNCMDB.C:5350 | [C] |
| `trap_disarm_fail` | You failed to disarm any trap to the north. | `You failed to disarm any trap to the (?P<dir>\w+)\.` | LJNCMDB.C:5397 | [C] |

### Combat

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `combat_engaged` | *Combat Engaged* | `\*Combat Engaged\*` | LJNCOMBT.C:1742; LJNSPELL.C:2112 | [C] T681 F |
| `combat_off` | *Combat Off* | `\*Combat Off\*` | LJNACMB.C:153 | [C] T679 F |
| `breaks_off` | Bob breaks off combat. | `(?P<who>.+) breaks off combat\.` | LJNACMB.C:161 | [C] |
| `attacks_you` | Bob moves to attack you! | `(?P<who>.+) moves to attack you!` | LJNCOMBT.C:1765; LJNSPELL.C:5352 | [C] F |
| `attacks_room` | Bob moves to attack everyone in the room. | `(?P<who>.+) moves to attack everyone in the room\.` | LJNSPELL.C:2115 | [C] F |
| `attacks_other` | Bob moves to attack giant rat. | `(?P<who>.+) moves to attack (?P<target>.+?)[.!]` | LJNCOMBT.C:1780,2603 | [C] T2 F |
| `protects` | ice golem moves to protect ice sorceress | `(?P<who>.+) moves to protect (?P<target>.+)` | LJNCOMBT.C:2515 | [C] F |
| `casts_upon` | Bob moves to cast mend upon Ann. | `(?P<who>.+) moves to cast (?P<spell>.+) upon (?P<target>.+)\.` | LJNSPELL.C:7428 | [C] T1 F |
| `you_hit` | You smack giant rat for 4 damage! | `You (?:(?P<crit>critically) )?(?:(?P<surprise>surprise) )?(?P<verb><hit verbs>) (?P<target>.+) for (?P<dmg>\d+) damage!` | LJNCOMBT.C:1533,2918 + item hitmsg (WCCMSG) | [C] T978 F |
| `you_miss` | You swipe at thin carrion beast! | `You (?P<verb><miss verbs>) (?P<target>.+?)!` | LJNCOMBT.C:1547,3120 + item missmsg | [C] T126 F |
| `you_dodged` | You swing at orc who dodges your attack! | `You (?P<verb><miss verbs>) (?P<target>.+) who dodges your attack!` | LJNCOMBT.C:1559,3105 | [C] F |
| `you_glance` | Your swing at Y hits, but glances off its armour. | `Your (?P<verb><miss verbs>) (?P<target>.+) hits, but glances off (?:its\|his\|her) armour[.!]` | LJNCOMBT.C:3093,2186 | [C] T4 F |
| `other_hits` | Thornwyn whaps giant rat for 4 damage! | `(?P<who>[A-Z][A-Za-z]+) (?:(?P<crit>critically) )?(?:(?P<surprise>surprise) )?(?P<verb><hit verbs 3rd>) (?P<target>.+) for (?P<dmg>\d+) damage!` | LJNCOMBT.C:1535,2926 (room view) | [C] T2 F |
| `other_miss` | Kasimir swings at short kobold thief! | `(?P<who>[A-Z][A-Za-z]+) (?P<verb><miss verbs 3rd>) (?P<target>.+?)!` | LJNCOMBT.C:1549 (room view of a miss) | [C] T1 F |
| `other_dodged` | Bob swings at orc but it dodges. | `(?P<who>[A-Z][A-Za-z]+) (?P<verb>.+) but it dodges\.` | LJNCOMBT.C:3115 | [C] F |
| `other_glance` | Bob's swing at orc hits, but glances off its armour. | `(?P<who>.+)'s (?P<verb>.+) hits, but glances off (?:its\|his\|her) armour\.` | LJNCOMBT.C:3098 | [C] F |
| `mon_deflect_builtin` | Orc's slash hits you, but your armour deflects. | `(?P<mon>.+)'s (?P<verb>.+) hits you, but your armour deflects[.!]` | LJNCOMBT.C:3768,2192 | [C] |
| `pvp_dodges` | Nester dodges your attack! | `(?P<who>.+) dodges your attack!` | LJNCOMBT.C:2209 | [C] F |
| `took_damage` | You took 3 damage. | `You took (?P<dmg>\d+) damage[.!]` | LJNCOMBT.C:1987,2867 (damage shield) | [C] |
| `smashed_it` | You smashed Bob to the ground! | `You smashed (?P<target>.+) to the ground!` | LJNCOMBT.C:2164 | [C] |
| `smashed_you` | You are smashed to the ground! | `You are smashed to the ground!` | LJNCOMBT.C:2168 | [C] |
| `smashed_down` | The orc is smashed to the floor defenseless! | `(?:The )?(?P<target>.+) is smashed to the (?:ground\|floor) defenseless!` | LJNCOMBT.C:2171,3073 | [C] F |
| `rises` | Slightly dazed the orc rises from the floor. | `Slightly dazed (?:you get back onto your feet\|(?:the )?(?P<who>.+) rises from the floor)\.` | LJNCOMBT.C:1884,2682 | [C] |
| `need_rest_attack` | You must rest before you may attack again. | `You must rest before you may attack again\.?` | LJNCOMBT.C:1916 | [C] |
| `unbalanced` | Such an attack would result in a very unbalanced combat round. | `Such an (?:attack\|action) would result in a very unbalanced (?:combat round\|game)\.` | LJNCOMBT.C:1720 | [C] |
| `no_effect_weapon` | Your weapon has no effect against this monster! | `Your (?:weapon\|(?P<what>.+)) (?:has\|have) no effect against this monster!` | LJNCOMBT.C:2570,2580 | [C] T1 F |
| `wrong_weapon` | You are not using the required weapon to hit this monster! | `You are not using the required (?:weapon\|spell) to hit this monster!` | LJNCOMBT.C:2466; LJNSPELL.C:7045 | [C] |
| `target_missing` | You do not see rat here! | `You do not see (?P<target>.+) here!\|You don't see your target here!` | LJNCOMBT.C:1635; LJNACMB.C:559 | [C] |
| `attack_self` | You cannot attack yourself! | `(?:You cannot attack yourself!\|Why would you want to attack yourself\?)` | LJNCMDA.C:297 | [C] |
| `too_afraid` | You are too afraid! | `You are too afraid!` | LJNCMDA.C:246 | [C] |
| `guilt` | You are overcome with a feeling of guilt and break off your attack. | `You are overcome with a feeling of guilt and (?:break off your attack\|return your hands to your own pockets)\.?` | LJNCMDA.C:313 | [C] |
| `evil_warning` | To do this action, you must turn off your evil warnings. | `To do this action, you must turn off your evil warnings\.` | LJNEVIL.C:304 | [C] T1 F |
| `too_evil` | You have progressed too far to the evil side to do this action. | `You have progressed too far to the evil side to do this action\.` | LJNEVIL.C:312 | [C] |
| `lawful_block` | You have chosen a way of life which prevents this action. | `You have chosen a way of life which (?:does not allow\|prevents) this action\.` | LJNEVIL.C:320 | [C] |
| `evil_gained` | A dark cloud passes over you | `A dark cloud passes over you` | LJNEVIL.C:327 | [C] F |
| `no_backstab` | You cannot backstab with this weapon. | `You cannot backstab with this weapon\.` | LJNCMDA.C:503 | [C] |
| `exp_gain` | You gain 13 experience. | `You gain (?P<exp>\d+) experience\.` | LJNUTIL.C:1777; LJNMISC5.C:763 | [C] T649 F |
| `exp_capped` | You have progressed too far without training to acquire anymore experience! | `You have progressed too far without training to acquire anymore experience!` | LJNUTIL.C:1694 | [C] |
| `coins_drop` | 9 copper drop to the ground. | `(?P<n>\d+) (?P<coin>platinum\|gold\|silver\|copper\|.+?) drop to the ground\.` | LJNMONST.C:1053-1073 | [C] T297 F |
| `pvp_killed` | Bob just killed Ann. | `(?P<who>.+) just killed (?P<victim>.+)\.` | LJNCOMBT.C:4057 | [C] |
| `pvp_attacked` | Bob just attacked Ann! | `(?P<who>.+) just (?:attacked (?P<v1>.+)!\|dodged an attack from (?P<v2>.+)\.\|missed an attack against (?P<v3>.+)\.)` | LJNCOMBT.C:4092 | [C] |
| `summoned_appears` | skeleton appears! | `(?P<who>.+) appears!` | LJNCOMBT.C:3344 | [C] |
| `confused` | You look around stupidly and do nothing! | `You look around stupidly and do nothing!` | LJNUTIL.C:10421 | [C] |
| `confused_other` | Bob looks around stupidly and foams at the mouth! | `(?P<who>.+) looks around stupidly and foams at the mouth!` | LJNUTIL.C:10423 | [C] |
| `shattered` | Your shield has been shattered! | `(?:You have shattered (?P<who>.+)'s (?P<item>.+)\|Your (?P<mine>.+) has been shattered!)` | LJNSPELL.C:5707 | [C] |
| `mon_hits_you_generic` | The orc slashes you for 7 damage! | `(?P<mon>[A-Z].+?) (?P<verb>[a-z][a-z -]+?) you(?: with .+?)? for (?P<dmg>\d+) damage!` | LJNCOMBT.C:3668 (fallback when no WCCMSG) | [C] |
| `mon_dodge_builtin` | Orc slashes you with sword, but you dodge! | `(?P<mon>.+) (?P<verb>.+) you with (?P<weapon>.+), but you dodge!` | LJNCOMBT.C:3829 | [C] |
| `mon_miss_builtin` | Orc swings at you with club. | `(?P<mon>[A-Z].+) (?P<verb>[a-z].+) (?:at )?you with (?P<weapon>.+)\.` | LJNCOMBT.C:3872 | [C] |

**Combat message details** [C LJNCOMBT.C unless noted]:
- `you_glance` / `other_glance` (:3093, :3098): both views use the first-person *miss* verb (`missverbs[0]`), not the hit verb. The room copy of a glance, a dodge and a plain miss goes out at message level `TRN_HIGH` only; hits go at `TRN_ALWAYS` (:2925-2940).
- `other_dodged` (:3115): your own dodge line (the monster dodged you) is printed without a prompt (`tell_user` only); a plain miss and a glance add `prf_prompt`.
- `mon_hits_*` (:3666 and the like): every WCCMSG-based monster line is formatted with two extra `"*"` arguments and the first letter upper-cased. Hit: text 1 to the player, text 2 to the room. Deflect (armour absorbed all): defmsg text 1 / 2 (:3745-3760), else the built-in `X's <verb> hits you, but your armour deflects.` / `X's <verb> hits <Name>, but glances off <his/her> armour.`. Miss: missmsg text 2 / 3 (:3851-3871), else `X <verb> you with <weapon>.`. Dodge: defmsg text 3 to the player and missmsg text 1 to the room when the slot has both records (:3796-3845), else `X <verb> you with <weapon>, but you dodge!` / `X <verb> <Name> with its <weapon>, but <he/she> dodges.`.
- `summoned_appears` (:3344) is printed because `confirm_monster_in_room` failed (the monster is not in its room list), not because the attacker was hidden.
- `Slightly dazed the %s rises from the floor.` comes from three sites: the player's round (:2682, :2686), the monster's attack round (:3477, :3481) and the fast monster pass (LJNBKGND.C:1424, brown, room only). The player-versus-player form `Slightly dazed %s rises from the floor.` is at :1887 and :1902.
- `breaks_off`: `<Name> breaks off combat.` (LJNACMB.C:161) is printed only for BREAK (`display_autocombat_broken(unum,1)`, LJNCMDS.C:752); every other stop shows only `*Combat Off*`.

### Spells

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `cast_fail` | You attempt to cast vine strike, but fail. | `You attempt to cast (?P<spell>.+?)(?: at (?P<target>.+))?, but fail\.` | LJNSPELL.C:633,5439 | [C] T8 F |
| `cast_resisted` | You attempt to cast mmis at orc, but the spell is resisted. | `You attempt to cast (?P<spell>.+) at (?P<target>.+), but the spell is resisted\.` | LJNSPELL.C:5417 | [C] |
| `target_resists` | The orc resists your spell! | `(?:The )?(?P<target>.+) resists your spell!` | LJNSPELL.C:1143,3013 | [C] |
| `you_resisted` | You resisted orc shaman's cast of chill. | `You resisted (?P<caster>.+)'s (?:cast of )?(?P<spell>.+)\.` | LJNSPELL.C:5422; LJNMONST.C:3415 | [C] F |
| `other_resisted` | Bob resisted orc shaman's cast of chill. | `(?P<who>.+) resisted (?P<caster>.+)'s (?:cast of )?(?P<spell>.+)\.` | LJNSPELL.C:5427 | [C] F |
| `cast_at_you_failed` | The orc shaman attempted to cast chill at you, but failed. | `(?:The )?(?P<caster>.+) attempted to cast (?P<spell>.+) (?:at\|on) you, but failed\.` | LJNSPELL.C:5444; LJNMONST.C:3437 | [C] |
| `other_cast_failed` | Bob attempted to cast mend, but failed. | `(?:The )?(?P<caster>.+) attempted to cast (?P<spell>.+?)(?: (?:at\|on) (?P<target>.+))?, but failed\.` | LJNSPELL.C:1760,5449 | [C] F |
| `no_effect_on` | Your spell has no effect on skeleton. | `Your spell has no effect on (?P<target>.+)\.` | LJNSPELL.C:2976 | [C] F |
| `no_effect_room` | Your spell has no effect in this room! | `Your spell has no effect in this room!` | LJNSPELL.C:2048 | [C] F |
| `no_mana` | You do not have enough mana to cast that spell. | `You do not have enough (?P<what>mana\|kai)(?: to cast that spell\| to invoke that power)?\.` | LJNSPELL.C:4789-4791 (`%s` = "mana to cast that spell" / "kai to invoke that power") | [C] |
| `too_powerful` | This spell is too powerful for you to control. | `This spell is too powerful for you(?: to control\.\| - .*)` | LJNSPELL.C:4794; LJNUTIL.C:9786 | [C] |
| `already_this_round` | You have already cast a spell this round! | `You have already (?P<what>cast a spell\|invoked a power) this round!` | LJNSPELL.C:2196, :4783 | [C] F |
| `need_target` | You must specify a target for that spell! | `You must specify a target for that spell!` | LJNSPELL.C:2025 | [C] |
| `bad_target` | You may not cast that spell on a user! | `(?:You may not cast that spell on (?:a user\|a monster\|an item)!\|Why would you want to cast a spell on that\?\|(?P<who>.+) is not in this room!)` | LJNSPELL.C:5137,6970,8526 | [C] |
| `unknown_spell` | You do not know how to cast fireball. | `You do not know how to cast (?P<spell>.+)\.` | LJNCMDB.C:671 | [C] |
| `no_spells` | You have no spells to cast! | `You have no (?:spells to cast\|powers to invoke)!` | LJNCMDB.C:435,466 | [C] |
| `spell_fails` | Your spell fails! | `Your (?:spell\|power) fails!` | LJNCMDB.C:443,473 | [C] |
| `may_not_cast` | You may not cast this spell. | `You may not cast this spell\.\|You may not (?:invoke\|cast)\.\.\. You are (?:not )?KAI!.*` | LJNCMDB.C:431,459,529 | [C] |
| `level_too_low` | You are not of a high enough level to cast that spell. | `You are not of a high enough level to cast that spell\.` | LJNSPELL.C:7009 | [C] |
| `no_component` | You do not have the required material component to cast this spell! | `You do not have the required material component to cast this spell!` | LJNSPELL.C:1749 | [C] |
| `alone` | You are the only one in the room! | `You are the only one in the room!` | LJNSPELL.C:3922 | [C] |
| `bash_spell_fail` | Your spell fails to bash one of the exits! | `Your spell fails to bash one of the exits!` | LJNSPELL.C:1688 | [C] |
| `spells_header` | You have the following spells: | `You have the following (?P<what>spells\|powers):` | LJNDISP.C:4770 | [C] T25 |
| `spells_columns` | Level Mana Short Spell Name | `Level (?:Mana\|Kai)\s+Short Spell Name` | LJNDISP.C:4772 | [C] T25 |
| `spells_none` | You have no spells. | `You have no (?P<what>spells\|powers)\.` | LJNDISP.C:4784 | [C] T11 |
| `spells_wrong_kind` | You may not list your spells. You are KAI! ... | `You may not list your (?:spells\|powers)\..*` | LJNDISP.C:4733 | [C] |
| `learn_scroll` | You read scroll of vines and learn the spell vine strike. | `You read (?P<item>.+) and learn the spell (?P<spell>.+)\.` | LJNUSE.C:158 | [C] T2 |
| `learn_spell` | You learn the spell vine strike. | `You learn the spell (?P<spell>.+)\.` | LJNSPEC.C:740 | [C] |
| `learn_spell_refused` | You don't know what to do with this! | `You don't know what to do with this!` | LJNSPEC.C:732, :744 (also an item use) | [C] |
| `monster_nothing_to_tell` | troll has nothing to tell you! | `(?P<mon>.+) has nothing to tell you!` | LJNUTIL.C:10134, :10243, :10251 (no talk block, or no ASK line matches; the bare monster name, no article) | [C] |
| `monster_doesnt_understand` | wild dog doesn't understand you. | `(?P<mon>.+) doesn't understand you\.` | LJNUTIL.C:10155 (GREET of a monster whose talk block links nowhere) | [C] |
| `already_known` | You realize that you already know this scroll! | `You realize that you already know this scroll!` | LJNUSE.C:154 | [C] |
| `use_unknown` | You don't know what to do with this! | `You don't know what to do with this!` | LJNUSE.C:145 | [C] |
| `spell_purged` | Purging spell x from your spellbook. | `Purging spell (?P<spell>.+) from your spellbook\.` | LJNCMDB.C:5941 | [C] |
| `kai_learned` | You learn the following Kai abilities: | `You learn the following Kai abilities:` | LJNUTIL.C:8178 | [C] |
| `detect_magic` | torch glows faintly, indicating a small amount of magic within. | `(?:(?P<item>.+) glows (?:faintly\|softly\|brightly), indicating .+ magic within\.\|You are almost blinded by the aura from .+\|You detect no magic in that item!)` | LJNSPELL.C:8801 | [C] |
| `you_cast_generic` | You cast vine strike at acid slime for 7 damage! | `You cast (?P<spell>.+?)(?: (?:at\|on\|upon) (?P<target>.+?))?(?: for (?P<dmg>\d+) damage\| for (?P<heal>\d+)\|, regenerating (?P<regen>\d+) damage)?[.!]` | LJNSPELL.C:308 + spell msgnum (WCCMSG) | [C] |
| `cast_on_you_generic` | Thornwyn casts mend on you, regenerating some of your wounds! | `(?P<caster>.+) casts? (?P<spell>.+?) (?:on\|upon\|at) you(?P<rest>.*)[.!]` | LJNSPELL.C:309; LJNMONST.C:1617 | [C] |
| `cast_other_generic` | Thornwyn casts vine strike on rat for 5 damage! | `(?P<caster>.+) casts? (?P<spell>.+?)(?: (?:on\|upon\|at) (?P<target>.+?))?(?: for (?P<dmg>\d+) damage)?[.!]` | LJNSPELL.C:310 | [C] F |
| `spell_row` (row after `spells_columns`) |   1   2    vine  vine strike | `(?P<level>\d+)\s+(?P<mana>\d+)\s+(?P<short>\S{1,4})\s+(?P<name>.+?)` | LJNDISP.C:4776 | [C] T50 |
| `kai_row` (row after `kai_learned`) | Kai: punch | `(?P<ability>.+)` | LJNUTIL.C:8181 | [C] |

### Loot, coins, items

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `picked_up_coins` | You picked up 9 copper farthings | `You picked up (?P<n>\d+) (?P<coin>runic coins?\|platinum pieces?\|gold crowns?\|silver nobles?\|copper farthings?)` | LJNCMDB.C:1086 | [C] T290 F |
| `other_picks_coins` | Bob picks up some gold crowns. | `(?P<who>.+) picks up some (?P<coin>.+)\.` | LJNCMDB.C:1091 | [C] F |
| `took_item` | You took torch. | `You took (?P<item>.+)\.` | LJNCMDB.C:824 | [C] F |
| `other_picks_item` | Bob picks up torch. | `(?P<who>.+) picks up (?P<item>.+)\.` | LJNCMDB.C:827 | [C] F |
| `not_here` | You don't see rto here. | `You don't see (?:any )?(?P<what>.+?)(?: here\.)?` | LJNCMDB.C:1100,1279,1286 | [C] F |
| `too_much` | You cannot carry that much! | `You cannot carry that much!` | LJNUTIL.C:5349 | [C] |
| `strange_force` | A strange force stops you from getting this item. | `A strange force stops you from getting this item\.` | LJNUTIL.C:5338 | [C] |
| `dropped` | You dropped quarterstaff. | `You dropped (?P<item>.+)\.` | LJNCMDB.C:3555,3638 | [C] T12 |
| `other_dropped` | Bob dropped torch. | `(?P<who>.+) dropped (?:some )?(?P<item>.+)\.` | LJNCMDB.C:3558 | [C] F |
| `drop_missing` | You don't have torch to drop! | `You don't have (?P<item>.+) to drop!` | LJNCMDB.C:3652,3676 | [C] |
| `drop_forbidden` | You may not drop that item! | `You may not (?:drop\|give\|hide) (?:that item\|that item away)!` | LJNCMDB.C:3518,4744,3814 | [C] |
| `room_full` | There is no room to drop torch here. | `There is no room to (?:drop\|hide) (?P<item>.+) here\.` | LJNCMDB.C:3566 | [C] |
| `hid_item` | You hid torch. | `You hid (?P<item>.+)\.` | LJNCMDB.C:3855 | [C] |
| `hide_attempt` | Attempting to hide... You don't think you are hidden. | `Attempting to hide\.\.\.(?P<rest>.*)` | LJNCMDB.C:3751 | [C] F |
| `gave_coins` | You gave Bob 10 copper | `You gave (?P<who>.+) (?P<n>\d+) (?P<coin>\w+)` | LJNCMDB.C:4527; LJNSUPP.C:2064 | [C] |
| `got_coins` | Bob gave you 10 copper | `(?P<who>.+) gave you (?P<n>\d+) (?P<coin>.+)` | LJNCMDB.C:4531 | [C] |
| `gave_item` | You just gave torch to Bob. | `You just gave (?P<item>.+) to (?P<who>.+)\.` | LJNCMDB.C:4823 | [C] |
| `got_item` | Bob just gave you torch. | `(?P<who>.+) just gave you (?P<item>.+)\.` | LJNCMDB.C:4828 | [C] |
| `give_refused` | Bob refuses your offer. | `(?P<who>.+) (?:refuses your offer\|cannot accept your offer)\.` | LJNCMDB.C:4485,4843 | [C] |
| `not_enough_coins` | You do not have that much copper! | `You do not have (?:that much\|that many) (?P<coin>.+)!` | LJNCMDB.C:4544 | [C] |
| `share_none` | There is nobody to share with. | `You do not have enough to share!\|There is nobody to share with\.` | LJNSUPP.C:1939,2160 | [C] |
| `uses_gone` | It's uses gone, potion disappears from your inventory! | `It's uses gone, (?P<item>.+) disappears from your inventory!` | LJNUTIL.C:5628 | [C] F |
| `no_uses` | There are no more uses in wand. | `There are no more uses in (?P<item>.+)\.` | LJNCMDB.C:332 | [C] |
| `use_forbidden` | You may not use that item! | `You may not use that item!\|You must have that item readied to use it!\|You must be wearing that item to use it!` | LJNCMDB.C:191-214 | [C] |
| `dont_have` | You don't have potion. | `You don't have (?P<item>.+)\.` | LJNCMDB.C:354 | [C] |
| `not_carrying` | You are not carrying wand. | `You are not carrying (?P<item>.+)\.` | LJNCMDB.C:650 | [C] |
| `receive_hp` | You receive 10 hit points. | `You receive (?P<n>\d+) (?P<what>hit points\|mana)\.` | LJNUSE.C:193,205 | [C] |
| `eat_drink` | You drink the potion. | `You (?P<act>eat\|drink) the (?P<item>.+)\.` | LJNCMDB.C:4200,4312 | [C] F |
| `other_eat_drink` | Bob drinks potion. | `(?P<who>.+) (?P<act>eats\|drinks) (?P<item>.+)\.` | LJNCMDB.C:4204 | [C] |
| `item_returned` | Your ruby has returned to its rightful place. | `Your (?P<item>.+) has returned to its rightful place\.` | LJNUTIL.C:4465 | [C] |
| `item_removed` | Your sword has been removed. | `Your (?P<item>.+) has been removed\.` | LJNUTIL.C:255 | [C] |
| `rob` | Your skills fail as you try to rob orc. | `(?:You stole (?P<n>\d+) (?P<coin>\S+) from (?P<v1>.+)\.\|You successfully stole (?P<item>.+) from (?P<v2>.+)\.\|Your skills fail as you try to rob (?P<v3>.+)\.\|You bump (?P<v4>.+) as you try to rob .+\.\|(?P<who>.+) bumps you as .+ tries to rob you!)` | LJNUTIL.C:9150-9355 | [C] |
| `track` | Your tracking skills fail you this time. | `(?:(?P<who>.+) went (?P<dir>\w+) from here\.\|Your tracking skills fail you this time\.)` | LJNUTIL.C:7492,7510 | [C] |

### Inventory

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `inv_nothing` | You are carrying Nothing! | `You are carrying Nothing!` | LJNDISP.C:4040 | [C] T18 |
| `inv_carrying` | You are carrying 18 silver nobles, padded vest (Torso), quarterstaff (Two handed) | `You are carrying (?P<items>.+)` | LJNDISP.C:3858-4040 | [C] T393 F |
| `inv_no_keys` | You have no keys. | `You have no keys\.` | LJNDISP.C:4603 | [C] T411 |
| `inv_keys` | You have the following keys:  brass key. | `You have the following keys:\s+(?P<keys>.+)\.` | LJNDISP.C:4586 | [C] F |
| `inv_wealth` | Wealth: 223 copper farthings | `Wealth: (?P<copper>\d+) copper farthings?` | LJNDISP.C:4045 | [C] T411 F |
| `inv_encumbrance` | Encumbrance: 520/3792 - None [13%] | `Encumbrance: (?P<cur>\d+)/(?P<max>\d+) - (?P<level>None\|Light\|Medium\|Heavy) \[(?P<pct>\d+)%\]` | LJNDISP.C:4057 | [C] T411 F |
| `money_none` | No money! | `No money!` | LJNDISP.C:6094 | [C] |

### Equipment and light

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `wear_ok` | You are now wearing padded vest. | `You are now wearing (?P<item>.+)\.` | LJNUTIL.C:6650 | [C] T21 F |
| `wield_ok` | You are now holding quarterstaff. | `You are now holding (?P<item>.+)\.` | LJNSUPP.C:2415 | [C] T10 F |
| `removed` | You have removed torch and extinguished it. | `You have removed (?P<item>.+?)(?P<ext> and extinguished it)?\.` | LJNUTIL.C:6556,6729 | [C] T5 F |
| `unwielded` | You now have no weapon readied. | `You now have no weapon readied\.` | LJNSUPP.C:2326 | [C] |
| `wear_fail` | You may not wear that item! | `You may not wear that item!\|You have no more room to wear that item!\|(?P<item>.+) may not be worn!` | LJNUTIL.C:6472,6490,6658 | [C] |
| `not_have_any` | You do not have quarterstaff left unequipped. | `You do not have (?:any \|a )?(?P<item>.+?)(?: left unequipped)?\.` | LJNUTIL.C:6664; LJNSUPP.C:2442; LJNCMDB.C:4915 | [C] T17 |
| `not_wearing` | You are not wearing chain coif. | `You are not wearing (?P<item>.+)\.` | LJNUTIL.C:6765 | [C] |
| `cant_remove` | You cannot remove cursed ring! | `You cannot remove (?P<item>.+)!\|You are already wearing (?P<item2>.+) and it may not be removed\.` | LJNUTIL.C:6714,6588 | [C] |
| `offhand_2h` | You may not ready a 2-handed weapon with your buckler worn! | `You may not wear an off-hand item while you have a 2-handed weapon readied\.\|You may not ready a 2-handed weapon with your (?P<item>.+) worn!` | LJNUTIL.C:6618; LJNSUPP.C:2378 | [C] |
| `weapon_forbidden` | You may not use that weapon. | `You may not use that weapon\.\|(?P<item>.+) is not a weapon!` | LJNSUPP.C:2362,2437 | [C] |
| `weapon_heavy` | This weapon feels heavy in your hands. | `This weapon feels heavy in your hands\.` | LJNSUPP.C:2418 | [C] |
| `weapon_quick` | You are extremely quick and deadly with this weapon. | `You are extremely quick and deadly with this weapon\.` | LJNSUPP.C:2423 | [C] F |
| `other_wears` | Bob wields quarterstaff! | `(?P<who>[A-Z]\w+) (?P<act>wears\|wields\|removes\|lights) (?P<item>.+?)[.!]` | LJNUTIL.C:6646; LJNSUPP.C:2411; LJNCMDB.C:4098 | [C] F |
| `light_ok` | You lit the torch. | `You lit the (?P<item>.+)\.` | LJNCMDB.C:4096 | [C] T3 |
| `light_fail` | You already have something lit! | `You may not light that item!\|You already have something lit!\|You cannot light (?P<item>.+)!\|You must recharge that before you may light it again\.` | LJNCMDB.C:4073-4114 | [C] T2 |
| `light_out` | torch is no longer lit! | `(?P<item>.+) is no longer lit!` | LJNBKGND.C:1976 | [C] |
| `light_out_other` | Bob's torch just went out. | `(?P<who>.+)'s (?P<item>.+) just went out\.` | LJNBKGND.C:1978 | [C] |
| `not_lit` | You do not have torch lit. | `You do not have (?P<item>.+) lit\.` | LJNUTIL.C:6734 | [C] |
| `light_level` | The current light level is dimly lit | `The current light level is (?P<light>.+)` | LJNCMDB.C:4031 | [C] |

### Shops, healer, bank

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `shop_list_intro` | The following items are for sale here: | `The following items are for sale here:` | LJNDISP.C:4130 | [C] T3 |
| `shop_list_header` | Item                          Quantity    Price | `Item\s+Quantity\s+Price` | LJNDISP.C:4132 | [C] T3 |
| `shop_list_rule` | ------------------------------------------------------ | `-{20,}` | LJNDISP.C:4135 | [C] T3 |
| `bought` | You just bought torch for 2 silver nobles, 5 copper farthings. | `You just bought (?P<item>.+) for (?P<price>(?:(?:\d+ (?:runic coins?\|platinum pieces?\|gold crowns?\|silver nobles?\|copper farthings?)(?:, )?)+\|nothing))\.` | LJNUTIL.C:6157 | [C] T58 |
| `buy_not_here` | You cannot buy torch here! | `You cannot buy (?P<item>.+) here!` | LJNUTIL.C:6119 | [C] T2 |
| `buy_unknown` | sowrd is not a known item. | `(?P<item>.+) is not a known item\.` | LJNUTIL.C:6116 | [C] |
| `buy_poor` | You cannot afford chainmail. | `You cannot afford (?P<item>.+)\.\|You cannot afford to do that!` | LJNUTIL.C:6122 | [C] |
| `not_in_shop` | You cannot BUY if you are not in a shop! | `You cannot (?P<cmd>BUY\|SELL\|LIST\|DEPOSIT\|WITHDRAW\|STOCK\|UNSTOCK\|MARKUP) if you are not in (?:a shop\|a bank\|your gangs shop)!` | LJNCMDA.C:2236,2327,2390 | [C] T2 |
| `other_bought` | You see Bob buy a torch. | `You see (?P<who>.+) (?P<act>buy\|sell) a (?P<item>.+)\.` | LJNUTIL.C:6154,6335 | [C] |
| `sold` | You sold club for 2 copper farthings. | `You sold (?P<item>.+) for (?P<n>\d+) (?P<coin>.+)\.` | LJNUTIL.C:6339 | [C] |
| `sell_value` | You would get 2 copper farthings for your club. | `You would get (?P<n>\d+) (?P<coin>.+) for your (?P<item>.+)\.` | LJNUTIL.C:6326 | [C] |
| `sell_refused` | You cannot sell club here. | `You cannot sell (?P<item>.+) here\.\|You may not sell (?:that item!\|items to a gang shop\.)\|You don't have (?P<item2>.+) to sell!` | LJNUTIL.C:6372,6276,6376 | [C] T2 |
| `healed_paid` | You hand over 3 silver nobles and all your wounds are healed. | `You hand over (?P<paid>(?:(?:\d+ (?:runic coins?\|platinum pieces?\|gold crowns?\|silver nobles?\|copper farthings?)(?:, )?)+\|nothing)) and all your wounds are healed\.` | LJNUTIL.C:5805 | [C] |
| `cured_paid` | You hand over 1 silver noble and your poisoning is cured. | `You hand over (?P<paid>(?:(?:\d+ (?:runic coins?\|platinum pieces?\|gold crowns?\|silver nobles?\|copper farthings?)(?:, )?)+\|nothing)) and (?:your poisoning is cured\|find that you were not poisoned!?)\.?` | LJNUTIL.C:5828,5840 | [C] |
| `heal_poor` | You do not have sufficient funds to buy full healing! | `You do not have sufficient funds to buy (?:full healing\|curing)!` | LJNUTIL.C:5813,5834 | [C] |
| `healer_done` | There is nothing more that healer can do for you. | `There is nothing more that (?P<who>.+) can do for you\.` | LJNCMDA.C:3548 | [C] |
| `bank_deposit` | You deposit 5 gold crowns. | `You deposit (?P<paid>(?:(?:\d+ (?:runic coins?\|platinum pieces?\|gold crowns?\|silver nobles?\|copper farthings?)(?:, )?)+\|nothing))\.` | LJNUTIL.C:7047 | [C] |
| `bank_withdraw` | You withdrew 5 gold crowns. | `You withdrew (?P<n>\d+) (?P<coin>.+)\.` | LJNUTIL.C:7098 | [C] (the coin is always `copper farthing(s)`) |
| `bank_list` | (BANKBOOK outside a bank: one `bank_balance` pair per book, no header) | as `bank_balance` | LJNBUFF.C:4119-4161 | [C] |
| `bank_balance` | Your balance at First Bank (#7) is: | `Your balance at (?P<bank>.+) \(#(?P<num>\d+)\) is:\|On deposit: (?P<dep>.+)` | LJNDISP.C:4999,5009 | [C] |
| `bank_misc` | You see Bob making a deposit. | `The bank cannot accept your deposit at this time\.\|Please specify a more reasonable amount\.\|You see (?P<who>.+) making a (?:deposit\|withdrawal)\.` | LJNUTIL.C:7056; LJNCMDB.C:4974 | [C] |
| `buy_curing_poor` | You do not have sufficient funds to buy curing! | `You do not have sufficient funds to buy curing!` | LJNUTIL.C:5835-5836, :5874 | [C] |
| `gang_shop_stock_refused` | You may not stock that item! | `You may not stock (?:limited items\|that item)!` | LJNCMDA.C:1659, :1680, :1690 | [C] |
| `gang_markup_set` | New gang shop markup value set to 20 percent. | `New gang shop markup value set to (?P<pct>\d+) percent\.` | LJNCMDA.C:2175 | [C] |
| `gang_shop_not_owned` | This is not a gang owned shop. | `This is not a gang owned shop\.` | LJNCMDA.C:1623, :1924, :2137 | [C] |
| `hand_over_generic` | You hand over 1 gold crown and you are now female! | `You hand over (?P<paid>(?:(?:\d+ (?:runic coins?\|platinum pieces?\|gold crowns?\|silver nobles?\|copper farthings?)(?:, )?)+\|nothing)) and (?P<what>.+)` | LJNCMDA.C:6004 | [C] |
| `shop_row` (row after `shop_list_rule`) | quarterstaff                  31           Free | `(?P<item>.+?)\s+(?P<qty>\d+)\s+(?:(?P<price>\d+) (?P<cur>[a-z ]+?)\|Free)(?: \((?P<flag>Too powerful\|You can't use)\))?` | LJNDISP.C:4171-4222 | [C] T20 |

### Training and levels

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `trained` | You hand over 5 silver nobles and you receive training to attain level 2. | `You hand over (?P<paid>(?:(?:\d+ (?:runic coins?\|platinum pieces?\|gold crowns?\|silver nobles?\|copper farthings?)(?:, )?)+\|nothing)) and you receive training to attain level (?P<level>\d+)\.` | LJNUTIL.C:8338 | [C] T4 |
| `train_receive` | You receive the following: | `You receive the following:` | LJNUTIL.C:8346 | [C] T4 |
| `train_cp` | 10 additional character points | `(?P<cp>\d+) additional character points` | LJNUTIL.C:8361 | [C] |
| `train_lives` | 1 additional lives | `(?P<lives>\d+) additional lives` | LJNUTIL.C:8381 | [C] |
| `train_no_exp` | You do not have the required experience to train yet! | `You do not have the required experience to train yet!` | LJNUTIL.C:8404 | [C] |
| `train_no_money` | You do not have the money required for your training. | `You do not have the money required for your training\.` | LJNUTIL.C:8330 | [C] |
| `train_wrong_room` | You must be in an appropriate training room to train! | `You must be in an appropriate training room to train!\|This shop is not suitable for your training\.` | LJNUTIL.C:8238,8251 | [C] |
| `changerace_prompt` | Have you read all the help files on the races and classes? | `Have you read all the help files on the races and classes\?` | LJNGAME.C:3742 (CHANGERACE; the second line is `(You must answer YES to continue): `; offered only when the change is open) | [C] |
| `train_level_band` | You have progressed too far to use the training provided here. | `You have (?P<which>not progressed far enough\|progressed too far) to use the training provided here\.` | LJNUTIL.C:8260,8264 | [C] |
| `train_capped` | You may not train any further.  Please contact your sysop | `You may not train any further\.  Please contact your sysop\|for appropriate access so that you may continue your training\.` | LJNUTIL.C:8272 | [C] |
| `train_stats_refused` | You may not train your stats here! | `You may not train your stats here!\|Your stats are unnaturally altered!  You may not train stats now\.\|You may not do any training while in tournament mode!` | LJNCMDA.C:4455,4425,4406 | [C] |
| `stats_reset_notice` | Please TRAIN STATS as soon as possible to re-apply your CPs | `Please TRAIN STATS as soon as possible to re-apply your CPs\|Your stats have been reset due to an earlier bug\.` | LJNUTIL.C:930 | [C] |
| `train_row` (row after `train_receive`) | 10 additional character points | `(?P<what>.+)` | LJNUTIL.C:8361 | [C] T4 |

### Status screens (st, exp, health)

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `st_name` | Name: Kasimir Drakemoor                Lives/CP:      9/12 | `Name: (?P<name>.+?)\s+Lives/CP:\s+(?P<lives>-?\d+)/(?P<cp>-?\d+)` | LJNDISP.C:470 | [C] T172 F |
| `st_race` | Race: Kang        Exp: 1712            Perception:     33 | `Race: (?P<race>.+?)\s+Exp: (?P<exp>\d+)\s+Perception:\s+\*?\s*(-?\d+)` | LJNDISP.C:474 | [C] T176 F |
| `st_class` | Class: Witchunter Level: 2             Stealth:         0 | `Class: (?P<cls>.+?)\s+Level: (?P<level>\d+)\s+Stealth:\s+\*?\s*(-?\d+)` | LJNDISP.C:481 | [C] T176 F |
| `st_hits` | Hits:    48/56    Armour Class:  19/1  Thievery:        0 | `Hits:\s+(?P<hp>-?\d+)/(?P<maxhp>\d+)\s+Armour Class:\s+(?P<ac>-?\d+)/(?P<dr>-?\d+)\s+Thievery:\s+\*?\s*(-?\d+)` | LJNDISP.C:487 | [C] T176 F |
| `st_mana` | Mana: *  9/9    Spellcasting: 40     Traps:           0 | `(?P<mtype>Mana\|Kai):\s+\*?\s*(?P<mana>-?\d+)/(?P<maxmana>\d+)(?:\s+Spellcasting:\s+\*?\s*(-?\d+))?\s+Traps:\s+\*?\s*(-?\d+)` | LJNDISP.C:507,516 | [C] T25 F |
| `st_traps` |                                        Traps:           0 | `Traps:\s+\*?\s*(-?\d+)` | LJNDISP.C:500 | [C] T151 |
| `st_picklocks` |                                        Picklocks:       0 | `Picklocks:\s+\*?\s*(-?\d+)` | LJNDISP.C:525 | [C] T176 F |
| `st_str` | Strength:  79     Agility: 42          Tracking:        0 | `Strength:\s+\*?\s*(-?\d+)\s+Agility:\s*\*?\s*(-?\d+)\s+Tracking:\s+\*?\s*(-?\d+)` | LJNDISP.C:532 | [C] T176 F |
| `st_int` | Intellect: 40     Health:  64          Martial Arts:   0 | `Intellect:\s*\*?\s*(-?\d+)\s+Health:\s+\*?\s*(-?\d+)\s+Martial Arts:\s*\*?\s*(-?\d+)` | LJNDISP.C:538 | [C] T176 F |
| `st_wil` | Willpower: 40     Charm:   30          MagicRes:       30 | `Willpower:\s*\*?\s*(-?\d+)\s+Charm:\s+\*?\s*(-?\d+)\s+MagicRes:\s+\*?\s*(-?\d+)` | LJNDISP.C:544 | [C] T176 F |
| `st_poisoned` | You are Poisoned! | `You are Poisoned!` | LJNDISP.C:552 | [C] |
| `st_rank` | You are in the front rank of your group. | `You are in the (?P<rank>front\|back) rank of your group\.` | LJNDISP.C:558 | [C] F |
| `st_spells_on` | You have the following spell cast upon you: | `You have the following spell cast upon you:` | LJNDISP.C:577 | [C] |
| `exp` | Exp: 346 Level: 1 Exp needed for next level: 1304 (1650) [21%] | `Exp: (?P<exp>\d+) Level: (?P<level>\d+) Exp needed for next level: (?P<left>\d+) \((?P<total>\d+)\) \[(?P<pct>\d+)%\]` | LJNCMDA.C:6776,6792 | [C] T174 |
| `exp_not_restructured` | Your experience has not been restructured for the new system. | `Your experience has not been restructured for the new system\.\|Please exit and re-enter The Realm\. This should restructure your experience points\.` | LJNUTIL.C:1704 | [C] |
| `health` | Health:    48/56    [85%] | `Health:\s+(?P<hp>-?\d+)/(?P<maxhp>\d+)\s+\[(?P<hppct>-?\d+)%\](?:\s+(?P<mtype>Mana\|Kai):\s+(?P<mana>-?\d+)/(?P<maxmana>\d+)\s+\[(?P<mpct>-?\d+)%\])?` | LJNDISP.C:669,676 | [C] |
| `spells_on_row` (row after `st_spells_on`) |   bless (3,10), mend. | `(?P<spells>.+)` | LJNDISP.C:584 | [C] |

Confirmed from `show_status` (LJNDISP.C:442-628) while porting silvermere T-027 [C]:

- The red `*` marks only two things: one of the six stats (Strength, Agility, Intellect,
  Health, Willpower, Charm) that differs from its base value (`str != basstr` and so on,
  LJNDISP.C:528-546), and the mana or kai value of a character with the Mana ability
  (`user_has_ability(ABL_MANA)`, :503-519). The skills (Perception, Stealth, Thievery, Traps,
  Picklocks, Tracking, Martial Arts, MagicRes) never carry it; the `\*?` in their regexes is
  harmless but never needed.
- Field widths: name `%-29.29s`, lives `%5d`, CP `%-5d`, race `%-11.11s`, exp `%-15.15s`
  (`%lu%09lu` once the billions are not 0), class `%-10.10s`, level `%-5d`, hits `%5d/%-5d`,
  armour class `%3d/%-2d` (tenths of `get_armour_rating`), mana `%4d/%-5d`, the six stats
  `%-5d`, every skill `%5d`. So the `st_mana` example above is not what the original prints
  for 9/9 mana: it prints `Mana: *   9/9     Spellcasting: 40     Traps:           0`, and
  `st_int` with no martial arts prints `Martial Arts:    0`.
- `You have the following spell cast upon you:` and the `name (value,rounds)` list appear
  only while the character shown has sysop debug on (`uflags[usernum].debug`, :570-606);
  everybody else sees, for each spell on him with a Descriptive Message, that message's third
  text on its own line (:613-627). Neither the list nor the texts appear when nothing is cast.
- `health` (LJNDISP.C:643-678): `Health: %5d/%-5d [%d%%]`, and for a character with maximum
  mana `  Mana: %3d/%-3d [%d%%]` (`Kai:` for a Kai class); nothing for a character with 0
  maximum hit points.
- WHO (`display_scan_of_users(0)`, LJNDISP.C:4275-4463): players in alphabetical order of
  their first names (`rank_users_by_name`, LJNSUPP.C:278), the name column as wide as the
  longest "first last" in the game (at least 5, at most 29); a player who is hiding is listed
  like anyone else (only a sysop's BBS invisibility hides one); a Neutral player's reputation
  column is blank. There is no count line for players; `There are N users in the game.` is the
  sysop scan's.
- TOPTEN (`display_topten`, LJNDISP.C:298-344) prints the heading and then one line per poll
  of the channel; a one-word line beginning with `X` typed meanwhile prints `Command
  interrupted.` (no prompt) and stops it, any other line is swallowed (LJNGAME.C:4911-4919).
  `TOPTEN {class}` puts `Class` in front of the column titles (`ClassRank Name ...`, sic).

### Resting, meditation, healing

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `rest_start` | You are now resting. | `You are now resting\.` | LJNCMDS.C:983 | [C] T438 F |
| `rest_other` | Bob stops to rest. | `(?P<who>.+) stops to rest\.` | LJNCMDS.C:973 | [C] F |
| `rest_sick` | You are too sick to rest! | `You are too sick to (?P<what>rest\|meditate)!` | LJNCMDS.C:960,1003 | [C] |
| `meditate_start` | You are now meditating. | `You are now meditating\.` | LJNCMDS.C:1027 | [C] F |
| `meditate_other` | Bob kneels to meditate. | `(?P<who>.+) kneels to meditate\.` | LJNCMDS.C:1017 | [C] F |
| `meditate_useless` | Meditation will not help at this time. | `Meditation will not help at this time\.` | LJNCMDS.C:1010 | [C] |
| `meditate_done` | You awake from deep meditation feeling stronger! | `You awake from deep meditation feeling stronger!` | LJNBKGND.C:2202 | [C] |
| `poison_tick` | You feel ill. | `You feel ill\.` | LJNBKGND.C:1598 | [C] |
| `aided` | Bob has aided you. | `You have aided (?P<who>.+), (?P<pron>\w+) wounds are now healing\.\|(?P<who2>.+) has aided you\.\|(?P<who3>.+) is in no need of assistance\.` | LJNCMDA.C:5168 | [C] |

### Party, group, gang

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `party_header` | The following people are in your travel party: | `The following people are in your travel party:` | LJNDISP.C:5117 | [C] F |
| `party_none` | You are not in a party at the present time. | `You are not in a party at the present time\.` | LJNDISP.C:5174 | [C] F |
| `following` | You are now following Bob | `You are (?:now )?following (?P<who>.+?)\.?` | LJNCMDA.C:2565; LJNDISP.C:5094 | [C] F |
| `follower_joined` | Bob started to follow you. | `(?P<who>.+) started to follow you\.` | LJNCMDA.C:2571 | [C] F |
| `invited_follow` | Bob has invited you to follow him. | `(?P<who>.+) has invited you to (?P<what>follow\|join) (?P<target>.+)\.` | LJNCMDA.C:4134,4069 | [C] |
| `you_invited` | You have invited Bob to follow you. | `You have invited (?P<who>.+) to (?:follow you\|join your gang)\.` | LJNCMDA.C:4141 | [C] F |
| `invite_needed` | You must be invited first! | `You must be invited first!` | LJNCMDA.C:2581 | [C] |
| `unfollowed` | You are no longer following Bob. | `You are no longer following (?P<who>.+)\.` | LJNUTIL.C:7723 | [C] |
| `follower_removed` | Bob has been removed from your followers. | `(?P<who>.+) has been removed from your followers\.` | LJNCMDA.C:4355 | [C] |
| `rank_moved` | You have moved to the front ranks of your group. | `(?:You have moved to the (?P<rank>front\|middle\|back) ranks of your group\.\|(?P<who>.+) just moved to the (?P<rank2>front rank\|middle\|back rank) (?:in\|of) your group\.)` | LJNUTIL.C:7959-8060 | [C] F |
| `rank_refused` | You are the only one in the front rank.  You may not leave. | `You (?:may not (?:enter the backrank of your own party\|leave your current rank in your own party)\|are the only one in the (?:front\|back) rank\.  You may not leave)\.\|There would not be enough people left in the frontrank if you did that\.` | LJNUTIL.C:7982 | [C] |
| `group_left` | You just left group 3. | `You just left group (?P<n>\d+)\.\|(?P<who>.+) just left your group \((?P<n2>\d+)\)` | LJNCMDA.C:2467 | [C] |
| `gang_msg` | You have joined the gang Wolves. | `(?:You have joined the gang (?P<gang>.+)\.\|(?P<who>.+) just joined your gang\.\|You have left (?P<gang2>.+)\.\|(?P<who2>.+) has left (?P<gang3>.+)\.)` | LJNGANG.C:473; LJNCMDA.C:2435 | [C] |
| `party_row` (row after `party_header`) |   Bob Smith  (Warrior)      [H:100%]  - Frontrank | `(?P<name>.+?)\s+\((?P<cls>[^)]+)\)\s*(?:\[(?P<mtype>[MK]):\s*(?P<mpct>\d+)%\]\s*)?\[H:\s*(?P<hpct>-?\d+)%\](?P<flags>.*)\|(?P<invited>.+?)\s+\[Invited\]` | LJNDISP.C:5120-5156 | [C] F |

**`PARTY` layout** [C LJNDISP.C:5071-5205]: `You are following <leader>.` first (bold cyan) when you follow
someone; then the header and one row per other slot of your list in slot order (members and
`[Invited]` rows); a row is `  %-30 name  %-12 (class)`, then ` [M:%3d%%]`/` [K:%3d%%]` or nine blanks
when mana is 0, then ` [H:%3d%%]`, then `P` (poisoned), `R` (resting) or `M` (meditating) in two
one-character columns, then ` - Frontrank`/` - Midrank`/` - Backrank`. **Your own row is always printed
last**, in the same form, also after `You are not in a party at the present time.` (light red; it resets
your rank to 0). A slot whose member left the game is dropped silently.

### Talk and messages

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `says_to_you` | Bob says (to you) "hi" | `(?P<who>.+) says \(to (?P<target>you\|.+)\) \"(?P<msg>.*)\"` | LJNCMDA.C:4679 | [C] F |
| `says` | Bob says "hi" | `(?P<who>\S+) says \"(?P<msg>.*)\"` | LJNPEND.C:525 | [C] F |
| `you_say` | You say "sysop goto silvermere" | `You (?P<verb>say\|ask\|exclaim\|\w+) \"(?P<msg>.*)\"` | LJNPEND.C:587 | [C] T17 F |
| `monster_says` | The fanatic screams "Death to those who oppose the Blood God!" | `The (?P<mon>.+?) (?P<verb>says\|screams\|shouts\|yells\|whispers\|mutters\|growls\|cries) \"(?P<msg>.*)\"` | monster random text (WCCTEXT.DAT randtext) | [C] T1 |
| `well_spoken` | Well Spoken... | `Well Spoken\.\.\.` | LJNPEND.C:584 | [C] |
| `yell` | Someone yells from the north "help" | `From (?P<who>.+) \(Yelling\): \"(?P<msg>.*)\"\|Someone yells from (?:above\|below\|the (?P<dir>\w+)) \"(?P<msg2>.*)\"` | LJNPEND.C:535,548 | [C] |
| `telepath` | Bob telepaths: @heal | `(?P<who>.+) telepaths:\s+(?P<msg>.*)` | LJNCMDA.C:4564 | [C] F |
| `telepath_status` | --- Telepath Sent to Bob --- | `--- (?:Telepath Sent to (?P<who>.+)\|Telepath Not Sent\|Message Directed to (?P<who2>.+)\|Message Not Sent) ---` | LJNCMDA.C:4582 | [C] F |
| `user_unknown` | Cannot find user! | `Cannot find user!` | LJNCMDA.C:4596 | [C] |
| `gossip` | Bob gossips: hello | `(?P<who>.+) (?P<chan>gossips\|auctions\|gangpaths): (?P<msg>.*)` | LJNCMDB.C:5708,5776; LJNCMDA.C:5408 | [C] F |
| `broadcast` | Broadcast from Bob "hi" | `Broadcast from (?P<who>.+) \"(?P<msg>.*)\"` | LJNCMDA.C:2767 | [C] F |
| `profanity` | You are using too much profanity - your message is not sent. | `You are using too much profanity - your message is not sent\.` | LJNPEND.C:479 | [C] |
| `cannot_speak` | You cannot speak! | `You cannot speak!` | LJNPEND.C:469 | [C] |
| `channel` | You just joined channel 1. | `(?:You just joined channel (?P<n>\d+)\.\|(?P<who>.+) just (?:joined\|left) your channel \((?P<n2>\d+)\))` | LJNCMDA.C:2707 | [C] |
| `gods_tell` | The gods tell you: behave | `The gods tell you: (?P<msg>.*)` | LJNCMDB.C:2630 | [C] |
| `sysop_speak` | You have spoken. | `You have spoken\.` | LJNCMDB.C:2125 (sysop `SPEAK <text>`: everyone else in the game gets the bare text with a prompt, no colour prefix, the speaker only this line) | [C] |

### Death and unconsciousness

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `drops_unconscious` | Thornwyn drops to the ground! | `(?P<who>.+) drops to the ground!` | LJNDISP.C:6007 (knock_out_user; you see your own name) | [C] T5 F |
| `you_killed` | You have been killed! | `You have been killed!` | LJNUTIL.C:4350 | [C] T1 |
| `other_dead` | Bob is dead. | `(?P<who>.+) is dead\.` | LJNUTIL.C:4371 | [C] F |
| `saved` | But, due to a miracle, you have been saved. | `But, (?:due to a miracle\|because you were in a colliseum), you have been saved\.` | LJNUTIL.C:4567,4408 | [C] T1 |
| `lives_left` | You have 8 lives left. | `You (?:have\|now have) (?P<lives>\d+) lives (?:left\|remaining)\.` | LJNUTIL.C:4409; LJNMISC5.C:576 | [C] T1 |
| `body_appears` | Bob appeared on the floor in the middle of the room. | `(?P<who>.+) appeared on the floor in the middle of the room\.` | LJNUTIL.C:4412 | [C] |
| `no_lives` | You have no lives remaining! | `You have no lives remaining!` | LJNUTIL.C:4604 | [C] |
| `life_ebbing` | You feel your life-force ebbing. | `You feel your life-force ebbing\.` | LJNUTIL.C:1295 | [C] |
| `suicide` | After a LONG thought, you take your own life. | `After a LONG thought, you take your own life\.?\|(?P<who>.+) took their own life!` | LJNMISC5.C:555 | [C] |
| `mortally_wounded` | You may not do that while you are mortally wounded! | `You may not do that while you are mortally wounded!` | LJNCMDS.C:199 | [C] T1 |

### Errors

| Event | Example | Regex | Source | Seen |
|---|---|---|---|---|
| `no_effect` | Your command had no effect. | `Your command had no effect\.` | LJNPEND.C:577 | [C] F |
| `why_would_you` | Why would you want to follow yourself? | `Why (?:would\|are) you .+[?.]` | many | [C] |
| `not_implemented` | Following a monster - Not implemented yet. | `Following a monster - Not implemented yet\.` | LJNCMDA.C:2592 | [C] |
| `rob_self` | Why would you want to rob yourself? | `Why would you want to rob yourself\?` | LJNUTIL.C:9114 (ROB on yourself) | [C] |
| `may_not_here` | You may not do that here! | `You may not do that here!` | LJNCMDA.C:6041 | [C] |
| `no_autocombat` | You may not be in autocombat when you do that. | `You may not be in autocombat when you do that\.\|You may not teleport during a PVP retaliation period\.` | LJNCMDB.C:1860 | [C] |

## 6. What the bot misreads today

These are cases in `test/play/progress.py` and `bridge.py` as of 2026-10-13. Each one is
checked against the source or the transcripts. The migration plan handles each of them.

1. **Room titles behind a mana or resting prompt are lost.**
   - `room_name()` and `observe()` strip only `^\[HP=\d+\]:\s*`.
   - A title glued behind `[HP=n/MA=n]:`, `[HP=n (Resting) ]:` or a negative HP keeps its
     prefix, so it isn't recognised.
   - Our transcripts have 75 glued titles. The bot's strip handles 71 of them and misses 4.
     All 4 are behind mana or resting prompts, which is the caster bot's position tracking. [C]
2. **Up and down doors aren't seen as exits.**
   - `observe()` keeps only words in `md.DIRS`, and `closed trap door above` contains
     neither `up` nor `down`.
   - So a room with a trap door never fits its data and position tracking falls back to
     "ambiguous". [C, LJNDISP.C:1706]
3. **The death regex never fires on real death text.**
   - The bot looks for `you have been slain|you are dead|you die|killed you|you have died`.
   - The game prints `You have been killed!`, then (with lives left) `But, due to a miracle,
     you have been saved.` / `You have N lives left.` (LJNUTIL.C:4350-4568).
   - Today death is only caught by HP ≤ 0 in a prompt.
   - Being knocked out is `<YourName> drops to the ground!` with a bell (LJNDISP.C:6007). [C]
4. **`ATTACKED_RE` gives false positives.**
   - It matches `you!` anywhere, including `The giant rat creeps in after you!`
     (monster_follows), friendly heals (`Thornwyn casts mend on you, ...!`) and quest texts.
   - It misses monster texts that don't say "at you" or "you!", such as
     `The fierce kobold thief stabs you for 7 damage!` (that one only matches because of
     `*Combat Engaged*` elsewhere in the block). [C]
5. **A light source burning out isn't always noticed.**
   - Items with a destroy message print it (`Your torch flickers and goes out.`, matched by
     "goes out").
   - Items without one print `<item> is no longer lit!` (LJNBKGND.C:1976), and the bot doesn't
     check for that. [C]
6. **`status()` ignores Kai.** Mystics and Ninjas have `Kai:  n/m` instead of `Mana:`
   (LJNDISP.C:516), so `max_mana` stays 0 for them. [C]
7. **`cleanup()` never drops sellable junk a shop refuses.** Both branches `continue`. The
   real refusals are `You cannot sell X here.`, `You may not sell that item!` and
   `You don't have X to sell!`. [C]
8. **`update()` misses most move refusals.**
   - Its check is `no exit|can't go|cannot go|is closed`.
   - Missed: `You may not go through this exit!`, `You are not permitted in that room!`,
     `You need to cast a spell to go that way!`, `You do not have the appropriate item ...`,
     level and alignment gates, tolls, and `You are too heavy to move!`.
   - This only matters when a refusal comes with an exits line. [C]
9. **Statline assumptions.** See section 2: HP_RE, MA_RE, `where()` (Enter shows the room only
   in FULL), and resting detection (`"(Resting)" in prompt`, which is fine for both standard
   layouts). [C]
10. **Typed-ahead echo inside output.**
    - `Name: Kasimir st` corrupts the name field.
    - The bot sends `st` and then `exp` back to back with only `wait=1.0`. [C]
11. **The `You ...` attack counter counts some non-attacks.**
    - `record_attack()` counts every `You ...` line minus a small exclusion list.
    - So `You have been killed!`, `You took N damage.`, `You attempt to cast ...` and
      `You resisted ...` count as attack actions. [C]
12. **Room parsing in `here()` drops capitalised monsters.**
    - `here()` filters names by lower-case first letter, so named NPC monsters (`Champion
      Gudruk`) are dropped.
    - It doesn't strip `*`, `(Hidden)` or ` (Charmed)`. [C, F]

## 7. Version and capture differences

- **Version.** No forum capture states its version. The races and classes (Kang, Gaunt One,
  Witchunter, Missionary, Mystic) suggest 1.11-era, probably 1.11p. That is an inference.
  - Our server says `MajorMUD v1.11p-WG (Dec 30 2005 14:20:05)`, `{Realm Of Legends}`.
  - Release differences reported elsewhere ("Two-handed" vs "Two handed") were not seen. 1.11p
    prints `(Two handed)` / `(Weapon Hand)` (LJNDISP.C:4005).
- **Capture artefacts, not game text:**
  - `(Dmg:733 Tot:733)` and the `³Repeating:³` CP437 box are MegaMud/Blood client overlays.
  - Blood2Html collapses double spaces (`Also here:  X` becomes `Also here: X`).
  - phpBB strips table tags.
- **Message settings change what you see:**
  - `SET MESSAGES HIGH|MEDIUM|LOW` (player.msglevel): the room only sees other people's misses
    and deflections at HIGH (TRN_HIGH, LJNCOMBT.C:3130).
  - `SET STYLE TECHNICAL|FANTASY` changes only the `who` list, which becomes a table
    (LJNDISP.C:4260).
  - `SET FOLLOW BLIND` hides room displays while following.
  - `SET INTERRUPT ON` makes PvP attacks flush your output buffer (LJNCOMBT.C:1755).
  - `QUIET` / `VERBOSE` (brief/verbose descriptions).
- **The sysop-named currency** is `runic coins` here (WCCMMUD.MSG USRCURNM). Another board can
  rename it, which changes inventory, `You notice` and coin-drop lines (`1 runic coin drop to
  the ground.`, forum t=3326).
- **Sysop DEBUG** adds `DBG: <room> Map: <map>` above titles (LJNDISP.C:2490), the
  `{Energy=..}` prompt suffix, combat tables (`AV DV DR ...`) and many `DEBUG :` lines. Don't
  run the bot with SYSOP DEBUG on.

## 8. Known gaps

The static registry is also available as JSON in `test/play/tests/fixtures/patterns.json` (regenerate with `python3 test/play/patterns.py export --out test/play/tests/fixtures/patterns.json`).

- Not yet observed anywhere:
  - shop list rows with prices, `(Too powerful)` and `(You can't use)`
  - party listing rows from our own play (forum only)
  - Kai prompts, `(Meditating)` in the HP-only prompt
  - gates being locked, secret passages, NONE!!!
  - the BBS "more" pager (`(N)onstop, (Q)uit, or (C)ontinue?`, [I])
  - sysop page and telepath from the BBS layer
- **Character creation** (`train stats`, new character) is a full-screen ANSI form with cursor
  addressing. The registry only labels its fragments (`chargen_*`). Drive it by the field
  prompts and error lines, not by parsing the form.
- **Seen in the captures, not yet in the registry** (found by wayfarer's parser port, C-010,
  jnfisher/wayfarer#6) [C]:
  - MBBSEmu log-off: `Ok, thanks for calling!` then `Have a nice day...`, after
    `Are you sure (Y/N, or R to re-logon)? Y`.
  - Character creation's lawful question comes after an 8-line explanation, from *"You must
    now choose if you want to be a truly 'lawful' citizen of the realm."* to *"you may not
    remove this title, unless you start a new character."*, then
    `Do you want to be Lawful?  [Yes/No]`.
  - `wealth` with no coins prints `You have No money!` (the static `money_none` is the bare
    `No money!`).
  - Line-mode creation (NAWS 79x24) draws its `Point Cost Chart` with ASCII `+-|`, which
    `chargen_box` (box-drawing characters only) misses.
  - The `top` list header `Rank Name   Class   Gang/Guild   Experience` and a `-=-=...` rule
    that starts with `-`, which `enter_rule` (leading `=` only) misses.
  - Weapon-verb ties: two WCCMSG patterns can tie on priority and specificity under different
    index keys (8294 and 8310 on *"The angry kobold thief lunges at Kasimir with their
    shortsword!"*); `patterns.py` breaks the tie by `set` order, so its answer varies with
    `PYTHONHASHSEED`. wayfarer breaks ties by list order.
- **Look-at-player descriptions** (`X is a ..., ... He moves ...`) wrap over 3 to 5 lines;
  only the first line is classified (`look_player`).
- **The `wccmsg` role is generic.**
  - It recognises any WCCMSG text but doesn't know what it means.
  - Promote the ones the bot needs (status effects such as `You are entangled!`, potions, quest
    steps) to named events as needed. Do it by message number, in `Registry.load_world()`.
