# wayfarer-gateway

The gateway's core (`docs/plan/02-architecture.md`, "The shape"): one telnet session per
configured character, a sequence-numbered output log with history, fan-out to any number of
subscribers, and input to the game, with a WebSocket server in front of it (below). No logon
scripts yet (C-005). How to configure it: [`docs/gateway.md`](../../docs/gateway.md).

```rust
let gw = Gateway::start(&Paths::in_dir(data_dir), Options::default())?; // autostart sessions
let mut output = gw.subscribe("kasimir")?;          // broadcast::Receiver<Chunk>
gw.open("kasimir")?;                                // connecting, then connected
gw.send_line("kasimir", "look").await?;             // CP437 plus CR
gw.send_keys("kasimir", &[13]).await?;              // bytes as given
let h = gw.history("kasimir", HistoryQuery::after(42, 2000)).await?;
gw.close("kasimir").await?;                         // closed
```

- **`Gateway`** is an `Arc` handle (clone it freely). `sessions()` lists every character with
  its state and `last_seq`; `state_changes()` streams `StateChange`s. Dropping the last clone
  aborts every session at once; `shutdown()` closes them first (C-005 makes that a clean
  log-off).
- **A session** is one tokio task that owns the socket. Each socket read goes through
  `Telnet::receive`; negotiation replies are written back, and the decoded text becomes one
  `Chunk` with the next `seq`, written to disk, kept in memory and then published. Input is a
  queue the task drains one command at a time, so lines from two clients never interleave.
- **States** (M0): `closed` before the first `open` and after `close`, `connecting` while the
  connection opens, `connected` once it is up (the player logs on by hand until C-005's scripts),
  `lost` when the connection fails or drops. No reconnect here (C-005). Input is refused
  (`NotConnected`) unless the session is connected.
- **Subscribers** get a `tokio::sync::broadcast` receiver of `Options::subscriber_capacity`
  (1024) chunks. One that falls further behind gets `RecvError::Lagged(missed)` (sent on the
  wire as `lagged`) and fetches the gap with `history`; it never holds up the session or other
  subscribers. A client that also wants the past subscribes first, then asks for history, and
  drops live chunks whose `seq` it already has.
- **History** (`HistoryQuery`, the wire's `history_request`): `after_seq` gives the chunks after
  it, oldest first, up to `max_chunks`; `before_seq` the `max_chunks` chunks just before it;
  neither the latest. The newest `Options::memory_bytes` of text (4 MB) are answered from
  memory, anything older from disk.
- **Credentials** never reach a log: `Login`'s `Debug` hides the password, a broken
  `credentials.toml` is reported by line number only, nothing traces input text, and a line
  that is exactly the character's password is recorded in `sent.bin` as `********` (the game
  still gets the password).

## The server

`wayfarer gateway [--config PATH] [--bind ADDR] [--ui-dir DIR] [--new-token]` (and `wayfarer
serve`, the same in M0) runs `Gateway::start` behind `serve()`. Default bind `127.0.0.1:7373`,
default UI directory `./ui/dist`. No TLS: reach it over SSH or Tailscale.

| Route | What |
|---|---|
| `GET /ws` | WebSocket speaking the `wayfarer-proto` messages as JSON text frames: `hello` → `welcome` (or `refused`, then the connection closes), then `output`, `session_state`, `history`, `lagged` and `error` going out and `input_line`, `input_keys`, `history_request` and `subscribe` coming in. A message that cannot be handled gets an `error` and the connection stays up. A connection receives every session until it sends `subscribe`. |
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

**Token.** On a loopback bind `hello.token` is not checked. On any other bind the gateway
refuses to start until `wayfarer gateway --new-token` has written `gateway-token` (mode 0600,
beside `wayfarer.toml`) and printed it once; every client must then send it, and a missing or
wrong one gets `refused`. Running `--new-token` again replaces the token.

**Stopping.** SIGINT or SIGTERM stops accepting, ends the WebSocket connections and calls
`Gateway::shutdown` (a clean log-off once C-005 lands) before the process exits.

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
