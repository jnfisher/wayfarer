# The gateway: configuration and logs

The gateway holds one telnet session per character. It reads two files from your data
directory (the platform's per-user data directory, on Linux `~/.local/share/wayfarer/`), or
from the directory of the file given with `--config`.

## `wayfarer.toml`

The game servers and the characters. A full example is
[`examples/wayfarer.toml`](../examples/wayfarer.toml).

```toml
[[server]]
id = "home"
host = "bbs.example.org"
port = 23
preset = "mbbsemu"          # the logon script (mbbsemu or silvermere)

[[character]]
id = "kasimir"              # letters, digits, - and _
name = "Kasimir"
server = "home"
autostart = true            # connect when the gateway starts
```

Ids must be unique; every character's `server` must be a listed server.

## `credentials.toml`

The account name and password per character id, kept apart from the settings so those can be
shared without them ([`examples/credentials.toml`](../examples/credentials.toml)):

```toml
[character.kasimir]
user = "ACCOUNT"
password = "PASSWORD"
```

If the file is missing the gateway creates an empty one that only you can read (mode 0600 on
Linux and macOS), and it warns at start-up if the file's permissions let anyone else read it.
The gateway never writes a password itself: not in its tracing output, and in `sent.bin` a
password sent to the game is recorded as `********`.

## Logs

`logs/<character id>/` holds, append-only and kept across restarts:

- `output.log`: everything the game showed, one JSON line per piece of output with its
  sequence number and time (the format is in `crates/wayfarer-gateway/README.md`).
- `raw.bin`: every byte received, exactly as it came.
- `sent.bin`: every byte sent; a password is recorded as `********`.

Sequence numbers never restart: a restarted gateway carries on from the last one in
`output.log`, so a browser that reconnects asks for everything since the last number it saw
and misses nothing.
