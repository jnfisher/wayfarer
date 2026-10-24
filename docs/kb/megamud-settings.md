# MegaMud settings catalogue

Every setting in MegaMud v2.0 Alpha's dialogs, from `reference/megamud/` (2026-10-17): the dialogs, menus and strings in `MegaRes.dll`, tied to INI keys and defaults by disassembling `megamud.exe`'s settings loader (`0x4322a0`), its new-character defaults routine (`0x430c00`) and each settings page. Nothing was run. All of it is **[M]** (MegaMud's behaviour); within it, "code" means the control, INI key and default were traced to the same field, and "name" means matched by name only. Overview: `kb/megamud.md`; what each option did, per the manual: `kb/megamud-manual.md`.

Defaults are what a new character gets when a key is missing from its INI.

Storage: character settings live in `Chars\<name>.ini` (Open/Save character). The global `MegaMud.INI` next to the exe holds `AutoLoadIni`, `LastIni1..5` and `WindowPos`. Per-BBS settings are in `BBS\<bbs>\BBS.INI`. Game data (items, monsters, spells, messages, rooms, paths, players, macros, races, classes) are `.md` databases under `Default\`, `Chars\All\` or `Chars\<char>\`, with paths in `*.mp`.

## 1. Settings dialog (Options > Settings...): 16 tabs

Property sheet "Settings...", tabs in display order: General (dlg 370), Display (dlg 310), Toolbar (dlg 367), BBS (dlg 304), Events (dlg 341), Health (dlg 315), Spells (dlg 345), Combat (dlg 308), PvP (dlg 336), Party (dlg 327), Cash (dlg 307), Sounds (dlg 301), Auto-roam (dlg 302), Talk (dlg 361), Statline (dlg 365), Other (dlg 326).

### 1.1 General tab (dialog 370)

Startup behaviour and which auto-toggles are switched on when MegaMud is put into manual vs auto mode.

**Directories**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Data Files | edit | max 256 chars | MegaMud/DataDir | `%s` | code |
| Backup data files when making changes | checkbox |  | MegaMud/BackupData | `1` | code |

**Initial**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Do nothing (StartTask=0) | radio |  | MegaMud/StartTask | `0` | code |
| Begin looping at (StartTask=1) | radio |  | MegaMud/StartTask | `0` | code |
| Begin auto-roaming (StartTask=2) | radio |  | MegaMud/StartTask | `0` | code |
| ...loop path to begin | combo (editable) |  | MegaMud/DefaultLoop |  | code |
| Auto-connect when INI loaded | checkbox |  | MegaMud/AutoConnect | `0` | code |
| Ignore manual/auto mode defaults | checkbox |  | MegaMud/NoModeDefs | `0` | code |

**Manual-Mode Defaults** (checklist, list 1978)

| Option | INI [Section]/Key | Default | Conf. |
|---|---|---|---|
| Auto-Combat | MegaMud/DefCombat1 | `1` | code |
| Auto-Nuke | MegaMud/DefNuke1 | `0` | code |
| Auto-Heal/Rest | MegaMud/DefHeal1 | `1` | code |
| Auto-Bless | MegaMud/DefBless1 | `1` | code |
| Auto-Light | MegaMud/DefLight1 | `1` | code |
| Auto-Get Cash | MegaMud/DefCash1 | `1` | code |
| Auto-Get Items | MegaMud/DefGet1 | `1` | code |
| Auto-Sneak | MegaMud/DefSneak1 | `1` | code |
| Auto-Hide | MegaMud/DefHide1 | `1` | code |
| Auto-Search | MegaMud/DefSearch1 | `0` | code |
| Auto-Tracking | MegaMud/DefTrack1 | `0` | code |

**Auto-Mode Defaults** (checklist, list 1979)

| Option | INI [Section]/Key | Default | Conf. |
|---|---|---|---|
| Auto-Combat | MegaMud/DefCombat2 | `1` | code |
| Auto-Nuke | MegaMud/DefNuke2 | `1` | code |
| Auto-Heal/Rest | MegaMud/DefHeal2 | `1` | code |
| Auto-Bless | MegaMud/DefBless2 | `1` | code |
| Auto-Light | MegaMud/DefLight2 | `1` | code |
| Auto-Get Cash | MegaMud/DefCash2 | `1` | code |
| Auto-Get Items | MegaMud/DefGet2 | `1` | code |
| Auto-Sneak | MegaMud/DefSneak2 | `1` | code |
| Auto-Hide | MegaMud/DefHide2 | `1` | code |
| Auto-Search | MegaMud/DefSearch2 | `0` | code |
| Auto-Tracking | MegaMud/DefTrack2 | `0` | code |

### 1.2 Display tab (dialog 310)

Terminal/ANSI and window behaviour. Fonts are chosen from the menu (Options > Font) and saved as Display/AnsiFont, AnsiSize, AnsiWeight, AnsiCharSet, AnsiFamily; conversation window font as TalkFont/TalkSize/...

**ANSI Screen**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Rows | edit | max 2 chars | Display/AnsiRows | `25` | code |
| Columns | edit | max 3 chars | Display/AnsiCols | `80` | code |
| Scale ANSI font | checkbox |  | Display/ScaleAnsi | `1` | code |
| Show information messages | checkbox |  | Display/ShowInfo | `1` | code |
| Show reason for running | checkbox |  | Display/ShowRunMsg | `1` | code |
| Show combat round totals | checkbox |  | Display/ShowRounds | `0` | code |
| Show data being sent | checkbox |  | Display/ShowSend | `0` | code |

**Show Confirmations**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Confirm exit | checkbox |  | Display/ConfirmExit | `1` | code |
| Confirm hangup | checkbox |  | Display/ConfirmHangup | `1` | code |
| Confirm save settings | checkbox |  | Display/ConfirmSettings | `1` | code |
| Confirm deletes | checkbox |  | Display/ConfirmDelete | `1` | code |

**Color Mapping**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| ANSI colour slot list (16 colours) | list |  | Display/ForeClr%d, BackClr%d |  | name |
| Edit: Foreground | radio |  |  |  |  |
| Edit: Background | radio |  |  |  |  |
| Edit: Border | radio |  | Display/BorderClr | `8421504` | name |

**Other**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Popup AFK messages | checkbox |  | Display/PopupAfkMsgs | `1` | code |
| Show all AFK messages | checkbox |  | Display/ShowAllAfk | `0` | code |
| Auto-hide party status window | checkbox |  | Display/AutoHideParty | `1` | code |
| Use taskbar status icon | checkbox |  | Display/UseTaskBar | `0` | code |
| Always start minimized | checkbox |  | Display/StartMinimized | `0` | code |

### 1.3 Toolbar tab (dialog 367)

Which buttons appear on the toolbar.

**Toolbar Items**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Toolbar items (Visible / Separator per item) (value 0 hidden, 1 visible, 2 visible + separator (inferred)) | list |  | Toolbar/<item> (New, Open, Save, Connect, ... CmdSearch) |  | name |
| Visible (selected item) | checkbox |  |  |  |  |
| Separator (selected item) | checkbox |  |  |  |  |

**Toolbar Options**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Show toolbar | checkbox |  | Toolbar/ShowTBar | `1` | code |
| Vertical toolbar | checkbox |  | Toolbar/VertTBar | `0` | code |

**Custom command**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Custom command text (for "Custom Command 1-4" items) | edit | max 80 chars | Toolbar/Custom%d |  | name |

### 1.4 BBS tab (dialog 304)

Connection target and account. Each BBS has its own BBS\<name>\BBS.INI (see "BBS Details" dialog below); account and reconnect policy live in the character INI.

**BBS**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| BBS list (New/Edit/Delete -> "BBS Details" dialog) | list | Password | BBS/BbsName (selected) |  | code |

**Account**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| User ID | edit | max 30 chars | BBS/UserID |  | code |
| Password (not read by the plain-text loader; stored separately/encoded) | edit (password) | max 30 chars | BBS/Password |  | name |
| Has MudOp _access | checkbox |  | BBS/IsMudop | `0` | code |
| sys goto (MudOp teleport command alias) | edit | max 20 chars | BBS/IsMudop, BBS/SysGoto | `0` | code |

**Connection**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Reconnect if dial fails | checkbox |  | Comms/RedialConnect | `1` | code |
| Reconnect if carrier lost | checkbox |  | Comms/RedialCarrier | `1` | code |
| Reconnect if no BBS response | checkbox |  | Comms/RedialNoResponse | `0` | code |
| Reconnect after cleanup | checkbox |  | Comms/RedialCleanup | `1` | code |
| Disconnect if low exp. rate | checkbox |  | Comms/LogoffLowExp | `0` | code |
| Max. re-dials | edit | max 4 chars | Comms/RedialMax | `10` | code |
| Re-dial pause | edit | (seconds); max 4 chars | Comms/RedialPause | `60` | code |
| Cleanup period | edit | (minutes); max 4 chars | Comms/CleanupPeriod | `60` | code |
| Min. exp. rate | edit | (per hour); max 6 chars | Comms/MinExpRate | `10000` | code |
| Lag pause | edit | (seconds); max 6 chars | Comms/LagWait | `0` | code |

### 1.5 Events tab (dialog 341)

Timed logon/logoff/relog/goto/loop/command events.

**(ungrouped)**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Scheduled events list (New/Modify/Remove -> "Scheduled Event" dialog) | list view |  | Schedule/Event%d = "h:m:s:text" |  | name |
| AFK only | checkbox |  | MegaMud/EventsAfkOnly | `1` | code |
| Disable | checkbox |  | MegaMud/DisableEvents | `0` | code |

### 1.6 Health tab (dialog 315)

THE core survival thresholds: when to rest, heal, run and hang up, as % of max HP and mana/kai.

**Health (HPs)**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Rest max. (rest until HP reaches) | edit | %; 0..100; max 3 chars | Health/HpFull% | `99` | code |
| Rest if below | edit | %; 0..100; max 3 chars | Health/HpRest% | `90` | code |
| Heal (rest) | edit | %; 0..100; max 3 chars | Health/HpHeal% | `60` | code |
| Heal (combat) | edit | %; 0..100; max 3 chars | Health/HpHealAtt% | `40` | code |
| Run if below | edit | %; 0..100; max 3 chars | Health/HpRun% | `30` | code |
| Hang if below (disconnect) | edit | %; 0..100; max 3 chars | Health/HpLogoff% | `10` | code |
| Heal period | edit | (seconds); max 3 chars | Health/HpHealPeriod | `4` | code |

**Mana/Kai**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Rest max. (rest until mana reaches) | edit | %; 0..100; max 3 chars | Health/ManaFull% | `100` | code |
| Rest if below | edit | %; 0..100; max 3 chars | Health/ManaRest% | `20` | code |
| Heal (rest) | edit | %; 0..100; max 3 chars | Health/ManaHeal% | `50` | code |
| Heal (combat) | edit | %; 0..100; max 3 chars | Health/ManaHealAtt% | `10` | code |
| Run if below | edit | %; 0..100; max 3 chars | Health/ManaRun% | `0` | code |
| Bless if above | edit | %; 0..100; max 3 chars | Health/ManaBless% | `80` | code |
| Use meditate ability | checkbox |  | Health/UseMeditate | `0` | code |
| Meditate before resting (enabled only when "Use meditate ability" is ticked) | checkbox |  | Health/MeditateB4Rest | `0` | code |

**Resting commands**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Pre-rest command | edit | max 100 chars; max 3 chars | Health/PreRestCmd |  | code |
| Post-rest command | edit | max 100 chars; max 100 chars | Health/PostRestCmd |  | name |
| Party wait command | edit | max 20 chars | Party/PartyWaitCmd | `@wait` | name |
| Party resume command | edit | max 20 chars | Party/PartyResumeCmd | `@ok` | name |

### 1.7 Spells tab (dialog 345)

Which spell (short code, e.g. from the spell database) to use for each support role. Combo boxes list known spells; free text is allowed.

**Healing/Regeneration**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Heal HPs | combo (editable) |  | Spells/HealCmd |  | code |
| HP Regen | combo (editable) |  | Spells/RegenCmd |  | name |
| Mana Regen | combo (editable) |  | Spells/FluxCmd |  | name |
| Min. rate per click (mana regen) | edit (numeric) | max 3 chars | Spells/FluxMin | `0` | code |
| When HP full (cast) | combo (editable) |  | Spells/HpFullCmd |  | name |
| When Mana full (cast) | combo (editable) |  | Spells/MaFullCmd |  | name |

**Bless Spells**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Bless spell 1 | combo (editable) |  | Spells/BlessCmd1 |  | name |
| Bless spell 2 | combo (editable) |  | Spells/BlessCmd2 |  | name |
| Bless spell 3 | combo (editable) |  | Spells/BlessCmd3 |  | name |
| Bless spell 4 | combo (editable) |  | Spells/BlessCmd4 |  | name |

**Other Spells**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Cure blindness | combo (editable) |  | Spells/BlindCmd |  | name |
| Cure poison | combo (editable) |  | Spells/PoisonCmd |  | name |
| Cure disease | combo (editable) |  | Spells/DiseaseCmd |  | name |
| Freedom | combo (editable) |  | Spells/FreedomCmd |  | name |
| Room light | combo (editable) |  | Spells/LightCmd |  | name |

**Bless Spells**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Bless spell 5 | combo (editable) |  | Spells/BlessCmd5 |  | name |
| Bless spell 6 | combo (editable) |  | Spells/BlessCmd6 |  | name |
| Bless spell 7 | combo (editable) |  | Spells/BlessCmd7 |  | name |
| Bless spell 8 | combo (editable) |  | Spells/BlessCmd8 |  | name |
| Bless spell 9 | combo (editable) |  | Spells/BlessCmd9 |  | name |
| Bless spell 10 | combo (editable) |  | Spells/BlessCmd10 |  | name |

### 1.8 Combat tab (dialog 308)

Weapons, backstab, monster limits and the three-tier attack-spell system.

**Weapon Combat**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Normal attack | edit | max 10 chars | Combat/AttackCmd | `a` | code |
| BS weapon | combo (editable) |  | Combat/BsWeapon |  | code |
| Normal weapon | combo (editable) |  | Combat/NrmWeapon |  | code |
| Alternate | combo (editable) |  | Combat/AltWeapon |  | code |
| Shield | combo (editable) |  | Combat/Shield |  | code |
| Use shield with BS weapon | checkbox |  | Combat/UseShieldForBS | `0` | code |
| Use normal weapon for attack spells | checkbox |  | Combat/UseNrmWeapForSpells | `0` | code |

**Options**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Do BS attacks | checkbox |  | Combat/CanBackStab | `0` | code |
| Dont BS if multi-attack | checkbox |  | Combat/DontBsIfMulti | `0` | code |
| Run if BS fails | checkbox |  | Combat/RunIfBsFails | `0` | code |
| Attack all monsters (key AttackNeutral: also attack neutral (cyan) monsters, not just evil ones; per-monster Relationship overrides (help file)) | checkbox |  | Combat/AttackNeutral | `0` | code |
| Polite attacks | checkbox |  | Combat/PoliteAttacks | `1` | code |
| Max. monsters | edit | max 2 chars | Combat/MaxMstrs | `2` | code |
| Max. exp. (run if room monster exp exceeds) | edit | max 12 chars | Combat/MaxMstrExp | `5000` | name |
| Run distance | edit | max 2 chars | Combat/RunRooms | `4` | code |

**Spell Combat**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Multi-attack spell (area) | combo (editable) |  | Combat/MultAttack |  | code |
| Multi-attack: Min. enemies | edit | count; max 2 chars | Combat/MultMstrCnt | `3` | code |
| Multi-attack: Max. damage | edit | max 5 chars | Combat/MultMaxDmg | `99999` | code |
| Multi-attack: Max. casts | edit | max 2 chars | Combat/MultCastCnt | `1` | code |
| Multi-attack: Required mana % | edit | % of max mana; 0..100; max 3 chars | Combat/ManaMultAtt% | `20` | code |
| Pre-attack spell | combo (editable) |  | Combat/PreAttack |  | code |
| Pre-attack: Max. damage | edit | max 5 chars | Combat/PreMaxDmg | `99999` | code |
| Pre-attack: Max. casts | edit | max 2 chars | Combat/PreCastCnt | `1` | code |
| Pre-attack: Required mana % | edit | % of max mana; 0..100; max 3 chars | Combat/ManaPreAtt% | `50` | code |
| Attack spell | combo (editable) |  | Combat/AttackSpl |  | code |
| Attack spell: Max. damage | edit | max 5 chars | Combat/AttMaxDmg | `99999` | code |
| Attack spell: Max. casts | edit | max 2 chars | Combat/MaxCastCnt | `1` | code |
| Attack spell: Required mana % | edit | % of max mana; 0..100; max 3 chars | Combat/ManaAttack% | `20` | code |

### 1.9 PvP tab (dialog 336)

What to do when another player attacks.

**Action**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Hangup immediately (PvpAction=0 (default)) | radio |  | PvP/PvpAction | `0` | code |
| Flee then hangup (PvpAction=1) | radio |  | PvP/PvpAction | `0` | code |
| Flee (come back later) (PvpAction=2) | radio |  | PvP/PvpAction | `0` | code |
| Attack (dangerous) (PvpAction=3) | radio |  | PvP/PvpAction | `0` | code |
| Chase and attack (dangerous) (PvpAction=4) | radio |  | PvP/PvpAction | `0` | code |
| Do nothing (for non-pvp boards) (PvpAction=5) | radio |  | PvP/PvpAction | `0` | code |
| Flee to | combo (editable) |  | PvP/PvpFleeRoom |  | code |
| No. of rooms to flee | edit | max 3 chars | PvP/FleeRooms | `10` | code |
| Flee hangup delay | edit | seconds; max 3 chars | PvP/FleeTimeout | `30` | code |

**Options**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Notify gang members | checkbox |  | PvP/NotifyGang | `1` | code |
| Re-connect after PvP in | checkbox |  | PvP/RedialPvp | `0` | code |
| Re-connect after PvP in [edit] minutes | edit | minutes; max 4 chars | PvP/PvpSafePeriod | `30` | code |
| Track enemies every | checkbox |  | PvP/TrackEnemies | `0` | code |
| Track enemies every [edit] seconds | edit | seconds; max 3 chars | PvP/TrackDelay | `60` | code |
| If forced to hide, wait | edit | seconds; max 3 chars | PvP/HideDelay | `30` | code |

**PvP Spells**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| PvP spell 1 (cast once) | combo (editable) |  | PvP/PvpSpell1 |  | code |
| PvP spell 2 (cast once) | combo (editable) |  | PvP/PvpSpell2 |  | code |

### 1.10 Party tab (dialog 327)

Party rank, heal/bless duties and party etiquette.

**Party Rank**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Front (PartyRank=0) | radio |  | Party/PartyRank | `1` | code |
| Mid (PartyRank=1 (default)) | radio |  | Party/PartyRank | `1` | code |
| Back (PartyRank=2) | radio |  | Party/PartyRank | `1` | code |

**Party Heal**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Minor heal spell | combo (editable) |  | Party/PartyHeal1 (older key: Party/PartyHeal) |  | code |
| ...cast minor heal on member at | edit | %; 0..100; max 3 chars | Party/PartyHeal1% | `50` | code |
| Major heal spell | combo (editable) |  | Party/PartyHeal2 |  | code |
| ...cast major heal on member at | edit | %; 0..100; max 3 chars | Party/PartyHeal2% | `20` | code |
| Request healing at | edit | %; 0..100; max 3 chars | Party/PartyAskHeal% | `50` | code |

**Party Bless**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Party bless spell 1 | combo (editable) |  | Party/PartyBless1 |  | code |
| Bless 1 timeout (secs) | edit | seconds; max 4 chars; max 4 chars | Party/PartyBlessWait1 | `0` | code |
| Party bless spell 2 | combo (editable) |  | Party/PartyBless2 |  | code |
| Bless 2 timeout (secs) | edit | seconds; max 4 chars; max 4 chars | Party/PartyBlessWait2 | `0` | code |
| Party bless spell 3 | combo (editable) |  | Party/PartyBless3 |  | code |
| Bless 3 timeout (secs) | edit | seconds; max 4 chars; max 4 chars | Party/PartyBlessWait3 | `0` | code |
| Party bless spell 4 | combo (editable) |  | Party/PartyBless4 |  | code |
| Bless 4 timeout (secs) | edit | seconds; max 4 chars; max 4 chars | Party/PartyBlessWait4 | `0` | code |

**Options**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Max. monsters when partying | edit | max 2 chars | Party/PartyMaxMstrs | `4` | code |
| Max. monster experience | edit | max 12 chars | Party/PartyMaxExp | `10000` | name |
| Wait if members are below | edit | %; 0..100; max 3 chars | Party/PartyWait% | `50` | code |
| If leading, wait only (mins) | edit | max 4 chars | Party/PartyWaitMax | `2` | code |
| 'par' frequency (seconds) | edit | max 3 chars | Party/ParPeriod | `15` | code |

**Options checklist** (checklist, list 1965)

| Option | INI [Section]/Key | Default | Conf. |
|---|---|---|---|
| Attack last in party | Party/AttackLast | `0` | code |
| Share damage | Party/ShareDamage | `0` | code |
| Request party health | Party/AskHealth | `1` | code |
| Attack in reverse order | Party/AttackReverse |  | code |
| Attack what other members attack | Party/AttLeaderMstr | `1` | code |
| Defend party when PvP | Party/DefendParty | `1` | code |
| Auto-share collected cash | Party/ShareCash | `1` | code |
| Help leader bash doors | Party/HelpBash | `1` | code |
| Ignore @party when following | Talk/NoPartyCmds, Party/IgnoreParty | `0` | code |
| Ignore @wait when leading | Party/IgnoreWait | `0` | code |
| Say @panic when leading | Party/SendPanic | `1` | code |
| Ignore @panic when following | Party/IgnorePanic | `0` | code |

### 1.11 Cash tab (dialog 307)

Coin pickup, encumbrance limits and banking.

**Options**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Per coin type: Discard / Ignore / Collect (tree of radio items) (value -1=Discard, 0=Ignore, 1=Collect; defaults copper 0, silver 0, gold 1, platinum 1, runic 1) | list |  | Cash/WantCopper, WantSilver, WantGold, WantPlat, WantRunic |  | code |
| Don't collect if it will make you heavy | checkbox |  | Cash/DontBeHeavy | `0` | code |
| Don't collect if it will make you medium | checkbox |  | Cash/DontBeMedium | `0` | code |
| Only stash coin up to | checkbox |  | Cash/StashCoin | `0` | code |
| ...coin type (Copper/Silver/Gold/Platinum) (index; default 3 = Platinum) | combo (list) |  | Cash/StashCoinType | `3` | code |
| Collect after combat finished | checkbox |  | Cash/GetAfterCombat | `0` | code |
| Drop smaller currency to get larger currency | checkbox |  | Cash/DropCoins | `1` | code |

**Auto-deposit/sell**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Auto-deposit if wealth exceeds | checkbox |  | Cash/LimitWealth | `0` | code |
| Auto-deposit if wealth exceeds [edit] (copper) | edit | max 10 chars | Cash/MaxWealth | `0` | code |
| Auto-deposit if coins exceed | checkbox |  | Cash/LimitCoins | `1` | code |
| Auto-deposit if coins exceed [edit] (count) | edit |  | Cash/MaxCoins | `1000` | code |
| Minimum cash to keep on hand | edit | max 10 chars | Cash/MinWealth | `0` | code |

**Miscellaneous**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Banking done at | combo (editable) |  | Cash/Bank | `Bank of Godfrey` | code |
| Name of runic currency | edit | max 30 chars | BBS.INI RunicName (per BBS) |  | name |

### 1.12 Sounds tab (dialog 301)

Sound (.wav) per alert event.

**Sounds**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Alert events tree: Idle, Low HP's, Training, Hangup, PvP, Conversations (Local, Page, Telepath, Gossip, Auction, Gangpath, Broadcast) | tree |  | Alerts/AlertIdle, AlertLowHPs, AlertTrain, AlertHangup, AlertPvP, AlertTalk, AlertPage, AlertTele, AlertGossip, AlertAuction, AlertGangpath, AlertBroadcast |  | name |
| Sound file (.wav) for selected alert | edit | max 256 chars |  |  |  |
| Play alerts only when AFK | checkbox |  | Alerts/AlertWhenAfk | `1` | code |

### 1.13 Auto-roam tab (dialog 302)

Which exits auto-roam may take / open, and per-room special commands.

**Move**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Move NW | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move N | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move NE | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move W | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move E | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move SW | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move S | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move SE | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move Up | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |
| Move Down | checkbox |  | Auto-roam/AutoMove (bitmask) | `255` | name |

**Open**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Open NW | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open N | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open NE | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open W | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open E | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open SW | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open S | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open SE | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open Up | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |
| Open Down | checkbox |  | Auto-roam/AutoDoor (bitmask) |  | name |

**Special Rooms**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Special rooms (room + command pairs) | list |  | Auto-roam/AutoRoom%d + AutoCmd%d | `--Unused--` | name |
| Room | combo (editable) |  | (edits selected AutoRoom%d) |  | name |
| Command | edit | max 30 chars | (edits selected AutoCmd%d) |  | name |

### 1.14 Talk tab (dialog 361)

Conversation logging, remote control permissions, diverting and AFK.

**Options**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Log conversations to | checkbox |  | Talk/LogTalk | `1` | code |
| ...log file path | edit | max 256 chars | Talk/LogFile | `%s\\MegaMud.LOG` | code |
| Greet players when first met | checkbox |  | Talk/GreetPlayers | `1` | code |
| Look at players (to learn their inventories) | checkbox |  | Talk/LookPlayers | `1` | code |
| Disallow all remote control commands | checkbox |  | Talk/NoRemoteCmds | `0` | code |
| Disallow @party commands (from leader) (loader stores NoPartyCmds in the same field as Party/IgnoreParty) | checkbox |  | Talk/NoPartyCmds | `0` | name |
| Disallow remote control from gangpaths | checkbox |  | Talk/NoGangCmds | `1` | code |
| Warn user if remote command is invalid | checkbox |  | Talk/WarnRemote | `1` | code |

**Remote Diverting**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Local Talk | checkbox |  | Talk/DivertLocal | `1` | code |
| Telepaths/pages | checkbox |  | Talk/DivertTele | `1` | code |
| Gangpaths | checkbox |  | Talk/DivertGang | `0` | code |
| Gossips/Auctions | checkbox |  | Talk/DivertGossip | `0` | code |
| Broadcasts | checkbox |  | Talk/DivertBroad | `0` | code |

**AFK Mode**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Auto-AFK in | checkbox |  | MegaMud/AutoAfk, Talk/AutoAfk | `1` | code |
| Auto-AFK in [edit] minutes | edit | minutes; max 2 chars | MegaMud/AfkTimeout, Talk/AfkTimeout | `5` | code |
| Auto-AFK when minimized | checkbox |  | MegaMud/AfkMinimized, Talk/AfkMinimized | `1` | code |
| User input cancels AFK mode | checkbox |  | MegaMud/AutoAfkOff, Talk/AutoAfkOff | `1` | code |
| AFK response message | edit | max 256 chars | MegaMud/AfkReply, Talk/AfkReply | `{AFK}` | code |
| Remote control failure message | edit | max 256 chars | Talk/CmdReply | `{command invalid or not allowed}` | code |

### 1.15 Statline tab (dialog 365)

The MajorMUD status line MegaMud asks the server for (it parses HP/mana from it).

**Current Statline**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Current statline (read-only preview) | read-only text |  |  |  |  |

**Statline Command**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Statline command (sent as "set statline <x>") | edit (multi-line) | max 256 chars | MegaMud/StatLine | `full` | code |

**Customize**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Item to insert (Current/Max HP, mana/kai, wealth, exp, exp needed, warning/resting state, newline) | combo (editable) |  |  |  |  |
| Colour to insert (fg/bg/bold/underline/blink/reverse...) | combo (editable) |  |  |  |  |

### 1.16 Other tab (dialog 326)

Doors/traps/searching, hangup policy, ailments, misc. behaviour checklist and game entry/exit commands.

**Miscellaneous**

| Setting | Control | Units / range | INI [Section]/Key | Default | Conf. |
|---|---|---|---|---|---|
| Attempt bash | edit | times; max 2 chars | Other/BashMax | `3` | code |
| Attempt pick-lock | edit | times; max 2 chars | Other/PickMax | `3` | code |
| Attempt disarm | edit | times; max 2 chars | Other/DisarmMax |  | code |
| Search each room | edit | times; max 2 chars | Other/SearchMax | `1` | code |
| Back-scroll buffer | edit | bytes; max 7 chars | Other/ScrollMem | `31337` | code |
| Command splitter character | edit | max 1 chars | Other/CmdSplitChar | `;` | code |
| Delete inactive players after | edit | days; max 3 chars | Other/DelPlayers | `90` | code |
| Game entry cmd | edit | max 100 chars | Other/EntryCmd |  | code |
| Game exit cmd | edit | max 100 chars | Other/ExitCmd |  | code |

**Options checklist** (checklist, list 1965)

| Option | INI [Section]/Key | Default | Conf. |
|---|---|---|---|
| Pick locks instead of bashing | Other/CanPickLocks | `0` | code |
| Attempt to disarm traps | Other/CanDisarmTraps | `0` | code |
| Auto-train | Other/AutoTrain | `1` | code |
| Relog instead of hanging | Other/RelogInstead | `0` | code |
| Allow hangup when not AFK | Other/HangupNotAfk | `1` | code |
| Allow hangup in all-off mode | Other/HangupAllOff | `0` | code |
| Hangup if naked | Other/HangupNaked | `1` | code |
| Ignore poison | Other/IgnorePoison | `0` | code |
| Ignore blindness | Other/IgnoreBlind | `0` | code |
| Ignore confusion | Other/IgnoreConfusion | `0` | code |
| Bless while resting | Other/BlessResting | `0` | code |
| Bless during combat | Other/BlessCombat | `0` | code |
| Search rooms if item needed | Other/SearchNeedItem | `1` | code |
| Go backwards if running | Other/RunBackwards | `1` | code |
| Break combat before running | Other/BreakB4Running | `1` | code |
| Provide light in dimly lit rooms | Other/LightDimRooms | `1` | code |
| Dont move unless sneaking | Other/MustSneak | `0` | code |
| Use supernatural stealth | Other/SuperStealth | `0` | code |
| Auto-engage auto-combat | Other/AutoAutoCombat | `1` | code |
| Auto-engage auto-rest/heal | Other/AutoAutoHeal | `1` | code |

_Settings-tab total: about 277 individual settings (each checklist item and each per-coin choice counts as one; list editors count as one)._


Notes on the tabs above:
- Auto-roam `AutoMove` defaults to `255` (8 bits set) and `AutoDoor` to `0`. The bit-to-direction order was not traced.
- Party `PartyRank`: 0 Front, 1 Mid (default), 2 Back. The Party dialog code compares against these values (code).
- PvP `PvpAction`: 0 Hangup immediately (**default**), 1 Flee then hangup, 2 Flee (come back later), 3 Attack, 4 Chase and attack, 5 Do nothing (code for 0, 3 and 4; the rest follow radio order).
- General `StartTask`: 0 Do nothing, 1 Begin looping at `DefaultLoop`, 2 Begin auto-roaming (code).
- Toolbar keys (`[Toolbar] New, Open, Save, Connect, NoHangup, Upload, Download, Copy, Paste, BScroll, Capture, Goto, Loop, AutoRoam, BackTrack, Stop, Record, AllOff, AutoCombat, AutoNuke, AutoHeal, AutoBless, AutoLight, AutoGet, AutoCash, AutoSneak, AutoHide, AutoSearch, AFK, ShowTalk, ShowWho, ShowStats, ShowSession, ShowAnalysis, ShowExpGraph, ShowParty, ShowTools, Settings, GameData, Font, Sound, ExpCalc, Help, GetAll, DropAll, EquipAll, DepositAll, CmdInv, CmdExp, CmdStat, CmdWho, CmdRest, CmdSneak, CmdHide, CmdSearch, Custom1..4`) take 0, 1 or 2. Defaults: 2 for Copy, Goto, AllOff, AFK, Settings and Help; 1 for Connect, Paste, BScroll, Loop, AutoRoam, BackTrack, Stop and most auto-toggles; 0 for the rest. 2 probably means "visible, with a separator before it" (inferred).

## 2. Other configuration dialogs (opened from the Settings tabs)

### 2.1 BBS Details... (dialog 306, from BBS tab New/Edit). Stored in `BBS\<name>\BBS.INI`

| Setting | Control | BBS.INI key (conf. name) | Default (inferred from adjacent strings) |
|---|---|---|---|
| Name | edit | (directory name) | |
| Telnet: Address | edit | `TelnetAddr` | |
| Telnet: Port | edit | `TelnetPort` | |
| Logon Automation: list of Message -> Reply pairs | list + 2 edits | `LogonPrompt%d` / `LogonReply%d`, `MenuPrompt` | `your user-id` -> `{userid}^M`; `your password` -> `{pswd}^M`; `your selection` -> `/go majormud^M`; realm prompt `[MAJORMUD]:` (also recognises `[PARADIGM]:`) |
| Commands: Max. input size | edit | `MaxInputSize` | |
| Logoff command | edit | `LogoffCmd` | |
| Relog command | edit | `RelogCmd` | `;relog`? |
| Toggle ANSI | edit | `AnsiCmd` | |
| Speech prefix | edit | `SpeechPrefix` | |
| Broadcast prefix | edit | `BroadcastPrefix` | |
| PvP level | edit | `PvpLevel` | |
| Max. hours/day | edit | `MaxHrsPerDay` | |
| Global Messages... button | opens 2.2 | | |

### 2.2 BBS Global Messages (dialog 305): text patterns that identify BBS-wide events

| Setting | Control | BBS.INI key | Default (inferred) |
|---|---|---|---|
| Logon | edit | `LogonMsg` | `logs ON` |
| Logoff | edit | `LogoffMsg` | `logs OFF` |
| Shutdown | edit | `ShutDownMsg` | `Please finish up and log off` |
| Broadcast | edit | `BroadcastMsg` | `Broadcast` |
| Global 1 / 2 / 3 (+ "2 lines" checkbox each) | edit + checkbox | `Global1Msg..Global3Msg`, `Global1Two..Global3Two` | |

### 2.3 Scheduled Event (dialog 311, from Events tab). Stored as `[Schedule] Event%d=h:m:s:text`

- **Event** (radio): Logon / Logoff / Re-log / Go to [room combo] / Loop [path combo] / Command [text]. Also an "Event disabled" checkbox.
- **When** (radio): At time [time picker] / Every [n] [hours|minutes|seconds].
- Events tab extras: "AFK only" (`EventsAfkOnly`, default 1) and "Disable" (`DisableEvents`, default 0).

### 2.4 Macro Details (dialog 320, Game Data > Macros)
Use (scope), Keystroke [key list: F-keys, cursor keys, keypad 0-9 / * - + . Enter, ...] + Shift/Ctrl/Alt, macro text. Syntax: `^M` = control character (CR), `~` = half-second pause.

## 3. Game Data editor (Options > Game Data...): 10 tabs of per-record settings

Each tab is a list with New / Modify / Remove / Find... / Find Next. Items and Monsters also have **Clean** (remove records not in the installed DB), Paths has **Scan...**, and Rooms has **Re-learn**. Every record has a **Use** scope: installed defaults / for all characters / only for this BBS / only for this character.

| Tab (list dlg) | Record dialog | Per-record settings |
|---|---|---|
| Players (333) | Player Details (332) | Name, User ID, Use, Relationship (Friend/Avoid/Enemy/Hangup/Unknown), Race, Class. Checkboxes: Doesn't use MegaMud, Invite to party if seen, Join party if invited, Chase player if seen, Play alert when player enters, Don't heal or bless, Don't auto-delete. **Allowed Remote Control**: Query version, Query experience, Query health/status, Query location, Query inventory, Request invite, Move player, Execute commands, Hangup/disconnect, Alter settings, Divert conversations, Sysop commands (with an "All" button). Read-only info: surname, gang, reputation, exp, level, last online/seen, last seen wearing (and a "Don't look" option). |
| Monsters (325) | Monster/NPC Details (324) | Name, Use, Relationship, Experience, Max HP, Pre-attack spell + Max casts, Attack spell + Max casts. **Attack Priority** radio: First/High/Normal/Low/Last. Options: Find first, Not hostile, Don't backstab, Check if alive. Info pane shows game stats (level, AC, drops, regen, group). |
| Items (317) | Game Item Details (316) | Name, Use. Options: Auto-collect, Auto-discard, Auto-equip, Auto-find, Auto-open, Auto-buy, Auto-sell (or stash), Cannot be taken, Can use to backstab, Must have minimum, Loyal item. Details: Min. to keep, Max to get, Weight, Price, Item type, Body location, "If needed, do" [command], Bought/sold [shop]. |
| Spells (344) | Spell Details (343) | Name, Code (short cast word), Use, Minimum level, Required Mana/Kai, Timed duration, Evil in combat, target Self/Player/Area/Monster, Special command. Type radio: Priest 1-3, Mage 1-3, Druid 1-3, Bardic 1, Mystic 1, Any. |
| Messages/Responses (323) | Game Message Details (322) | Name, Use, Message (start text), Ends with, Response (text to send). **Effects** flags: Blinded, Confused, Poisoned, Diseased, Losing HPs, HPs regenerating, Mana regenerating, Movement prevented, Attack prevented, Ends combat, Last action failed. **Action** radio: Ignore / Check who is in the room / Wait until it wears off / Rest until full HP / Rest until full Mana / Don't rest, run! / Hangup. Find anywhere in text, Find in conversations, Use when chasing, Disabled. |
| Rooms (340) | Room Details (339) | Name, Group, Code (4 chars), Use. Flags: Shop, Bank, Trainer, Stop before entering room, Avoid this room (if possible), Hide in goto list, Always in common rooms popup. Restrictions: Min. level, Max. level, Class. |
| Paths (329) | Path Details (328) + Path Steps (330) | Name, Author, File, Use, From/To/No. steps (read-only), Requirements: Gold, Item, "If path fails, do", "When finished, do". Each step has a Command plus flags Dark room, Pitch Black, Rest up here, Don't rest here, Don't attack here, Disarm Trap, Pick lock, Stash point, Re-learn room. |
| Macros (321) | Macro Details (320) | see 2.4 |
| Races (375) | Race Details (376) | Number, Name, Use, Bonus HPs, Experience %, min/max CP for each of the 6 stats, Abilities list. |
| Classes (374) | Class Details (373) | Number, Name, Use, Experience %, Combat, HPs/level range, Weapons, Armour, Magic, Abilities list. |

## 4. Tool and information windows (not settings, listed for completeness)

- **Goto location...** (318): filter box, room tree, Go / Run / How? / Cancel. "How MegaMud will get there..." (331) lists the route.
- **CP/Exp. Calculator** (312): race/class, CP per stat (base/max/current/used), level required, exp table.
- **Inventory Calculator** (368): per-slot item picker (Weapon, Shield, Head, Ears, Eyes, Face, Neck, Back, Torso, Arms, Wrist 1-2, Waist, Hands, Finger 1-2, Legs, Feet, Worn, Everywhere). Show filters: restrict to level/strength/alignment/race-class. "Find best" [criterion], Reset to current inventory, "Mark as auto-collect/auto-equip", Update Game Data. Saved under `[InvCalc]`.
- **Spell Book** (362), **Who Is Home?** (381, checks whether boss monsters are alive via `sys stat room`; MudOp only), **Quick-Shop** (303), **Quick Tools** (348).
- Stats windows: Player Statistics (335), Session Statistics (342), Time Analysis (358), tabbed Combat/Exp./Misc./Session/Times (383-387), Experience Graph (388, menu: show as exp/hour or k/hour, update period 1 s to 10 min).
  - **Combat** (383; read from `MegaRes.dll` 2026-10-24, name only): an "Accuracy" box with columns count, Min-Max and Avg and rows Miss (count only), Hit, Extra, Crit, BS, Pre, Cast and Round (Min-Max and Avg only); a "Monsters" box: Killed (`0 monsters`), Had to run (`Never`), Health low (`999 hps`), Mana low (`999 Mana`); Copy and Reset buttons. The Player Statistics window (335) has the same Combat rows with Cast1, Cast2, Multi and PreM besides, under Experience (Duration, Exp. made, Exp. needed, Exp. rate, Will level in) and over Other (Sneak and Walk rates, Dodge, Copper and Items Collected / Deposit-Sold / Stashed, Income rate).
  - **Exp.** (384): Duration, Exp. made, Exp. needed, Exp. rate, Will level in; Copy, Reset. **Misc.** (385): Sneak, Dodge, the coins and items block, Visitors (People seen, Attacked, List). **Session** (386): Total and Online time, Comms (Dialed, Failed, Connected, Lost carrier), the next scheduled Event. **Times** (387): Duration, Moving, Attacking, Resting, Other; time spent resting for HP, for Mana/Kai, waiting for the party, unable to move, blinded, poisoned, diseased, confused, other conditions; Reset.
  - What Extra, Pre, PreM and Multi count is not in the dialog (likely extra blows in a round, pre-attack spells and multi-target spells; not traced in the exe).
- Room Conflict resolver (366), Import RayMud Route (355), Find (314), Changelog (297), About (300).
- Map editor property pages (356 "Room", 499 "Exits"): flags Shop/Lair/Dark/Boss/Trainer/Search/Stash/Forbidden; exit types Normal/Door/Toll/Hidden/Command/Trap/Class/Race/Level/Alignment restricted. The exe calls this feature "still under development".
- **Not referenced by the exe** (leftover UI): 309 Port Settings (Telnet radio, "Auto-detect ZModem transfer"), 313 Data Files paths, 377 Rules editor (Name/When/Action). The rule engine strings still exist: conditions on current HP, mana/kai, party member health, party size, no. of monsters, current target, room, sneaking/hiding/blind/poisoned/held/dark/heavy; actions Rest, Rest to full health/mana, Drop, Equip, Attack, Attack with, Cast bless/heal/attack spell. `[Hangup rule (%s)]` appears in a message.

## 5. Menu commands (feature list)

Main menu (102). Shortcuts are in angle brackets.

- **File**: New character <Alt+N>, Open character <Alt+O>, Save character, Save As, Connect, Hangup <Alt+H>, Disable Hangup, Upload <PgUp>, Download <PgDn> (ZModem, which the readme says is removed), Auto-load last INI file, Last INI file 1-5, Exit.
- **Edit**: Copy <Ctrl+C>, Paste <Ctrl+V>, Back-scroll <Alt+B>, Capture <Alt+C>, Reset ANSI, Reset custom profile, Record path, Import (MudWalk path, MMTerm path, MMClient path, RayMud route, Synterm run), Fix path steps.
- **View**: Conversations, Online players, Player statistics, Session statistics, Time Analysis, Exp. Rate Graph, Party status, Quick Tools.
- **Action**: Goto <Alt+G>, Loop <Alt+L>, Auto-Roam <Alt+R>, Back Track, Stop <Alt+S>, Get All, Drop All, Equip All, Deposit All, **All Auto-Responses Off**, and the toggles **Auto Combat, Auto Nuke (area spells), Auto Rest/Heal, Auto Bless, Auto Light, Auto Get Items, Auto Get Cash, Auto Sneak, Auto Hide, Auto Search, Auto Track** (their states are saved as `[MegaMud] AutoCombat..AutoTrack`), Away From Keyboard <Alt+K>.
- **Options**: Settings, Game Data, Font, Sound, CP/Exp. Calculator, Inventory Calculator, Spell Book, Who Is Home?
- **Help**: Help Topics (MEGAMUD.HLP), Changelog, About.
- Accelerators that are not on the menu: Ctrl+Alt+D (cmd 2087, probably the debug dump: "DEBUGGING INFORMATION" block in the trace) and Alt+M (cmd 2064).
- Conversation window menu (101): Show All, Toggle Local Talk / Gossips / Auctions / Gangpaths / Telepaths / User Pages / Global Messages / Logons-Logoffs / Enter-Exits / BBS Notifications, While AFK, Font, Show Time, Pause, Clear, Search (Find, Find Next F3).
- Map menu (104): New/Load/Save/Save As/Close map. Back-scroll menu (382): Find <Shift+F3>, Find Next <F3>. Graph menu (389): show exp/hour or k/hour, update interval, Clear.

## 6. INI keys with no settings-tab control

Global `MegaMud.INI`: `[MegaMud] AutoLoadIni, LastIni1..5, WindowPos`.

Character INI, written by menus or windows or kept as state:
- `[MegaMud]`: DataDir, ImportDir, CaptureDir, DownloadDir, SelPath, CurPath, CurPathStep, CurMap, FixSteps (the "halt on path steps that need correction" option), NoHangup (File > Disable Hangup), Sound, window toggles ShowTalk/ShowWho/ShowStat/ShowSess/ShowTime/ShowRate/ShowParty(default 1)/ShowTools, TalkTime, TalkView, Minimized, Maximized, window rectangles `*WinPos` and BackScrollPos, Graph1000, GraphPeriod(60), the auto-toggle states AutoCombat/AutoNuke/AutoHeal/AutoBless/AutoLight/AutoCash/AutoGet/AutoSneak/AutoHide (default 1) and AutoSearch/AutoTrack (default 0), FavRoom%d, Sort* and LastEdit* for the Game Data lists.
- `[Display]`: AnsiFont (Courier New), AnsiSize, AnsiWeight, AnsiCharSet, AnsiFamily, TalkFont (MS Sans Serif), TalkSize/Weight/CharSet/Family, ForeClr%d, BackClr%d, BorderClr.
- `[Comms]`: ShowCommErrs (1), AutoZModem (1). Also saved: UseDUN, UseRASEntry (dial-up leftovers).
- `[Alerts]`: the 12 sound-file paths (Sounds tab) and AlertWhenAfk.
- `[InvCalc]`: InvLock plus one slot per body location (Weapon ... Everywhr).
- `[Player]`: a snapshot of the last known character (Name, Surname, Gang, Exp, Need, Lives, CPs, Race, Class, Level, MaxHP, MaxMana, AC, the stats, Perception, Stealth, Thievery, Traps, Picklocks, Tracking, MartialArts, MagicRes, SpellCasting). This is state, not settings.
- `[Other] HangupWhenAfk` is read, but no control writes it.

## 7. "@" remote / party command words (sent by telepath `/name @cmd` or said in the room)

Party coordination: `@join`, `@invite`, `@forget`, `@comeback`, `@wait` / `!wait`, `@ok` / `!ok`, `@panic!`, `@heal` / `!help`, `@blind`, `@held`, `@diseased`, `@health` (reply `{HP=x/y,MA=..,Resting,Blind,...}`), `@share`, `@party <cmd>` (leader-issued command for followers), `@attack-last`, `@kill <target>`, `@OI!` (PvP alert).
Queries: `@ver`, `@exp`, `@level`, `@where`, `@path`, `@who`, `@seen <player>`, `@what`, `@status`, `@lives`, `@wealth`, `@enc`, `@have <item>`.
Control: `@do <cmd>` / `!cmd <cmd>`, `@stop`, `@rego`, `@goto <room>`, `@loop <path>`, `@looponce <path>`, `@roam [off]`, `@get-all`, `@drop-all`, `@equip-all`, `@deposit-all`, `@hangup`, `@relog`, `@home`, `@reset`, `@debug`, `@capture`, `@divert` (forward conversations to the caller).
Settings: `@settings`, `@auto-all`, `@auto-combat`, `@auto-nuke`, `@auto-heal`, `@auto-bless`, `@auto-light`, `@auto-cash`, `@auto-get`, `@auto-sneak`, `@auto-hide`, `@auto-search` (reply `{auto-x on|off}`).
Replies are wrapped in `{...}`, for example `{can't - I'm not the leader}`, `{no - player is a friend}` and `{command invalid or not allowed}`. The Player Details "Allowed Remote Control" checkboxes gate these per player. The likely grouping (inferred): Query version = `@ver`; Query experience = `@exp`/`@level`; Query health = `@health`/`@status`; Query location = `@where`/`@path`; Query inventory = `@have`/`@wealth`/`@enc`; Request invite = `@invite`; Move player = goto/loop/roam/stop; Execute commands = `@do`; Hangup = `@hangup`/`@relog`; Alter settings = `@auto-*`/`@settings`; Divert = `@divert`; Sysop = MudOp commands.

## 8. Command words MegaMud sends to the game (from exe strings)

- **Session setup:** `set statline <StatLine>`, `set palette 0`, `set talk fast`, `set warning on`, `set interrupt on`, `set messages high`, `set keep on`, `set tele on`, `brief`. BBS entry runs the logon automation pairs, then the realm prompt answer `enter`.
- **Status polling:** `stat`, `exp`, `spells` / `powers` (Kai users), `who`, `i`/inventory parsing, `par` (party status, every ParPeriod seconds), `l <player>` (look at players).
- **Movement and doors:** direction words, `open <dir>`, `bash <dir>`, `pi <dir>` (pick lock), `search <dir>`, `sea` (search room), `disarm trap <dir>`, `use <boat>` (wooden skiff, log raft, silverbark canoe), `sys goto <town>` (MudOp: newhaven, silvermere, rhudaur, khazarad, support, lostcity).
- **Combat:** AttackCmd (default `a`) + target, `bs <target>`, `<spellcode> [target]`, `break`, `hide`, `sneak`, `track <player>`, `aid <player>`.
- **Rest:** `rest`, `med` / `meditate`, plus the user's pre-rest and post-rest commands.
- **Items and cash:** `g <item>`, `g <n> <coin>`, `drop <item>`, `drop <n> <coin>`, `eq <item>` / `equip <item>`, `remove <item>`, `stash <n> <coin>` / `stash <item>`, `buy <item>`, `sell <item>`, `deposit <n>`, `withdraw <n>`, `dep <n>`, `share <n> copper|silver|gold|platinum|runic with party`.
- **Training:** `train` (Auto-train), going to a trainer room first if needed.
- **Party and social:** `invite <p>`, `join <p>`, `frontrank`, `backrank`, `disband party`, `uninvite <p>`, `leave`, `greet <p>`, `forgive <p>`, `gb {Being attacked by <p>}` (gangpath when PvP and NotifyGang), telepath `/<p> @...`.
- **MudOp only:** `sys stat room <map> <room>` (Who Is Home?), `sys god <name> add evil -100` (sent with MudOp access after "too far to the evil side").
- `suicide` and `reroll` are matched specially. This is probably a safety block on remote `@do` and pending commands (inferred).

## 9. Ease-of-use observations

**Size.** The Settings sheet has **16 tabs** with about **277 individual settings**. That counts each checklist item, coin choice and direction box as one and a radio group as one. Behind it the loader reads about **450 INI values**. On top of that are **10 Game Data tabs** whose records each carry 10-30 options (items, monsters, messages, rooms, paths, players), about 15 more per-character dialogs (BBS details, events, macros) and 13 Action-menu toggles. MegaRes.dll holds **72 dialog templates** (5 duplicated in the exe, 3 unused) and 5 menus.

**Essential for a new player.** A good "first-run wizard" would cover these, in roughly this order:
1. **BBS tab + BBS Details:** address/port, user ID/password, logon prompts. Nothing works without these.
2. **Health tab:** Rest if below / Rest max. (HP 90/99, mana 20/100), Run if below (30%), Hang if below (10%), Heal (rest/combat) thresholds. This is the whole survival model, shown as six numbers per resource.
3. **Combat tab:** normal attack command and weapon, Max. monsters (2), Run distance (4), attack spell + required mana %. Backstab options only for thieves and similar classes.
4. **Spells tab:** Heal HPs spell, bless spells, cure spells. Casters only.
5. **Cash tab + item Auto-collect flags:** which coins to pick up, auto-deposit and the bank. Looting of items is set **per item** in Game Data > Items (Auto-collect/equip/sell), not on a settings tab.
6. **Action menu toggles** (Auto Combat / Rest-Heal / Get Items / Get Cash / Sneak / ...) and **General > Manual/Auto-mode defaults**. These decide what actually runs.
7. **PvP tab:** the default action is **Hangup immediately**, which surprises people on non-PvP realms (pick "Do nothing").

**Advanced / niche.** Statline customisation, Toolbar layout, Talk remote-control permissions and diverting, per-player remote ACLs, the Party tab (heal/bless duties, @panic etiquette), the multi-attack and pre-attack spell grid (min enemies / max damage / max casts / mana %), Auto-roam exit masks and special-room commands, scheduled Events, MudOp/sys goto, back-scroll size, data directories, message/response rules, path step flags and room re-learning.

**Confusing points.**
- **Terminology:** "Hang if below" means *disconnect*, "Nuke" means area-effect spells, "Flux" means mana regen, "Polite attacks", "Attack all monsters" (the key is `AttackNeutral`) and "Rest max." means "rest until".
- **Unlabelled grids:** the Move/Open direction grids are 20 bare checkboxes laid out as a compass. The spell-combat grid has column headers ("Min. Enemies / Max. Damage / Max. Cast / Required Mana") floating above rows with no per-field labels.
- **Settings split across places:** looting lives in Cash (coins) plus Game Data > Items (per item) plus the Action toggles. Healing lives in Health (thresholds) plus Spells (which spell) plus Party (party heals) plus the Other checklist (bless while resting/combat). Running away lives in Health (Run if below) plus Combat (Max. monsters/exp, Run distance) plus Other (Go backwards, Break combat before running).
- **Hidden checklists:** checklists inside listboxes (Other: 20 options; Party: 12; General: 2 x 11) hide important behaviour, such as Auto-train, Hangup if naked and Relog instead of hanging, without descriptions.
- **Duplicates and dead UI:** two AFK key sets (`[MegaMud]` and `[Talk]`), `NoPartyCmds` on the Talk tab sharing its field with Party's "Ignore @party when following", ZModem menu items despite its removal, unused dialogs (Port settings, Data Files, Rules) and a "still under development" map editor.
- **Weak validation:** most numeric fields are free-text edits limited only by character count. Only the % fields have 0..100 spin ranges, and spell fields are free text with a suggestion list.
- **Odd defaults:** back-scroll default 31337 bytes, ZModem auto-detect on, Auto-search off but Auto-sneak/hide on, and HP rest threshold 90% (very conservative, which slows progress).
