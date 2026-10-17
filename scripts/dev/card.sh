#!/usr/bin/env bash
# Start a task card as a background Claude Code session, with the standard brief from
# docs/plan/tasks/README.md. Usage: scripts/dev/card.sh C-005
# Run it from the main checkout; the session makes its own worktree. Does nothing if the card
# already has a session, so it is safe to repeat.
set -euo pipefail

id=${1:?usage: card.sh C-NNN}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"

shopt -s nullglob
files=("docs/plan/tasks/$id"-*.md)
[ ${#files[@]} -eq 1 ] || { echo "card.sh: no single card file for $id in docs/plan/tasks/" >&2; exit 1; }
file=${files[0]}
slug=$(basename "$file" .md)

if claude agents --json 2>/dev/null | grep -q "$slug"; then
  echo "card.sh: $id already has a session (claude agents)" >&2
  exit 0
fi

claude --bg "Run task card $id for wayfarer: read CLAUDE.md, docs/plan/tasks/README.md and $file, work on branch card/$slug, and finish with a pull request against main with green CI (wait for it with gh pr checks --watch). Do not merge."
