#!/usr/bin/env bash
# Run the brain against a running gateway, restarting it on every engine change.
# Usage: scripts/dev/brain.sh [extra `wayfarer brain` arguments]
# Needs cargo-watch (a developer tool, not a build dependency). The gateway keeps the
# characters logged in while the brain restarts; on attach the brain re-syncs them.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
data=${WAYFARER_DATA_DIR:-$HOME/Library/Application Support/wayfarer}

if ! cargo watch --version >/dev/null 2>&1; then
  echo "brain.sh: cargo-watch is not installed; install it with" >&2
  echo "            cargo install cargo-watch    (or: brew install cargo-watch)" >&2
  exit 1
fi

mkdir -p "$data"
cd "$root"
# The world pack, with MegaMud's path set when the zip is at hand (see gateway.sh and
# docs/dev-rig.md "The world pack").
megamud="${WAYFARER_MEGAMUD:-$root/../mmud-exploration/reference/megamud/MegaMUD_v2.0_Alpha_-_With_Paths.zip}"
if [ ! -f "$data/world.sqlite" ]; then
  echo "brain.sh: building the world pack into $data/world.sqlite" >&2
  if [ -f "$megamud" ]; then
    cargo run -q -p wayfarer -- import-world --export tests/fixtures/world --megamud "$megamud" --out "$data/world.sqlite"
  else
    echo "brain.sh: no MegaMud zip at $megamud; the pack has no MegaMud loops" >&2
    cargo run -q -p wayfarer -- import-world --export tests/fixtures/world --out "$data/world.sqlite"
  fi
fi

# The gateway address defaults to the dev gateway's; the token comes from $data/gateway-token
# (a loopback gateway checks none). `wayfarer brain` reads both from the data directory.
exec cargo watch -w crates -w bin -x "run -q -p wayfarer -- brain --config '$data/wayfarer.toml' $*"
