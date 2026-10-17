# 03. The engine (brain)

The brain turns terminal text into events, keeps a model of each character and the world, and
decides what to send. Its core is deterministic: given the same events and the same clock it
sends the same commands. Time and randomness are injected, so replays and a simulator can
drive it (`07-development-and-testing.md`).

## 1. Parser: text to events

- **Incremental and prompt-bounded.** Output is split into lines as it arrives, and a block
  closes at the prompt (`[HP=..]:` in its five statline modes, `kb/screen-patterns.md`).
  Nothing above the parser sees raw text. (The prototype runs regexes over "everything since
  the last command"; `kb/architecture.md` lists why that hurt.)
- **The pattern table is generated, not discovered.** Start from `test/play/patterns.py`
  (404 static patterns and about 6,400 generated from WCCMSG, 0.2% of transcript lines
  unclassified) and generate the Rust table from the same sources: the module's message
  catalogue (silvermere-text holds it in modern form) plus the static patterns. A test fails
  when the catalogue and the table drift.
- **Colour is kept** while classifying, because monster names are coloured by alignment.
- **Every line is accounted for.** Lines no pattern matches are counted and shown in a dev
  panel; the unclassified rate is a soak metric.

Event families (one Rust enum, `wayfarer-proto`): prompt and vitals; room seen (title, exits,
items, people, monsters); movement results (moved, blocked, no exit, door, trap door, glued);
combat (engaged, our blow, their blow, miss, kill, flee, combat off); damage and healing;
spells and effects (cast, fizzle, no mana, effect on and off, from MegaMud's message list and
WCCMSG); loot and coins; inventory and equipment; shop lists and purchases; experience and
level; training; status screens (`st`, `exp`, `i`); party (invite, join, `par` lines,
telepaths); death, unconsciousness and the death pile; chat; errors and refusals.

## 2. World and character model

- **Static world** from the world pack (`02-architecture.md`); **live overlay** per server:
  rooms seen, last cleared times, monsters met there, measured danger, shop stock, names known
  to be immune or unfightable.
- **Position is a belief**: the set of rooms matching the observed title and visible exits,
  advanced through each move and collapsed by the next room seen. "There is no exit in that
  direction!" means the belief is wrong: look and relocate (`kb/pitfalls.md` 34). Many rooms
  share a name (three identical "River Street" rooms in Silvermere).
- **Character**: HP and mana (and their maxima), level, exp and exp to next level, class and
  race, stats, spells known, inventory, equipment, coins and encumbrance, statuses (poisoned,
  held, blind, diseased), auto-combat state, the current target.
- **Measured statistics** in the store: exp/hr, kills, deaths and flees per hunting ground and
  level, attack results per mode (bash vs attack), heal amounts. Measured beats estimated
  (the slums looked better on paper; the arena gave three times the exp).

## 3. Executor

- An **action** is a command plus the events that confirm it, a timeout, and what to do on
  each kind of failure. A purchase isn't done until "You just bought"; a move isn't done until
  a room arrives (`kb/architecture.md`).
- One action in flight per character, except moves along a known route, which may be pipelined
  a few steps ahead the way MegaMud ran paths, cancelled the moment a room disagrees with the
  belief or combat starts.
- Hard guards live here, so no mode can break them: never `train stats`; no attack command
  while auto-combat is engaged. Only the rules in `01-goals.md` ("Rules") are guards; every
  other safety policy is a setting with a default.

## 4. The mode stack

Each character runs a small set of modes, highest priority first. Every tick, the highest mode
that wants control gets it; a mode can interrupt only those below it.

| Mode | Wants control when | Does |
|---|---|---|
| **Flee** | HP below the flee threshold, a monster far above our danger limit, or a party flee | Leaves by a confirmed exit toward a room with no spawns; keeps running if followed |
| **Fight** | Engaged, or a target worth fighting is here | Chooses target and attack (melee mode, backstab opener, attack spell), keeps auto-combat on |
| **Recover** | HP or mana below the rest threshold; poisoned or diseased; a death pile to fetch | Moves out of spawn rooms, heals, rests to the rest-until threshold; one death-pile attempt |
| **Maintain** | Each only if its setting is on: level ready to train, coins to bank, loot to sell, encumbered, out of light | Walks to the trainer, bank or shop; trains (never stats), banks, sells, restocks light. Never buys gear |
| **Task** | Always, when nothing above wants control | Walks to the area the player chose and runs its loop. When outlevelled: tells the player and recommends; moves on only if "advance automatically" is on |
| **Idle** | Paused, or nothing to do | Sends nothing |

Modes are small components with `wants(state) -> Option<reason>`, `step(state) -> Action`,
and `on_event`. Policies (flee check, target choice, rest, heal and spell choice, shopping,
training, which area) are pure functions of state, world data and settings, testable offline.
Each mode emits **narration**: one line saying what it is doing and why ("Resting to 80%:
Arena spawns, walked to the Hallway first"). The UI shows it, and the logs keep it.

**Areas and loops.** The player picks the hunting area; the bot walks there and runs its
loop. Loops come from three places: loops generated from the world data, loops the player
records or edits, and, when the player imports their own MegaMud copy, **MegaMud's community
loop library** (195 loops in the v2.0 path set, read with the room-id scheme in
`kb/megamud-formats.md` and walked through the world pack's rooms: all 183 that walk to the
end close). MegaMud's data is never shipped with wayfarer or kept in its repository.

**Trusting MegaMud's data** (John, 2026-10-17). The world pack's game data (rooms, exits,
monsters, items, spells, shops) comes only from the real game records; MegaMud's own
databases are never used. Its paths and waypoints are kept apart (`mm_*` tables) and checked
against the real rooms, and two rules bound their use:

- The bot runs a MegaMud loop only if its verification walk reached the end and closed in its
  start room. Broken or unverified paths are never run (they stay in the pack, flagged).
- "Where next?" ranks areas on the real monster data and measured results. A MegaMud loop is a
  ready-made route through an area already chosen, never a reason to choose it.

**Area recommendations** are a query, not a decision. "Where next?" ranks areas for this
character from the world data (monster exp, HP and hits against the character's measured
damage and defence, distance, spawn rates), measured results in the store, the class guides
and the forum-derived level paths (`kb/progression.md`, `test/play/corpus/`), and says why
for each ("Arena: 2,830 exp/hr measured at level 5; nothing hits harder than 12"). The bot
asks the same question when it is outlevelled, and either notifies the player or, if
"advance automatically" is on, takes the top recommendation and narrates it.

## 5. Pause, manual and take-over

| State | What the bot does | Typical use |
|---|---|---|
| **Running** | Everything its settings allow | Normal play |
| **Paused** | Nothing new from Task or Maintain; Flee, Fight back and Recover still act if the setting "guard while paused" is on (default on) | Look around, chat, steer by hand for a moment |
| **Stopped** (manual) | Nothing, except the assists the player turned on (fight back, rest and heal, keep a light, pick up coins; `04-settings.md`); parser and status keep running | Full hand play; the client is a plain multi-session client. The default for a new character |

Pause and resume work per character, per party, or for all characters, from one key each
(`05-ui.md`). Pausing takes effect after the action in flight, never mid-exchange, so the
belief stays right. Resume re-syncs first (look) and narrates what it will do next.

## 6. Player input beside the bot

Typing in a view must not break the bot. The executor classifies each typed line with the
game's own command table (silvermere's `tools/parsegen` produced every abbreviation the
original's parser accepts and the command it maps to, so `n`, `a rat` and `gos hi` are
classified exactly as the game would):

| Kind | Examples | What happens |
|---|---|---|
| Talk and look | gossip, telepath, say, look, stat, exp, inventory, who, health | Sent at the next prompt boundary between the bot's actions. The bot keeps going and ignores that output for its own confirmations |
| Acts | move, attack, cast, rest, get and drop, buy and sell, equip | Pauses the bot for that character (narrated: "You took over Kasimir"), then sends. Resume with one key |
| Forbidden while engaged | attack commands while auto-combat is on | Held with a warning; the player can force it |
| Raw keys | the character editor, `Hit any key` | Straight through; pauses the bot |

The player can also queue a line for the bot to send at its next safe point ("after this
fight, `sell sword`").

## 7. Restarts

The brain may restart at any time (`07-development-and-testing.md`). On attach it reads the
gateway's state for each session, loads its own persisted state (plan, loop position, pile,
pause state) from the store, re-syncs with `st`, `i`, `exp` and a look, rebuilds the position
belief, and resumes, narrating that it did. A restart in the middle of a fight resumes in
Fight or Flee.

## 8. Several characters and the party seam

All characters live in one brain from the start; each has its own model, executor and mode
stack. Solo play is the first target, but nothing assumes one character. The **party
coordinator** (later, `kb/plan-grouping.md` stages G3 on) sits above the characters and works
by constraints and requests to their mode stacks: "wait", "follow the leader", "assist on
target X", "heal Y", "flee together", "cross this text exit together". It sees every member's
real state, so it needs none of MegaMud's telepath round-trips. The `@` telepath protocol
(`test/play/party.py`) is kept for parties that mix our characters with other people's
clients.
