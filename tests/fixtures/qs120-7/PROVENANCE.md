# Provenance

Quicksilver 1.20.7 (jnfisher/quicksilver-dev PRs #62, #63, #64, merged 2026-11-25): the LOOT
command's lines, from the server source (`crates/silvermere-text/messages/v120.toml`, keys
`loot_you`, `loot_room`, `loot_dropped`, `loot_kept`, `corpse_crumbles`, `loot_no_corpse`,
`loot_no_such`) as John quoted them in his notes for wayfarer on 2026-11-25. No live capture yet.
`loot_took` (`You took 3 gold crowns, quarterstaff.`) has no pattern of its own: it reads as the
stock `took_item`, which it is one of. Do not edit these files; a card that needs different data
adds its own subdirectory.

| File | From |
|---|---|
| `patterns-1.20.7.json` | The seven lines, each with its regex, category `loot`, priority 40 and example; `"seen": "C"` (server source) and `"rules": "1.20"` on every entry |
| `lines.txt` | Every example line with the pattern it must classify as, and the `loot_took` line as `took_item` |

1.20.7 also removed `guard_resting` (`You cannot guard while resting or meditating.`): a guard
now lasts through rests. The 1.20.0 fixture keeps the line; `wayfarer_parse::table::qs120`
leaves the pattern out.
