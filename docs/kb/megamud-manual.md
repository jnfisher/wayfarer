# The MegaMud manual, summarised

From `MEGAMUD.HLP` in `reference/megamud/` (MegaMud 1.03u for MajorMUD 1.11n, 2003), decoded in full on 2026-10-17 (103 topics, 60 with text). This is MegaMud's documented behaviour, tagged **[M]** in the rest of the KB; it says what MegaMud did, not what the game does. Exact option labels are in **bold** or `code`; defaults are only those the manual states; **[P]** marks what the manual itself calls problematic. Overview and what a successor takes from it: `kb/megamud.md`.

## 0. Architecture and core concepts (read first)

- **What it is.** A BBS terminal emulator (ANSI screen, modem/Telnet, ZModem) with a built-in automation engine for MajorMud. It was written to play "unsupervised ... around the clock". It descends from **MudWalk**, a Telix SALT script. It supports both automated play and "an intuitive extension to manual playing".
- **Modes.**
  - **Auto-mode vs Manual-mode.** Each mode has its own set of default auto-action toggles (see General Settings).
  - **AFK mode.** It changes hangup behaviour, auto-replies and alert sounds.
  - **All Auto-Responses Off ("All-Off").** MegaMud behaves as a dumb terminal.
- **Tasks and actions.** The status bar shows the current *task* (e.g. "Walking to...", "Looping at...") and the current *action* (e.g. "Resting", or whatever it is waiting for). This is the main debugging aid. A successor should expose an equivalent "task / action / waiting-for" state.
- **Game data.** These are the data tables that drive behaviour: Spells, Items, Monsters, Players, Rooms, Paths, Messages, Macros, Events, Races, Classes. All are editable via **Options > Game Data...**.
  - Data is merged in layers: **Installed defaults < For all characters < For this BBS only < For this character only**.
  - **Auto-Learn Game Data** fills the tables automatically from play.
- **Navigation model.**
  - Rooms are identified by a **checksum of room name + standard exits**, so two rooms with the same name and exits can't be told apart.
  - Paths are short recorded segments between *defined* rooms. MegaMud chains them to compute the "most direct (and hopefully shortest)" route to a room (**Goto**), or repeats a path that starts and ends in the same room (**Loop**).
  - Room flags such as **Avoid This Room** and level/class restrictions affect the route.
  - Path, room and item records carry prerequisites (gold, item, shop, fallback paths). MegaMud satisfies these automatically, e.g. it withdraws cash, buys keys or runs a "find item" path.
- **Text triggers.** The **Game Message** table is the trigger engine. A message is a substring match anywhere in a line. It can have an "ends with" message that closes a timed condition, and a response.
  - Flags map messages to states: blind, confused, poisoned, diseased, losing HP, regenerating, movement prevented, attack prevented, ends combat, last action failed.
  - Spell wear-off is detected by message name = spell name.
  - Anti-spoofing: messages are ignored inside conversation lines unless **Find In Conversations** is set.
- **Relationships.** Players and monsters can be tagged **Friend / Avoid / Enemy / Flee / Hangup**. This drives attack, avoid, flee and hangup decisions.
- **Statline dependence.** Game entry and all HP/mana tracking depend on the MajorMud prompt. The FAQ says to use `set statline full`. Custom statlines must follow strict rules (section 15).
- **Multi-character.** Each character has an INI file (`<base-dir>\Chars`). Several instances can run at once. The taskbar tray icon manages all instances. Party features assume several MegaMud instances talking to each other through telepaths and speech (`@` commands).

---

## 1. Connection and BBS logon automation

### 1.1 Communication Settings page
- **Dial-Up**: normal async modem (the default). **Telnet**: TCP/IP, which needs an existing connection or **Use Dial-Up Networking**.
- **Port**: COM1-8. **Baud Rate**: the maximum device speed; lower it if you see data errors or have a slow PC. **Parity**: normally None. **Data Bits**: 4-8, normally 8. **Stop Bits**: 1/1.5/2, normally 1.
- **Modem Initialisation String**: default `ATZ^M`.
- **Modem Hangup String**: the default contains `+++` and `ATH^M`. `~` means a half-second delay.
- **Modem Dial Prefix**: `ATDT` or `ATDP`, plus any outside-line code. **Modem Dial Suffix**: normally `^M`. The full dial command is prefix + number + suffix.
- **Direct Connection**: null-modem, which skips modem status checks. Default off.
- **Drop DTR To Hangup**: turn it off if redialling after a hangup fails.
- **Re-initialize Modem Before Redial**: default Yes.
- **Show Communication Errors**: displays `<CE-code-desc>` on screen.
- **Auto-detect ZModem Transfer**: starts an upload or download automatically.
- **Use Dial-Up Networking**: for Telnet. Turn it off when on a LAN or PPP connection.
- **Use RAS Phone Book Entry**: Windows NT only. Uses the RAS entry with the same name as the BBS.
- **RTS/CTS**: default off. **DTR/DSR**: default off. **Xon/Xoff**: default **on**.

### 1.2 BBS Settings page (per character)
- **BBS**: chooses the BBS record. New / Edit / Delete buttons; "New" copies the selected BBS.
- **User ID** and **Password**: the BBS account, which is not the character name. The password is shown masked. If the BBS account doesn't exist yet, it has to be created manually first.
- **Has MudOp Key**: the player has MudOp commands. "In low health conditions, MegaMud will break combat and **teleport back to Silvermere**".
- **Reconnect if dial fails**: default on (60 seconds). **Reconnect if carrier lost**: default on. **Reconnect if no BBS response**: disconnects, pauses and redials if the BBS doesn't respond on connect. Normally off.
- **Reconnect at cleanup**: pauses for the nightly BBS cleanup, then redials. Default on.
- **Disconnect If Exp. Rate Low** with **Mininum Exp. Rate**: logs off and does **not** redial if the exp rate drops below the minimum. Only checked while looping.
- **Max. Redials**: default 10. Resets to 0 after each successful logon. Exists to "help reduce phone bills".
- **Redial Pause**: default 60 s. **Cleanup Period**: default 60 min. Ask the sysop for the real figure.
- **Lag Pause**: extra seconds to wait before retrying an action that wasn't confirmed. Default 0, max 10.
  - [P] On laggy systems MegaMud otherwise "continually tr[ies] to repeat the same command".
  - This is the retry-on-no-confirmation model: every action has an expected confirmation and a timeout.

