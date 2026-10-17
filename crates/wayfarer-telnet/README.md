# wayfarer-telnet

A small telnet layer with no sockets: the gateway feeds it bytes from the socket and writes
back what it returns.

- `Telnet::receive(&[u8]) -> Received { text, replies }`: strips telnet (state survives chunk
  boundaries), decodes CP437 to Unicode (controls and ANSI stay as they are), and returns the
  negotiation bytes to send. `DO NAWS` is answered with `WILL NAWS` and an 80x24 size; every
  other `DO` with `WONT`; `WILL` is accepted silently; `DONT`/`WONT` get nothing; the same
  answer is never sent twice in a row.
- `Telnet::encode_line(&str)`: CP437 (unmappable characters become `?`) plus a single CR.
- `Telnet::encode_keys(&str)`: CP437, sent as given. Both double any 255 byte.
- `Capture`: appends every received byte (before stripping) and every sent byte to two files,
  flushed per write.
