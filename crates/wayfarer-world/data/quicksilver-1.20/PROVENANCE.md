# Provenance

Quicksilver 1.20's spell data (card C-066), shipped with wayfarer and embedded in
`wayfarer-world` at build time. The source is the Quicksilver 1.20.0 handoff,
`docs/plan/quicksilver-1.20/handoff.md` in this repository: the server's frozen 1.20.0 release.

| File | From |
|---|---|
| `spells.csv` | Section 4.5, the CSV block, byte for byte: every Mage, Priest and Druid player spell as 1.20.0 has it (219, 132 of them changed from 1.11p) and the regen line's Mage, Priest and Druid tiers (6000-6016). The column key is in 4.5 |
| `songs.csv` | **Built**, not copied: the four regen songs (6017-6020) are missing from 4.5's CSV, so these rows are made from section 4.2's tables (level, mana, duration, difficulty, regen) in 4.5's columns: magery Bard with magery level 0, target 1 (self), type 3, resist mode 1, element 4, ability 145 (alter mana regeneration) with the regen, and ability 122 removing each other song. No 115 message (its number is not known) |
| `regen.csv` | Section 4.2's table, one row per regen-line spell: family (`attunement`, `devotion`, `communion`, `song`), line (`full` for the full casters, `half` for Gypsy, Warlock, Cleric and Missionary, `song` for the Bard), level, regen %, who casts it, the scroll's item number and name, the shop's number and name, and the scroll's cost (currency 2 gold, 3 platinum). Serenity's scroll is the stock one, as 1.11p (no item number here) |
| `shops.csv` | Section 4.2, "Prices" and "Stock": each regen shop's markup and its stock (quantity, restock time, restock chance) |
| `texts.csv` | Section 3.8: each family's cast line to the caster and to the room (`%s` the caster), its "when it ends" line and how it shows on the caster |
