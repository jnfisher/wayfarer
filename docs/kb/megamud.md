# MegaMud: what it did, and what a successor takes from it

MegaMud (G-Wiz, 1998-2003; re-released as v2.0 Alpha in 2026 by Apathy and Syntax) was the
Windows terminal and automation client most MajorMUD players used. Our copy, with its manual,
resources, game data and 1,957 community paths, is in `reference/megamud/`. Details:

| File | Covers |
|---|---|
| `megamud-manual.md` | The 1.03u manual, summarised by topic: every option and what it did |
| `megamud-settings.md` | Every setting in the v2.0 dialogs, grouped as MegaMud grouped them |
| `megamud-formats.md` | The room id, `.mp` paths, `Rooms.md`, `Messages.md`, `Macros.md`, the MDB2 databases |

Provenance: **[M]** is MegaMud's own documentation, resources or data: what MegaMud did, not
what the game does. Facts checked against the 1.11p game data are **[C]** as elsewhere.

## How MegaMud worked, in brief [M]

- **One program per character.** Each instance played one character from one `.INI`; parties
  were several instances talking through telepaths (`@wait`, `@ok`, `@party`…). A tray icon
  managed the instances.
- **A status bar was the state model**: the current *task* ("Looping at…") and *action*
  ("Resting", or what it waited for).
- **Navigation by recorded path segments.** Rooms are identified by a 32-bit id: a 12-bit hash
  of the title plus two bits per exit (`megamud-formats.md` section 0). Short point-to-point
  paths between named rooms (`Rooms.md`) form a graph; Goto chains them, Loop repeats a closed
  path. Each step's room id confirms position; when it disagrees, MegaMud re-synchronises.
- **Game-data tables drive behaviour**: spells, items, monsters, players, rooms, paths,
  messages, macros, events, races, classes. They layer: installed defaults < all characters <
  this BBS < this character, and fill themselves in from play (auto-learn).
- **A message table is the trigger engine**: a substring per line, an optional "ends with"
  text, condition flags (blind, poisoned, held, ends combat…) and an optional response.
- **Settings everywhere**: 16 settings tabs with about 277 settings (about 450 INI values), plus
  10 game-data tabs whose records carry 10 to 30 options each. Related settings are split across
  tabs (healing: Health, Spells, Party and the Other checklist), labels are terse ("Hang if
  below" means disconnect, "Nuke" means area spells), and important switches (Auto-train,
  Hangup if naked) sit undescribed in checklists (`megamud-settings.md` section 9).
- **Two default sets of switches**: what runs in *manual* mode (auto-combat, heal and rest,
  light, coins, items, sneak, hide all on) and in *auto* mode. Manual play in MegaMud was
  assisted play.

## Take, adapt or leave

