#!/usr/bin/env bash
# The live check: the M0 scenario against the game server already running on this machine
# (tests/live/README.md). Run it before pushing a change to the gateway; paste the summary
# into the pull request. Never run in CI. It never starts, stops or resets the server.
#
#   scripts/live-check.sh [--char ACCOUNT]
#
# WAYFARER_LIVE_ADDR: the server (default 127.0.0.1:2323).
# WAYFARER_LIVE_CREDENTIALS: the test accounts' passwords (default: the main checkout's
# tests/live/.credentials.toml, shared by every worktree).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
addr="${WAYFARER_LIVE_ADDR:-127.0.0.1:2323}"
host="${addr%:*}"
port="${addr##*:}"

# Fail fast, before building, when no server answers.
if command -v nc >/dev/null 2>&1; then
    up() { nc -z -w 3 "$host" "$port" >/dev/null 2>&1; }
else
    up() { (exec 3<>"/dev/tcp/$host/$port") 2>/dev/null; }
fi
if ! up; then
    echo "live check: nothing answers at $addr: start silvermere first (docs/dev-rig.md):" >&2
    echo "  cd ~/repos/wayfarer-dev/silvermere-wayfarer && ~/repos/wayfarer-dev/silvermere/target/release/silvermere serve" >&2
    exit 2
fi

cd "$root"
# One password file for every worktree: the main checkout's (worktrees share its .git).
main="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"
credentials="${WAYFARER_LIVE_CREDENTIALS:-$main/tests/live/.credentials.toml}"
mkdir -p "$(dirname "$credentials")"

cargo build --quiet -p wayfarer -p wayfarer-live
target="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
out="$target/live-check/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$out"
exec "$target/debug/wayfarer-live" --addr "$addr" --credentials "$credentials" \
    --wayfarer "$target/debug/wayfarer" --out "$out" "$@"
