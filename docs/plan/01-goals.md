# 01. Goals, non-goals and the name

## What we are building

A MajorMUD client that plays: a modern successor to MegaMud. One program runs a whole party
of up to six characters, shows them however the player likes, and can play each of them
automatically. It speaks only telnet, so it works against silvermere, MBBSEmu running the
original module, and any BBS still running MajorMUD 1.11p.

## Goals (from John, 2026-10-17)

1. **A bot engine first, solo before party.** Automated play for one character (travel to the
   chosen area, run its loop, fight, rest, flee, loot), as good as `test/play/progress.py` and
   then better. Get solo right, then run several solo characters side by side, and only then
   build party play. The engine holds several characters from day one, so that order costs
   nothing later.
2. **The player decides; the bot carries out.** Training, banking, selling and moving on to a
   new area are each a setting the player turns on or off. Where to hunt is the player's
   choice: the client *recommends* an area when asked ("Where next?", with reasons), and
   moves on by itself only if the player turns that on. No automatic gear buying.
3. **Per-character control.** Every character's tile and card has a start/stop button for its
   bot and one click into that character's bot settings.
4. **Many characters in one program.** Up to six sessions in one process, with no copies of
   the program per character (MegaMud's biggest pain). Views: all six tiled, one in focus with
   status cards for the others, a party view. See `05-ui.md`.
5. **Browser, desktop and terminal.** One web UI that runs in a browser and, wrapped with
   Tauri, as a desktop app; plus a text-mode UI for terminals and SSH. Old-school ANSI look,
   modern features. See `05-ui.md`.
6. **Scalable fonts and deep scrollback**, readable with six tiles on one screen.
7. **Settings like MegaMud's, but easy.** The settings that matter most (rest and flee
   thresholds, attack mode, sneak and backstab, attack spells and mana, heals) on one page,
   with sensible defaults per class, and everything else one level down. See `04-settings.md`.
8. **Less pain setting up accounts.** Known servers, generated logon scripts, guided account
   and character creation, stored credentials. See `06-accounts-and-setup.md`.
9. **Type beside the bot.** Typing in a view does not interrupt the bot unless the command
   needs it to; one key pauses one character or all of them. See `05-ui.md`.
10. **One-file installer** that a non-technical player can use. See `08-packaging.md`.
11. **Fun to build.** From the first milestone John watches the bot-run character in the real
   UI while it is developed, and the game server never has to be rebuilt or restarted for
   client work. See `07-development-and-testing.md`.
12. **Not a MegaMud clone.** Take its concepts, not its shape. `kb/megamud.md` lists what we
    take and what we leave.

## Rules: nothing carries over by default

The prototype in `test/play/` and the rules in `mmud-exploration/CLAUDE.md` were written for
those Python scripts. **None of them applies to the client unless it is listed here.** The
client's own CLAUDE.md is written fresh from this plan, not copied. The prototype and the KB
are sources of *facts about the game*; their policies ("the bot does X") are not a spec. When
a card wants to reuse a prototype behaviour, the card says so and why.

**Carried over, as client rules (John, 2026-10-17):**

- The bot never trains stats, by any setting. The player spends CP, in character creation too
  (`06-accounts-and-setup.md`).
- The player decides where to hunt and whether to train, bank, sell or move on (goal 2).

**Game facts that become guards** (the game punishes the alternative, `kb/`):

- No attack command while auto-combat is engaged: it restarts the swing timer
  (`kb/combat.md`).
- Log off cleanly; dropping in the realm is punished (`kb/connection.md`).

**Prototype policies that become settings with defaults, not rules:**

| Prototype rule | In the client |
|---|---|
| Never rest in a room that spawns monsters | Setting, on by default |
| Flee by a confirmed exit to a room with no spawns; keep running if followed | How Flee works; distance and direction are settings |
| Death pile: one recovery attempt, then abandon | Setting: never, once (default), or until recovered |
| SYSOP GOTO to named towns only | Setting for sysop characters, off by default; towns only |
| Progression plans move to the next area when outlevelled | "Where next?" recommendation; moving on automatically is opt-in |
| Bot may take on tough fights, relying on fleeing | The *Bold* style preset |

**Prototype practices that don't apply at all:** one bot per bridge session (the gateway owns
sessions), restarting every bot after a code change and killing bots by PID (the brain
re-syncs on restart, `07-development-and-testing.md`), the `--hit-frac`, `--risk` and other
command-line knobs (settings replace them), `plans/*.json` as the source of truth for areas
(one input to "Where next?").

## Non-goals (for now)

- A world editor, or any server administration beyond what a player can do.
- Reading server internals. The client knows only what comes over the wire plus a world data
  set (`02-architecture.md`).
- Scripting languages for users (MegaMud-style trigger scripting). Settings and the mode stack
  come first; a small rules table may follow if the fixed settings are not enough.
- Worlds other than 1.11p "Realm of Legends" beyond what a world data import gives for free.
- Dial-up, modem or ZModem (MegaMud v2.0 dropped them too).

## The name

**wayfarer** (John, 2026-10-17): a traveller who walks paths, which is most of what the bot
does. Crate prefix `wayfarer-`, binary `wayfarer`, repository `jnfisher/wayfarer`. Not yet
checked for clashes. The names considered:

| Name | Why |
|---|---|
| **wayfarer** (chosen) | A traveller who walks paths, which is most of what the bot does |
| newhaven | Lore pair with silvermere; "where you start" fits a client |
| lodestar | A guiding star: navigation and guidance; reads as a product |

**More in the lodestar vein** (astronomy and celestial navigation; none checked for clashes yet):

| Name | Why |
|---|---|
| astrolabe | The instrument for finding your place by the stars: position belief and routing |
| polestar | Lodestar's twin: the fixed star you steer by |
| orrery | A clockwork model of the heavens, many bodies moving at once: six characters in one view |
| ephemeris | The table of where every body will be: a data-driven bot that predicts spawns and routes |
| sextant | Navigator's instrument; short, solid, a good binary name |
| meridian | The line every star crosses; also "the high point", and a hub between towns |
| azimuth | The bearing to a target; sounds like a tool |
| zenith | The highest point overhead; aspirational, short |
| asterism | A pattern of stars seen together, like a party of characters |
| almanac | The navigator's yearly book of tables; friendly, old-school |

A rename is a find-and-replace on this plan.
