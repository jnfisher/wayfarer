# Connection, login, entering the realm

## Transport
- Plain telnet. The server sends option negotiations; refusing every `DO` with `WONT` works fine,
  and nothing else is needed. [C] `test/play/bridge.py` `strip_telnet`.
  - MBBSEmu's opening offers, verbatim: `IAC WILL BINARY, IAC DO BINARY, IAC WILL ECHO,
    IAC DONT ECHO, IAC WILL SGA, IAC DO SGA, IAC WONT NAWS, IAC DO NAWS`. [C] 2026-10-14 capture
  - Answering `DO NAWS` with `IAC WILL NAWS` and `IAC SB NAWS w h IAC SE` sets the channel's
    `scnwid` (columns) and `scnfse`/`scnbrk` (rows); without an answer the account defaults are
    80x24 with ANSI on. [C] MBBSEmu `TelnetSession.OnIacSubnegotiationReceived`, `UserAccount`
    defaults; `test/play/capture.py` `CaptureSession` sends it.
- Text is **CP437** with ANSI escapes (colours, cursor positioning, clear line). Decode as CP437
  before display or parsing. [C]
- Line ends are CR LF, sometimes bare CR. The full-screen editor (FSD) uses cursor positioning
  heavily. [C]
- Monster names are coloured by alignment, so colour carries meaning; keep the ANSI when
  classifying. [F, from John] The data gives alignment directly too (see `combat.md`).

## MBBSEmu (test server) specifics
- Telnet port is set in `appsettings.json` (we use 2323 for the original DLL and 2324 for the
  built one). It has only a full-screen console UI, so it needs a TTY: run it in tmux.
  [C] `test/run.sh`
- **`Timer.Hertz` must be raised** (we use 1000, the maximum). At the default of 36, a channel's
  polling routine only runs once per idle tick, so new-character name validation takes about
  30 s. Game timers are wall-clock seconds, so this doesn't speed gameplay up. [C]
- `GSBL.BTURNO` sets the BBS registration number that MajorMUD's activation code depends on.
  `test/keygen.py` makes valid codes from the module's own generator. [C]
- MBBSEmu converts each Btrieve `.DAT` or `.VIR` to SQLite `.db` (`data_t.data` holds the raw
  record), so live game state can be read with sqlite while it runs. [C] See `game-data.md`.

## Login flow (MBBSEmu)
1. `Username:` → user, then `Password:` → password. New accounts: `NEW`, then username,
   password twice, e-mail, gender `M`/`F`. [C]
2. BBS main menu: `Make your selection (X to exit):` → `M` for MajorMUD. [C]
3. MajorMUD menu ends in `[MAJORMUD]:` → `E` enters the realm. Options: `H` help, `A` about,
   `C` counts, `T` top ten, `G` top gangs, `W` who, `R` release notes, `I` info, `X` exit. [C]
4. In the realm the prompt is `[HP=nn]:`. [C]
- Disconnecting while in the realm is punished on the next entry: "Last time you were on, you
  disconnected while playing. The gods have punished you appropriately." Log off cleanly. [C]
  The notice is printed by `load_player`, before the entry banner (LJNUTIL.C:750-755,
  called at LJNGAME.C:3887; the banner is :3969), and it also appears after a hang-up that cost nothing (`PUNHUP NONE`).
  Rules in `recovery.md` "Hanging up and the daily cleanup". [C]
- **Main menu input** (LJNGAME.C:4413-4430): an empty line reprints the menu; any other unknown
  word prints `Invalid Option.` (with a leading CR) and the `[MAJORMUD]: ` prompt again. [C]
- **Entering the realm with `E`** (LJNGAME.C:3969-4002), in this order: the entry banner, a
  tournament notice if one is on, the room description, the prompt, then
  `<name> just entered the Realm.` to every player in the game (not only the room; blocked
  by their "no entry messages" setting; skipped for an invisible sysop), then the
  death check (a player who died offline is dealt with here). Leaving by `x` ends with
  `<name> just left the Realm.` to the game in the same way (`recovery.md`, "When the
  character is saved"). [C]
- **Tournament play** (`TOURNAMENT_PLAY=on` in `wccmmud.ini`, LJNLOG.C:232): `*** MajorMUD
  Tournament Play enabled! ***` at logon and again on entering the realm; a player with no
  play time yet this login (`sesstime` 0) is placed in map 1 room 224 (0xE0, LJNGAME.C:3975-3985);
  nobody may be Lawful and the PvP level gap is 100 (LJNGAME.C:529-535); players may fight in
  protected rooms (LJNCOMBT.C:1699, LJNCMDA.C:309, :375); and `TRAIN`, `SUICIDE` and `REROLL`
  are refused (`recovery.md`, `progression.md`). [C]
- **MBBSEmu's own login dialogue** (from its `MenuRoutines.cs`; not re-run here): after `Invalid Credentials` it returns to the `Username:` prompt with
  unlimited retries; a second session on the same user is refused at the username prompt with
  `<user> is already logged in -- only 1 connection allowed per user.`; sign-up refuses a
  blank, over-29-character or taken username, a password that is not 5 to 9 characters or is
  `password`, a bad e-mail, and anything but `M`/`F`; the password echoes as `*`. [I]
- **Prompts carry colour inside them.** Under MBBSEmu the MajorMUD menu prompt arrives as
  `ESC[1;30m[ESC[0mMAJORMUD ESC[1;30m]ESC[0m: `, so a plain substring match for `[MAJORMUD]:`
  fails. Strip ANSI escapes before matching login prompts. [C: capture `logoff-in-realm/raw.bin`;
  found by wayfarer's logon scripts, card C-005]
- **silvermere's login dialogue differs** (the Rust server, `silvermere-auth` `messages/auth.toml`):
  a second session is refused *after* the password, with `This account is already logged on.`,
  and the connection then closes (MBBSEmu refuses at the username prompt and asks again);
  repeated bad passwords lock the account with `Too many failed logins: this account is locked
  for N more minute(s).` [C: silvermere-auth; wayfarer card C-005]
- **Paged lists.** The game asks the BBS for page mode (`btuxnf`, 22 lines, `Hit any key to
  continue...`) for the help screens, the release notes and `SYSOP GOD <user> INFO`
  (LJNGAME.C:3625-3639, :4039, LJNCMDB.C:2374, LJNHELP.C:129). MBBSEmu (v1.0-alpha-090826)
  ignores `btuxnf`, so under it these pauses never show and the whole text arrives at once [C: MBBSEmu Galgsbl.cs:350-354, btuxnf returns 0 without acting, tag v1.0-alpha-090826]; a real MajorBBS pauses every 22 lines until a key is pressed. [C]

## Character creation
- Pick race, then class (numbered lists), then "Do you want to be Lawful? [Yes/No]". Lawful
  characters can never start evil actions. [C]
  - At either list only the first character of the first word counts for `X` (back to the
    main menu); `?` alone reprints the list and `? <topic>` or `help <topic>` shows help text.
    A race or class is `atoi` of the first word, so a number is needed: `0`, text or an
    out-of-range number gets `You must choose a valid race. [ ? for help ]` (`class` for the
    second list) (LJNGAME.C:4541-4570, :4645-4694). [C]
  - The lawful answer must be exactly one word starting with `Y` or `N` (`y n` or `yes please`
    gets `You must respond with a Y or an N.` and the question again). `Y` sets lawful to −10
    and evil points to −51; `N` sets lawful 0 and lifts negative evil points to 0
    (LJNGAME.C:4729-4748). With `TOURNAMENT_PLAY` on, nobody can be Lawful. [C]
- Then the **full-screen stat editor**. Fields in order: given name (defaults to the BBS user
  id), family name (required unless BLANKLAST), Str, Int, Wil, Agi, Hea, Cha, hair length, hair
  colour, eye colour, Exit (SAVE/QUIT). Enter accepts a field and moves on; typing a number sets
  a stat. The tail of the screen dump lists `min max current` for each stat. [C]
  `LJNGAME.C edit_character_stats`
- **The first name must not be a game command**: `sysop` fails. The game prints, in light red,
  `The name you have chosen is already in use!` / `You will have to start over again.` /
  `Press Enter.`, the half-made character is cleared and Enter returns to the menu (an earlier note here called this "silently returning to the menu"; the text above comes from the C). [C] `LJNGAME.C:2262-2266` (`parse_command` check)
- After SAVE a new name is checked in the background (`Please wait - Validating your name.`
  in light cyan with a trailing space, LJNGAME.C:2400-2401), walking WCCUSERS and then all
  known monsters one record per poll. [C] `LJNGAME.C:1431`
  - A taken name (a character or a monster has it) is answered with a leading backspace:
    `\bYou may not use <name> as your name.` then `Please enter a new name: `
    (:1473-1475, :1518-1520); that prompt checks the typed word for letters only
    (`The name you have chosen is invalid!` / `Please try another one: `) and for a game
    command (`You may not use that name.`, :3236-3262). A name typed with a blank is one
    word to the check, so a monster name such as `giant rat` can never match. If the game
    is full when the name is accepted, the full message is shown and then the check still
    runs (the polling routine repeats the test, :1559); a new character that is not saved
    (QUIT) sees the main menu printed twice (:2378-2384, :2408-2412). [C]
- **Stat editor rules** (`ljnvfy`, LJNGAME.C:1850-2090; `statflds` :296-312). [C]
  - The given name is at most 10 characters (the length test runs before blanks are stripped),
    the family name 18 (tested after stripping, so `Your family name can only be 18 characters
    long!` cannot be reached on the 18-wide field); letters only (`Only ALPHA characters are
    allowed in a name!`), first letter upper-cased and the rest as typed; an empty given name
    is `You have entered an invalid name!`; an empty family name is `You must enter a last
    name!` unless `BLANK_LASTNAME_ALLOWED` is set. A name that fails when the editor closes
    prints `The name you have chosen is invalid!` / `You will have to start over again.` /
    `Press Enter.` (:1855-1921, :2192-2250).
  - Each stat field takes the **total**, not an increment (kernel bounds 10 to 250). Refusals,
    in this order: `<Stat> may not be lower than N.` where N is the character's current value
    (for a new character the race minimum; for `TRAIN STATS` what it already has, so a stat
    can never be lowered), `<Stat> may not be higher than N.` (race maximum), and
    `You may not assign that much to <Stat>` when the total cost exceeds the character's
    **lifetime** CP total (`totalcp`), not the unspent `cp` (:1936-2075). The stat names are
    Strength, Intellect, Wisdom, Agility, Health, Charm.
  - CP left is redrawn only after the family name and after an accepted stat (the `showcp`
    exit), not after the given name or the looks. On the form it is `ESC[20;33f` then `%4d`
    of `totalcp - used`; in line mode it is the text `You have N CP to distribute.`
    (:1768-1776, :2084-2086). The form is used on an ANSI terminal of at least 80x23
    (:2474); `TRAIN STATS` uses the same test (:2603).
  - **TRAIN STATS** (`cmd_train` LJNCMDA.C:4404-4468) is refused, in this order: in tournament
    mode (`You may not do any training while in tournament mode!`, which also stops plain
    `TRAIN`); outside a shop room; while any `Alter Int/Wis/Str/Hea/Agi/Cha` effect is
    on (`Your stats are unnaturally altered!  You may not train stats now.`); in a shop that
    is not a Training shop, or whose trainer is for another class (`trainclass` 0 = any
    class): all three of the shop cases print `You may not train your stats here!`. Otherwise
    the character is saved, the whole game hears `<name> just left the Realm.`, and the
    editor opens with the **given name locked** (`FFFAVD`, :2593-2595), so the form starts at
    the family name. Bots must not use it (`train stats` is for a human).
  - **SAVE** (`ljndun`, :2160-2420) stores the stats as both base and current, `cp =
    totalcp - used`, the looks, and the family name; then a character with no experience is
    refilled to full HP and mana (recomputed with mode 0), one with experience keeps its HP
    (mode 2). An existing character gets the status screen and its prompt (no room
    description) and the whole game hears `<name> just entered the Realm.`; a character with no
    suicide password also gets the `SET SUICIDE` notice. QUIT re-enters the realm unchanged
    (unless the stats were being reset, then it goes to the main menu).
  - **Suicide password**, `SET SUICIDE`: with one set it asks `Enter the current password:`,
    else `Enter New Password:`; both echo `*`, 8 characters at most (`The password may not be
    longer than 8 characters.`); an empty answer to either is `Password NOT changed`, a wrong
    current one `Invalid password!`, success `Password changed`. Matching ignores case
    (LJNCMDA.C:5695-5712, LJNGAME.C:4846-4902). [C]
- **CHANGERACE** (hidden main-menu word; also `ENABLE_CLASS_CHANGE` for the class, off by
  default, LJNLOG.C:116, :481). Every character made by 1.11p has the race-changed flag set
  with `racechg` 0, so the change is **never offered to it**; it is open only to older records
  (record version 18 or later without the flag) and to a developer's own character after
  `SYSOP TWEAK RACE 0` (LJNCMDB.C:3245-3252).
  After a first change it can be redone for `RACE_CHANGE_DURATION` minutes of play (default
  300, range 1-720, LJNLOG.C:30-33), counted as `sesstime/2 + lifemins - racechg` in unsigned
  arithmetic; `racechg` is set only by the first race change, never by a class change. A
  race or class change sets `cp = -1`: the next entry resets the stats and opens the stat
  editor (`recovery.md`). Typed when the change is closed, the word falls through to the
  menu's `C` (game counts) (LJNUTIL.C:1520, LJNSUPP.C:1583-1594, :1636-1642, :1690-1694,
  LJNGAME.C:3739-3740). [C]
- CP cost per stat point above the race minimum: 1 each for the first 10, 2 each for the next
  10, then 3, and so on. [C] `statplan.py` / `stat_amount_used`
- **Driving the creation editor over ANSI** (worked first time on 2026-10-13, Brynja): the
  editor moves the cursor with `ESC[row;colf`, so the last such sequence in the stream tells you
  the active field (given name 4;20, family name 5;20, stats 9-14;34, hair length 16;22, hair
  colour 17;22, eye colour 18;22, Exit 20;14; CP left is redrawn at 20;33). Type the value then
  CR; a stat field accepts the new number typed over the old one. Description fields cycle with
  **Space** and redraw the choice in upper case. Go one key at a time and check the cursor row
  after each (pitfall 13). [C]
  - Form behaviour seen in the `creation-fsd` capture (raw bytes checked 2026-10-16): before
    each field the game repaints **row 23** (`ESC[23;1f ESC[1;31m` plus 61 spaces) to clear the
    last error text, so an error shows there and goes at the next move; the initial looks are
    drawn in lower case (`none`, `black`, `black`) and, after Space, in upper case (`SHORT`);
    on SAVE the form ends with `ESC[24;0f ESC[0m` before the next text. [C] The first key typed into
    a field blanks it only when it already holds something [I: reported by a later port, not checked in the capture bytes].
  - Hair length: none, short, shoulder-length, long, waist-length, ankle-length.
  - Hair colour: black, white, silver, red, brown, dark-brown, blonde, green, blue, grey.
  - Eye colour: black, crimson, yellow, pale-blue, sea-blue, dark-blue, grey-blue, slate-grey,
    bright-green, forest-green, pale-green, … (the list goes on past these).
- **Which editor you get is decided by the terminal size**: the full-screen form needs ANSI on,
  at least **80 columns and at least 23 rows** (`usaptr->scnwid >= 80 && usaptr->scnfse >= 23`,
  LJNGAME.C:1769, 2475, 2527, 2602). Measured with NAWS on 2026-10-14: 80x24 and 80x23 get the
  form; **79x24, 79x23 and 80x22 get the line-mode path**. [C]
  - The line-mode path prints `M A J O R   M U D` / `Character Generator`, the text
    `Point Cost Chart`, and one range line per stat with the maximum printed as 0
    (`Strength  35 - 0`: a stray `'\0'` argument, LJNGAME.C:2549), and after every accepted
    field `You have N CP to distribute.` instead of redrawing CP left at 20;33. [C]
  - Under MBBSEmu the field editing is the same either way: MBBSEmu's FSD engine still places
    every field with `ESC[row;colf` at the positions above, so the same keystrokes work. On a
    real MajorBBS the non-ANSI path would be the host's line-mode FSD prompts. [C] capture
    `test/play/tests/captures/creation-line-mode`
- Arrow keys inside the creation editor (MBBSEmu): up and down move to the previous and next
  field and the editor stays open. A bare ESC swallows the next keys typed (the editor waits for
  the rest of an escape sequence): `x`, CR and a digit after it were lost, the following CR
  moved on. [C] 2026-10-14 probe; capture `creation-fsd` has the arrow pair. See pitfall 14.
- After SAVE with no suicide password set, the game asks you to set one with `SET SUICIDE`. [C]
- Starting money is **zero**. Newhaven's shops give the starter gear free (padded armour set,
  quarterstaff or club, torch). [C]

## Sysop commands and operator keys [C]
Only relevant when you run your own server or meet a sysop; `SYSOP GOTO` is in `navigation.md`
and the developer-code commands (`MONSTER`, `SUMMON`, `EXPRESS_TRAIN`, `TWEAK`, `DEBUG`) are in
`game-data.md`. From `cmd_sysop` (LJNCMDB.C:1420-3000).
- **`SYSOP LANCE` answers every player**, before the operator test (LJNCMDB.C:1447-1480):
  `Reg# <BBS registration number>`, `Lcon <max users in game>`, `IP: :<ip>:` when the BBS has
  one, and `Death Setting: <DEATHHP>`; an operator then also gets `Command not recognized.`
  Any other `SYSOP` word from a non-operator falls through to speech.
- **Who may use what.** Everything else needs the `SYSOPKEY` or the game-operator key; typed
  alone, `SYSOP` lists the sub-commands, the first nine (BUFFERS, STATUS, MAP, GOD, LIST,
  REPORT, LIGHTNING, GOTO, RELOAD) for every operator, INITIALIZE, CONFIGURE, RESET, DISBAND,
  DISABLE, ENABLE, GANGSIZE and ARENA for the sysop key only (:1486-1511). Nineteen
  `SYS_*_KEY` options (`SYS_MAP_KEY`, `SYS_REPORT_KEY`, `SYS_GOTO_KEY`, `SYS_LIGHTNING_KEY`,
  `SYS_STATUS_KEY`, `SYS_ADDLIFE_KEY`, `SYS_RETRAIN_KEY`, `SYS_ALIGNMENT_KEY`,
  `SYS_EDITEVIL_KEY`, `SYS_JAIL_KEY`, `SYS_SPEAK_KEY`, `SYS_LISTUSER_KEY`, `SYS_LISTLIMIT_KEY`,
  `SYS_USERINFO_KEY`, `SYS_CHANGE_KEY`, `SYS_BUFFERS_KEY`, `SYS_TOPTEN_KEY`, `SYS_DISBAND_KEY`,
  `SYS_CLEARSUICIDE_KEY`; LJNLOG.C:137-155, :264-364) plus `SYS_RESET_ROOMS` (:289, checked
  at LJNCMDB.C:1617) and `SYS_DISABLE_GANGS` (:409, checked at :2196) name an **extra** BBS key
  that the operator must also hold for that sub-command; all are empty by default, which
  means no extra lock. That makes 21 key options; `SYS_LOG_DEATHS` is a number (log deaths,
  suicides and rerolls), not a key. `SYSOP DISBAND` lets a game operator in only when
  `SYS_DISBAND_KEY` is set and held (:2084-2086). Each sub-command is logged as `Sysop Command
  by <id>: <line>` when `LOG_SYSOP_COMMANDS` is on (default). The `GOSSKEY` and `AUCTKEY`
  keys give gossip and auction rights to ordinary players only: a sysop or game operator
  holding one keeps the right through the staff flags (LJNGAME.C:3005-3027).
- **Unreachable in 1.11p:** `SYSOP DUMP` (needs `strstr("1.11p","beta")`, :1608) and
  `SYSOP VALUES`, `EMULATE`, `NOEMULATE` (they test `devlevel == 2`, which is fixed at 1,
  LJNGAME.C:268, :1156; :2012, :2745, :2773). `devmode` (extra developer lines and
  CONFIGURE options) is set only from the BBS registration number plus a file in the BBS
  folder (LJNGAME.C:845-882).
- **`SYSOP CONFIGURE SHOW`** (SYSOP key; LJNCMDB.C:2958-2975) prints `Current Configuration
  Settings:`; with `devmode` `Monster Generation Rate:     N seconds` and `Minutes before
  regenerating: N minutes`; `Normal Credit Consumption:   N`, `Exempt Credit Conusmption:   N`
  (sic), `Free time for exempt users:  N`, `Maximum users in game:       N` (from the
  activation code, LJNGAME.C:977), `Current pollrate:            N`, `Current initrate:
  N`, `Limited item tracking enabled` when tracking is on, and `Logfile: <name>`. Any other
  third word is `No configure command done.` with no prompt after it; two words, or three
  that are not SHOW, list `Valid options: SHOW, MAXUSERS` (plus GENRATE, MINWAIT under
  `devmode`).
- **`SYSOP LIGHTNING <user>`** strikes for 25 to 50 per cent of the victim's maximum HP: the
  victim sees `A bolt of lightning from the heavens strikes you for N points damage!` and the
  room `A bolt of lightning just struck <name>!` (both yellow). The victim must be a **FIEND**
  (legal level 5, 210+ evil points, `combat.md`) unless `LIGHTEVY` is on, otherwise the sysop
  gets `You may not lightning that user.`; a conscious victim who falls below 0 is knocked out,
  not killed; the sysop sees `Done` and the BBS console gets `Lightning struck: <victim first
  name> by <sysop user id>` (LJNCMDB.C:1900-1990). [C]
- **`SYSOP GOD <user> ...`**: `GOD` alone prints the syntax and the option list; the list
  printed for an unknown four-word option leaves out `, ABILITIES` (:2292 vs :2486).
  `RETRAIN` sets `cp = -1` and says `The next time <name> logs into MajorMUD they will have to
  retrain their stats.` (:2397); `DISABLE TOPTEN` prints nothing on success (`No disable
  command done.` otherwise), `ENABLE TOPTEN` prints `Done` (:2676-2700).
- **Lists.** `SYSOP LIST USERS` is the unpaged `display_scan_of_users(1)`; `SYSOP REPORT USERS`
  is a paged report that `X` stops with `Sysop user report interrupted` and no prompt
  (LJNCMDB.C:2827, :2914-2930; LJNDISP.C:6113-6245; LJNGAME.C:4921-4927). The sysop's menu
  options `I`, `N` and `S` (release notes) end at the player main menu, not the sysop menu
  (LJNGAME.C:2620-2627).
