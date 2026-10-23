# Provenance

Party lines from the local silvermere (`127.0.0.1:2323`, the long-lived dev server), captured
on 2026-10-22 through a running gateway for card C-038. Four characters in one room: the
leader `aldis` (Witchunter), `isolde` (Cleric), `wren` (Druid) and `corvin` (Bard). The
script, sent one line per 1.5 s: `aldis: par`, `aldis: invite isolde`, `isolde: follow
aldis`, `aldis: par`, `isolde: par`, `wren: follow aldis` (not invited yet), `aldis: invite
wren`, `wren: follow aldis`, `aldis: invite corvin` (never follows: the `[Invited]` row),
`aldis: par`, `isolde: leave`, `aldis: uninvite wren`, `aldis: uninvite corvin`, `aldis:
par`, `isolde: par`. The leader was walked north by hand once mid-script, so `isolde.txt` and
`wren.txt` hold a real ` -- Following your Party leader north --` line.

Each file is one session's output for that window, the gateway's output-log chunks
(`logs/<session>/output.log`, `text` fields) joined as they came: ANSI, CRLF, the game's
backspace-decorated directions (`nM\borth`) and the other things happening in the room
(the bots were fighting) included.

The characters' names were replaced, before the files were first written here, by invented
names of the same length (so the fixed-width `par` columns are unchanged); nothing else was
edited. Do not edit these files; a card that needs other party lines adds its own
subdirectory.