### 1.3 BBS Details (stored in MEGAMUD.BBS, later `<base-dir>\BBS`)
- **Name**, **Phone Number** (no formatting characters), **Alternate Number** (used on the next redial after a failure).
- **Telnet Address**, **Telnet Port** (usually 23).
- **Logon Automation**: up to **20 prompt → response pairs**.
  - Each prompt is a substring match anywhere in the received line.
  - Responses need `^M` when Enter is required, and may use control sequences, `{userid}` and `{pswd}`.
  - Remember to handle mail, telegrams and announcements.
- **Maximum Input Size**: default 128 characters per line.
- **Logoff Command**: optional quick logoff. Otherwise MegaMud hangs up or drops the Telnet session.
- **Relog Command**: logs off and on without disconnecting. Required for **Relog Instead Of Hanging**.
- **Toggle ANSI**: usually `=a`. Sent if MegaMud detects it logged on without ANSI.
- **Speech Prefix**: default `.`. **Broadcast Prefix**: default `'`.
- **PvP Level**: default 99. MegaMud won't attack players whose level differs from yours by this much or more. 0 disables PvP.
- **Max. Hours/Day**: scripting-hours limit. Only used to calculate days-to-level.
- **Global Messages** (sub-dialog, BBS Global Messages):
  - **Logon/Logoff Message**: substring of the login and logout broadcasts. These are ignored so they can't be misparsed.
  - **Shutdown Message**: on match, MegaMud logs off and waits **Cleanup Period**.
  - **Broadcast Message**: identifies global broadcasts, which go to the conversations window.
  - **Global Messages (1-3)**: miscellaneous BBS lines that are logged to conversations and the LOG, then ignored.

### 1.4 Scheduled Events (Event Details, Events tab)
- Event types:
  - **Logon**: ignored if already connected.
  - **Logoff**.
  - **Re-log**: needs a relog command.
  - **Go To**: a room, e.g. to check whether a monster is home, or visit the bank or a shop.
  - **Loop**: a path. MegaMud walks to its start first, and loops until another event or the user overrides it.
  - **Command**: as if typed.
- Timing: **At Time** (once a day) or **Every n Minutes**. **Event Disabled** turns an event off without deleting it.
- [P] At Time needs Windows Regional Settings to show AM/PM.

### 1.5 Game entry and exit
- MegaMud recognises game entry by seeing the MajorMud HP prompt (FAQ).
- **Game Entry Cmd** and **Game Exit Cmd** are optional commands (Other Settings).
- **Relog Instead Of Hanging**: when forced to hang up (PvP or low HP), MegaMud relogs at BBS level but does **not** re-enter the game. A BBS cleanup still causes a real hangup.

---

## 2. Character, account and files setup

- **File menu**: **New Character... (Alt+N)** resets to defaults and opens the settings sheet. **Open Character... (Alt+O)** loads an .INI and its game data. **Save Character**, **Save As...**, **Auto-load Last INI File**, **Recently Used Characters (1-5)**, **Connect**, **Hangup (Alt+H)**, **Disable Hangup**, **Upload** / **Download** (ZModem), **Exit** (prompts to save and to hang up).
  - **Disable Hangup** overrides every hangup condition. It is for manual play only and is turned off automatically when AFK engages.
- **General Settings**:
  - **Data Files**: the base directory.
  - **Backup data files**: on corruption, offers to restore the backup.
  - **Manual-Mode Defaults** and **Auto-Mode Defaults**: checkbox sets of auto-actions applied when the mode switches. Temporary toggle overrides reset on every mode change. [P] The help text titles both sections "Manual-Mode Defaults".
  - **Ignore Manual/Auto-Mode Defaults**: keeps the current toggles across mode changes.
- **Configuring Game Data** (directory layering):
  - `<base-dir>\Default` holds installed data. It is overwritten on reinstall.
  - `<base-dir>\Chars\All` holds data for all characters.
  - `<base-dir>\BBS\<BBS-name>` holds per-BBS data, typically Player records.
  - `<base-dir>\Chars\<char-name>` holds per-character data.
  - All four layers are merged at load. Every record has a **Use** field choosing its layer.
  - To revert a custom record, press **Remove**. Don't set its usage back to Installed Defaults; MegaMud reloads and falls back to the next least specific level.
- **Files Settings**: file names for **Macros, Events, Players, Monsters, Items, Spells, Messages, Rooms**.
  - Characters can have separate item lists, for example.
  - Characters on different BBSes should use separate player files.
- **Race Details**: **Number** (the WCC record number, unused), **Name**, **Use**, **Bonus HP's** per level, **Experience** (% added to the class base), **Attributes** (min = starting CP, max = cap per stat), **Abilities** (predefined only).
- **Class Details**: **Number**, **Name**, **Use**, **Experience** (base %), **Combat** (melee aptitude level), **HP's/Level** (random range), **Weapons**, **Armour**, **Magic** (type and level), **Abilities** (predefined only).
  - Both tables exist because "edited BBS" servers change races and classes.
- **Auto-Train** (Other Settings): trains automatically when experience to level reaches 0. It "does not train stats". The **Training** alert sound plays at 0 exp to level.

---

## 3. Navigation: rooms, paths, goto, loop, roam

### 3.1 Room Details (Rooms game data)
- Add the current room with **Game Data/Rooms/New...**.
  - A path's start and end rooms **must** be defined rooms.
  - Recording from or to an undefined room prompts to add it. Saying no cancels the recording.
