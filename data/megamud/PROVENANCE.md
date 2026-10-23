# Provenance

The path set that ships with stock MegaMud v2.2: every `.mp` file (1,957 paths, 195 of them
loops) and `Rooms.md` (the waypoints) from MegaMud's `Default/` directory. The dev scripts
build the world pack's MegaMud loops from it (`docs/dev-rig.md`, "The world pack"); the paths
are free to use. The rest of the zip (the program, its help and its item, monster, spell,
class, race and message databases) stays out of the repository.

Copied on 2026-10-22, byte for byte, from `MegaMMUD v2.2 (Stock).zip` (2,301,107 bytes,
SHA-256 `b284231c16786df370d3da08a98fb92105439dceb1376c645fc71fef582af3f0`), downloaded from
<https://www.mudinfo.net/download/file.php?id=912>.

`wayfarer import-world --megamud data/megamud` against `tests/fixtures/world` gives 783
waypoints (726 resolve to a game room), 1,957 paths (1,781 walk to the end), and 183 loops
walked to the end, all closing in their start room; 99.6% of 44,384 steps match by full room id.

Don't edit these files: `.gitattributes` keeps their CRLF line ends. A newer set replaces the
whole directory, with this file updated.
