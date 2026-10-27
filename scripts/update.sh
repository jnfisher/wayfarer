#!/usr/bin/env bash
# Update wayfarer on a server from a GitHub release: download the release for this machine,
# install it beside the previous one, and restart the `wayfarer` systemd user unit.
# Usage: update.sh [TAG] [--force]    the latest release, or that one (e.g. v0.2.0)
#        update.sh --rollback          back to the release installed before this one
#
# Releases are unpacked under the data directory as releases/<tag>/, with two symlinks:
# releases/current (what the unit runs) and releases/previous (what --rollback goes back to).
# Nothing is stopped until the new release is downloaded, checked and unpacked. The unit is
# stopped with `systemctl --user stop`, whose SIGTERM makes the gateway log every character
# off cleanly first (dropping in the realm is punished, docs/kb/connection.md); that can take
# a few minutes. It never kills wayfarer any other way. The first run installs the unit
# from the release's template. Needs only bash, curl, tar, sha256sum and systemctl (and
# tailscale for the address the first time); docs/server.md "Running a release".
#
# Environment: WAYFARER_DATA_DIR (default ~/.local/share/wayfarer), WAYFARER_REPO (default
# jnfisher/wayfarer), WAYFARER_BIND_IP (default `tailscale ip -4`) and WAYFARER_PORT (default
# 7373) for a new unit, WAYFARER_RELEASE_BASE (tests: a directory URL holding `latest`,
# `releases` and <tag>/<asset> in place of GitHub).
set -euo pipefail

repo=${WAYFARER_REPO:-jnfisher/wayfarer}
data=${WAYFARER_DATA_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/wayfarer}
releases=$data/releases
unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
unit_file=$unit_dir/wayfarer.service
docs="docs/server.md (https://github.com/$repo/blob/main/docs/server.md)"
# How long to wait for the gateway to log everyone off (the unit's TimeoutStopSec is 390 s),
# and for it to listen after a start.
logoff_wait=420
start_wait=180
tick=0.2

if [ -t 1 ]; then
  bold=$'\e[1m' dim=$'\e[2m' red=$'\e[31m' green=$'\e[32m' yellow=$'\e[33m' reset=$'\e[0m'
else
  bold='' dim='' red='' green='' yellow='' reset=''
fi
step=0
started=$SECONDS
say() { step=$((step + 1)); printf '\n%s[%d] %s%s\n' "$bold" "$step" "$*" "$reset"; }
ok() { printf '    %s✓%s %s\n' "$green" "$reset" "$*"; }
note() { printf '    %s!%s %s\n' "$yellow" "$reset" "$*"; }
die() { status_done; printf '\n%s✗ %s%s\n' "$red" "$*" "$reset" >&2; exit 1; }

# Overwrites the current line with `label (Ns): the journal's last line`, cut to the terminal.
status() {
  [ -t 1 ] || return 0
  local line
  line=$(printf '    %s (%ds): %s' "$1" "$2" "$3" | tr -d '\r' | cut -c1-"$(($(tput cols 2>/dev/null || echo 100) - 1))")
  printf '\r\e[K%s%s%s' "$dim" "$line" "$reset"
}
status_done() { [ -t 1 ] && printf '\r\e[K'; return 0; }

usage() { sed -n '4,5s/^# \{0,1\}//p' "${BASH_SOURCE[0]}"; }

tag='' force=0 rollback=0
for arg in "$@"; do
  case "$arg" in
    --force) force=1 ;;
    --rollback) rollback=1 ;;
    -h | --help) usage; exit 0 ;;
    -*) usage >&2; die "unknown option $arg" ;;
    *)
      [ -z "$tag" ] || die "one tag at a time"
      tag=$arg
      ;;
  esac
done
if [ "$rollback" = 1 ] && { [ -n "$tag" ] || [ "$force" = 1 ]; }; then
  die "--rollback takes no tag and no --force"
fi

# --- helpers -------------------------------------------------------------------------------