- Room identity is a **checksum (unique identifier) of the room name and its standard available exits**. Same name and exits means the rooms can't be distinguished.
- Fields:
  - **Room Name**: defaults to the game name and can be renamed. [P] Don't rename the default rooms, because other records reference rooms by name, e.g. an item's shop.
  - **Group**: grouping in the Goto/Loop dialog. A blank group shows at top level.
  - **Code**: a 4-letter alphanumeric code. A path file name is start code + end code (8 characters). `@goto` also accepts the code.
  - **Use**.
  - **Shop**, **Bank**: banking needs it. [P] "the Bank of Godfrey room should not be modified".
  - **Trainer**: with Trainer set, min/max level and class mean training eligibility.
  - **Stop Before Entering Room**: stops outside when this room is the destination, e.g. a boss room.
  - **Avoid This Room**: avoided in route calculation where possible, e.g. the wastelands.
  - **Hide In Goto List**: for junction rooms that exist only to join path segments.
  - **Always In Common Rooms Popup**: the right-click favourites menu shows only the first 10.
  - **Minimum Level**, **Maximum Level**: route warnings, or MegaMud looks for an alternate path.
  - **Class Restriction**: e.g. the level 10 quest halls. "Unknown" means any class may train.

### 3.2 Recording and importing paths
- Path files have the extension **.MP** and live in the Path sub-directory, which MegaMud enumerates at startup.
- The default path set is split into **small segments** to minimise duplicated steps and let MegaMud combine them into the shortest route.
- **Edit > Record Path**: start in the first room, play the moves, then re-select Record or press Escape.
  - A details dialog follows. A loop path (start = end) asks for a file name; otherwise the name is built from the room codes.
  - The path is then usable from Goto/Loop.
- **Edit > Import...**: MudWalk (.PTH), MMTerm, MMClient, RayMud, Synterm.
  - You must stand in the start room. The import replays the old commands as if recording live.
  - Ignore Blindness doesn't apply while recording or importing, because sight is needed.
- **Fix Path Steps** (Edit menu toggle): on path errors, offers to auto-fix the path. Useful after game updates add exits, which break room recognition.
  - [P] You must stay at the keyboard to confirm.

### 3.3 Path Details
- **Name** (optional), **Use**.
- Info shown: file name, start and end room, step count.
- **Gold**: the gold needed to complete the path, e.g. tolls. If short, MegaMud scavenges coins from the ground (ignoring Cash settings) or goes to the bank to withdraw.
- **Item**: the item needed, e.g. a key. MegaMud picks one up if seen (even if not normally collected) or runs the item's acquire path first.
- **If Fails, Do**: a fallback path run when the path fails, e.g. an exit it can't pass.
- **Edit...** opens Path Steps.

### 3.4 Path Steps (step syntax and per-step flags)
- A step is a command. A movement can carry optional pre-steps in brackets, separated by commas:
  - `s[search s]`: if the south exit exists, go south. Otherwise search south until it appears, then go.
  - `e[use black star key e]`: special key. MegaMud opens the door itself.
  - `e[]` or `e[fail]`: if the exit is missing or closed, fail the path and run **If Fails, Do**. Used for timed exits.
- Per-step flags:
  - **Dark Room**: keep a light source lit. MegaMud *extinguishes* torches and lamps when it enters a step without this flag, to save them.
  - **Pitch Black**: never try to light.
  - **Rest Up Here**: rest to full HP and mana before this step, e.g. before heavy combat or a trap.
  - **Don't Rest Here**: e.g. a moat that constantly causes damage.
  - **Disarm Trap**: a trap lies in that direction. Needs the Traps ability and **Attempt To Disarm Traps**.
  - **Pick Lock**: needs the pick-locks ability and **Pick Locks Instead Of Bashing**. Otherwise the step command runs, which usually needs a key.
  - **Don't Attack**: auto-combat off for this step, restored in the next room. Used to "go into a room and out again without attacking (to promote monster regeneration)".
  - **Stash Point**: stash excess cash and items here before the step. Several stash points spread the loot and avoid full rooms.
  - **Re-Learn Room**: re-learn this step's room checksum on the next pass, then clear the flag. Fixes paths where MegaMud "always gets lost at (or re-synchronises at)" a room after game changes.
- Each step stores an **internal checksum of room name + exits** to verify position. MegaMud "re-synchronises" when lost.

### 3.5 Goto/Loop dialog and Actions menu movement
- **Goto... (Alt+G)**: room list grouped by Group. **Loop... (Alt+L)**: loop path list.
- Dialog buttons:
  - **Go**: walk there. For a loop entry, walk to its start and loop.
  - **Run**: same, but auto-combat is **off** until arrival.
  - **How**: shows the chain of paths, plus the gold and items needed on the way.
- Right-clicking the ANSI screen shows a popup of the last 10 visited locations and common rooms.
- **Auto-Roam (Alt+R)**: random roaming in the current area.
- **Back-Track**: steps back one step of the current path, e.g. to find a lost party member. [P] Its help entry is mislabelled "Auto-Roam (Alt+R)".
- **Stop (Alt+S)**: stops roaming or the path and waits. De-selecting it resumes.
- **Run Backwards** (Other Settings): when running away during a loop, move backwards along the path.

### 3.6 Auto-Roam Settings
- **Move Restrictions**: allowed directions to move. **Open Restrictions**: allowed directions to open doors.
- **Special Rooms**: up to 10 defined rooms, each with a command run on entry. Usually a direction back into the roam area, to fence roaming in.

### 3.7 Doors, locks, traps, searching, light
- **Pick Locks Instead Of Bashing**: needs the picklocks ability.
- **Max. Pick-Lock Attempts**: then falls back to bashing.
- **Max. Bash Attempts**: e.g. when helping a party member. If roaming and the character can pick, it switches to picking.
- **Attempt To Disarm Traps** with **Max. Disarm Attempts**: after that, it walks on and assumes the trap is disarmed.
- **Search When Item Needed**: searches each room until a needed item is found. **Search Each Room** sets the number of searches per room when auto-search is on.
- **Provide Light In Dimly-Lit Rooms**. Room Light spell (Spells page), otherwise a lamp or torch (item **Light Source**).
- **Help Leader Bash Doors** (Party Settings).

---

## 4. Combat

