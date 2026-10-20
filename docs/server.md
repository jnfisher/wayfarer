# Running wayfarer on a server

Keep the characters logged in around the clock on a Linux box and watch them from a browser
on another machine over Tailscale. Written for Debian or Ubuntu; no installer yet (that is
M6, [plan/08-packaging.md](plan/08-packaging.md)). Placeholders: `USER` is the Linux user,
`100.x.y.z` the box's Tailscale address.

> **Status:** the commands are checked against the binary's flags and messages, but this page
> has not yet been tried end to end on a Linux machine (see the pull request for C-026).

Anyone who can reach the gateway's port with the token can play your characters. Read
[Token and bind](#3-token-and-bind) before opening it to anything.

## 1. Build

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
  and prints the command that builds it.
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

```sh
mkdir -p ~/.config/systemd/user
cp packaging/wayfarer.service ~/.config/systemd/user/
$EDITOR ~/.config/systemd/user/wayfarer.service    # replace USER and 100.x.y.z
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

```sh
cd ~/wayfarer && git pull
cargo build --release && npm --prefix ui ci && npm --prefix ui run build
systemctl --user restart wayfarer          # everyone logs off and back on
```

Back up the data directory (`tar czf wayfarer-backup.tgz -C ~/.local/share wayfarer`); it
holds your credentials and token, so keep the archive private.

## 7. Development stays on the laptop

Cards and the live check need the local rig ([dev-rig.md](dev-rig.md)); don't develop on the
server.
