---
id: C-006
title: WebSocket server: token auth, localhost by default, static files, `wayfarer gateway`
tier: C
size: S
wave: 1
depends: [C-002, C-004]
crate: wayfarer-gateway, bin/wayfarer
touches:
  - crates/wayfarer-gateway/src/server.rs
  - bin/wayfarer/src/main.rs (the `gateway` and `serve` subcommands)
---

## Goal

`wayfarer gateway` runs the gateway from C-004/C-005 behind an HTTP server: a WebSocket
endpoint speaking the C-002 protocol, the web UI's static files, and a health check. On a
personal machine it needs no token; exposed beyond localhost it requires one.

## Inputs

- `docs/plan/02-architecture.md` "The wire protocol" and its "Security" paragraph.
- C-002 messages; C-004 `Gateway` handle; C-005 shutdown.
- `docs/plan/07-development-and-testing.md` "John's development rig" (reached through an SSH
  tunnel, which arrives as localhost, or Tailscale, which does not).

## Behaviour

- `wayfarer gateway [--config PATH] [--bind ADDR] [--ui-dir DIR]`; default bind
  `127.0.0.1:7373`, default UI directory `./ui/dist` (missing directory: the page says the UI
  is not built, the WebSocket still works).
- Routes: `GET /` and static files from the UI directory; `GET /ws` (WebSocket); `GET
  /healthz` (`{"status":"ok","sessions":n}`).
- Token: on a loopback bind, `hello.token` is not checked. On any other bind the gateway
  refuses to start unless a token exists (`gateway-token` in the data directory, created with
  `wayfarer gateway --new-token`, printed once); a wrong or missing token gets `refused`.
- Per WebSocket connection: `hello` → `welcome` with the session list (or `refused`), then
  output for the subscribed sessions as it arrives, `session_state` changes, answers to
  `history_request`, and input passed to the gateway. A client that lags is told so (C-004)
  and recovers with history.
- `wayfarer serve` is the same as `gateway` in M0 (the brain joins it in M1).
- SIGINT and SIGTERM: stop accepting connections, run the C-005 clean log-off, exit.

## Design notes

- axum with its WebSocket support, `tower-http` `ServeDir` for static files. One task per
  WebSocket connection reading `ToGateway` and writing `ToClient` as JSON text frames.
- Malformed messages get an `error` reply; the connection stays up.

## Done when

- An in-process test starts the server on `127.0.0.1:0` with a fake game server (as in C-004)
  and a WebSocket client (`tokio-tungstenite`): handshake, protocol mismatch refused, output
  arrives, `input_line` reaches the fake server, `history_request` answers, two clients at once.
- A non-loopback bind without a token refuses to start; with one, a wrong token is refused.
- `curl localhost:7373/healthz` documented in the crate README with real output.

## Tests

- The in-process tests above, each under a second.

## Notes for the agent

- No TLS: remote access goes through SSH or Tailscale.
- Keep the UI directory a flag in M0; embedding the UI in the binary is a packaging card (M6).
