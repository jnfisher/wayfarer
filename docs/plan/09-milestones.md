# 09. Milestones

Ordered so that John watches a real character in the real UI from the first milestone, and
the bot engine comes before party play. Each milestone ends with something to look at.

| # | Milestone | What John can do at the end |
|---|---|---|
| **M0** | Gateway and a live terminal | Open a browser, see each character's session in its own tile, type into it, scroll back. Characters stay logged on across UI reloads |
| **M1** | Parser, events and status | Tiles show live HP, mana, exp and room in their headers; a dev panel shows the event stream and unclassified lines. The brain can restart without logging anyone off |
| **M2** | World and navigation | A map panel with the position belief; click a room and the character walks there, confirming each step. Loops generated from the world data; a player's own MegaMud loops imported and checked against it |
| **M3** | Solo bot | A character runs the area John picks: walk there, loop, fight, rest, flee, with narration on its card. Start/stop and pause per character, typing beside the bot. Replay mode plays back any recorded session. Measured against `progress.py` |
| **M4** | Settings and upkeep | The essentials page and the full settings, generated from the declarations, with class presets; one click from a card into that character's settings. Attack spells, heals; training, banking, selling and light as switches. "Where next?" recommendations |
| **M5** | Many solo characters | Up to six characters, each running its own solo bot: tile, focus and cards views, the command bar with prefixes, pause all, the merged log, notifications. Soak-tested with several solo bots at once (shared areas, competing for spawns) before any party work |
| **M6** | Setup and installers | The first-run wizard, account creation and the hand-off to the game's stat editor, local credentials, one-file installers for Windows, macOS and Linux, updates |
| **M7** | Party play | Only after M5 holds up: the party coordinator (`kb/plan-grouping.md` G3 on), the party view |
| **M8** | Extras | Terminal UI, the simulator, MegaMud import |

The terminal UI (M8) needs only the protocol, so it can start any time after M1 if John wants
it sooner.

**Parity bar for M3.** On silvermere and on MBBSEmu with the original, over the same hours
and class: exp/hr within the noise of `progress.py`'s measured baseline, no more deaths, fewer
position relocations, unclassified-line rate under 0.2%. Then silvermere's soak layer can
switch from `progress.py` to wayfarer.

## First wave of task cards (M0)

Written in full in [tasks/](tasks/README.md): C-001 to C-009, from the repository scaffold to
the live check. The index there has the dependencies, tiers and what "M0 done" means.

## Second wave (M1), in outline

Written early, running beside wave 1: C-010 (the parser, with golden parity against
`patterns.py`) and C-011 (the world pack, with MegaMud waypoints and verified loops). Written
on 2026-10-19, once M0 was live (`tasks/README.md`): the protocol's brain families (C-014),
typed events (C-015), the brain process with attach and re-sync (C-016; nothing to persist
yet), the status header (C-017), the dev panel (C-018), `serve` and the watcher (C-019), and
two things John asked for after M0: a help overlay (C-012) and resizable tiles (C-013).

## Third wave (M2, plus two pulls), written 2026-10-19

After John tried M1 and said go (`tasks/README.md`): the room graph, routing, the position
belief and the map layout (C-021); the executor and the walker with the M2 messages (C-022);
the map panel (C-023); generated loops (C-024); the M1 follow-ups first (C-020). Pulled
forward at John's request: the first slice of M3, "fight here" (C-025: Fight and Recover in
the current room, start/stop/pause, no flee), and a page for running wayfarer on a server
over Tailscale (C-026), ahead of M6's installers.
