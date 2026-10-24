# Provenance

Three paths of the stock MegaMud v2.2 path set, for the room-command and crossing tests (card
C-052): `SLMCSSME.mp` (Slum Crossroads to the Strange Mansion's Entrance, ending with `@party
ring chime` at Slum Street, 1:1182), `SSMESMST.mp` (the Entrance to the spectral mage,
through `move bookcase`) and `ELSAHDRU.mp` (`go arch` at 7:141, where the pack has no way
through). Copied on 2026-10-24, byte for byte, from this repository's
`data/megamud/Default/` at commit `bfa9fc1d4bbb596a6b3513add74e7182dfd0a75d` (its own
provenance is `data/megamud/PROVENANCE.md`).

The test pack is built from `tests/fixtures/megamud` with these files added beside it
(`crates/wayfarer-world/tests/common/mod.rs`). Do not edit these files; a card that needs other
paths adds its own subdirectory.
