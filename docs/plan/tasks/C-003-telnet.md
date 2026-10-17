---
id: C-003
title: wayfarer-telnet: option negotiation (NAWS 80x24), CP437 both ways, raw capture
tier: C
size: S
wave: 1
depends: [C-001]
crate: wayfarer-telnet
touches:
  - crates/wayfarer-telnet/src/
  - tests/fixtures/captures/ (read only; copied in by C-001)
---

## Goal

A small, synchronous-core telnet layer: feed it bytes from the socket, get back bytes to send
(negotiation replies) and decoded text for the terminal; give it a line or keys, get bytes for
the socket. It also tees the raw bytes to a capture file.

## Inputs

- `mmud-exploration/kb/connection.md` "Transport": MBBSEmu's opening offers verbatim (`IAC
  WILL BINARY, IAC DO BINARY, IAC WILL ECHO, IAC DONT ECHO, IAC WILL SGA, IAC DO SGA, IAC WONT
  NAWS, IAC DO NAWS`), the NAWS answer and what it sets, CP437, CR LF and bare CR.
- `mmud-exploration/test/play/bridge.py` `strip_telnet` (:86-114): the state machine that has
  worked for weeks (refuse every `DO` with `WONT`, keep state across chunks, `IAC IAC` is a
  literal 255, skip subnegotiations); and `send` (:175): lines go out CP437-encoded, ending in
  a single CR.
- `mmud-exploration/test/play/capture.py`: `CaptureSession` answers `DO NAWS` with the window
  size.
- `tests/fixtures/captures/*/raw.bin` (12 scenarios, telnet already stripped) and their
  `plain.txt`, copied in by C-001.
- `silvermere/docs/net.md` for what silvermere sends (it aims to match MBBSEmu).

## Behaviour

- Negotiation: answer `DO NAWS` with `IAC WILL NAWS` then `IAC SB NAWS 0 80 0 24 IAC SE`
  (80x24; doubled 255 bytes if a size ever contains 255); answer every other `DO` with `WONT`;
  answer `WILL` with nothing (accept the server's echo and binary, as the prototype did); answer
  nothing to `DONT`/`WONT`. Never answer the same option twice in a row (no negotiation loops).
- State survives chunk boundaries: an `IAC` at the end of one read and its command at the start
  of the next must work.
- Decoding: bytes 0x80-0xFF map to their CP437 Unicode characters; 0x00-0x7F pass through
  unchanged (ESC and the other controls stay controls, so ANSI survives). Output is a `String`.
- Encoding outgoing text: Unicode → CP437 (characters with no CP437 form become `?`), then a
  single `\r` for a line. Keys (`input_keys`) are sent as given, with any 255 doubled.
- Capture: every byte received, before telnet stripping, appended to a file the caller names
  (`raw.bin` style); and a second file of what was sent. Writes are unbuffered or flushed per
  chunk so a crash loses nothing.

## Design notes

- `Telnet` struct with `fn receive(&mut self, bytes: &[u8]) -> Received { text: String,
  replies: Vec<u8> }` and `fn encode_line(&self, s: &str) -> Vec<u8>`, `fn encode_keys(...)`.
  No sockets here; the gateway owns the socket. This keeps it testable without a network.
- CP437 table as a `const [char; 128]`; do not pull a crate for it.
- `Capture` is a separate small type wrapping two files.

## Done when

- The MBBSEmu opening offers produce exactly `IAC WONT BINARY, IAC WONT SGA, IAC WILL NAWS,
  IAC SB NAWS 0 80 0 24 IAC SE` (in the order the `DO`s arrive), tested byte for byte.
- Every vendored `raw.bin` decodes without error, and its text contains the box-drawing and
  block characters of the MBBSEmu banner (`╔`, `█`).
- Split-chunk tests pass for every split point of a negotiation sequence.

## Tests

- Unit tests per rule above; a property-style test that splitting any input at any point gives
  the same result as feeding it whole; decode tests over the vendored captures.

## Notes for the agent

- The captures are already in `tests/fixtures/captures/` (C-001); read them, don't copy them
  again.
- The captures have telnet stripped, so negotiation tests use hand-built byte strings.
