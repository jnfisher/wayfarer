# Provenance

silvermere's oracle for MajorMUD's command parser: every letter sequence the game tells apart
as the first word of a line, with the `CMD_` code it runs (generated in silvermere by
`tools/parsegen/parsegen.py` from the original's parser).

Copied by card C-031 from `jnfisher/silvermere` at commit
`b5dfbc9026e65adbd747a2653ab0147db17aba9c` (2026-10-19), byte for byte; the file was last
changed there at `77a4d5815c6dfccd87d793491103aac58f8b6e29`. Do not edit or regenerate it here;
a card that needs different data adds its own subdirectory.

| File | Source path |
|---|---|
| `command_words.json` | `crates/silvermere-engine/tests/fixtures/command_words.json` |

Read by `crates/wayfarer-brain/src/commands.rs`'s tests, which feed every entry through the
brain's command table.
