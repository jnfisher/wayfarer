#!/usr/bin/env bash
# Tests for scripts/update.sh (C-061), with no network and nothing of the real machine's:
# each case runs in a temporary HOME, with fake releases built here (a stub `wayfarer` that
# listens on a loopback port and logs a log-off line on SIGTERM) served from a file:// base,
# and stub systemctl, journalctl, tailscale, uname and pgrep first on PATH. The stub systemctl
# runs the unit's ExecStart itself; it never talks to systemd. The stub pgrep lists the unit's
# process and $HOME/.stub/procs, never the machine's own (a dev rig may be running); `ss` is
# the real one, so the port checks see the stub wayfarers' real sockets.
# Usage: scripts/tests/update-test.sh    (UPDATE_TEST_VERBOSE=1 prints update.sh's output too)
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
update=$root/scripts/update.sh
command -v python3 >/dev/null || { echo "update-test: needs python3 (the stub wayfarer)" >&2; exit 1; }

tmp=$(mktemp -d "${TMPDIR:-/tmp}/wayfarer-update-test.XXXXXX")
cleanup() {
  local pidfile
  for pidfile in "$tmp"/home*/.stub/pid; do
    [ -f "$pidfile" ] && kill -TERM "$(cat "$pidfile")" 2>/dev/null || true
  done
  rm -rf "$tmp"
}
trap cleanup EXIT

free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'; }
port=$(free_port)
port2=$(free_port)
rel=$tmp/rel
bin=$tmp/bin
mkdir -p "$rel" "$bin"

# --- stubs -----------------------------------------------------------------------------------

real_uname=$(command -v uname)
cat >"$bin/uname" <<EOF
#!/usr/bin/env bash
if [ "\${1:-}" = -m ]; then echo "\${FAKE_ARCH:-x86_64}"; else exec $real_uname "\$@"; fi
EOF

cat >"$bin/tailscale" <<'EOF'
#!/usr/bin/env bash
[ "$*" = "ip -4" ] && echo 127.0.0.1
EOF

cat >"$bin/journalctl" <<'EOF'
#!/usr/bin/env bash
n=10
while [ $# -gt 0 ]; do case "$1" in -n) n=$2; shift ;; esac; shift; done
[ -f "$HOME/.stub/journal" ] && tail -n "$n" "$HOME/.stub/journal"
exit 0
EOF

cat >"$bin/pgrep" <<'EOF'
#!/usr/bin/env bash
pidfile=$HOME/.stub/pid
if [ -f "$pidfile" ] && kill -0 "$(cat "$pidfile")" 2>/dev/null; then
  echo "$(cat "$pidfile") $HOME/.local/share/wayfarer/releases/current/wayfarer serve --config x"
fi
[ -f "$HOME/.stub/procs" ] && cat "$HOME/.stub/procs"
exit 0
EOF

# The unit's state is $HOME/.stub/pid (running: alive; failed: its process died). Every call
# goes to $HOME/.stub/calls; a stop records where releases/current pointed at the time.
cat >"$bin/systemctl" <<'EOF'
#!/usr/bin/env bash
state=$HOME/.stub
mkdir -p "$state"
[ "${1:-}" = --user ] && shift
unit=$HOME/.config/systemd/user/wayfarer.service
pidfile=$state/pid
alive() { [ -f "$pidfile" ] && kill -0 "$(cat "$pidfile")" 2>/dev/null; }
current() { basename "$(readlink "$HOME/.local/share/wayfarer/releases/current" 2>/dev/null)" 2>/dev/null; }
case "$1" in
  daemon-reload | enable) echo "$1" >>"$state/calls" ;;
  is-active) alive ;;
  show)
    prop=$3
    if alive; then a=active s=running
    elif [ -f "$pidfile" ]; then a=failed s=failed
    else a=inactive s=dead; fi
    case "$prop" in
      ActiveState) echo "$a" ;;
      SubState) echo "$s" ;;
      MainPID) if alive; then cat "$pidfile"; else echo 0; fi ;;
      NRestarts) echo 0 ;;
    esac
    ;;
  start)
    echo "start current=$(current)" >>"$state/calls"
    alive && exit 0
    cmd=$(grep -v '^[[:space:]]*#' "$unit" | sed -n '/^ExecStart=/,/[^\\]$/p' | sed 's/^ExecStart=//; s/\\$//' | tr '\n' ' ')
    cmd=${cmd//%h/$HOME}
    # shellcheck disable=SC2086
    $cmd </dev/null >>"$state/journal" 2>&1 &
    echo $! >"$pidfile"
    ;;
  stop)
    echo "stop current=$(current)" >>"$state/calls"
    if alive; then
      pid=$(cat "$pidfile")
      kill -TERM "$pid"
      while kill -0 "$pid" 2>/dev/null; do sleep 0.05; done
    fi
    rm -f "$pidfile"
    ;;
  *) echo "stub systemctl: unexpected $*" >&2; exit 1 ;;
