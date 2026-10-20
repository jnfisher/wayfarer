# Provenance

The gigantic black ooze fight of the M3 soak (card C-035), for
`crates/wayfarer-brain/tests/rules.rs`. Copied on 2026-10-20 from a `scripts/live-check.sh
--scenario soak` run against the local silvermere at `127.0.0.1:2323` (run
`20261020-080407-61354`, the gateway's `01-logon/logs/probe/output.log` on John's laptop),
recorded with this repository's gateway and brain as they were on `main` that morning.

`output.jsonl` holds ten of that log's lines, byte for byte: seq 252 (a Labyrinth Passage with
the ooze in it), 255 (`*Combat Engaged*` after the brain's `a ooze`) and 256 to 263 (eight
rounds of the character's swings glancing off the ooze's armour, and the ooze's blows). They
name no account or character; nothing else from the run was copied. Do not edit the file; a
new recording goes into a new subdirectory.

| File | SHA-256 |
|---|---|
| `output.jsonl` | `e130a69ecf146e33d7096eaa1c87d69df9fd61bfac0cea3a6e33765f70832233` |
