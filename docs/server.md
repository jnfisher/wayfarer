# Running wayfarer on a server

Keep the characters logged in around the clock on a Linux box and watch them from a browser
on another machine over Tailscale. Written for Debian or Ubuntu; no installer yet (that is
M6, [plan/08-packaging.md](plan/08-packaging.md)). Placeholders: `USER` is the Linux user,
`100.x.y.z` the box's Tailscale address.

> **Status:** the commands are checked against the binary's flags and messages, but this page
> has not yet been tried end to end on a Linux machine (see the pull requests for C-026 and
> C-061).

Anyone who can reach the gateway's port with the token can play your characters. Read
[Token and bind](#3-token-and-bind) before opening it to anything.

There are two ways to run it:

- **From a release** (a box that only runs wayfarer, such as a Raspberry Pi 5): no Rust or
  Node on the box. [Running a release](#running-a-release) is all of it; read
  [3](#3-token-and-bind) and [5](#5-silvermere-beside-it) for the background.
- **Built from source** (a box that builds its own): sections [1](#1-build) to
  [4](#4-the-unit).

## Running a release

A tag pushed to GitHub builds Linux tarballs for x86_64 and aarch64 and attaches them to a
GitHub Release ([releases](https://github.com/jnfisher/wayfarer/releases)).
`scripts/update.sh` downloads the one for the box, checks it against `SHA256SUMS`, unpacks it
under the data directory, and restarts the systemd user unit. It needs only `bash`, `curl`,
`tar`, `sha256sum` and `systemctl` (all on a stock Debian, Ubuntu or Raspberry Pi OS), and
`tailscale` the first time, for the address.

```
~/.local/share/wayfarer/              the data directory: only what is yours
  wayfarer.toml  credentials.toml  gateway-token  (the store and the logs, as the gateway makes them)
  releases/
    v0.2.0/        wayfarer  ui/dist/  world.sqlite  wayfarer.service  update.sh  VERSION
    v0.1.0/
    current -> v0.2.0                 what the unit runs
    previous -> v0.1.0                what --rollback goes back to
```

The world pack ships in each release and is not copied into the data directory. A
`world.sqlite` left in the data directory from a source build is no longer read once the unit
runs `--world …/releases/current/world.sqlite`; delete it when you like.

### First install

1. **Tailscale** up on the box (`tailscale ip -4` prints its address).
2. **The data directory**, with the three files `update.sh` checks for:

   ```sh
   D=~/.local/share/wayfarer && mkdir -p $D
   curl -fsSL https://raw.githubusercontent.com/jnfisher/wayfarer/main/examples/wayfarer.toml -o $D/wayfarer.toml
   $EDITOR $D/wayfarer.toml          # [[server]] and your [[character]] entries, as in step 2 below
   curl -fsSL https://raw.githubusercontent.com/jnfisher/wayfarer/main/examples/credentials.toml -o $D/credentials.toml
   chmod 600 $D/credentials.toml && $EDITOR $D/credentials.toml   # an account per character id
   (umask 077 && head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n' >$D/gateway-token) && cat $D/gateway-token
   ```

   The last line makes the token the browser asks for (48 hex digits, as `wayfarer gateway
   --new-token` makes them); keep it.
3. **`update.sh`**: download it once from the release page (each release carries it beside
   the tarballs), or copy it from a checkout, and run it:

   ```sh
   curl -fLO https://github.com/jnfisher/wayfarer/releases/latest/download/update.sh
   # a pre-release isn't "latest": …/releases/download/v0.1.0-rc1/update.sh
   bash update.sh                    # or: bash update.sh v0.1.0-rc1
   ```

   It installs the release, writes `~/.config/systemd/user/wayfarer.service` from the
   release's template with the Tailscale address and port 7373 filled in, enables and starts
   it, and waits until the port answers.
4. **Linger**, once, so the unit starts at boot and keeps running after you log out
   (`update.sh` prints this; it never runs sudo itself):

   ```sh
   sudo loginctl enable-linger $USER
   ```

Then open `http://100.x.y.z:7373` from another machine on the tailnet and enter the token.

### Updates and rollback

```sh
~/.local/share/wayfarer/releases/current/update.sh              # the latest release
~/.local/share/wayfarer/releases/current/update.sh v0.2.0       # that release
~/.local/share/wayfarer/releases/current/update.sh v0.2.0 --force   # reinstall it
~/.local/share/wayfarer/releases/current/update.sh --rollback   # back to the one before
```

Each step prints as it goes: the checks, the release found and the one installed now, the
download and its checksum, the unit, the stop, the switch, the start, and the tidy. Nothing is
stopped until the new release is downloaded, checked and unpacked. The stop is `systemctl
--user stop wayfarer`, which logs every character off cleanly (it shows the journal's last
line and a timer meanwhile; up to six minutes). It keeps the current, previous and one more
release and deletes older ones. It counts the new release as started only when the unit's own
process holds the port and is still running a few seconds later with no restart; when it
does not start, it prints the journal's last lines and says to run `update.sh --rollback`.

It refuses, before downloading or stopping anything, when another wayfarer runs on the machine
outside the unit (a dev rig in tmux, or a `wayfarer serve` started by hand: two would log the
same characters in), or when something else holds the unit's port (the unit could not bind it,
and systemd would restart it every few seconds). It names the processes; stop them first. A
machine that runs the dev rig refreshes with `scripts/dev/refresh.sh` instead
([dev-rig.md](dev-rig.md)). The same tag again stops with "already
installed" and changes nothing, unless `--force`.

`update.sh` leaves an existing unit alone; if its `ExecStart` does not run
`releases/current/wayfarer` (a unit from a source build) it says so: move the unit aside and
run `update.sh --force`, which writes a new one from the template. Environment:
`WAYFARER_DATA_DIR` (default `~/.local/share/wayfarer`), `WAYFARER_REPO` (default
`jnfisher/wayfarer`), and for a new unit `WAYFARER_BIND_IP` (default `tailscale ip -4`) and
`WAYFARER_PORT` (default 7373).

## 1. Build

For a box that builds from source; a release box skips to [Running a release](#running-a-release).

```sh
sudo apt install build-essential pkg-config git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh     # rustup; reopen the shell
# Node LTS (nodejs.org or nvm), then:
git clone https://github.com/jnfisher/wayfarer.git ~/wayfarer && cd ~/wayfarer
cargo build --release                                  # target/release/wayfarer
npm --prefix ui ci && npm --prefix ui run build        # ui/dist
```

The first build takes a few minutes. To skip Node on the server, run the two `npm` commands
on the laptop and copy `ui/dist` over (`rsync -a ui/dist/ USER@server:wayfarer/ui/dist/`).

## 2. Data directory

```sh
D=~/.local/share/wayfarer && mkdir -p $D
cp examples/dev.toml $D/wayfarer.toml
./target/release/wayfarer import-world --export tests/fixtures/world --out $D/world.sqlite
```

- `wayfarer.toml`: edit `[[server]]` to point at the game: `host = "127.0.0.1"`, `port = 2323`
  for a silvermere on the same box ([step 5](#5-silvermere-beside-it)), or a BBS on the
  internet. Replace the placeholder `[[character]]` entries with yours.
- `credentials.toml`: account and password per character id, see [gateway.md](gateway.md).
  The gateway creates it mode 0600 on first start; if you write it yourself, `chmod 600` it.
- `world.sqlite`: the world pack the brain needs. Without it `serve` runs the gateway alone
  and prints the command that builds it. (A release box uses the pack in the release
  instead.)
- Logs are kept under the same directory ([gateway.md](gateway.md)).

## 3. Token and bind

```sh
./target/release/wayfarer gateway --config $D/wayfarer.toml --new-token
```

prints the token once and writes it to `$D/gateway-token`. Any bind address that is not
loopback needs it: without one the gateway refuses to start and tells you to run this
command. The web page asks for the token once per browser.

Bind to the Tailscale address (`tailscale ip -4`), not `0.0.0.0`:
`--bind 100.x.y.z:7373`. `0.0.0.0` also listens on the box's public interface. In the
Tailscale admin console, add an ACL that lets only your own devices reach port 7373 on this
machine.

There is no TLS. Inside a tailnet traffic is already encrypted end to end by WireGuard, so
that is fine. If the port were ever reachable from the internet it would not be: don't do
that, and don't put it behind a public reverse proxy.

## 4. The unit

`packaging/wayfarer.service` is written for a release box (`update.sh` installs it). A box
built from source points its `ExecStart` at the checkout instead, as the comment at the top of
the file shows:

```sh
mkdir -p ~/.config/systemd/user
cp packaging/wayfarer.service ~/.config/systemd/user/
$EDITOR ~/.config/systemd/user/wayfarer.service    # ExecStart from the comment, with 100.x.y.z
loginctl enable-linger $USER                       # start at boot, without a login
systemctl --user daemon-reload
systemctl --user enable --now wayfarer
```

| To | Command |
|---|---|
| status | `systemctl --user status wayfarer` |
| logs | `journalctl --user -u wayfarer -f` |
| stop (logs everyone off) | `systemctl --user stop wayfarer` |
| restart | `systemctl --user restart wayfarer` |

A stop sends SIGTERM; the gateway logs each character off cleanly (waiting up to 360 s: a
character under attack may need many tries) and exits, and `TimeoutStopSec=390` keeps systemd
waiting that long. The game punishes dropping a
character in the realm, so don't `kill -9` it.

Then open `http://100.x.y.z:7373` from another machine on the tailnet, enter the token, and
the characters with `autostart = true` appear logged in. A missing `ui/dist` gives a 503 page,
but the WebSocket (`/ws`) still works.

## 5. silvermere beside it

Run silvermere on the same box with its own unit, bound to `127.0.0.1:2323` (its
`silvermere service install`, in its `docs/install.md`). Start it before wayfarer; if it
restarts, the characters reconnect and log on again through their logon scripts. A server has
no sleep to worry about; reconnects are what to test. A remote BBS needs nothing here beyond
the `[[server]]` entry.

## 6. Upgrading and backups

A release box upgrades with `update.sh` ([Updates and rollback](#updates-and-rollback)). A box
built from source:

```sh
cd ~/wayfarer && git pull
cargo build --release && npm --prefix ui ci && npm --prefix ui run build
systemctl --user restart wayfarer          # everyone logs off and back on
```

Back up the data directory (`tar czf wayfarer-backup.tgz -C ~/.local/share wayfarer`); it
holds your credentials and token, so keep the archive private. (`--exclude wayfarer/releases`
leaves out the releases, which can be downloaded again.)

## 7. Development stays on the laptop

Cards and the live check need the local rig ([dev-rig.md](dev-rig.md)); don't develop on the
server.
