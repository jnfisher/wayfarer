# Provenance

Quicksilver 1.20 test data (wave 12, card C-063). Every file here comes from the Quicksilver 1.20.0
handoff, `docs/plan/quicksilver-1.20/handoff.md` in this repository: the server's frozen 1.20.0
release, its patterns export as quoted there. No capture from a live server yet. Do not edit these
files; a card that needs different data adds its own subdirectory.

| File | From |
|---|---|
| `patterns-1.20.json` | Section 3: the 22 new patterns (`opportunist_*`, `weave_*`, `guard_*`, `look_weapon_strreq`), each with its regex, category, priority and example, as written there; `"seen": "Q"` and `"rules": "1.20"` on every entry |
| `lines.txt` | Section 3: every example line with the pattern it must classify as, plus the opener, backstab, intercept and rest lines of 3.1, 3.3, 3.4 and 3.7, and `help_none` from section 2 |
| `help-1.20.txt` | Section 2: the server's whole answer to `help 1.20`, line by line |