### 4.1 Combat Settings page
- **Attack Command**: usually `a`. Mystics use `ju` (jumpkick).
- **BS Weapon**: the backstab weapon. Blank means bare hands; MegaMud removes the current weapon before backstabbing.
- **Normal Weapon**: melee weapon. Blank for pure casters or hand-to-hand.
- **Alternate Weapon**: equipped against monsters that need a higher magical weapon level.
- **Shield**: set it only if the BS or normal weapon is two-handed. MegaMud equips and removes the shield as needed. A **BS** checkbox means the shield is used only while backstabbing.
- **Use Normal Weapon For Spells**: re-equip the normal weapon right after a BS, for its AC or regen. Otherwise it is only equipped when melee starts, and the BS weapon stays on if spells finish the monster.
- **Do BS Attacks**: ignored without stealth, or if sneak broke on room entry. Needs a valid BS weapon.
- **Don't BS If Multi-Attack**: skip the BS when the monster count would trigger the multi-attack spell, unless there's no spell or not enough mana.
- **Run If BS Fails**: run to the next room if the BS fails or you enter a room not sneaking.
- **Attack All Monsters**:
  - By default MegaMud attacks only **evil** (magenta) monsters. White = good, cyan = neutral.
  - This option adds neutral (cyan) monsters.
  - Monster game data relationships override it.
  - [P] Some neutrals give evil points; use `set warnings on`.
- **Polite Attacks**: in auto-mode, attack only if no non-party player is in the room. Otherwise run one room along the path.
- **Maximum Monsters**: if the room has more monsters than this, don't attack; run to the next room. Ignored with no active path or when following.
- **Maximum Experience**: the same check on the room's summed monster exp.
- **Run Distance**: rooms to run without attacking. 0 means never run.
  - [P] A large value risks running into more monsters while low on health.
- **Multi-Attack Spell**: an area spell cast before attacking when the enemy count is at least N. Typically max cast count 1.
  - It is recast until max casts are reached, **total round damage** exceeds the max damage, the monster count falls below the minimum, or mana/kai falls below the threshold.
- **Pre-Attack Spell**: cast at the start of each new monster, up to N times. For an expensive nuke or a DoT/debuff.
  - It can be conditional on max damage vs total round damage, e.g. cast only if the BS failed or the previous cast did little damage.
- **Attack Spell**: the default combat spell.
  - Optional cast limit; failed casts don't count.
  - Stops at the cast limit, when round damage exceeds the max, or when mana/kai falls below the % threshold.
  - Per-monster overrides live in Monster Details.

### 4.2 Monster Details
- Monsters not in the table are attacked by default, subject to alignment.
- **Monster Name**: a **substring** match anywhere in the real name ("guardsman" matches "nasty guardsman").
- **Find First**: test longer names before shorter ones ("dark bishop" before "bishop").
- **Use**, **Relationship** (Friend/Avoid/Enemy/Flee/Hangup).
- **Experience** and **Max HP's**: the largest value when several monsters share a name. Used by the Maximum Experience checks.
- **Pre-Attack Spell** and **Attack Spell** overrides, with optional max casts. Blank or 0 uses the combat defaults. Example: turn undead on non-living monsters.
- **Attack Priority**: higher first, then room order. Enemy players always outrank monsters. Boss monsters are preset to High. Set a boss to Low or Last to clear the adds first.
- **Not Hostile**: won't attack unless provoked, so MegaMud may rest in its room.
- **Don't Backstab**: for monsters nearly immune to backstab.
- **Check If Alive**: included in "Who Is Home?" (MudOp only). Needs a predefined map and room number.

### 4.3 Toggles (Actions menu and toolbar)
- **Auto-Combat**:
  - Off means MegaMud won't attack anything, *even if attacked*.
  - It auto-enables when HP falls below the run %, as a safety measure.
  - Goto with **Run** turns it off temporarily.
- **Auto-Nuke**: room-effect spells. Only works while auto-combat is on.
- **Auto-Sneak**: needed for backstabs. Ignored without stealth.
- **Auto-Hide**: hides when not moving or resting.
- **Use Supernatural Stealth**: sneak even when the stealth stat reads 0.
- **Dont Move Unless Sneaking**: MajorMud won't allow sneaking for about a round after being attacked ("appears to be a MajorMud bug"). By default MegaMud moves on unsneaked until about **4 s** after the last attack. This option waits until sneaking works.
- Sneak detection: entering a room without the "Sneaking..." message counts as a failed sneak, so there's no BS.
- **Break Combat Before Running**: sends `break` before fleeing. May reduce the chance of being followed. [P] Slower on lag.
- **Show Combat Round Totals** (Display): shows per-round damage and the running total on the current monster.
- Combat end is detected by `*** Combat Off ***` or by messages flagged **Ends Combat**.

---

## 5. Health, resting, healing, running, hanging up (Health Settings page)

All values are percentages. HP and Mana/Kai have parallel ladders.

| Option | Meaning |
|---|---|
| **HP Rest Maximum** | HP % counted as "full". Set a little under 100 so MegaMud doesn't wait a whole rest cycle for a few HP. Low-level characters use 100%. |
| **HP Rest If Below** | Keep moving along the path until HP drops below this, then stop and rest (optionally casting heals). |
| **HP Heal (Resting)** | While resting, cast heals only below this %. Otherwise rest normally to save mana. |
| **HP Heal (Combat)** | Below this % in combat: break combat, heal, re-engage. Keep it low; re-engaging is costly. |
| **HP Run If Below** | Assume the fight is lost and run to the next room. Keep running if followed or the new room has monsters, until it can rest above this %. 0 = never run. |
| **HP Hang If Below** | Hang up and don't redial (with relog, don't re-enter the game). [P] Some BBSes penalise hanging up in combat (item loss). 0 disables; All-Off also disables. |
| **HP Heal Period** | Minimum seconds between heal casts. 0 = heal as often as needed. |
| **Mana/Kai Rest Maximum** | Mana % counted as full. Set it low to rest only HP. |
| **Mana/Kai Rest If Below** | Stop and rest to Mana Rest Maximum when below this. |
| **Mana/Kai Heal (Resting)** | Minimum mana to cast heals while resting. |
| **Mana/Kai Heal (Combat)** | Minimum mana to heal in combat. Set it high to save mana for attack spells. |
| **Mana/Kai Run If Below** | For casters that depend on mana: run, and keep running if followed, until mana can be rested above this. 0 = never run. |
| **Mana/Kai Bless If Above** | Cast blesses only above this %. It must be at least the highest bless cost or the casts fail. |
| **Pre/Post Rest Commands** | Sent before and after resting, e.g. swap regen rings. |

