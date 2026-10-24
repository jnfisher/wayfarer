# The gateway: configuration and logs

The gateway holds one telnet session per character. It reads two files from your data
directory (the platform's per-user data directory: on macOS `~/Library/Application Support/wayfarer/`, on Linux `~/.local/share/wayfarer/`), or
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

## Logon scripts

Each character logs itself on: when its session connects, the gateway answers the server's
prompts from its logon script, from `Username:` to the realm prompt `[HP=..]:`. A server picks
a script with `preset`:

| Preset | For |
|---|---|
| `mbbsemu` | MajorMUD under MBBSEmu: `Username:`, `Password:`, `M` at the BBS menu, `E` at `[MAJORMUD]:` |
| `silvermere` | silvermere: the same prompts; the BBS menu may be switched off |

The presets are in [`crates/wayfarer-gateway/src/presets/`](../crates/wayfarer-gateway/src/presets/),
which also explains the format. A server whose prompts differ gives its own steps, or any part
of a preset to replace, in a `[server.logon]` table:

```toml
[[server]]
id = "odd"
host = "bbs.example.org"
port = 23
preset = "mbbsemu"          # keeps the preset's realm prompt, failures and log-off

[[server.logon.step]]
expect = "Login:"           # a substring, or a regex written 're:...'
send = "{user}"

[[server.logon.step]]
expect = "re:[Pp]assword:"
send = "{password}"
timeout = 10                # seconds (30 if not given)
```

The log-off steps are `[[logoff]]` tables (`[[server.logon.logoff]]` in a server's own
script): the first is sent at the realm, each later one when its `at` text shows, and
`timeout` (20 s if not given) is how long to wait for the next step's `at`, from the step's
first send. A step can name texts that mean "not yet":

```toml
[[logoff]]
send = "x"
retry_on = ["meditation has been interrupted"]  # substrings, or 're:...'
retries = 20                # sends after the first (20 if not given)
timeout = 240               # covers every retry

[[logoff]]
at = "[MAJORMUD]:"
send = "X"
```

On a `retry_on` text the step is sent again when its place shows again (the realm prompt, for
the first step), up to `retries` times and within its `timeout`. Both presets retry the realm
step this way.

A session's state shows where it is: `connecting`, `logon` while the script runs, `menu` at
the BBS or MajorMUD menu, `realm` at the realm prompt. After that the gateway follows the text,
so a player who leaves the realm by hand is shown at the `menu`. You can type into the session
while the script runs. A session with no script, or no credentials, is `connected` and you log
on by hand (the gateway still follows the text once you do). If the script has sent everything
but the realm prompt never comes (a new account goes to character creation, say), it stops and
the session is `connected`: carry on in the tile.

**Entering the realm waits for a brain** (C-054). `wayfarer gateway` and `wayfarer serve` hold
each logon's last step (`E`, Enter the Realm) at the MajorMUD menu until a brain is attached:
at the menu nothing can attack the character, and in the realm with no brain nothing would
fight back. The session stays in `logon` with the reason "waiting for a brain to enter the
realm", and goes in the moment a brain attaches (at once for `serve`, whose brain is in the
process). `--enter-realm always` goes straight in, for playing by hand; `serve` without a world
pack has no brain and always does.

When a logon fails:

- **Wrong password** (`Invalid Credentials`): the session is `closed` with the reason "bad
  credentials" and not retried; fix `credentials.toml` and open it again.
- **Already logged in**: the old connection may still be closing, so the gateway retries.
- **A prompt doesn't come** within its step's timeout: it disconnects and retries.

## Reconnecting and logging off

A session whose connection drops (or whose logon is retried) is `lost`, and the gateway
reconnects after 5 s, then 10 s, 30 s, and every 60 s after that, until a logon succeeds; each
attempt is a line in the gateway's log. A session you close is never reconnected.

Dropping a character in the realm is punished the next time it enters, so closing a session
logs it off first: from the realm `x` (the character meditates for six ticks), `X` at the
MajorMUD menu, `X` at the BBS menu and `Y` to confirm; from a menu, only the steps that are
left.

`x` works in a fight too (it ends auto-combat), but any attack during the meditation, a miss
as much as a hit, interrupts it: `Your meditation has been interrupted - you may not exit
now!` (`docs/kb/recovery.md`). The gateway then sends `x` again at the next prompt, up to 20
times within four minutes, so it gets out as soon as a meditation passes with nothing
attacking: the attacker left or died, or the character got away. A monster that keeps
attacking interrupts every try (the bot fleeing first, C-028, is what gets the character
out). Each retry is a line in the gateway's log (`log-off interrupted: "x" again (3 of 20)`)
and goes to the brain as `sent` like any line.

Stopping the gateway (Ctrl-C or SIGTERM) logs every character off at once. With a brain
attached it first tells the brain (`logging_off`) for each character in the realm and waits up
to 30 s for its `ready_to_leave` (the brain stops that character's bot; from C-028 it flees a
fight first), or for the brain to go. The brain's connection, and the UIs', stay open until
every log-off has finished; then the gateway closes them and exits. It waits up to six minutes
in all, then disconnects whatever is left. A second Ctrl-C disconnects at once.

When a character has to be left in the realm (every retry interrupted, the step's timeout or
the six minutes run out, or a second Ctrl-C), the gateway logs a warning naming it, "dropped in
the realm ... the game punishes it at the next entry", and the session's reason says "dropped
in the realm: ...".

## Logs

`logs/<character id>/` holds, append-only and kept across restarts:

- `output.log`: everything the game showed, one JSON line per piece of output with its
  sequence number and time (the format is in `crates/wayfarer-gateway/README.md`).
- `raw.bin`: every byte received, exactly as it came.
- `sent.bin`: every byte sent; a password is recorded as `********`.
- `sent.log`: every line sent, one JSON line each: the output `seq` it followed, its time,
  who sent it (`origin`: `ui`, `brain` or `script` for logon and log-off steps) and the text,
  a password masked as in `sent.bin`.
- `brain.log`: when a brain attached and went, and every message that reached it about this
  character other than its output (a browser's walk, bot and loop controls, typed lines the
  brain arbitrates, `logging_off`), one JSON line each.
- `notes.log`: the brain's notices and warnings about this character, one JSON line each:
  `{"seq":..,"ts":..,"kind":"notice"|"warning","text":".."}`, `seq` the latest output `seq` then.
  What the bot said it was doing ("Fight: large giant rat", "Loop Newhaven: to stop 2/3 ...")
  and why it stopped or paused ("Stopped: ...", "Paused: you took over"); the tile and the dev
  panel show the same lines only while they are open. Not part of a replay's input.

Together they are a recording that `wayfarer replay` plays back ([replay.md](replay.md)).

Sequence numbers never restart: a restarted gateway carries on from the last one in
`output.log`, so a browser that reconnects asks for everything since the last number it saw
and misses nothing.
