---
id: C-070
title: "KB: item and drop sources checked against the game data"
tier: C
size: M
wave: 13
depends: []
crate: none (docs)
touches:
  - docs/kb/classes/*.md (gear tables and quest notes)
  - docs/kb/progression.md, docs/kb/grouping.md, docs/kb/recovery.md, as needed
  - docs/kb/README.md, docs/kb/changelog.md
---

## Goal

Every hand-written item source in `docs/kb/` (where an item drops, is placed, sold or given)
agrees with the stock 1.11p game data. The Missionary's level-20 kit names the best limited items
it was missing. The KB says what a shop's "resell-only" stock means, so the mistake isn't made
again.

## Inputs

- **The lookup tool:** mmud-exploration's `test/play/kbq.py` (merged 2026-11-02 as KQ-001).
  `kbq item NAME` lists every source of an item:
  - placed, lying and hidden room items;
  - monster drops with odds, spawn room, limit and respawn;
  - shops, split into stocked and resell-only;
  - `giveitem` text chains with their odds;
  - item and spell chains such as the vault chest.

  `kbq monster`, `room`, `shop` and `items` (filters) cover the rest. Use it to check claims;
  it is not a dependency of this repository. Add mmud-exploration to the session read-only to
  run it.
- **The contradictions KQ-001 found** (its PR, mmud-exploration #41):
  - **Holy medallion** (`classes/missionary.md`): listed as Recycler-only. The guardian golem
    drops it at 5% (1:1797, limited 1, respawn 100 h). `classes/priest.md` has this right.
  - **Large silvery cross:** both shops listed are resell-only. Its source is the large chest
    in the Royal Treasure Vault (8:707): four 25% rolls, about 68% for at least one.
  - **Darkbane:** Arlysia only resells it. It comes from the level-50 class 1/3/6 quest.
  - **Faith keeper** (Missionary) and **lyrist's companion** (Bard): Recycler 6 only resells
    them. They come from the class quests (Chancellor Annora 1:1333, The Grey Lord 1:160,
    Balthazar 2:2577).
  - **Missionary level-20 kit:** misses the **pearl ring** (placed in Inside The Broken Willow
    3:214, level 15, limit 3, SC +10, mana +10) and the **mithril-runed tunic** (the vault
    chest, 25% per roll).
  - **Azure shamshir:** "not sold" is right, but its sources are a class-6 level-40 quest and
    a 1% feline prisoner drop.

## Behaviour (what the KB says when done)

- **Each claim listed above is fixed,** tagged `[C data]`.
- **A sweep of every class page's gear tables and the quest notes.** For each item named with
  a source, run `kbq item` and fix what disagrees. Add sources the data has that the page
  lacks, where they matter for that class.
  - The usual mistakes: Recycler and resell-only shops given as sources, drops missed, placed
    items missed.
  - When a page's choice of item changes because of a fixed source, say so on the page.
- **The Missionary level-20 kit** names the guardian golem's five items. It says a never-killed
  limited monster carries all its items, so on a fresh realm the first kill drops all five.
  Add the pearl ring and Thrag's golden belt. The mithril-runed tunic is a stretch target from
  the vault chest.
- **`README.md` gains a short "Item sources" note:**
  - a shop whose stock has `maxqty` 0 only resells what players sold it;
  - a room's minimum and maximum level are the level range of its spawns, not an entry gate;
  - item sources in this KB were checked against the stock data on 2026-11-02.
- **`changelog.md`:** one line per file changed, or one line listing them.

## Design notes

Docs only: no code, fixtures or tests change. This repository is public, so don't copy data
dumps into it. Write each fix as KB prose or table rows, as the pages already are.

## Done when

- Every claim in Inputs is fixed.
- Each class page's gear table has been checked against `kbq`. The PR lists, per page, the
  rows changed, and any rows left as they were with the reason.
- `README.md` and `changelog.md` are updated.
- CI is green (the docs and links checks).

## Tests

None beyond CI's docs checks.

## Notes for the agent

- **Quicksilver 1.20 content** isn't in `kbq` yet. Where a 1.20 page (`quicksilver-1.20.md`)
  names an item source, leave it alone and note it in the PR.
- **Odds:** when a text's `random` clause appears K times on one line, each run rolls again.
  Give "per roll" odds and "at least one" odds as kbq prints them.
