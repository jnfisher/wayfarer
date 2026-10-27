#!/usr/bin/env bash
# Refresh the dev rig: stop the gateway and brain running in tmux, pull main, and start them
# again in tmux, bound to this machine's Tailscale address.
# Usage: scripts/dev/refresh.sh
#
# The tmux session is `wayfarer`, with a `gateway` window (scripts/dev/gateway.sh) and a
# `brain` window (scripts/dev/brain.sh); attach with `tmux attach -t wayfarer`. The gateway
# stops first, while the brain is still attached: it asks the brain to get each character
# ready to leave and logs them off cleanly (dropping in the realm is punished,
# docs/kb/connection.md). That can take a few minutes. Nothing is stopped unless the
# checkout can pull main (on main, no uncommitted changes, no commits origin lacks). On an
# ARM machine it asks first: a release box updates with scripts/update.sh instead.
#
# Environment: WAYFARER_BIND_IP (default: `tailscale ip -4`), WAYFARER_PORT (default 7373),
# WAYFARER_TMUX_SESSION (default wayfarer).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmux_session=${WAYFARER_TMUX_SESSION:-wayfarer}
port=${WAYFARER_PORT:-7373}
# How long to wait for the gateway to log everyone off (its own limit is 360 s), and for a
# release build (slow on a Raspberry Pi).
logoff_wait=420
start_wait=3600

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
die() { printf '\n%s✗ %s%s\n' "$red" "$*" "$reset" >&2; exit 1; }

# Overwrites the current line with `label (Ns): the pane's last line`, cut to the terminal.
status() {
  [ -t 1 ] || return 0
  local line
  line=$(printf '    %s (%ds): %s' "$1" "$2" "$3" | tr -d '\r' | cut -c1-"$(($(tput cols 2>/dev/null || echo 100) - 1))")
  printf '\r\e[K%s%s%s' "$dim" "$line" "$reset"
}
status_done() { [ -t 1 ] && printf '\r\e[K'; return 0; }

has_window() { tmux list-windows -t "$tmux_session" -F '#W' 2>/dev/null | grep -qx "$1"; }
pane_dead() { [ "$(tmux display-message -p -t "$tmux_session:$1" '#{pane_dead}' 2>/dev/null)" = 1 ]; }
pane_pid() { tmux display-message -p -t "$tmux_session:$1" '#{pane_pid}'; }
last_line() { tmux capture-pane -p -J -t "$tmux_session:$1" 2>/dev/null | grep -v '^[[:space:]]*$' | tail -n 1; }
pane_tail() { tmux capture-pane -p -J -t "$tmux_session:$1" -S -200 2>/dev/null | grep -v '^[[:space:]]*$' | tail -n "$2"; }
gone() { ! has_window "$1" || pane_dead "$1"; }

# Starts `cmd` in window `name`, kept open when it exits so a failed build can be read.
start_window() {
  local name=$1 cmd=$2
  if tmux has-session -t "$tmux_session" 2>/dev/null; then
    tmux new-window -d -t "$tmux_session" -n "$name" -c "$root"
  else
    tmux new-session -d -s "$tmux_session" -n "$name" -c "$root"
  fi
  tmux set-option -w -t "$tmux_session:$name" remain-on-exit on
  tmux respawn-pane -k -t "$tmux_session:$name" -c "$root" "$cmd"
}

printf '%swayfarer refresh%s %s(%s)%s\n' "$bold" "$reset" "$dim" "$root" "$reset"

# An ARM box is most likely a release box (a Raspberry Pi), which updates from a GitHub
# release with scripts/update.sh instead of building here (docs/server.md).
case "$(uname -m)" in
  aarch64 | arm64 | armv7l)
    printf '\n%s! This is an ARM machine (%s).%s A release box updates with scripts/update.sh,\n' \
      "$yellow" "$(uname -m)" "$reset"
    printf '  which needs no build. This script builds from source here, which is slow.\n'
    answer=''
    if [ -r /dev/tty ]; then
      { read -r -p '  Continue? [y/N] ' answer </dev/tty; } 2>/dev/null || true
    fi
    case "$answer" in
      y | Y | yes | YES) ;;
      *) die "stopped; nothing was changed" ;;
    esac
    ;;
esac

say "Checking the tools and this machine's address"
for tool in tmux git cargo; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool is not installed"
done
ok "tmux, git and cargo found"
if [ -n "${WAYFARER_BIND_IP:-}" ]; then
  ip=$WAYFARER_BIND_IP
  ok "address $ip (from WAYFARER_BIND_IP)"
else
  command -v tailscale >/dev/null 2>&1 || die "tailscale is not installed; set WAYFARER_BIND_IP instead"
  ip=$(tailscale ip -4 2>/dev/null | head -n 1) || true
  [ -n "$ip" ] || die "tailscale has no IPv4 address (is it up? try: tailscale status)"
  ok "Tailscale address $ip"
