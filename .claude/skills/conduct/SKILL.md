---
name: conduct
description: Run the task cards for John. On each tick, start the cards whose dependencies are merged as background sessions, check their pull requests, merge the finished ones and report what needs John. Only for the conductor session John starts with `/loop /conduct` in ~/repos/wayfarer; never from a card session.
---

# Conduct the task cards

You are the conductor: the planning session that `docs/plan/tasks/README.md` calls "John, or
one local planning session he asks". Card sessions do the work; you start them, watch them
and merge their pull requests. You run under `/loop`, so each invocation is one **tick**: look
at the state, act, report in a few lines, and pick the next wake-up. GitHub and
`claude agents` are the source of truth, not your memory.

## Each tick

1. **Sync.** `git fetch origin main` and read the index with
   `git show origin/main:docs/plan/tasks/README.md`: the cards, their tier, dependencies and
   whether they need a review. A card's file is `docs/plan/tasks/<id>-<slug>.md`; its branch
   is `card/<id>-<slug>`.
2. **State of every card in the current wave.**
   - Pull request: `gh pr list --state all --head card/<id>-<slug> --json number,state,isDraft,mergeable,headRefOid,url`.
   - Session: `claude agents --json --all` (working, blocked, done, failed, stopped);
     sessions you start are named after the card (`C-005`, `C-005-fix`).
   - A card is **merged** when its PR state is `MERGED`. C-001 to C-004, C-010 and C-011
     are merged already.
3. **Act, in this order:**
   1. **Blocked or failed sessions.** Blocked means waiting on a prompt or a question: tell
      John its name and `claude attach <name>`. Failed: read `claude logs <name>`, then
      start a fix session or tell John why not.
   2. **Open card PRs** whose session is done:
      - CI: `gh pr checks <n>`. Red: start a fix session. Pending: leave it.
      - Merge conflict: start a fix session ("merge main into the branch and resolve").
      - Review: read `gh pr diff <n>` against the card's "Done when"; every box really done,
        with the evidence the card asks for. Cards marked **review** in the index get a
        careful second-reviewer read. Anything missing or wrong: `gh pr comment`, then a
        fix session.
      - **Needs John** (C-009's test-character stats; any "Needs John" or "Decided by John"
        section; a change to `docs/plan/01-goals.md` "Rules"): don't merge; tell John what
        to look at.
      - Otherwise merge it yourself, without asking John: `gh pr merge <n> --squash
        --delete-branch` (John, 2026-10-17). Only with green CI, no conflict and your review
        passed.
   3. **Start ready cards**: every card in `depends` merged, no PR, no live session. At most
      **three** card sessions working at once. From this directory:

      ```sh
      claude --bg --name <id> --model <opus for tier B, sonnet for tier C> "Run task card <id> for wayfarer: read CLAUDE.md, docs/plan/tasks/README.md and docs/plan/tasks/<id>-<slug>.md, work on branch card/<id>-<slug>, and finish with a pull request against main with green CI (wait for it with gh pr checks --watch). Do not merge."
      ```

      A **fix session** is the same with `--name <id>-fix` and a brief naming the PR, the
      branch and exactly what to fix ("push to the same branch; do not merge"). At most one
      per card at a time.
4. **Report** in a few lines: merged, started, waiting on John. Nothing when nothing changed.
5. **Next wake-up**: about 5 minutes while sessions or CI are running; about 45 when
   everything waits on John.

## End of a wave

When every card in the wave is merged, stop the loop and tell John, with the milestone's
"done" check from the index. The next wave's cards are written only after John has tried it
and says go; then write them on a branch, open a PR, and run them once it is merged.

## KB notes

Collect the "KB notes" of PRs you merge. At the end of the wave, add them to
`../mmud-exploration/kb/` (right file, provenance, a line in `kb/changelog.md`) on a branch
there and open a PR for John.

## Never

- Start, stop, rebuild or reset the local silvermere server.
- Write code or push to `main`; your own changes go through a pull request.
- Merge with red or pending CI, a merge conflict, or something waiting on John.
- Train stats or spend CP for any character. The player does that; C-009's test characters
  get only the stats John approved in its PR.
