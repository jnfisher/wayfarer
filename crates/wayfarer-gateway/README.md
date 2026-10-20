# wayfarer-gateway

The gateway's core (`docs/plan/02-architecture.md`, "The shape"): one telnet session per
configured character, a sequence-numbered output log with history, fan-out to any number of
subscribers, input to the game, logon scripts, reconnects and clean log-off, with a WebSocket
server in front of it (below). How to configure it: [`docs/gateway.md`](../../docs/gateway.md).

```rust
let gw = Gateway::start(&Paths::in_dir(data_dir), Options::default())?; // autostart sessions
let mut output = gw.subscribe("kasimir")?;          // broadcast::Receiver<Output>
gw.open("kasimir")?;                                // connecting, logon, menu, realm
gw.send_line("kasimir", "look").await?;             // CP437 plus CR
gw.send_keys("kasimir", &[13]).await?;              // bytes as given
let h = gw.history("kasimir", HistoryQuery::after(42, 2000)).await?;
gw.close("kasimir").await?;                         // logged off, then closed
gw.shutdown_on_signal().await?;                     // Ctrl-C/SIGTERM: everyone logs off
```

- **`Gateway`** is an `Arc` handle (clone it freely). `sessions()` lists every character with
  its state and `last_seq`; `state_changes()` streams `StateChange`s. Dropping the last clone
  aborts every session at once, logged off or not (which the game punishes): `shutdown()`
  logs them all off in parallel, giving up after `Options::logoff_patience` (360 s);
  `shutdown_after(ready)` first waits for `ready(id)` for each session in the realm;
  `shutdown_now()` and `close_now(id)` disconnect at once; `shutdown_unless(now)` is `shutdown()`
  until `now` completes. `shutdown_on_signal()` (for a program without the server) and `serve`
  share `stop_signals()`: the first SIGINT or SIGTERM shuts down cleanly, a second one at once.
- **A session** is one tokio task that owns the socket. Each socket read goes through
  `Telnet::receive`; negotiation replies are written back, and the decoded text becomes one
  `Chunk` with the next `seq`, written to disk, kept in memory and then published. Input is a
  queue the task drains one command at a time, so lines from two clients never interleave.