Related behaviour:
- **FAQ, "Why won't my character rest?"**: set the HP rest %, turn on **Auto-Rest/Heal**, and check you aren't poisoned. Poison prevents resting.
- **Auto-Rest/Heal** toggle: off means no heals and no `rest`. Used when sneaking manually so resting doesn't give your position away.
- **Ignore Poison**: carry on while poisoned; otherwise wait it out. Good for the Kang race and high-HP characters.
- **Ignore Blindness**: otherwise wait for sight. **Ignore Confusion**: otherwise wait it out.
- **If Forced To Hide** (PvP page): seconds to wait when forced to hide, e.g. while running or waiting off poison.
- **Hangup When Not AFK**: on means hang up on low HP or mana even when not AFK. Off means low-HP hangups only happen in AFK mode.
- **Hangup In All-Off Mode**: re-allows safety hangups while All-Off is active.
- **Hangup If Naked**: hang up if carrying nothing. Only in AFK mode and not with All-Off.
- **Has MudOp Key**: on low health, break combat and teleport to Silvermere.
- Sounds: the **Low HP's** alert plays when HP falls below the run %. The **Hangup** alert plays when MegaMud is forced to hang up.

---

## 6. Spells

### 6.1 Spells Settings page
- **Heal HPs**: the heal spell, governed by the Health thresholds.
- **HP Regen** and **Mana Regen**: cast before resting. Skipped if a **Regenerating** message is active.
- **When HP Full** and **When Mana Full**: cast once resting reaches the max %.
- **Bless Spells**: up to **10**, kept active at all times. Not cast in combat (unless **Bless During Combat**) and only above **Mana/Kai Bless If Above**.
- **Cure Blindness**: blank means wait it out. **Cure Poison**: MegaMud normally doesn't stop for poison unless HP is below the rest %. **Cure Disease**: blank means rest it out. **Freedom**: when it can't move; blank means wait. **Room Light**: blank means use a lamp or torch. All of these are cast out of combat only, except Cure Disease (section 8).
- **Bless When Resting**: default off. [P] It disrupts regeneration and spends mana.
- **Bless During Combat**: default off. [P] It reduces combat effectiveness.

### 6.2 Spell Details (Spells game data)
- **Code**: the in-game short command. Item-only spells get an arbitrary `#`-prefixed code.
- **Name**: descriptive only.
- **Minimum Level**: hides spells above your level from the Spells and Combat pickers.
- **Required Mana/Kai**: the cost, used for threshold decisions.
- **Timed Duration**: wear-off is tracked through Messages; the message name must equal the spell name.
- **Evil In Combat**: evil if cast on players. These are listed on the Combat and PvP pages.
- **Self / Player / Monster / Area**: valid targets. Area spells are cast without a target.
- **Type**: spell class and level. "Any" means anyone who can use magic items.
- **Special Command**: e.g. using an item. Blank means cast by code.
- **Spells Book** (Options): lists every possible spell for your character so you can spot missing ones.

---

## 7. Looting: cash, items, selling, stashing, banking

### 7.1 Cash Settings page
- **Cash Collection Options**: per currency, **Collect / Ignore / Discard**.
  - Default: Copper Farthings and Silver Nobles = **Ignore**; higher currencies = **Collect**.
  - Discard drops the currency. Useful in chest areas.
  - Collection needs auto-get.
- **Don't Become Heavy**: don't pick up anything that would make you Heavy. Normally off, because when Heavy MegaMud goes to the bank, or to a shop to sell auto-sell items. **Don't Become Medium**: the same at Medium.
- **Collect After Combat**: loot only once the room is clear. For laggy connections, where you take hits while picking up coins.
- **Auto-Deposit On Wealth**: detours to the bank when the wealth carried (in copper) exceeds X. Deposits everything and returns to the same path position.
- **Auto-Deposit On Coins**: the same, on total coin count.
- **Banking Done At...**: Bank of Godfrey by default. Evil players use Rhudaur; players near the dwarven village use Khazarad.
- **Name Of Runic Currency**: default "runic coin". Stored per BBS.
- FAQ [P] "MegaMud collects copper and silver when it's not supposed to": when it needs money for a toll or purchase, it collects any cash until it has enough, then reverts to the Cash settings.
- **Deposit All** (Actions): deposits cash minus "your specified minimum to keep on hand". [I] There is a keep-on-hand setting, but the manual doesn't describe it elsewhere.

### 7.2 Item Details (Items game data)
- Unknown items are **auto-added** when seen.
- **Item Name**, **Use**.
- **Auto-Collect**: pick up when seen, up to the max.
- **Auto-Equip**: equip if the item's usage (slot) is known and nothing of that usage is equipped.
- **Auto-Find**: with **If Needed, Do** = a path that obtains the item, e.g. a key.
- **Auto-Buy**: buy it when needed. Needs **Price** and **Bought/Sold**.
- **Auto-Sell (or Stash)**: sell at its shop above the minimum count. At a path **Stash Point**, stash it instead.
- **Light Source**: MegaMud lights it when needed.
- **Cannot Be Taken**: scenery ("furniture") that is never collected.
- **Must Have Minimum**: mandatory. If the count drops below the minimum, MegaMud stops looping and fetches or buys more, up to Maximum To Get. Example: torches.
- **Minimum To Keep**: default 0 (None). Always pick up until reached.
- **Maximum To Get**: default 0 (no limit). Also guards against players dumping items in your path to overload you.
- **Weight**: default 0 (unknown). Used to predict Heavy. MegaMud auto-calculates a new item's weight when all other inventory weights are known.
- **Price**: the shop's listed price. MegaMud adjusts for your charm and fetches money from the bank if needed.
- **Item Usage**: required for Auto-Equip. **If Needed, Do**: required for Auto-Find. **Bought/Sold**: the shop room name, required for Auto-Sell.

### 7.3 Inventory helpers
- **Get All**, **Drop All** (first press drops only non-equipped items, a second press drops everything), **Equip All**, **Deposit All** (Actions menu).
- **Inventory Calculator** (Options): lists items you can wear given class, level, strength and alignment. It can mark everything you're wearing as auto-get and auto-equip.
- **Auto-Get Items** and **Auto-Get Cash** toggles: when off, MegaMud collects nothing unless it is needed to reach a destination.

---

## 8. Status conditions and Game Messages (trigger engine)

