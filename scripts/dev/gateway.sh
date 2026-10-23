#!/usr/bin/env bash
# Build wayfarer in release mode and run the gateway with the dev config.
# Usage: scripts/dev/gateway.sh [extra `wayfarer gateway` arguments]
# The config lives in WAYFARER_DATA_DIR (default: the per-user data directory); the first
# run copies examples/dev.toml there. Re-running never overwrites it.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# The platform's per-user data directory, as the binary picks it (Paths::default_data_dir).
case "$(uname -s)" in
  Darwin) default_data="$HOME/Library/Application Support/wayfarer" ;;
  *) default_data="${XDG_DATA_HOME:-$HOME/.local/share}/wayfarer" ;;
esac
data=${WAYFARER_DATA_DIR:-$default_data}

mkdir -p "$data"
if [ ! -e "$data/wayfarer.toml" ]; then
  cp "$root/examples/dev.toml" "$data/wayfarer.toml"
  echo "gateway.sh: wrote $data/wayfarer.toml; edit the characters, and put their accounts in" >&2
  echo "            $data/credentials.toml (see examples/credentials.toml)" >&2
fi

cd "$root"
cargo build --release -p wayfarer
# The gateway serves ui/dist as it is; rebuild it so the page matches the checkout.
if command -v npm >/dev/null 2>&1; then
  (cd ui && npm ci --silent && npm run build --silent) || { echo "gateway.sh: the UI build failed" >&2; exit 1; }
else
  echo "gateway.sh: npm not found; serving ui/dist as it is" >&2
fi
# The world pack is the brain's (and `wayfarer serve`'s); build it here too so either script may go first.
# With MegaMud's path set: data/megamud (stock v2.2 paths), or a directory or zip in
# $WAYFARER_MEGAMUD; docs/dev-rig.md "The world pack".
megamud="${WAYFARER_MEGAMUD:-$root/data/megamud}"
if [ ! -f "$data/world.sqlite" ]; then
  echo "gateway.sh: building the world pack into $data/world.sqlite" >&2
  if [ -e "$megamud" ]; then
    ./target/release/wayfarer import-world --export tests/fixtures/world --megamud "$megamud" --out "$data/world.sqlite"
  else
    echo "gateway.sh: no MegaMud path set at $megamud; the pack has no MegaMud loops" >&2
    ./target/release/wayfarer import-world --export tests/fixtures/world --out "$data/world.sqlite"
  fi
fi
exec ./target/release/wayfarer gateway --config "$data/wayfarer.toml" --ui-dir "$root/ui/dist" "$@"