- **Logon scripts** (`logon.rs`): plain state machines fed with decoded text, no sockets. A
  `Runner` answers the server's prompts (steps `{expect, send, optional, timeout}`, then the
  `realm` prompt) and reports failures (`Invalid Credentials`: close; "already logged in":
  retry); a `Tracker` follows every chunk to the latest place shown (the realm prompt or a
  log-off step's prompt); a `Logoff` sends the log-off steps from that place, and a step
  answered by one of its `retry_on` texts again at its place's next prompt, up to `retries`
  times (`LogoffAction::Retry`, then `GiveUp`). Leaving a character in the realm (given up,
  timed out, or hung up on) logs a warning, "dropped in the realm", and sets the session's
  reason. Matching is on a
  rolling window of recent text with ANSI escapes removed, consumed to the end of each match.
  The presets (`src/presets/*.toml`) are compiled in; `Server::script()` lays a server's
  `[server.logon]` table over its preset.
- **States**: `closed` before the first `open` and after `close`, `connecting` while the
  connection opens, `logon` while the script runs, then `menu` and `realm` as the tracker sees
  them; `connected` when there is no script or no credentials (the player logs on by hand) or
  the realm prompt never came. `lost` when the connection fails or drops, or a logon is to be
  retried: the task waits `logon::backoff(attempt)` (5, 10, 30, then 60 s; reset by a logon
  that reaches the realm) and reconnects, unless `Options::reconnect` is off. `reason(id)`
  says why a session is closed or lost. Input is refused (`NotConnected`) while connecting or
  lost; it works during logon.
- **Connectors**: sessions reach servers through `Connect` (`Tcp` by default);
  `Gateway::with_connector` takes another, which the tests use for in-memory pipes on a paused
  clock (`tests/logon.rs`).
- **Subscribers** get a `tokio::sync::broadcast` receiver of `Options::subscriber_capacity`
  (1024) `Output` items: `Output::Chunk` for output and `Output::Sent` for each line that went
  to the game (below), in the order they happened. One that falls further behind gets
  `RecvError::Lagged(missed)` (sent on the wire as `lagged`) and fetches the gap with
  `history`; it never holds up the session or other subscribers. A client that also wants the
  past subscribes first, then asks for history, and drops live chunks whose `seq` it already
  has.
- **History** (`HistoryQuery`, the wire's `history_request`): `after_seq` gives the chunks after
  it, oldest first, up to `max_chunks`; `before_seq` the `max_chunks` chunks just before it;
  neither the latest. The newest `Options::memory_bytes` of text (4 MB) are answered from
  memory, anything older from disk.
- **Sent lines**: a session publishes a `Sent {session, seq, text}` on its output stream for
  every line it writes to its game (typed lines, not keys, and logon and log-off steps) once
  it is on the socket; `seq` is the session's latest output `seq` then. Only the session task
  publishes, so a `Sent` follows every chunk up to its `seq` and precedes the chunk that
  echoes it. Passwords are masked as in `sent.bin`. The server forwards them to the brain as
  `sent`, on the same task as its `output`, so the brain sees them in that order; UIs never
  get them.
- **Credentials** never reach a log: `Login`'s `Debug` hides the password, a broken
  `credentials.toml` is reported by line number only, nothing traces input text, a script's
  `{password}` is recorded in `sent.bin` as `********`, and so is a typed line that is exactly
  the character's password (the game still gets the password).

## The server

`wayfarer gateway [--config PATH] [--bind ADDR] [--ui-dir DIR] [--new-token]` (and `wayfarer
serve`, the same in M0) runs `Gateway::start` behind a `Hub` and `serve()`. Default bind `127.0.0.1:7373`,
default UI directory `./ui/dist`. No TLS: reach it over SSH or Tailscale.

| Route | What |
|---|---|
| `GET /ws` | WebSocket speaking the `wayfarer-proto` messages as JSON text frames: `hello` → `welcome` (or `refused`, then the connection closes), then `output`, `session_state`, `history`, `lagged` and `error` going out and `input_line`, `input_keys`, `history_request` and `subscribe` coming in, plus the brain's messages (below). A message that cannot be handled gets an `error` and the connection stays up. A connection receives every session until it sends `subscribe`. |
| `GET /healthz` | `{"status":"ok","sessions":n}` |
| `GET /`, other paths | The files in the UI directory. With no `index.html` there, a 503 page says the UI is not built; `/ws` still works. |

```console
$ curl -si localhost:7373/healthz
HTTP/1.1 200 OK
content-type: application/json
content-length: 28

{"sessions":1,"status":"ok"}
$ curl -si localhost:7373/
HTTP/1.1 503 Service Unavailable
content-type: text/html; charset=utf-8
content-length: 141

<!doctype html><title>wayfarer</title><p>The web UI is not built: nothing in <code>./ui/dist</code>. The WebSocket at <code>/ws</code> works.
```

**The hub and the brain.** `Hub::new(gateway, token)` holds what the connections share: the
token, the one brain and the relay. Each connection is a `Connection` running over a sink and
a stream of messages, so the same code serves a WebSocket (`serve`) and a pair of tokio
channels (`Hub::connect()`, for an in-process brain: send the `hello` first; dropping the
sender ends it). The rules are the same either way:

- At most one connection whose `hello` says `client: brain`; another is `refused` ("a brain is
  already attached") until it goes. The UIs get `brain_state` right after `welcome` and
  whenever a brain attaches or leaves.
- The brain receives what a UI does (output, states, history, for every session unless it
  subscribes) and also `sent`.
- Relay is by `Family` (`wayfarer-proto`), never by reading `fields` or `patch`: the brain's
  messages go into a `broadcast` channel (1024), and each UI has a forwarder that keeps those
  about its subscribed sessions (or about none) and waits on that UI's outbox. A slow UI only
  stalls its own forwarder, which lags and tells it `brain_lagged`; the brain never waits.
  Output forwarders still wait and drop nothing until the session's own channel lags.
- A UI's control message (`resync`, `state_request`) goes to the brain's outbox; with no brain
  it gets `error "no brain attached"`. A UI sending a brain message, or the brain sending a
  control one, gets an `error` and the message is dropped.
- **Input arbitration**: input goes straight to the game from any client, except a UI's line
  (`input_line`, or `user_input` with `force`) for a session the brain arbitrates: that goes to
  the brain as `user_input {from: "ui-<n>"}`, and nothing goes to the game. The brain turns a
  session on and off with `arbitrate {session, on}`; the set lives with the brain's
  registration, so it is empty for a new brain and gone the moment the brain detaches (lines go
  straight through again at once). A UI's keys always go straight through, and for an
  arbitrated session the brain gets `keys_sent`.

**Token.** On a loopback bind `hello.token` is not checked. On any other bind the gateway
refuses to start until `wayfarer gateway --new-token` has written `gateway-token` (mode 0600,
beside `wayfarer.toml`) and printed it once; every client must then send it, and a missing or
wrong one gets `refused`. Running `--new-token` again replaces the token.

**Stopping.** SIGINT or SIGTERM stops accepting and logs every character off
(`Hub::shutdown_unless`, over `Gateway::shutdown_after`: in parallel, up to 360 s). For each
session in the realm the brain gets `logging_off` first, and the log-off waits for its
`ready_to_leave` (`LEAVE_PATIENCE`, 30 s) or for the brain to go. Only once every log-off has
finished or given up are the connections ended (`Hub::stop`, channel ones included), so the
brain hears the log-off. A second signal during the log-off closes what is left at once
(`shutdown_now`), which the game punishes. `serve` takes the stops as a stream (`stop_signals()` in the binary), so tests drive
it with a channel.

## Files

Under the logs directory (`<data dir>/logs/`), one directory per character id:

| File | What |
|---|---|
| `output.log` | Every chunk, one JSON object per line: `{"seq":1,"ts":1760000000000,"text":"..."}`, `ts` in milliseconds since the Unix epoch, `text` UTF-8 with ANSI kept |
| `raw.bin` | Every byte received, before telnet stripping (`wayfarer_telnet::Capture`) |
| `sent.bin` | Every byte sent: negotiation replies and input |

All three are append-only and survive restarts. JSON escapes newlines, so a record is always
one line: at start-up the gateway reads only the tail of `output.log` to find the last `seq`
and carries on from it (a record cut short by a crash is dropped), and a history lookup on
disk is a binary search over byte offsets, since `seq` only rises through the file.
