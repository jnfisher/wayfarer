# Provenance

Quicksilver 1.20's regen-line lines (card C-066), from the Quicksilver 1.20.0 handoff,
`docs/plan/quicksilver-1.20/handoff.md` section 3.8 (the server's frozen 1.20.0 release): each
family's cast line to the caster and to the room, its "when it ends" line and how it shows on
the caster's status. In the server they are spell texts; wayfarer's stock world pack lacks them,
so they are static patterns here. Do not edit these files; a card that needs different data adds
its own subdirectory.

| File | From |
|---|---|
| `patterns-1.20.json` | Section 3.8's two tables, one pattern per family (`attunement`, `devotion`, `communion`, `song`) and kind (`cast`, `room`, `ends`, `status`): `regen_<family>_<kind>`, priority 40, `"seen": "Q"`, `"rules": "1.20"`; the room forms' `%s` is `(?P<who>[A-Z][A-Za-z]+)` |
