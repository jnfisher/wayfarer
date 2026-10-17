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

## Conventions (from docs/plan/tasks/README.md)

- **CI is one fast job**: the whole Rust suite in under a minute; a test over one second says
  why in a comment; tests are in-process (no network, game server, database or Docker).
  Anything that needs a game server runs in the agent's container (`scripts/live-check.sh`),
  never in Actions.
- **This repository is public.** Nothing personal goes in: no real hostnames, account names or
  passwords; examples use placeholders.
- **No dependency on mmud-exploration or silvermere at build or test time.** Test data is
  copied into `tests/fixtures/<subdir>/` with a `PROVENANCE.md`; never edit the copies, add a
  new subdirectory instead.
- **Knowledge**: confirmed facts about the game or MegaMud that the KB lacks go in the PR
  description under "KB notes". The KB lives in `jnfisher/mmud-exploration`, `kb/`; attach
  that repository read-only to read it.
- **Checks before pushing**: `cargo fmt --all`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo nextest run --workspace`.
- **Commits and PRs**: no model names.
