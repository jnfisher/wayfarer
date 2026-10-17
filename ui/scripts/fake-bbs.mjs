// A stand-in game server for looking at the UI without one: each port replays a recorded
// session (a `raw.bin` from tests/fixtures/captures/) to whoever connects, then echoes lines.
//
//   node scripts/fake-bbs.mjs 4001=../tests/fixtures/captures/fight-arena/raw.bin[:BYTES] [...]
//
// `:BYTES` stops the replay after that many bytes (the captures end with a log-off).
// Point a wayfarer.toml server at 127.0.0.1:<port>. Not used by the tests.

import { readFileSync } from "node:fs";
import { createServer } from "node:net";

const specs = process.argv.slice(2);
if (specs.length === 0) {
  console.error("usage: fake-bbs.mjs PORT=RAW.BIN[:BYTES] [...]");
  process.exit(2);
}

for (const spec of specs) {
  const [port, rest] = spec.split("=");
  const [file, bytes] = rest.split(":");
  const capture = readFileSync(file).subarray(0, bytes ? Number(bytes) : undefined);
  createServer((socket) => {
    socket.on("error", () => {});
    // Replay in pieces so it arrives as several chunks, like a real session.
    let at = 0;
    const timer = setInterval(() => {
      if (at >= capture.length || socket.destroyed) return clearInterval(timer);
      socket.write(capture.subarray(at, at + 4096));
      at += 4096;
    }, 20);
    socket.on("data", (data) => socket.write(data));
    socket.on("close", () => clearInterval(timer));
  }).listen(Number(port), "127.0.0.1", () => console.log(`fake-bbs: ${file} on 127.0.0.1:${port}`));
}
