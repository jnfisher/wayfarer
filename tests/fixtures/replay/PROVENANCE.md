# Provenance

Two gateway session recordings for the CI replay layer (`crates/wayfarer-brain/tests/replay.rs`,
`docs/replay.md`). Recorded by card C-032 on 2026-10-20 with this repository's gateway and brain
at the card's branch (parent commit `6704a08`), from `scripts/live-check.sh` runs against the
local silvermere at `127.0.0.1:2323`, with the test-pool character Waychecka (account
`Waychecka`, a test account on that local server only). The session id the live check gives the
character is `probe`, which names each directory.

Only `output.log`, `sent.log` and `brain.log` were copied, byte for byte, from the run's
`01-logon/logs/probe/` (not `raw.bin`, `sent.bin` or the run's `credentials.toml`). The
password appears nowhere: `sent.log` masks it as `********`. Do not edit these files; a
re-recording goes into a new subdirectory.

| Directory | Live-check run | Scenario | Length | Brain lines |
|---|---|---|---|---|
| `walk/probe/` | `20261020-030852` | `--scenario walk`: re-sync, walk to 1/2146, walk back to 1/2140 | 14 s | `st exp i look w w e e` |
| `fight/probe/` | `20261020-030930` | `--scenario fight`: re-sync, walk to the Arena, Start, a kill of a fierce kobold thief, Stop, walk out | 55 s | 17 lines, `a thief` among them |

| File | SHA-256 |
|---|---|
| `walk/probe/output.log` | `1548a68c1872c49033ea56bfc527a15f41c880b4edce2ad7800f2f08d0951c3e` |
| `walk/probe/sent.log` | `b10f24417336d9842026be81dd4ef60c664676b01d8c5bf77cc4f3b31e8035cd` |
| `walk/probe/brain.log` | `7d3499aaec115ab4fa1fc3a74fd5627a733f4d0562a2bc5b140f8d3d301fd6f5` |
| `fight/probe/output.log` | `4936263a82a46bf8d5e6f1efd6b04abbb746a62e790c7b8c35e48a5594ca5af6` |
| `fight/probe/sent.log` | `3e33ef26f2c06ecf59019f85225281502ec6b9c4e08dba64f2618edd45d42d48` |
| `fight/probe/brain.log` | `9863e777df4bc95578ad40e12cba17cda1afb88e1351d4aab82cc70b59d250f7` |
