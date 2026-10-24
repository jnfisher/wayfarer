# wayfarer: project notes for Claude

wayfarer is a MajorMUD playing client and bot: one program runs up to six characters, shows
them in a browser, desktop or terminal UI, and can play each one. Start at
[docs/plan/README.md](docs/plan/README.md); the task cards are in
[docs/plan/tasks/](docs/plan/tasks/README.md).

Nothing from mmud-exploration's Python prototype (`test/play/`) or its CLAUDE.md applies here
unless the rules below list it.

## Rules (copied from docs/plan/01-goals.md "Rules"; change them there first)

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

- Log off cleanly; dropping in the realm is punished (`kb/connection.md`).

**Prototype policies that become settings with defaults, not rules:**

| Prototype rule | In the client |
|---|---|
| Never rest in a room that spawns monsters | Setting (`health.rest_quiet_room`), off by default: spawns are slow enough to rest anywhere, lairs too (John, 2026-10-24) |
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

## Conventions (from docs/plan/tasks/README.md)

- **CI is one fast job**: the whole Rust suite in under a minute; a test over one second says
  why in a comment; tests are in-process (no network, game server, database or Docker).
  Anything that needs a game server runs against the long-lived local silvermere at
  `127.0.0.1:2323` (`scripts/live-check.sh`), never in Actions. Never start, stop, rebuild or
  reset that server from a card.
- **This repository is public.** Nothing personal goes in: no real hostnames, account names or
  passwords; examples use placeholders.
- **No dependency on mmud-exploration or silvermere at build or test time.** Test data is
  copied into `tests/fixtures/<subdir>/` with a `PROVENANCE.md`; never edit the copies, add a
  new subdirectory instead.
- **Knowledge**: the KB is `docs/kb/` in this repository (moved from mmud-exploration on
  2026-10-19). A card that confirms a fact about the game or MegaMud that the KB lacks
  updates the right file with its provenance tag ([C], [I], [F], [M]) and adds a line to
  `docs/kb/changelog.md` in the same PR, and says so under "KB notes" in the PR description.
  Facts about silvermere's own behaviour go to mmud-exploration's `kb/server-behaviours.md`
  instead, as a note for John.
- **Checks before pushing**: `cargo fmt --all`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo nextest run --workspace`.
- **Commits and PRs**: no model names.