fi
gateway_cmd="scripts/dev/gateway.sh --bind $ip:$port"
brain_cmd="scripts/dev/brain.sh --gateway ws://$ip:$port/ws"

# Everything that could stop the pull is checked before anything is stopped.
say "Checking the checkout can pull main"
cd "$root"
branch=$(git rev-parse --abbrev-ref HEAD)
[ "$branch" = main ] || die "the checkout is on $branch, not main"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "the checkout has uncommitted changes: $(git status --short --untracked-files=no | head -n 5 | tr '\n' ' ')"
git fetch -q origin main || die "git fetch failed"
git merge-base --is-ancestor HEAD origin/main || die "main has commits that are not on origin/main; pull by hand"
behind=$(git rev-list --count HEAD..origin/main)
if [ "$behind" = 0 ]; then
  ok "already up to date at $(git log -1 --format='%h %s')"
else
  ok "$behind new commit(s) on origin/main"
  git log --format='      %h %s' HEAD..origin/main | head -n 10
fi

say "Stopping the gateway (logging every character off cleanly)"
if has_window gateway && ! pane_dead gateway; then
  kill -INT "$(pane_pid gateway)" 2>/dev/null || true
  t0=$SECONDS
  until gone gateway; do
    if [ $((SECONDS - t0)) -ge "$logoff_wait" ]; then
      status_done
      note "still running after ${logoff_wait}s: stopping it now (characters may drop)"
      kill -INT "$(pane_pid gateway)" 2>/dev/null || true
      sleep 3
      break
    fi
    status "logging off" $((SECONDS - t0)) "$(last_line gateway)"
    sleep 1
  done
  status_done
  ok "the gateway has stopped ($((SECONDS - t0))s)"
else
  note "no gateway running in tmux session $tmux_session"
fi
has_window gateway && tmux kill-window -t "$tmux_session:gateway"

say "Stopping the brain"
if has_window brain && ! pane_dead brain; then
  tmux send-keys -t "$tmux_session:brain" C-c
  t0=$SECONDS
  until gone brain || [ $((SECONDS - t0)) -ge 15 ]; do
    status "stopping" $((SECONDS - t0)) "$(last_line brain)"
    sleep 1
  done
  status_done
  if gone brain; then ok "the brain has stopped"; else note "the brain did not stop on Ctrl-C: closing its window"; fi
else
  note "no brain running in tmux session $tmux_session"
fi
has_window brain && tmux kill-window -t "$tmux_session:brain"

say "Pulling main"
git pull -q --ff-only origin main || die "git pull failed; start the rig by hand"
ok "at $(git log -1 --format='%h %s')"

say "Starting the gateway on $ip:$port (builds first)"
start_window gateway "$gateway_cmd"
t0=$SECONDS
until (exec 3<>"/dev/tcp/$ip/$port") 2>/dev/null; do
  if pane_dead gateway; then
    status_done
    pane_tail gateway 20 | sed 's/^/      /'
    die "the gateway exited; its window is kept: tmux attach -t $tmux_session"
  fi
  [ $((SECONDS - t0)) -lt "$start_wait" ] || die "the gateway is not listening after ${start_wait}s"
  status "building and starting" $((SECONDS - t0)) "$(last_line gateway)"
  sleep 1
done
status_done
ok "the gateway is listening ($((SECONDS - t0))s)"

say "Starting the brain"
start_window brain "$brain_cmd"
t0=$SECONDS
# Built once its binary runs (cargo-watch builds it first); up once it has logged something
# of its own, which it may not with no character in the realm, so that wait is short.
brain_dead() {
  pane_dead brain || return 0
  status_done
  pane_tail brain 20 | sed 's/^/      /'
  die "the brain exited; its window is kept: tmux attach -t $tmux_session"
}
# Only a brain in this pane's own process session counts.
until pgrep -s "$(pane_pid brain)" -f "wayfarer brain --config" >/dev/null; do
  brain_dead
  [ $((SECONDS - t0)) -lt "$start_wait" ] || die "the brain has not started after ${start_wait}s"
  status "building" $((SECONDS - t0)) "$(last_line brain)"
  sleep 1
done
status_done
ok "the brain is running ($((SECONDS - t0))s)"
t1=$SECONDS
until pane_tail brain 5 | grep -q 'wayfarer_brain'; do
  brain_dead
  if [ $((SECONDS - t1)) -ge 60 ]; then
    status_done
    note "no word from the brain yet; look at its window if the UI says no brain is attached"
    break
  fi
  status "connecting" $((SECONDS - t1)) "$(last_line brain)"
  sleep 1
done
status_done
if pane_tail brain 5 | grep -q 'wayfarer_brain'; then ok "the brain is talking to the gateway"; fi

printf '\n%s✓ Refreshed in %ds.%s The UI is at http://%s:%s/ ; watch with: tmux attach -t %s\n' \
  "$green" $((SECONDS - started)) "$reset" "$ip" "$port" "$tmux_session"
