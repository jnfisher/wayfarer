---
id: C-001
title: Repository scaffold: workspace, toolchain, the fast CI job, fresh CLAUDE.md, plan moved to docs/plan/
tier: B
size: S
wave: 1
depends: []
crate: (workspace)
touches:
  - Cargo.toml, rust-toolchain.toml, .config/nextest.toml, .gitignore
  - crates/wayfarer-proto/, crates/wayfarer-telnet/, crates/wayfarer-gateway/ (empty lib crates)
  - bin/wayfarer/ (CLI skeleton)
  - .github/workflows/ci.yml, .github/pull_request_template.md
  - CLAUDE.md, README.md, docs/plan/ (copied from mmud-exploration client-plan/), docs/README.md
  - tests/fixtures/{captures,transcripts,patterns,world}/ (copied from mmud-exploration, each with PROVENANCE.md)
---

## Goal

`jnfisher/wayfarer` has a Rust workspace that builds, one CI job that checks it in a couple of
minutes, a CLAUDE.md written for this project, and the plan in `docs/plan/`. Every later card
starts from here.

## Inputs

- `mmud-exploration/client-plan/` (all of it; it becomes `docs/plan/` here).
- `docs/plan/02-architecture.md` "Crates" and "Tech stack"; `07-development-and-testing.md`
  "CI: one fast job"; `01-goals.md` "Rules".
- For contrast only: `silvermere/.github/workflows/ci.yml` (what not to grow into: Postgres
  service, self-hosted runner, many workflows) and `silvermere/rust-toolchain.toml` (the pin
  style).

## Behaviour

- `cargo build` and `cargo nextest run` pass on a clean checkout; `cargo run -p wayfarer --
  --version` prints the version.
- `wayfarer` has subcommands `gateway` and `serve` that print "not yet" and exit 2 (filled by
  C-006); `--help` lists them.
- CI runs on pull requests only (and `workflow_dispatch`), skips `docs/**` and `**.md`
  changes, skips drafts, cancels a superseded run, and fails on fmt, clippy or tests.

## Design notes

- Workspace members: `crates/wayfarer-proto`, `crates/wayfarer-telnet`,
  `crates/wayfarer-gateway`, `bin/wayfarer`. Later crates are added by their cards, not now.
- `rust-toolchain.toml`: the newest stable on the day, `profile = "minimal"`,
  `components = ["rustfmt", "clippy"]`. `rust-version` in `Cargo.toml` to match.
- Shared dependencies in `[workspace.dependencies]`: tokio, serde, serde_json, thiserror,
  anyhow, clap (derive), tracing, tracing-subscriber.
- Fast tests: in `Cargo.toml`, `[profile.dev] debug = 0` (or `"line-tables-only"`) and
  `[profile.test] debug = 0`; CI sets `CARGO_INCREMENTAL=0`.
- `ci.yml`, one job on `ubuntu-latest`, `timeout-minutes: 10`: checkout,
  `Swatinem/rust-cache`, install the pinned toolchain, `taiki-e/install-action` for
  `cargo-nextest`, then `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --locked -- -D warnings`, `cargo nextest run --workspace --locked`. `concurrency: group:
  ci-${{ github.ref }}, cancel-in-progress: true`. `if: github.event.pull_request.draft ==
  false`. No other workflows in this card (C-007 adds the UI one).
- `CLAUDE.md`, written fresh, short:
  - What wayfarer is; start at `docs/plan/README.md`.
  - The rules, copied from `docs/plan/01-goals.md` "Rules" and nothing else, with the line
    "Nothing from mmud-exploration's prototype or its CLAUDE.md applies unless listed there."
  - The conventions in `docs/plan/tasks/README.md` (CI budget, fixtures with provenance, KB
    notes in PRs).
  - Where the KB lives (`jnfisher/mmud-exploration`, `kb/`) and how sessions attach it.
- `docs/plan/`: copy `client-plan/` verbatim, then fix links that pointed outside it
  (`../kb/...` become links to `jnfisher/mmud-exploration`). Add a line to its README: "Moved
  here from mmud-exploration on <date>; this copy is the live one."
- PR template: Summary, Card, Done-when checklist, Tests run, KB notes.
- **Fixtures, copied once here** so later cards running in parallel never collide. From
  `jnfisher/mmud-exploration` at one commit (named in each `PROVENANCE.md`, with the source
  path of every file):
  - `tests/fixtures/captures/`: `test/play/tests/captures/` (12 scenarios: `raw.bin`,
    `plain.txt`, `script.json`, README), about 150 KB.
  - `tests/fixtures/transcripts/`: `test/play/tests/fixtures/*.txt` (play transcripts with
    `>>> 'cmd'` markers: `kasimir-early.txt`, `thornwyn-early.txt`, the death and level-exit
    ones).
  - `tests/fixtures/patterns/`: `test/play/tests/fixtures/patterns.json` (the exported pattern
    registry).
  - `tests/fixtures/world/`: `test/play/tests/fixtures/world/` (the stock-world export: one
    `.jsonl.gz` per table, `MANIFEST.json`, README), about 2.5 MB.
  Later cards add fixtures of their own in new subdirectories, never by editing these.

## Done when

- The PR's CI run is green, and the job takes under 3 minutes on a re-run with a warm cache
  (report both times in the PR).
- A docs-only change does not start CI (show it with a second commit touching only
  `docs/`, or describe the path filter and how it was checked).
- `CLAUDE.md` contains no rule that is not in `01-goals.md` "Rules".
- `docs/plan/` matches `client-plan/` at the commit named in the PR, links fixed.
- The four fixture directories are present, each with a `PROVENANCE.md`, and byte-identical
  to their sources at the named commit.

## Tests

- One trivial test per crate (so nextest has something to run), and a CLI test that
  `wayfarer --version` exits 0.

## Notes for the agent

- The repository must exist first (John, `tasks/README.md` "Before the first card").
- After this card merges, the planning session marks `mmud-exploration/client-plan/` as moved
  (a one-line README change there); not part of this PR.
- Resist adding tools: no cargo-deny, no coverage, no doc tests, no Windows or macOS jobs.