unit_state() { systemctl --user show -p "$1" --value wayfarer 2>/dev/null || true; }
unit_running() { systemctl --user is-active --quiet wayfarer 2>/dev/null; }
journal_last() { journalctl --user -u wayfarer -n "$1" -o cat --no-pager 2>/dev/null | grep -v '^[[:space:]]*$' | tail -n "$1" || true; }
link_target() { [ -L "$releases/$1" ] && basename "$(readlink "$releases/$1")" || true; }
# Points releases/<name> at <tag> in one rename, so it is never missing or half written.
set_link() { ln -sfn "$2" "$releases/.$1.new" && mv -Tf "$releases/.$1.new" "$releases/$1"; }
human() { numfmt --to=iec --suffix=B "$(stat -c %s "$1")" 2>/dev/null || du -h "$1" | cut -f1; }
# The tag_name of the first release in a GitHub API answer (an object or a list).
tag_name() { grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -n 1 | sed 's/.*"\([^"]*\)"$/\1/'; }

# The address the unit listens on: from the unit's --bind, or (no unit yet) the Tailscale
# address and WAYFARER_PORT.
# The unit without its comments (the template's comments show other ExecStart lines).
unit_body() { grep -v '^[[:space:]]*#' "$unit_file" || true; }
bind_from_unit() { unit_body | grep -o -- '--bind[[:space:]]\+[^[:space:]\\]\+' | head -n 1 | awk '{print $2}' || true; }
pick_address() {
  if [ -f "$unit_file" ]; then
    bind=$(bind_from_unit)
    [ -n "$bind" ] || die "$unit_file has no --bind; add one (docs/server.md \"Running a release\")"
    [ "$bind" != @BIND@ ] || die "$unit_file still has the @BIND@ placeholder: replace it with this machine's address and port, e.g. 100.x.y.z:7373"
    ip=${bind%:*} port=${bind##*:}
    ok "the unit listens on $ip:$port"
  else
    port=${WAYFARER_PORT:-7373}
    if [ -n "${WAYFARER_BIND_IP:-}" ]; then
      ip=$WAYFARER_BIND_IP
      ok "address $ip (from WAYFARER_BIND_IP), port $port"
    else
      command -v tailscale >/dev/null 2>&1 || die "tailscale is not installed; set WAYFARER_BIND_IP instead"
      ip=$(tailscale ip -4 2>/dev/null | head -n 1) || true
      [ -n "$ip" ] || die "tailscale has no IPv4 address (is it up? try: tailscale status)"
      ok "Tailscale address $ip, port $port"
    fi
    bind=$ip:$port
  fi
}

stop_unit() {
  say "Stopping wayfarer (logging every character off cleanly)"
  if ! unit_running; then
    note "wayfarer is not running"
    return 0
  fi
  systemctl --user stop wayfarer &
  local stopper=$! t0=$SECONDS
  while kill -0 "$stopper" 2>/dev/null; do
    if [ $((SECONDS - t0)) -ge "$logoff_wait" ]; then
      status_done
      note "still stopping after ${logoff_wait}s; systemd ends it at the unit's TimeoutStopSec"
      t0=$SECONDS
    fi
    status "logging off" $((SECONDS - t0)) "$(journal_last 1)"
    sleep "$tick"
  done
  status_done
  wait "$stopper" || die "systemctl --user stop wayfarer failed; nothing was switched (journalctl --user -u wayfarer)"
  ok "wayfarer has stopped ($((SECONDS - t0))s): $(journal_last 1)"
}

start_unit() {
  say "Starting wayfarer"
  systemctl --user start wayfarer || true
  local t0=$SECONDS state sub
  until (exec 3<>"/dev/tcp/$ip/$port") 2>/dev/null; do
    state=$(unit_state ActiveState) sub=$(unit_state SubState)
    if [ "$state" = failed ] || [ "$state" = inactive ] || [ "$sub" = auto-restart ] ||
      [ $((SECONDS - t0)) -ge "$start_wait" ]; then
      status_done
      if [ $((SECONDS - t0)) -ge "$start_wait" ]; then
        note "not listening on $ip:$port after ${start_wait}s"
      else
        note "the unit is $state ($sub)"
      fi
      journal_last 20 | sed 's/^/      /'
      die "wayfarer did not start. Go back with: update.sh --rollback (logs: journalctl --user -u wayfarer)"
    fi
    status "starting" $((SECONDS - t0)) "$(journal_last 1)"
    sleep "$tick"
  done
  status_done
  ok "listening on $ip:$port ($((SECONDS - t0))s)"
}

finish() {
  printf '\n%s✓ wayfarer %s is running.%s The UI is at http://%s:%s/ ; logs: journalctl --user -u wayfarer -f %s(%ds)%s\n' \
    "$green" "$1" "$reset" "$ip" "$port" "$dim" $((SECONDS - started)) "$reset"
}

# --- 1. checks -----------------------------------------------------------------------------

printf '%swayfarer update%s %s(%s)%s\n' "$bold" "$reset" "$dim" "$data" "$reset"

say "Checking the tools, this machine and the data directory"
for tool in curl tar sha256sum systemctl; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool is not installed"
done
command -v journalctl >/dev/null 2>&1 || journalctl() { :; }
ok "curl, tar, sha256sum and systemctl found"
machine=$(uname -m)
case "$machine" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) die "this machine is $machine; releases are built for x86_64 and aarch64 only (build from source: $docs, \"Build\")" ;;
esac
ok "Linux $arch"
missing() { die "$data/$1 is missing: $2 ($docs)"; }
[ -f "$data/wayfarer.toml" ] || missing wayfarer.toml 'make it as in "Running a release", step 2 (the game server and your characters)'
[ -f "$data/credentials.toml" ] || missing credentials.toml 'make it as in "Running a release", step 2 (an account and password per character)'
[ -f "$data/gateway-token" ] || missing gateway-token 'make it as in "Running a release", step 2 (the token the browser asks for)'
ok "wayfarer.toml, credentials.toml and gateway-token found"
pick_address
mkdir -p "$releases"

