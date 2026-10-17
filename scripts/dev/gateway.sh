#!/usr/bin/env bash
# Build wayfarer in release mode and run the gateway with the dev config.
# Usage: scripts/dev/gateway.sh [extra `wayfarer gateway` arguments]
# The config lives in WAYFARER_DATA_DIR (default: the macOS per-user data directory); the first
# run copies examples/dev.toml there. Re-running never overwrites it.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
data=${WAYFARER_DATA_DIR:-$HOME/Library/Application Support/wayfarer}

mkdir -p "$data"
if [ ! -e "$data/wayfarer.toml" ]; then
  cp "$root/examples/dev.toml" "$data/wayfarer.toml"
  echo "gateway.sh: wrote $data/wayfarer.toml; edit the characters, and put their accounts in" >&2
  echo "            $data/credentials.toml (see examples/credentials.toml)" >&2
fi

cd "$root"
cargo build --release -p wayfarer
exec ./target/release/wayfarer gateway --config "$data/wayfarer.toml" --ui-dir "$root/ui/dist" "$@"
