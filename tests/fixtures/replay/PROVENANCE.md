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

## Re-recorded 2026-10-23: a Go to is the bot's journey

A Go to (`walk_to`) now turns the bot on with a journey there (`docs/brain.md` "The bot's
states"), so the brain sends a re-sync before each walk and fights on arrival: the recordings
above no longer replay to the same commands. Two new ones, recorded the same way (the same
pool character, `scripts/live-check.sh --char Waychecka --scenario walk`, then `fight`, at the
branch `bot-on-off-routes`), copied byte for byte from each run's `01-logon/logs/probe/`; the
live check turns the bot off again after each walk it started. `replay.rs` plays these; the
old directories are kept as they were.

| Directory | Live-check run | Scenario | Length | Brain lines |
|---|---|---|---|---|
| `walk-2/probe/` | `20261023-173200-956590` | `--scenario walk`: re-sync, walk to 1/2141 (the bot on: a re-sync), walk back to 1/2146 (again) | 28 s | `st exp i look st exp i look e e n st exp i look s w w` |
| `fight-2/probe/` | `20261023-173229-956683` | `--scenario fight`: re-sync, walk to the Arena (the bot on: it fights a lashworm there), off again, Start, a kill, Stop, walk out | 55 s | 21 lines, `a rat` among them |

| File | SHA-256 |
|---|---|
| `walk-2/probe/output.log` | `49477e6a50f2ecda18f90e8d91d4b3e29f6d2bfbb6e9f813c642cbd4c156b858` |
| `walk-2/probe/sent.log` | `99ec2f267004915a1f8900db0eb5bb42529467324f50e9826eaa7670c896190c` |
| `walk-2/probe/brain.log` | `2c64a4a451fd614f5afc67af57b42bd4913895ced60905856182059660da850b` |
| `fight-2/probe/output.log` | `fb89e27077abc8a364b80c5810e83808cb206f7310b851184559d0bca30914fb` |
| `fight-2/probe/sent.log` | `d92ff2ea5d9c56b552a03fcfac19e2c0e3f116e4974c81f39425fea7108bcf6d` |
| `fight-2/probe/brain.log` | `0f223035ccbccaac9fa5c03561e1ac69bceaa8c2578b5bab4c5c1108a7c32fec` |