# --- --rollback: steps 5 to 7 with current and previous swapped ------------------------------

if [ "$rollback" = 1 ]; then
  say "Finding the release to go back to"
  cur=$(link_target current) prev=$(link_target previous)
  [ -n "$prev" ] && [ -d "$releases/$prev" ] || die "there is no previous release to go back to; nothing was changed"
  [ -f "$unit_file" ] || die "$unit_file is missing; run update.sh without --rollback to install it"
  ok "now ${cur:-none}; going back to $prev"
  stop_unit
  say "Switching to $prev"
  set_link current "$prev"
  [ -z "$cur" ] || set_link previous "$cur"
  ok "current → $prev, previous → ${cur:-none}"
  start_unit
  finish "$prev"
  exit 0
fi

# --- 2. find the release ---------------------------------------------------------------------

say "Finding the release"
if [ -n "${WAYFARER_RELEASE_BASE:-}" ]; then
  api_latest=$WAYFARER_RELEASE_BASE/latest
  api_list=$WAYFARER_RELEASE_BASE/releases
  download=$WAYFARER_RELEASE_BASE
else
  api_latest=https://api.github.com/repos/$repo/releases/latest
  api_list="https://api.github.com/repos/$repo/releases?per_page=1"
  download=https://github.com/$repo/releases/download
fi
api() { curl -fsSL --retry 2 -H 'Accept: application/vnd.github+json' "$1" 2>/dev/null; }
if [ -z "$tag" ]; then
  tag=$(api "$api_latest" | tag_name) || true
  if [ -z "$tag" ]; then
    # GitHub's "latest" leaves out pre-releases; with only those, take the newest.
    tag=$(api "$api_list" | tag_name) || true
    [ -n "$tag" ] || die "found no release of $repo (is the network up? $api_latest)"
    note "no full release yet: taking the newest pre-release"
  fi
  ok "the latest release is $tag"
else
  ok "asked for $tag"