esac
EOF
chmod +x "$bin"/*

# --- fake releases ---------------------------------------------------------------------------

# make_release TAG [broken]: a release with the asset names the workflow uses, and its sums.
make_release() {
  local tag=$1 kind=${2:-good} stage=$tmp/stage/$1 out=$rel/$1
  local top=$stage/wayfarer-$tag
  mkdir -p "$top/ui/dist" "$out"
  echo '<!doctype html>' >"$top/ui/dist/index.html"
  echo "world $tag" >"$top/world.sqlite"
  cp "$root/packaging/wayfarer.service" "$top/wayfarer.service"
  cp "$update" "$top/update.sh"
  echo "$tag" >"$top/VERSION"
  if [ "$kind" = flaky ]; then
    # Listens, then dies a moment later (as a gateway whose brain cannot start might).
    cat >"$top/wayfarer" <<EOF
#!/usr/bin/env bash
[ "\${1:-}" = --version ] && { echo "wayfarer $tag"; exit 0; }
bind=''
while [ \$# -gt 0 ]; do [ "\$1" = --bind ] && bind=\$2; shift; done
exec python3 -c '
import socket, sys, time
tag, ip, port = sys.argv[1], sys.argv[2], int(sys.argv[3])
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind((ip, port))
s.listen()
print(tag + ": listening on " + ip + ":" + str(port), flush=True)
time.sleep(0.4)
print(tag + ": error: the brain stopped", flush=True)
sys.exit(1)
' "$tag" "\${bind%:*}" "\${bind##*:}"
EOF
  elif [ "$kind" = broken ]; then
    cat >"$top/wayfarer" <<EOF
#!/usr/bin/env bash
[ "\${1:-}" = --version ] && { echo "wayfarer $tag"; exit 0; }
echo "$tag: error: cannot open the world pack"
exit 1
EOF
  else
    cat >"$top/wayfarer" <<EOF
#!/usr/bin/env bash
[ "\${1:-}" = --version ] && { echo "wayfarer $tag"; exit 0; }
bind=''
while [ \$# -gt 0 ]; do [ "\$1" = --bind ] && bind=\$2; shift; done
exec python3 -c '
import signal, socket, sys, time
tag, ip, port = sys.argv[1], sys.argv[2], int(sys.argv[3])
def stop(*_):
    print(tag + ": the brain is ready: logging off session=tester1", flush=True)
    time.sleep(0.2)
    print(tag + ": logged off; stopping", flush=True)
    sys.exit(0)
signal.signal(signal.SIGTERM, stop)
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind((ip, port))
s.listen()
print(tag + ": listening on " + ip + ":" + str(port), flush=True)
while True:
    c, _ = s.accept()
    c.close()
' "$tag" "\${bind%:*}" "\${bind##*:}"
EOF
  fi
  chmod +x "$top/wayfarer"
  tar -czf "$out/wayfarer-$tag-linux-x86_64.tar.gz" -C "$stage" "wayfarer-$tag"
  (cd "$out" && sha256sum ./*.tar.gz | sed 's| \./| |' >SHA256SUMS)
}
set_latest() { printf '{\n  "url": "file",\n  "tag_name": "%s",\n  "name": "%s"\n}\n' "$1" "$1" >"$rel/latest"; }

# --- running it ------------------------------------------------------------------------------

fails=0 cases=0
new_home() {
  home=$tmp/home$1
  data=$home/.local/share/wayfarer
  mkdir -p "$data"
  touch "$data/wayfarer.toml" "$data/credentials.toml" "$data/gateway-token"
}
# run_update ARGS...: runs update.sh in $home; sets $out and $rc.
run_update() {
  rc=0
  out=$(cd "$tmp" && env -u WAYFARER_DATA_DIR -u XDG_DATA_HOME -u XDG_CONFIG_HOME -u WAYFARER_REPO \
    -u WAYFARER_BIND_IP HOME="$home" USER=tester PATH="$bin:$PATH" WAYFARER_PORT="$port" \
    WAYFARER_RELEASE_BASE="file://$rel" WAYFARER_START_STEADY=1 ${extra_env[@]+"${extra_env[@]}"} \
    bash "$update" "$@" 2>&1) || rc=$?
  if [ -n "${UPDATE_TEST_VERBOSE:-}" ]; then printf '%s\n' "$out" | sed 's/^/       | /'; fi
}
extra_env=()

check() {
  cases=$((cases + 1))
  if eval "$2"; then
    printf '  ok   %s\n' "$1"
  else
    printf '  FAIL %s\n       (%s)\n' "$1" "$2"
    fails=$((fails + 1))
    failed_output=1
  fi
}
has() { grep -qF -- "$1" <<<"$out"; }
link() { basename "$(readlink "$data/releases/$1" 2>/dev/null)" 2>/dev/null || true; }
calls() { cat "$home/.stub/calls" 2>/dev/null || true; }
answers() { (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; }
section() {
  if [ "${failed_output:-0}" = 1 ]; then printf '%s\n' "$out" | sed 's/^/       | /'; fi
  failed_output=0
  printf '%s\n' "$1"
}

# Never run against the real systemd: the stubs must come first.
[ "$(PATH="$bin:$PATH" command -v systemctl)" = "$bin/systemctl" ] || { echo "update-test: stub systemctl is not first on PATH" >&2; exit 1; }

for t in v0.1.0 v0.2.0 v0.3.0 v0.4.0; do make_release "$t"; done
make_release v0.5.0 broken
make_release v0.6.0 flaky
make_release v0.9.0
# A tampered release: the tarball no longer matches its sums.
echo junk >>"$rel/v0.9.0/wayfarer-v0.9.0-linux-x86_64.tar.gz"

section "first install (the address from tailscale)"
new_home 1
set_latest v0.1.0
run_update
check "exits 0" '[ "$rc" = 0 ]'
check "the unit is written with the address" 'grep -q -- "--bind 127.0.0.1:$port " "$home/.config/systemd/user/wayfarer.service"'
check "no placeholder left" '! grep -v "^#" "$home/.config/systemd/user/wayfarer.service" | grep -q @BIND@'
check "current is v0.1.0" '[ "$(link current)" = v0.1.0 ]'
check "no previous" '[ -z "$(link previous)" ]'
check "the unit is reloaded, enabled and started" 'calls | tr "\n" " " | grep -q "daemon-reload enable start current=v0.1.0"'
check "the port answers" 'answers'
check "the pre-flight check passes" 'has "no other wayfarer running, and port $port is free for the unit"'
check "the unit's own process holds the port" 'has "listening on 127.0.0.1:$port" && has "still running after 1s"'
check "every step is printed" 'has "[1] Checking" && has "[2] Finding" && has "[3] Downloading" && has "[4] Checking the systemd unit" && has "[5] Stopping" && has "[6] Switching" && has "[7] Starting" && has "[8] Tidying"'
check "the tag is found" 'has "the latest release is v0.1.0"'
check "linger is suggested" 'has "sudo loginctl enable-linger tester"'
check "the final line has the URL" 'has "✓ wayfarer v0.1.0 is running. The UI is at http://127.0.0.1:$port/"'

section "an update to a second tag"
set_latest v0.2.0
run_update
check "exits 0" '[ "$rc" = 0 ]'
check "stopped before the switch" 'calls | grep -q "^stop current=v0.1.0$"'
check "the old one logged off" 'grep -q "v0.1.0: the brain is ready: logging off" "$home/.stub/journal"'
check "current is v0.2.0, previous v0.1.0" '[ "$(link current)" = v0.2.0 ] && [ "$(link previous)" = v0.1.0 ]'
check "the old release is kept" '[ -x "$data/releases/v0.1.0/wayfarer" ]'
check "v0.2.0 is the one running" 'tail -n 1 "$home/.stub/journal" | grep -q "v0.2.0: listening"'
check "the unit is not rewritten" '[ "$(calls | grep -c daemon-reload)" = 1 ]'
check "installed now is shown" 'has "installed now: v0.1.0"'
check "no linger line on an update" '! has "enable-linger"'

section "the same tag again"
before=$(calls | wc -l)
run_update
check "exits 0" '[ "$rc" = 0 ]'
check "says already installed" 'has "v0.2.0 is already installed"'
check "nothing stopped or started" '[ "$(calls | wc -l)" = "$before" ]'
run_update v0.2.0 --force
check "--force: exits 0" '[ "$rc" = 0 ]'
check "--force: stopped and started again" 'calls | tail -n 2 | tr "\n" " " | grep -q "stop current=v0.2.0 start current=v0.2.0"'
check "--force: previous is still v0.1.0" '[ "$(link current)" = v0.2.0 ] && [ "$(link previous)" = v0.1.0 ]'
check "--force: the port answers" 'answers'

section "--rollback"
run_update --rollback
check "exits 0" '[ "$rc" = 0 ]'
check "the links swap" '[ "$(link current)" = v0.1.0 ] && [ "$(link previous)" = v0.2.0 ]'
check "the unit is restarted" 'calls | tail -n 2 | tr "\n" " " | grep -q "stop current=v0.2.0 start current=v0.1.0"'
check "v0.1.0 is running" 'tail -n 1 "$home/.stub/journal" | grep -q "v0.1.0: listening"'
check "the final line" 'has "✓ wayfarer v0.1.0 is running."'

section "tidying: current, previous and one more"
run_update v0.3.0
run_update v0.4.0
check "exits 0" '[ "$rc" = 0 ]'
check "current v0.4.0, previous v0.3.0" '[ "$(link current)" = v0.4.0 ] && [ "$(link previous)" = v0.3.0 ]'
check "the newest other one is kept" '[ -d "$data/releases/v0.2.0" ]'
check "the oldest is removed" '[ ! -e "$data/releases/v0.1.0" ] && has "removed v0.1.0"'
check "nothing left in the way" '[ "$(ls -A "$data/releases" | tr "\n" " ")" = "current previous v0.2.0 v0.3.0 v0.4.0 " ]'

section "a release that does not start"
run_update v0.5.0
check "exits non-zero" '[ "$rc" != 0 ]'
check "shows the journal" 'has "v0.5.0: error: cannot open the world pack"'
check "says how to go back" 'has "update.sh --rollback"'
run_update --rollback
check "--rollback brings v0.4.0 back" '[ "$rc" = 0 ] && [ "$(link current)" = v0.4.0 ] && answers'

section "a release that listens, then dies"
run_update v0.6.0
check "exits non-zero" '[ "$rc" != 0 ]'
check "says it did not stay up" 'has "listening on 127.0.0.1:$port" && ! has "still running after" && has "wayfarer did not start"'
check "shows the journal" 'has "v0.6.0: error: the brain stopped"'
run_update --rollback
check "--rollback brings v0.4.0 back" '[ "$rc" = 0 ] && [ "$(link current)" = v0.4.0 ] && answers'

section "a bad checksum"
before=$(calls | wc -l)
run_update v0.9.0
check "exits non-zero" '[ "$rc" != 0 ]'
check "says the checksum does not match" 'has "checksum of wayfarer-v0.9.0-linux-x86_64.tar.gz does not match"'
check "nothing stopped" '[ "$(calls | wc -l)" = "$before" ] && answers'
check "current is unchanged" '[ "$(link current)" = v0.4.0 ] && [ ! -e "$data/releases/v0.9.0" ]'

section "a tag with no release"
run_update v7.7.7
check "a clear refusal" '[ "$rc" != 0 ] && has "is there a release v7.7.7?"'

section "a missing wayfarer.toml"
new_home 2
rm "$data/wayfarer.toml"
run_update
check "exits non-zero" '[ "$rc" != 0 ]'
check "names the docs/server.md step" 'has "wayfarer.toml is missing" && has "\"Running a release\", step 2" && has "docs/server.md"'
check "nothing installed" '[ ! -e "$data/releases" ] && [ ! -e "$home/.config/systemd/user/wayfarer.service" ]'

section "--rollback with no previous release"
new_home 3
extra_env=(WAYFARER_BIND_IP=127.0.0.1 WAYFARER_PORT=$port2)
run_update --rollback
extra_env=()
check "exits non-zero" '[ "$rc" != 0 ]'
check "says so" 'has "no previous release"'
check "nothing changed" '[ -z "$(ls -A "$data/releases")" ] && [ -z "$(calls)" ]'

section "an unknown architecture"
new_home 4
extra_env=(FAKE_ARCH=riscv64)
run_update
extra_env=()
check "exits non-zero" '[ "$rc" != 0 ]'
check "a clear refusal" 'has "this machine is riscv64" && has "x86_64 and aarch64"'
check "nothing changed" '[ ! -e "$data/releases" ] && [ -z "$(calls)" ]'
section "another wayfarer running (a dev rig)"
new_home 5
mkdir -p "$home/.stub"
echo "4242 ./target/release/wayfarer gateway --config x --bind 127.0.0.1:$port2" >"$home/.stub/procs"
echo "4243 target/debug/wayfarer brain --config x" >>"$home/.stub/procs"
extra_env=(WAYFARER_PORT=$port2)
run_update
extra_env=()
check "exits non-zero" '[ "$rc" != 0 ]'
check "names them" 'has "another wayfarer is running" && has "4242 ./target/release/wayfarer gateway" && has "4243 target/debug/wayfarer brain"'
check "nothing installed or started" '[ ! -e "$data/releases" ] && [ ! -e "$home/.config/systemd/user/wayfarer.service" ] && [ -z "$(calls)" ]'

section "the port held by something else (home 1's unit still has it)"
new_home 6
run_update
check "exits non-zero" '[ "$rc" != 0 ]'
check "names the holder" 'has "port $port is already in use, by:" && has "$(cat "$tmp/home1/.stub/pid") python3"'
check "nothing installed or started" '[ ! -e "$data/releases" ] && [ ! -e "$home/.config/systemd/user/wayfarer.service" ] && [ -z "$(calls)" ]'
check "home 1's unit still answers" 'answers'
section ""

if [ "$fails" = 0 ]; then
  echo "update-test: all $cases checks passed (${SECONDS}s)"
else
  echo "update-test: $fails of $cases checks failed" >&2
  exit 1
fi