Game Message Details fields:
- **Name**: equals the spell name for bless and spell effects; used to detect wear-off.
- **Message** (mandatory): a substring anywhere in a received line.
- **Ends With**: optional. Only searched after the first message has matched. The condition lasts until it arrives.
- **Response**: optional text sent on match. Supports control sequences and substitution variables captured from the match: `{source}`, `{target}`, `{dmg}`, `{1}`..`{5}`.
- Condition flags:
  - **Blinded**: cast cure blindness.
  - **Confused**: rest until clear.
  - **Poisoned**: can't rest; cast cure poison.
  - **Diseased**: cure disease, *may be cast mid-combat* because disease drains HP fast.
  - **Losing HP's**: e.g. freezing or burning; rest until it ends.
  - **Regenerating**: skip the regen spell.
  - **Movement Prevented**: cast Freedom or wait.
  - **Attack Prevented**: e.g. fear; don't attack.
  - **Ends Combat**: combat ended without `*** Combat Off ***`.
  - **Last Action Failed**: repeat the last action.
- **Action**: a pick-list of actions; the manual calls them "self-explanatory" and doesn't list them.
- **Find In Conversations**: off by default, so players can't spoof game messages in gossip or speech.
- **Use When Chasing**: only evaluated while chasing a player. Usually carries a command to follow non-standard exits.
- **Disabled (Dont Use)**: soft delete.

---

## 9. PvP and player handling

### 9.1 PvP Settings page
- Reaction to a PvP attack (choose one):
  - **Hangup Immediately**. [P] There may be a combat-hangup penalty.
  - **Flee Then Hangup**: flee, then hang up **Flee Hangup Delay** seconds after the last PvP hit. Safe if the delay covers the combat timer.
  - **Flee**: to **Flee To**, or along the loop, until it reaches the destination or a safe spot to rest to full.
  - **Attack**: [P] "dangerous ... most people can out-smart a script".
- **Flee To**: a room. Blank means continue on the current course.
- **No. Of Rooms To Flee**: rooms to run without attacking. [P] Keep it small; running is risky for non-sneakers.
- **Notify Gang Members**: gang broadcast naming the attacker.
- **Reconnect After PvP**: wait the safety period, then redial. [P] The attacker may be camping the room.
- **Track Enemies**: for characters with tracking. Hunts players marked Enemy and returns to the path when the track is lost. **Auto-Track** toggle; the interval between attempts is set on this page.
- **If Forced To Hide**: seconds to wait.
- **PvP Spell 1** and **PvP Spell 2**: once-only spells cast on an attacker or an Enemy player in the room. Normal attacks follow.
- BBS **PvP Level** range (section 1.3). **Defend Party When PvP** (section 10).

### 9.2 Relationships (players and monsters)
- **Friend**: never attacked. May use limited remote queries. Same-gang players become Friend automatically.
- **Avoid**: don't attack and keep out of their way.
- **Enemy**: attacked on sight. [P] Evil points for attacking good players.
- **Flee**: run from it. Attack only if you can't move. Its presence overrides everything else in the room. Example: gigantic black ooze.
- **Hangup**: hang up on sight. For stalkers.

### 9.3 Player Details (Players game data)
- The table is built automatically from encounters.
- **Player Name** (first name), **User ID** (needed for paged remote commands), **Use**, **Relationship**.
- **Race** and **Class**: learned automatically by looking at the player and from the "top 50" list. Used by the exp calculator to infer level.
- **Doesn't use MegaMud**: don't send `@health` when partying; use MudWalk-style `!wait` / `!ok`.
- **Invite To Party If Seen**: re-invite stragglers when leading, e.g. to re-form the party after a cleanup.
- **Join Party If Invited**: auto-join.
- **Chase Player If Seen**: follow when they leave the room.
- **Play Alert When Player Enters**: plays the PvP sound when they enter the game.
- **Don't Heal Or Bless**: e.g. witchhunters, who resist anyway.
- **Don't Auto-Delete This Character**: default off.
- **Allowed Remote Control**: per-category permissions; see section 11.
- **Delete Inactive Players** (Other Settings): purge after N days. 0 = never.
- **Look At Players (To Record Their Inventories)** (Talk): periodically `look` at players to track their gear. [P] Custom descriptions can confuse the parser; it can be turned off per player.
- **Greet Players When First Met** (Talk): Friends only. Resets on restart.
- **Show Online Players** window. **Who Is Home?** (Options): MudOp check of whether boss monsters are alive; the list is set by the monster **Check If Alive** flag.

---

## 10. Party and group play (Party Settings page and protocol)

- **Party Rank**: rank taken when auto-joining.
- **Party Heal**: 2 spells, each with an HP % trigger; the stronger one at the lower %. Cast on members when they're low or when they say `@heal`.
- **Ask For Healing**: says `@heal` below X%. Set 0% if your group can't heal.
- **Party Bless Spells**: up to 4, cast only while partying.
- **Attack Last If In Party**: tank mode. Re-attack whenever anyone else attacks, because monsters target whoever hit them last. Also switches to the monster the other player attacked.
  - [P] Two MegaMuds with this on will keep re-attacking in response to each other.
- **Share Damage**: re-attack (take the aggro) if you have the most HP remaining. All members need it on, with Attack Last off.
- **Request Party Health**: telepath `@health` to members on join, to learn their max HP and work out actual HP from party data.
- **Attack In Reverse Order**: attack the last-listed monster first, to match MudWalk-style scripts. Only within equal priority, and only in a party.
- **Attack What Other Members Attack**: switch to a member's target.
  - [P] Can pull protector monsters ("protected by a leader monster"). Turn it off in such areas.
- **Defend Party When PvP**: when you're attacked, MegaMud says `@kill <attacker>`. Members with this option react as if attacked themselves (hang up, flee or attack per their PvP setting). It can also be said manually.
- **Auto-Share Collected Cash**: the leader issues `@share` at loop start and at the bank. Members with the option on share their takings.
- **Help Leader Bash Doors**.
- **Ignore @party If Following**: [P] `@party` "can also be easily abused" by an untrusted leader.
- **Ignore @wait If Leading**: still waits if anyone is below the wait %.
- **Ignore @panic If Following**: don't teleport or hang up when the leader panics; wait for the leader to return.
- **Maximum Monsters** and **Maximum Experience** (party versions): override the solo limits when leading a party.
- **Wait For Party Members**: as leader, rest if any member is below this HP %, even without `@wait`.
- **If Leading Wait No Longer Than**: minutes. Guards against a `@wait` whose `@ok` was lost.
- **Par Frequency**: default **15 s** in combat, twice that out of combat. Polls `par` for party HP.
- **Show Party Status** window: member list with health gauges. **Auto-Hide Party Status Window**: default on.

