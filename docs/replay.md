# Replay

`wayfarer replay` plays a recorded session back as if the game were sending it: the normal
page shows the tile, the header and the dev panel's events exactly as they were live, at the
recorded pace or faster. It is how you watch what a bot did while you were away. The same
machinery is the CI replay layer: a recording fed through the brain on the recorded clock,
whose decisions must match.

## Recording

Nothing to switch on: every gateway session is recorded, in `logs/<character id>/` under the
data directory ([gateway.md](gateway.md#logs)):

| File | What the replay does with it |
|---|---|
| `output.log` | Feeds each chunk at its `ts`, as the game's bytes |
| `sent.log` | Publishes each line as `sent` between the chunks it went between (for a brain; a browser never sees `sent`) |
| `brain.log` | Attaches a brain when the recorded one attached, detaches it when it went, and hands it what the browsers sent it (walks, bot and loop controls, typed lines it arbitrated, `logging_off`) when they did |

`raw.bin` and `sent.bin` are not needed, and a replay needs no credentials: the password is
never in any of these files. A recording from before `sent.log` and `brain.log` existed still
replays; a brain then attaches at the start and gets no recorded controls.

The logs are append-only across gateway restarts, so a directory may hold several runs; the
gaps between them play as recorded (use `--speed` or `--instant`).

## Replaying

```sh
wayfarer replay ~/Library/Application\ Support/wayfarer/logs/thorgrim            # as recorded
wayfarer replay <dir> --speed 10 --brain --bind 127.0.0.1:7374                     # beside a running gateway
wayfarer replay <dir> --instant                                                    # as fast as it goes
```

| Option | |
|---|---|
| `--speed N` | `N` recorded seconds per second (default 1) |
| `--instant` | As fast as the session takes the output: a recording plays in seconds |
| `--brain` | Run a brain in-process on the recording's clock, with a throwaway store, attached when the recorded brain was |
| `--world PATH` | The world pack for `--brain` (default `world.sqlite` beside the config) |
| `--config PATH` | Its server for this character gives the logon script whose prompts the session's state follows (default: the silvermere preset); its directory holds `gateway-token` and the world pack |
| `--bind ADDR` | Where to serve the page (default `127.0.0.1:7373`: pick another port while a gateway runs) |
| `--ui-dir DIR` | The built web UI (default `./ui/dist`) |

The replay serves one session, named after the directory. Its state follows the text
(`menu`, `realm`) as a live session's does. A replay writes its own logs into a fresh
directory under the system's temporary directory (the path is in its first log line), never
over the recording.

**What the page shows.** Exactly the live page: the tile scrolls the recorded output, and
with a brain attached the header fills, the map follows, the dev panel shows the event
stream and the bot's narration comes up. Lines typed into the tile are recorded in the
replay's own `sent.log` and go nowhere. There is no replay page: the normal page on a replay
is the feature.

**Stopping.** Ctrl-C. Nothing is logged off (there is nobody to log off). After the last
chunk the session stays as the recording left it until you stop it.

**The brain on a replay.** The replay's brain is a real brain: it parses, decides and sends,
on the recording's clock, so the re-sync's idle wait and the 12 s quiet rule fall where they
fell live. What it sends goes nowhere, and it hears its own lines as `sent` (the recorded
brain's are not fed to it). At `--speed 1` it decides what the live brain decided (that is
what CI checks). At higher speeds the moments the gateway hands it things drift by a few
milliseconds of real time, which is many recorded milliseconds, and it can decide otherwise
now and then (a walk replanned, a re-sync at another moment); with `--instant` the brain's
time jumps from chunk to chunk and its decisions are only a sketch. Watch the output for what
happened; the brain's narration is what this brain would have done.

## The CI replay layer

`crates/wayfarer-brain/tests/replay.rs` plays each fixture in `tests/fixtures/replay/` (a live
check's "walk" and "fight" sessions, with their `sent.log` and `brain.log`) through a replay
gateway into a brain on the recorded clock, on tokio's paused clock, and asserts that the
lines the brain sends equal, in order, the lines `sent.log` shows with origin `brain`. A
minute of recording takes no time; each test is about a third of a second.

A change that alters a decision the fixtures exercise fails it (changing `IDLE` from 2 s to
4 s drops the re-sync from both). When a card changes a decision on purpose, it re-records the
fixtures:

1. Run the scenarios against the local silvermere with the pool character:
   `scripts/live-check.sh --char Waychecka --scenario walk`, then `--scenario fight`.
2. Each prints its log directory; the session's recording is in
   `<run>/01-logon/logs/probe/`. Copy `output.log`, `sent.log` and `brain.log` (only those:
   never the run's `credentials.toml`) into a new `tests/fixtures/replay/<name>/probe/` (the
   copies are never edited; a new recording goes into a new subdirectory, and the test is
   pointed at it).
3. Update `tests/fixtures/replay/PROVENANCE.md` and say so in the pull request.

A recording only replays to the same commands if everything that reached the brain is in it:
lines typed while the brain arbitrates come back through `brain.log`'s `user_input`, so
they replay too.
