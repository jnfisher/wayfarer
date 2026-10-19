# MajorMUD client knowledge base

> Moved from `jnfisher/mmud-exploration` (`kb/`) on 2026-10-19; this copy is the live one
> (`PROVENANCE.md`). Paths like `test/play/`, `reference/`, `WCCMMUD-1.11p-source/` below are in
> that repository. Facts about silvermere's own behaviour stay in its `kb/server-behaviours.md`.

Facts a modern MajorMUD playing client (a successor to MegaMud) needs: how the game talks, how
to read the screen, how to move, fight, recover, shop, level and choose where to grind. It also
covers the game data a client can read directly, and the pitfalls we've hit.

Each fact carries its provenance and status:

- **[C] confirmed**: by the recovered source (file:line under `WCCMMUD-1.11p-source/SRC/`), the
  game data, or observed play (a transcript under `test/play/logs/`).
- **[I] inferred**: likely, but not proven.
- **[F] forum/web**: from community sources; verified against the data where marked.
- **[M] MegaMud**: from MegaMud's own manual, resources or data (`reference/megamud/`): what
  MegaMud did, not what the game does.

Scope: MajorMUD 1.11p (WCCMMUD.DLL), the stock "Realm of Legends" world data, played on
MBBSEmu. Other versions and custom worlds differ in data, and sometimes in rules.

| File | Covers |
|---|---|
| `architecture.md` | Recommended client architecture, and why it's data-driven |
| `connection.md` | Telnet, MBBSEmu, login flow, entering the realm, encoding |
| `screen-parsing.md` | Every text pattern a client must recognise (prompt, room, combat, loot, errors) |
| `screen-patterns.md` | The full pattern catalogue: about 410 source-confirmed patterns in 17 categories, the configurable statline, the WCCMSG-driven families, what the bot misreads today. The executable form is `test/play/patterns.py` |
| `plan-patterns-migration.md` | Plan for moving `progress.py`/`grind.py` onto the pattern registry |
| `plan-grouping.md` | Spec for leader/follower group scripting: MegaMud-style `@` telepath protocol, roles, build stages |
| `navigation.md` | Map model, exit types and how to take each, position tracking, routing |
| `combat.md` | Targets (alignment), attack modes (BASH), danger estimation, fleeing |
| `recovery.md` | Resting, regen, healers, light and darkness, death |
| `economy.md` | Currency, prices, markup, shops and stock, training fees, encumbrance, selling and dropping |
| `progression.md` | Levels and exp, training, CP and stats, choosing hunting grounds, respawn rules, patrol routing, quests |
| `game-data.md` | Data files and record layouts a client can read (rooms, monsters, items, shops, classes, races, messages) |
| `server-behaviours.md` | How the original server orders its timers, tracks who is "in the game", times out idle users, and builds the output buffer behind `tell_user`/`tell_room`/`tell_game` (what bytes a client receives, and in what order). Read it when a capture's colour codes or line order look odd, or when re-implementing the server |
| `pitfalls.md` | Things that went wrong in practice, and the rule that fixes each |
| `classes/` | Per-class guides for all 15 classes: race picks, stats, attack style, gear, grinding path, quests (`classes/README.md`) |
| `spells.md` | Learning spells from scrolls, casting syntax, mana, spell record fields, bot policy |
| `aoe-grinding.md` | Area-of-effect spell mechanics and the best AoE spots by level band |
| `grouping.md` | Party commands, the exp split (only autocombat on the victim counts), random aggro, spawns that scale with players, compositions, group spots by band, client rules for boxing several characters |
| `stat-builds.md` | Str-first vs balanced CP builds for two-handed melee: what Str does and does not do, a kills/hour model per level band, recommended ladders (`statplan.py`), race picks |
| `megamud.md` | MegaMud: how it worked, what a successor takes, adapts or leaves; with `megamud-manual.md` (the manual by topic), `megamud-settings.md` (every dialog setting) and `megamud-formats.md` (room ids, `.mp` paths, data files) |
| `changelog.md` | What was added to the KB, and when |

Related material elsewhere in the repo:

- `test/play/`: the working prototype. `bridge.py` is the session and viewer, `mmud_data.py`
  the data layer, `progress.py` the bot.
- `test/play/notes/`: deep dives on combat formulas, quests and published routes.
- `test/play/corpus/`: forum-derived progression corpus (levelling areas, gear, bosses).