Party protocol (MegaMud ↔ MegaMud, via telepath or speech):
- `@wait` (follower → leader, telepathed): follower needs to rest. `@ok`: follower is done resting. The leader tracks several followers and waits for all of them.
- `@comeback` (follower → leader when stuck): the leader backtracks **up to 5 rooms**, re-invites, then continues.
- `@heal`, `@blind`, `@diseased`, `@held` (spoken): requests for aid or a cure from capable members.
- `@party <cmd>` (spoken by the leader): all followers run the command, e.g. `go hole`. Often embedded in path files. The leader then re-checks and re-invites.
- `@kill <player>`: defend the party. `@share`: share cash. `@panic!` (leader, before teleporting or hanging up on low HP): followers do the same unless they ignore it.
- MudWalk compatibility: `!wait` and `!ok` from followers are recognised.

---

## 11. Remote control (`@` commands, telepathed, spoken or paged)

Each player has permissions per category (Player Details > **Allowed Remote Control**). A disallowed command is silently ignored. An allowed command that fails gets a short failure message.

**Talk** page settings:
- **Disallow All Remote Control Commands**: a global off switch.
- **Warn User If Remote Command Is Invalid** with **Invalid Remote Command Reply**: blank means no reply.
- **Remote Diverting**: which message types `@divert` forwards.

| Category | Commands |
|---|---|
| Query Version | `@version` (and `@home <monster>` if the target is a MudOp) |
| Query Experience | `@exp`: exp since start plus the current exp rate |
| Query Health/Status | `@health` (HP, mana/kai, states such as Resting or Blinded), `@status` (task + action), `@lives` |
| Query Location | `@where` (room + exits), `@who` (players and monsters in the room), `@what` (items in the room), `@seen <player>` (last seen where) |
| Query Inventory | `@have <item>` (exact full name; returns the count), `@enc`, `@wealth` (in copper) |
| Request Invite | `@invite` (invites the sender to your party) |
| Move Character | `@stop`, `@rego` (resume roaming or path after a stop), `@goto <room or 4-letter code>`, `@loop <path-file>` |
| Execute Commands | `@do <cmd>` (output not returned) |
| Hangup/disconnect | `@hangup`. [P] Watch for hangup penalties mid-combat; check `@status` first. `@do x` is safer. |
| Alter Settings | `@attack-last`, `@auto-all`, `@auto-combat`, `@auto-nuke`, `@auto-heal`, `@auto-bless`, `@auto-light`, `@auto-cash`, `@auto-get`, `@auto-sneak`, `@auto-hide`, `@auto-search`, each taking `[on|off]` or toggling |
| Divert conversations | `@divert [on|off]` (forward telepaths and pages to the sender, so one person can run several characters; cancel by saying `@divert`), plus `@debug` and `@capture` (listed but undocumented) |

Commands with no category listed in the manual:
- `@looponce <path>`: one lap of another loop, then back to the original loop.
- `@roam [on|off]`, `@settings` (lists the enabled auto settings), `@reset` (resets internal flags and statistics), `@relog`, `@join` (join the sender), `@forget` (remove the sender from the party).
- `@get-all`, `@drop-all`, `@equip-all`, `@deposit-all`.
- Party-internal: `@wait`, `@ok`, `@comeback`, `@heal`, `@blind`, `@diseased`, `@held`, `@party`, `@kill`, `@share`, `@panic!`.

- Uses: checking on your own bot from another account, or letting a trusted player drive your character while you're away.
- [P] "this facility can be quite dangerous in the hands of an enemy player."

---

## 12. Conversations, AFK and messaging

- **Show Conversations** window: history of player communication, filterable by type (e.g. telepaths only). BBS broadcasts and global messages go here too.
- **Log Conversations**: to a log file, usually **MEGAMUD.LOG**. [P] It can fill the disk on long runs.
- **AFK mode** (Alt+K):
  - Replies **AFK Reply** to telepaths. Blank means no reply.
  - Wrap the reply in braces so two MegaMud bots don't AFK-reply to each other forever. The manual's example appears as `"not here!"`; the braces seem to have been lost from the help source.
  - **Auto-AFK** after an idle period. The Actions-menu text says 5 minutes; the Talk page says "the period specified".
  - **Auto-AFK When Minimized**. **User Input Cancels AFK**.
  - AFK mode gates hangups (Hangup When Not AFK, Hangup If Naked), alert sounds (Play Alert Only When AFK) and Disable Hangup (turned off when AFK engages).
- **Popup AFK Messages**: shows messages received while AFK when you return. **Show All AFK Messages**: all messages, or only those addressed to you.
- **Random Responses**: in any output string, `a||b||c` picks one at random. An empty alternative means sometimes no response.

---

## 13. Macros, hotkeys, toolbar and input

- **Macro Details** (Macro tab): **Use**, **Keystroke** (with SHIFT/CTRL/ALT variants), **Macro** text (may contain several commands, control sequences and `^M` for Enter).
- **Control Character Sequences**, usable anywhere text is sent:
  - `^M` CR, `^[` ESC, `^~` `^^` `^|` literal characters.
  - `~` half-second delay, `|` (as `||`) random alternatives.
  - `{userid}`, `{pswd}`.
  - Message variables `{source}`, `{target}`, `{dmg}`, `{1}`..`{5}`.
- **Command Splitter Character** (Other Settings): splits multiple commands typed in Quick-Tools and Conversations input. Blank disables.
- **Paste (Ctrl+V)**: if text is already typed at the prompt, it is combined with *each* clipboard line and run as a series of commands.
- **ToolBar Settings**: **ToolBar Items** (Visible, Seperator), **Show Toolbar**, **Vertical Toolbar**, **Custom Command** (the last 4 slots send user commands). Groups that don't fit are hidden.
- **Show Quick-Tools**: a floating toolbar of one-click common commands.

