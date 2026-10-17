# Stock world export (T-006)

Every record of every stock Btrieve table, decoded by `test/play/mmud_data.py export` from the
`.db` files MBBSEmu creates when an instance starts once. One `<TABLE>.jsonl.gz` per Btrieve
file plus `MANIFEST.json`. Standard library only: `gzip`, `json`, `base64`.

```
cd test/play
python3 mmud_data.py export --data ../run/built/WCCMMUD --out tests/fixtures/world
```

Run it as `python3` from `test/play` (or `python3 -I` with `PYTHONPATH` set: `-I` drops the
current directory, so `python3 -I -m unittest tests.test_world_export` cannot find `tests`).

Stop the instance first (`test/run.sh built stop`): room, active-monster and item-owner rows
hold run-time fields that change while it runs.

## Files

`WCCMP001` rooms (`NEWMP001` too if staged), `WCCITEMS`, `WCCKNMSR` known monsters, `WCCACMSR`
active monsters, `WCCSPELS`, `WCCCLASS`, `WCCRACE`, `WCCSHOPS`, `WCCMSG`, `WCCTEXT`, `WCCACTS`,
`WCCGANGS`, `WCCBANKS`, `WCCITOWN`, `WCCUSERS`.

## Line format

Gzip, one JSON object per line, in `data_t.id` order (physical page order):

```
{"_id": 1, "_raw_len": 1528, "mapnum": 1, "roomnum": 1, "name": "...", "exits": [..10..], ...}
```

- `_id` is `data_t.id`, the import order, **not** the Btrieve address. Compare by key
  (room: `mapnum, roomnum`; others: the `number` field) or positionally; both work.
- `_raw_len` is the record length; it must equal `record_length` in the manifest.
- Field names, offsets and C types are the `struct` members of `WCCMMUD-1.11p-source/SRC/
  MAJORMUD.H`; the offsets are in `RECORDS` in `mmud_data.py` (`(name, offset, kind, dims)`).
  All integers are little-endian. `int` = i16, `unsigned` = u16, `long` = i32,
  `unsigned long` = u32, plain `char` = **u8** (`-funsigned-char`), `signed char` = i8.
- Arrays are JSON arrays (2-D `char` arrays are arrays of strings). Strings are the bytes up to
  the first NUL, decoded as CP437 and stripped (`mmud_data._s`); the JSON is ASCII-escaped.
- `unk_*` spans are omitted, except those T-009 needs (room `unk_0x555`, `unk_0x5AF`; item
  `unk_0x338/33a/33c/3a1/3a5/3a9/3d7/414/418/41c`; monster `unk_0x0AB/192/19F/1A1/1A3/1A4`;
  class `unk_0x05E`; race `unk_0x05C`), as integers, or base64 for byte spans.
- Known monsters carry `cash` (raw order, **runic first**) and `cash_copper_first` (reversed).
- `WCCTEXT` carries `text_raw`: base64 of the 2000 text bytes at +0x16, undecoded (each byte is
  stored plus 0x20; the decode is T-008's to define).
- Run-time fields (`dirty`, `changed`, `lastkill`, `nummonsters`, `monsters[]`, ...) are exported
  as found after the first start; they are not stock content.

## MANIFEST.json

Per table: `rows`, `record_length`, `file`, `file_bytes`, `file_sha256`, and, where the virgin
`.VIR` is staged, `vir_sha256`, `fcr_page_length` (u16 at 0x08) and `fcr_record_count`
(u16 at 0x1A high word, 0x1C low word). A row count that differs from the FCR count means the
`.DAT` to `.db` conversion dropped duplicate-key rows. `WCCACMSR` and `WCCITOWN` start empty in
the virgin files, so their FCR count is 0 and their rows are run-time state; `WCCUSERS` rows would
be test characters (none here). Per table `db_sha256` is the SHA-256 of the `.db` file read. Top level: `instance` (`built` here; same DLL bytes as `orig`), `module_dll_sha256` (the `WCCMMUD.DLL` that ran), `post_update: true` (`WCCUPDAT.DAT` was applied by the
DLL's own paced update path at the first start (LJNBUFF.C:520-577, one record per start-up slice
and per user poll at LJNGAME.C:1362), so this is **post-update** content; `wccmmutl -needed
-fast` is the offline recovery utility (WCCMMUD.RLN:1183-1185) and MBBSEmu never runs the
MDF's `BBS UP` line, so it did not apply the stream here; see `kb/game-data.md` and
silvermere#25 for how far the paced update provably got), `date`,
`dist_zip` and its SHA-256.