| Concept | MegaMud [M] | Successor |
|---|---|---|
| Task and action status | Status bar | **Adapt**: the mode stack plus a narration line saying what and why (`client-plan/03-engine.md`) |
| Room identity | Title hash + exits; ambiguous (26,720 rooms give 8,448 ids [C]) | **Adapt**: position belief from the world data; compute MegaMud ids only to read `.mp` paths |
| Routes | Graph of recorded segments | **Adapt**: routes from the world data; recorded paths optional |
| Hunting loops | 195 community loops (Winterhawk's set), 175 of 177 walked close [C] | **Take**: import as a loop library; they are human knowledge of where and how to hunt |
| Conditional steps | `dir[search dir]`, `[use key dir]`, `[]` = fail to a fallback path | **Take** in our loop format |
| Room flags | Avoid, stop before entering, level and class limits, shop, bank, trainer, boss | **Take** into the world overlay; most also come from the data |
| Errands | Bank on wealth, sell when heavy, restock "must have" items, resume the loop at the same step | **Take** as switches, off by default; no gear buying |
| Health ladder | HP and mana: rest maximum, rest below, heal resting, heal in combat, run below, hang up below | **Adapt**: the same ladder, shown as one bar with markers, with defaults |
| Spell limits | Casts per monster, stop above a round's damage, mana %, pre-attack and area spells, per-monster overrides | **Take** |
| Danger gates | Max monsters, max room exp, relationships, polite attacks, run if the backstab fails | **Take**, plus data-driven danger (monster hits vs our HP) |
| Relationships | Friend, avoid, enemy, flee, hang up, for players and monsters; substring names, longest first | **Take** |
| Message table | Hand-made substrings | **Adapt**: our patterns come from WCCMSG and the source (`screen-patterns.md`); `Messages.md` effect classes and chase commands are a cross-check |
| Data layers | defaults < all < BBS < character | **Take** for settings (`client-plan/04-settings.md`) |
| Auto-learn | Items (weights by subtraction), monsters, players | **Take** as the learned overlay |
| Party protocol | `@` telepaths between instances | **Adapt**: one process coordinates directly; `@` kept for mixed parties (`plan-grouping.md`) |
| Remote control | `@` commands with per-player permission categories | **Later**, permissions required |
| Logon automation | Up to 20 prompt → reply pairs per BBS, typed by hand | **Adapt**: presets, detection, learn by doing (`client-plan/06-accounts-and-setup.md`) |
| Lag handling | Confirmation timeouts, Lag Pause, "show data when sent" | **Take**: executor timeouts; echo the bot's commands in a dim colour |
| Unattended safety | Hang up on low HP, PvP or naked; cleanup redial; exp-rate floor; schedules | **Take** as Safety settings, off by default |
| Time analysis | Time per activity | **Take**: a per-mode time ledger in the statistics |
| Keypad macros | Numpad = directions, 5 = rest | **Take** as the default keys |
| One instance per character | Copies of the program | **Leave**: one process, many sessions |
| Back-scroll | 32,000 bytes (v2.0: 1 MB), slows the program | **Leave**: full logs, searchable |
| Settings defaults | Generic: rest below 90% and to 99%, run below 30%, hang up below 10%, at most 2 monsters, PvP = hang up at once | **Adapt**: class and style presets; no hang-up or PvP hang-up by default |
| Manual-mode helpers | Auto-combat, rest, light, coins, sneak stay on in manual play | **Take** as opt-in "assists" while the bot is stopped |
| Modem, ZModem, dial-up | Core | **Leave** |
| Trigger responses as scripting | Message responses | **Leave** for now |
| Auto-buy, auto-equip | Item flags | **Leave** (John, 2026-10-17) |
| Looking at players to record their gear | Periodic `look` | **Leave** |

## Pain points MegaMud admits [M]

- Lag made it repeat commands (hence Lag Pause).
- Game updates that add exits or rename rooms break room ids, leaving it lost; the fixes
  (Fix Path Steps, Re-Learn Room) need someone at the keyboard.
- Rooms with the same title and exits can't be told apart.
- It collected copper and silver against the player's settings whenever it needed money.
- Two "Attack Last" instances re-attacked each other's targets forever; "attack what others
  attack" pulled protector monsters.
- `@party` and remote control could be abused by an untrusted leader or player.
- Long runs filled the disk with the conversation log; a large back-scroll slowed it.
- MajorMUD won't let a character sneak for about a round after being attacked; MegaMud moved on
  unsneaked after about 4 s unless told to wait.
- The FAQ's top questions were all "why won't it…": rest (poisoned, or auto-rest off), backstab
  (no stealth, BS weapon, Do BS, auto-sneak or "Sneaking..."), act at all (statline not
  `full`). A successor should say why it isn't doing something, unprompted.

## Facts a client can use directly

- **Room id** [C]: `((sum((i+1)*ord(title[i])) & 0xFFF) << 20) | exit_bits`, two bits per
  exit in the order N, S, E, W, NE, NW, SE, SW, U, D: 01 plain, 10 door, gate or key, 00 not
  shown (hidden, text, remote action). Exact for 99.2% of 43,685 walked steps.
  `test/play/routecheck.py` implements it.
- **`.mp` paths** [C]: header `FROMID:TOID:steps:-1:cost:item:prepath:nextpath`; steps
  `ROOMID:FLAGS:command`, where ROOMID is the room the command is typed in. FROMID = TOID is a
  loop.
- **`Rooms.md`** [C]: 783 named waypoints with shop (0x02), bank (0x04), trainer (0x08) and boss
  (0x10) flags, level windows and trainer classes; the node list of the path graph.
- **`Messages.md`** [I]: 527 effect start and end texts with effect classes (blind, confused,
  poisoned, damage over time, held, can't act, regenerating, incapacitated), and 78 "chase"
  texts with the command that follows a monster through a special exit.
- **Prompt rules** [M]: the last prompt line starts `[HP=` and ends `]:`; casters need `MA=` or
  `KAI=` on it; `(Resting)` right after `]:` is detected.
