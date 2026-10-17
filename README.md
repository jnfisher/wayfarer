# wayfarer

A MajorMUD playing client and bot: one program runs a party of up
to six characters, shows them in a browser, desktop app or terminal with an old-school ANSI
look, and can play each of them. It speaks plain telnet, so it works with
[silvermere](https://github.com/jnfisher/silvermere), MBBSEmu running the original module,
and any BBS still running MajorMUD 1.11p.

**Status:** just started. The plan is in [docs/plan/](docs/plan/README.md); the first
milestone (M0) is a live terminal per character in the browser.

## Build

```sh
cargo build                    # the toolchain is pinned in rust-toolchain.toml
cargo nextest run --workspace  # or: cargo test --workspace
cargo run -p wayfarer -- --version
```

## Layout

| Path | What it is |
|---|---|
| `crates/wayfarer-proto` | Wire messages between gateway, brain and UIs |
| `crates/wayfarer-telnet` | Telnet negotiation, CP437, raw capture |
| `crates/wayfarer-gateway` | Telnet sessions, output log, logon scripts, WebSocket fan-out |
| `bin/wayfarer` | The `wayfarer` binary |
| `tests/fixtures/` | Test data copied from mmud-exploration, with provenance |
| `docs/plan/` | The design and the task cards |

## Licence

MIT; see [LICENSE](LICENSE).
