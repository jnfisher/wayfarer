# Provenance

Quicksilver 1.20's spell data (card C-066), shipped with wayfarer and embedded in
`wayfarer-world` at build time. The source is the Quicksilver 1.20.0 handoff,
`docs/plan/quicksilver-1.20/handoff.md` in this repository: the server's frozen 1.20.0 release.

| File | From |
|---|---|
| `spells.csv` | Section 4.5, the CSV block, byte for byte: every Mage, Priest and Druid player spell as 1.20.0 has it (219, 132 of them changed from 1.11p) and the regen line's Mage, Priest and Druid tiers (6000-6016). The column key is in 4.5 |
| `songs.csv` | The four regen songs (6017-6020), in 4.5's columns: missing from the handoff's 4.5 CSV, **copied from the server's 1.20.0 records (via the orchestrator, 2026-11-01)**. Their type, resist mode and element are blank, as in 4.5's key for songs: they are those of the stock song 803 (song of soothing) they were made from, which `wayfarer-world` reads from the pack. Each has flags 0, no level cap, no growth, record message 5106 and descriptive message 5107 (ability 115) |
| `regen.csv` | Section 4.2's table, one row per regen-line spell: family (`attunement`, `devotion`, `communion`, `song`), line (`full` for the full casters, `half` for Gypsy, Warlock, Cleric and Missionary, `song` for the Bard), level, regen %, who casts it, the scroll's item number and name, the shop's number and name, and the scroll's cost (currency 2 gold, 3 platinum). Serenity's scroll is the stock one, as 1.11p (no item number here) |
| `shops.csv` | Section 4.2, "Prices" and "Stock": each regen shop's markup and its stock (quantity, restock time, restock chance) |
| `texts.csv` | Section 3.8: each family's cast line to the caster and to the room (`%s` the caster), its "when it ends" line and how it shows on the caster |
