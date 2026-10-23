# Plan: wayfarer, a modern MajorMUD playing client and bot

Started 2026-10-17 for John, after the silvermere server plan. It is built from three
conversations with John (2026-10-17), the `kb/` knowledge base, the Python prototype in
`test/play/`, silvermere's docs, and a study of MegaMud v2.0 Alpha (`kb/megamud.md`).
Moved here from `jnfisher/mmud-exploration` (`client-plan/` at commit `55b129a`) on
2026-10-17 by card C-001; **this copy is the live one.**

**Paths that point outside this repository.** The plan names files in two other repositories
in backticks: `kb/...` is [docs/kb/](../kb/README.md) in this repository since 2026-10-19;
`test/play/...`, `reference/...`, `server-plan/...`, `HANDOFF.md` and `CLAUDE.md` (where it
means the old one) are in
[jnfisher/mmud-exploration](https://github.com/jnfisher/mmud-exploration); paths starting
`silvermere/` or naming silvermere's crates are in
[jnfisher/silvermere](https://github.com/jnfisher/silvermere). Fixtures the tests need are
copied into `tests/fixtures/` with provenance; nothing here reads those repositories at build
or test time.

**Status: draft.** Sections marked *Needs John* hold decisions that are John's to make.

| Section | What it settles |
|---|---|
| [01-goals.md](01-goals.md) | What we are building, for whom, what we take from MegaMud and what we leave; non-goals; the name |
| [02-architecture.md](02-architecture.md) | Gateway, brain and UIs; processes; the wire protocol; world data; tech stack; repositories |
| [03-engine.md](03-engine.md) | Parser and events, world model, executor, the mode stack, party seam |
| [04-settings.md](04-settings.md) | Settings declared once; layers and presets; the essentials page; the starter catalogue |
| [05-ui.md](05-ui.md) | Views (tile, focus, party), terminals, fonts and scrollback, typing beside the bot, pause, the terminal UI |
| [06-accounts-and-setup.md](06-accounts-and-setup.md) | First run, servers, logon scripts, account and character creation, credentials |
| [07-development-and-testing.md](07-development-and-testing.md) | Watching the bot live while it is built, never restarting the server, test layers |
| [08-packaging.md](08-packaging.md) | One-file installers, signing, updates, the headless build |
| [09-milestones.md](09-milestones.md) | Milestones in build order |
| [10-party-play.md](10-party-play.md) | Parties: forming, separation, party rests and flees, healing members, the Parties panel |
| [tasks/](tasks/README.md) | The task cards: wave 1 (M0) written; how cards run; conventions |

Reference: `kb/megamud.md` (what MegaMud did, its settings catalogue and data formats),
`kb/architecture.md` (the client architecture as prototyped), `kb/plan-grouping.md` (party
scripting), `server-plan/tasks/CARD-FORMAT.md` (the card format this plan's cards will reuse).
