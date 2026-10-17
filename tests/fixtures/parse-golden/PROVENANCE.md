# Provenance

Expected events for `wayfarer-parse` (card C-010), produced by the prototype's parser,
`test/play/patterns.py` in `jnfisher/mmud-exploration` at commit
`e2c3840768b41753d800fe0f8209729011cb5520` (2026-10-17), with Python 3.13.16. The inputs are
this repository's own fixture copies (`../captures/*/raw.bin`, `../transcripts/*.txt`,
`../world/`), which are byte-identical to that commit's.

Generated once, by hand, with the script in this directory (never in CI):

```
cd <mmud-exploration>/test/play
PYTHONHASHSEED=0 python3 -I <wayfarer>/tests/fixtures/parse-golden/make_golden.py . <wayfarer>/tests/fixtures
```

`PYTHONHASHSEED=0` matters: `Registry.match_line` gathers candidates from a `set`, so a tie
between two equally good patterns under different index keys follows the hash seed. Over these
fixtures exactly one line depends on it (see `crates/wayfarer-parse/tests/golden.rs`, `KNOWN`).

`patterns.load_world` reads the module's raw Btrieve rows (`mmud_data._rows`), which a checkout
does not have; the script rebuilds those rows from the export, writing each field the loader
reads at its struct offset.

| File | What |
|---|---|
| `make_golden.py` | The script |
| `events/<fixture>.jsonl.gz` | Every event `Registry.iter_events` yields for a fixture: line index `n`, `name`, `cat`, `fields`, `line`. Captures are read with `raw_to_lines(raw.bin)`; transcripts as `patterns.selftest` reads them (`### ` lines dropped, `>>> 'cmd'` lines as sent-command markers) |
| `registry.jsonl.gz` | Every pattern of the registry in list order: name, cat, prio, spec, key, regex source; for generated patterns the WCCMSG text and number and what the regex reads from that text |
| `patterns.json` | `patterns.py export` output at that commit. The copy in `../patterns/` (C-001) predates 37 static patterns added by T-065; `wayfarer-parse` embeds this one |
| `static-order.txt` | The static patterns' names in `patterns.STATIC` order (the export sorts them; ties need the order) |
| `summary.json` | Counts, unclassified lines and Python's timing per fixture |

Do not edit these files; regenerate them with the script, or add a new subdirectory.