fi
case "$tag" in
  */* | .* | '') die "\"$tag\" is not a release tag" ;;
esac
installed=$(link_target current)
ok "installed now: ${installed:-nothing}"
if [ "$tag" = "$installed" ] && [ "$force" = 0 ]; then
  printf '\n%s✓ %s is already installed.%s Nothing was changed; reinstall with: update.sh %s --force\n' \
    "$green" "$tag" "$reset" "$tag"
  exit 0
fi

# --- 3. download, check, unpack (nothing is stopped before this succeeds) ---------------------

asset=wayfarer-$tag-linux-$arch.tar.gz
say "Downloading $asset"
work=$releases/.download
rm -rf "$work" && mkdir -p "$work"
trap 'rm -rf "$work"' EXIT
if [ -t 1 ]; then progress=(--progress-bar); else progress=(-sS); fi
curl -fL --retry 2 "${progress[@]}" -o "$work/SHA256SUMS" "$download/$tag/SHA256SUMS" ||
  die "could not download SHA256SUMS for $tag (is there a release $tag? https://github.com/$repo/releases)"
curl -fL --retry 2 "${progress[@]}" -o "$work/$asset" "$download/$tag/$asset" ||
  die "could not download $asset from release $tag"
ok "$asset ($(human "$work/$asset"))"
sum=$(grep -E "^[0-9a-f]{64}[[:space:]]+\*?$asset\$" "$work/SHA256SUMS" | head -n 1) ||
  die "SHA256SUMS of $tag has no line for $asset; nothing was changed"
(cd "$work" && printf '%s\n' "$sum" | sha256sum -c --status) ||
  die "the checksum of $asset does not match SHA256SUMS; nothing was changed"
ok "checksum matches SHA256SUMS"
mkdir -p "$work/unpacked"
tar -xzf "$work/$asset" -C "$work/unpacked" --strip-components=1 || die "could not unpack $asset; nothing was changed"
for f in wayfarer ui/dist world.sqlite wayfarer.service update.sh VERSION; do
  [ -e "$work/unpacked/$f" ] || die "$asset has no $f; nothing was changed"
done
touch "$work/unpacked"
ok "unpacked ($(du -sh "$work/unpacked" | cut -f1)): $("$work/unpacked/wayfarer" --version 2>/dev/null || echo "wayfarer $(cat "$work/unpacked/VERSION")")"

# --- 4. the unit, the first time --------------------------------------------------------------

say "Checking the systemd unit"
if [ -f "$unit_file" ]; then
  ok "$unit_file is there (left as it is)"
  if ! unit_body | grep -q 'releases/current/wayfarer'; then
    note "its ExecStart does not run releases/current, so it will keep running what it runs now."
    note "To run releases: move it aside and run update.sh again, which writes a new one from"
    note "the template; or edit ExecStart as packaging/wayfarer.service shows."
  fi
  first_install=0
else
  mkdir -p "$unit_dir"
  default_data=$HOME/.local/share/wayfarer
  # The one placeholder is --bind @BIND@; a data directory elsewhere replaces the default's paths.
  sed -e "s|--bind @BIND@|--bind $bind|" "$work/unpacked/wayfarer.service" |
    if [ "$data" = "$default_data" ]; then cat; else sed -e "/^[^#]/s|%h/.local/share/wayfarer|$data|g"; fi \
      >"$unit_file.new"
  mv -f "$unit_file.new" "$unit_file"
  systemctl --user daemon-reload || die "systemctl --user daemon-reload failed"
  systemctl --user enable wayfarer >/dev/null 2>&1 || die "systemctl --user enable wayfarer failed"
  ok "wrote $unit_file (--bind $bind) and enabled it"
  first_install=1
fi

# --- 5. stop, 6. switch, 7. start ------------------------------------------------------------

stop_unit

say "Switching to $tag"
if [ -e "$releases/$tag" ]; then
  # Reinstalling (--force, or a release kept from before): the old copy goes once replaced.
  rm -rf "$releases/.replaced"
  mv -T "$releases/$tag" "$releases/.replaced"
fi
mv -T "$work/unpacked" "$releases/$tag"
rm -rf "$releases/.replaced"
set_link current "$tag"
if [ -n "$installed" ] && [ "$installed" != "$tag" ]; then
  set_link previous "$installed"
fi
ok "current → $tag, previous → $(link_target previous | grep . || echo none)"

start_unit

# --- 8. tidy ---------------------------------------------------------------------------------

say "Tidying old releases"
keep_cur=$(link_target current) keep_prev=$(link_target previous)
spare=''
removed=()
while IFS= read -r dir; do
  case "$dir" in "$keep_cur" | "$keep_prev" | .* | current | previous) continue ;; esac
  [ -d "$releases/$dir" ] && [ ! -L "$releases/$dir" ] || continue
  if [ -z "$spare" ]; then
    spare=$dir
  else
    rm -rf "${releases:?}/$dir"
    removed+=("$dir")
  fi
done < <(ls -1t "$releases")
if [ "${#removed[@]}" -gt 0 ]; then
  ok "removed ${removed[*]}"
fi
ok "kept $keep_cur${keep_prev:+, $keep_prev}${spare:+ and $spare}"

user=${USER:-$(id -un)}
if [ "$first_install" = 1 ] && [ ! -e "/var/lib/systemd/linger/$user" ]; then
  printf '\n%s! One more step, once:%s so wayfarer starts at boot and keeps running after you log out, run\n' \
    "$yellow" "$reset"
  printf '      sudo loginctl enable-linger %s\n' "$user"
fi
finish "$tag"