---

## 14. Statistics, time and exp analysis, calculators

- **Show Player Statistics**: exp made while scripting, combat accuracy, trivia.
- **Show Session Statistics**: dials, connects, carrier losses, PvP attacks, players seen, etc.
- **Show Time Analysis**: time spent per activity, to tune settings for a better exp rate. A successor should keep a per-state time ledger.
- **Exp rate** drives **Disconnect If Exp. Rate Low** / **Mininum Exp. Rate** and `@exp`. **Max. Hours/Day** drives the days-to-level estimate.
- **Experience Calculator** (**CP/Exp. Calculator**): exp table for levels 1-100 for any race and class, with race and class exp penalty %. Also used to infer a player's level from their exp (for low PvP level ranges) and for character planning.
- **Show Combat Round Totals**.

---

## 15. Display, terminal and logging

- **Rows/Columns**: should match the BBS account screen size. **Scale Font Size**. **Font...**: monospaced fonts only.
- **Show Information Messages**: MegaMud's own notes, shown white-on-blue inline.
- **Show Data When Sent**: echoes outgoing text in dark grey, overwritten when the echo returns. Shows lag visually.
- **Confirm Exit**, **Confirm Hangup**, **Confirm Save Settings**, **Confirm Deletes**. [P] The help titles the hangup entry "Confirm Exit" by mistake.
- **Color Mapping**: one-to-one remapping of host colours, with a Default button.
- **Use TaskBar Status Icon**: one tray icon for all instances. Double-click opens all; right-click picks a character and offers show, hide or close all. **Always Start Minimized**.
- **Back-Scroll... (Alt+B)** with **Back-Scroll Buffer Size**: max 32,000 bytes. [P] Larger buffers slow MegaMud.
- **Capture... (Alt+C)**: logs all traffic to a file. Debug captures and trace files are what support asks for.
- **Reset ANSI**: fixes a screen scroll region broken by stray ANSI size codes from line noise.
- **Reset Custom Profile**: sends commands to reset game profile settings that break MegaMud. "If something is not working as it should, try using this option".
- **Sound** toggle and **Alert Settings** (wave files): **Hangup, Idle** (stopped and can't do anything), **Low HP's, PvP, Training, Local** (say or yell), **Page, Telepath, Gossip, Auction, Gangpath, Broadcast**. **Play Alert Only When AFK**.
- **Custom Statline Settings**, the rules for prompt parsing:
  1. The prompt starts with `[HP=` and the current HP. With a multi-line prompt, only the last line needs this; earlier lines are ignored.
  2. The last line ends with `]:`. Everything else sits between `[HP=` and `]:`. `(Resting)` straight after `]:` is also detected.
  3. Casters must include `MA=` or `KAI=` and the value, on the last line.
  4. Anything else inside the delimiters is ignored.

---

## 16. "Smart" behaviours worth copying

- Route planning over a graph of path segments, using room flags (avoid, level and class limits, stop-before-entering, hidden junctions) and prerequisites (gold, item, fallback path, item-acquire path). **How** previews the route with its gold and item needs.
- Errand interrupts that resume the loop at the same position: bank on wealth or coin count, sell when Heavy, restock **Must Have Minimum** items, withdraw gold for tolls.
- Room verification by name+exits checksum per step, with resync when lost, **Re-Learn Room** after game changes, and **Fix Path Steps** interactive repair.
- Conditional step syntax `dir[prep,...]` including search-until-exit, key use and forced failure that branches to a fallback path.
- Light management: extinguish light sources in naturally lit rooms to save them; Dark Room and Pitch Black flags.
- Damage-aware spell logic: casts are bounded by count, per-round damage total and mana %. Pre-attack, multi-attack, attack and per-monster overrides.
- Danger gates: max monsters and max room exp (stricter solo, looser party), relationships, Polite Attacks, Run If BS Fails, run distance, run backwards on loops.
- Learning: auto-learn items (and weights by subtraction), monsters, player race and class (look, top 50), gang → Friend.
- Party coordination protocol, including health polling (`par`, `@health`), tank modes, and "wait for all followers' `@ok`".
- Anti-spoofing: game-message matching ignores conversation lines, and remote control has per-player permissions.
- Lag tolerance: action confirmation timeouts plus **Lag Pause**, **Show Data When Sent**, **Collect After Combat**.
- Unattended safety: AFK-gated hangups, hang up on low HP, PvP or naked, cleanup detection and redial, exp-rate floor, max redials, scheduled logon, logoff and relog.

## 17. Problems and annoyances the manual admits

- Lag makes MegaMud repeat commands, hence **Lag Pause**.
- Hanging up in combat can be penalised on some BBSes (applies to HP Hang, PvP hangup and `@hangup`).
- Attacking neutral monsters can give evil points; use `set warnings on`.
- Game updates add exits or rename rooms, which breaks checksums and leaves MegaMud lost. Workarounds: **Fix Path Steps**, **Re-Learn Room**.
- Rooms with the same name and exits can't be told apart.
- MajorMud won't let you sneak for about a round after being attacked; MegaMud moves on after about 4 s unless told otherwise.
- MegaMud collects copper and silver against its settings when it needs money.
- Two Attack-Last MegaMuds re-attack in an endless loop. Attack-What-Others-Attack can pull protector monsters.
- `@party` and remote control can be abused. PvP auto-attack is easily outsmarted. Reconnecting after PvP may meet a camping attacker.
- Long runs fill the disk with the conversation log. Large back-scroll buffers slow the program.
- Custom player descriptions confuse look-based inventory tracking.
- Blessing while resting or in combat hurts regen and combat.
- Running far while low on HP finds more monsters.
- FAQ items: MegaMud won't act in game until the prompt is the default statline. It won't rest when poisoned or with auto-rest off. It won't backstab without stealth, a BS weapon, Do BS, auto-sneak and a confirmed "Sneaking...". It hangs up on exit unless DTR is ignored (`&D0`).
- Errors in the help text itself: the Back-Track entry is mislabelled as Auto-Roam; both mode-defaults entries are titled "Manual-Mode Defaults"; Confirm Hangup is titled "Confirm Exit".

