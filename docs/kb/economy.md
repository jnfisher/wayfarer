# Economy

## Currency
- Five coins. In copper: runic 1,000,000, platinum 10,000, gold 100, silver 10, copper 1.
  [C] observed (`Wealth:` in copper and coin counts agree)
- Coins have weight: **three coins of a type weigh one unit, each type counted on its own**
  (`runic/3 + platinum/3 + gold/3 + silver/3 + copper/3`, so up to two of each weigh nothing),
  added to the carried weight for encumbrance. [C] `LJNUTIL.C:8985 get_coin_weight`, `:9051`
  So heavy copper loads hurt. Exchange them up: buying and then selling a free piece of gear
  makes change in higher denominations (a sale runs `cleanup_currency`, `LJNUTIL.C:8445`, which
  changes coins up, at most 5,000 per step). [C]
- Converting to copper (`convert_currency`, `LJNUTIL.C:8525`) is 32-bit: its overflow guard
  only catches a product that wraps *below* the coin count, so absurd amounts (tens of millions
  of runic) come out wrong rather than saturating. [C]
- Paying (`deduct_currency`, `LJNUTIL.C:8621`) goes by value, largest coins first while a whole
  coin still fits what is owed; when nothing fits it breaks the smallest coin available (a silver
  into copper, else a gold into silver, platinum into gold, runic into platinum) and goes on. The
  broken coin is not listed in "You hand over". [C]
- Banks: shop type 7 (`SHOP_BANK`), in a room of type shop. Balances are copper in an unsigned
  32-bit field, one book per (user id, bank); `BANKBOOK` is also `BALANCE`. [C] `MAJORMUD.H:1479`,
  `LJNCMDB.C:5050`
  - `DEPOSIT n` / `WITHDRAW n`: `n` is copper; ten or more characters count as 0, `atol` otherwise.
    Order: not in a bank → `You cannot DEPOSIT/WITHDRAW if you are not in a bank!` (store and plain
    room alike); no argument (`DEPOSIT`: fewer than 2 words, `WITHDRAW`: not exactly 2) → `Syntax:
    DEPOSIT {Amount to deposit in copper}`; amount 0 → `Please specify a more reasonable amount.`
    [C] `LJNCMDB.C:4863-5003`
  - A book needs no opening: any lookup makes a blank one and it is written once changed. A deposit
    the purse cannot cover, and a withdrawal with no book or too little balance, print nothing, yet
    the room still gets `You see <name> making a deposit.` / `making a withdrawal.` (not to the
    player). Both commands end resting, meditating, sneaking and hiding. [C] `LJNBUFF.C:3935-4040`,
    `LJNUTIL.C:7026-7107`, `LJNCMDB.C:4904`
  - A deposit pays with `deduct_currency` (largest whole coins first) and prints `You deposit <coins>.`;
    a balance that would wrap past 4,294,967,295 is refused with `The bank cannot accept your deposit
    at this time.` and the coins stay. Withdrawals always come back as copper, then the purse is
    tidied upward (`cleanup_currency`, also after a refused deposit) and the character saved. [C]
    `LJNUTIL.C:7045-7061`, `:7098`
  - `BANKBOOK` in a bank (exactly one word, else `Syntax: BANKBOOK`) shows that bank; elsewhere it lists
    every book on file in bank order (at most 30), `BANKBOOK n` shows bank `n` (`(int)atol`, 16 bits).
    Two lines: `Your balance at <bank> (#<n>) is:` and `On deposit: <copper> copper farthings
    [<gold, with commas>.<copper left over, 2 wide> gold crowns]`. The format asks for `%02d`
    (`spr("%02d", ...)`, [C] `LJNDISP.C:5009-5014`). Under MBBSEmu the cents print SPACE-padded, so an
    empty book shows `[0. 0 gold crowns]` and a remainder below 10 shows as `. 5` [C, MBBSEmu run:
    silvermere#73 `m3/bank` differential, jnfisher/silvermere live run 38772934402, the `bank_balance` event diff of its m3/bank setup run]. This
    is an emulator quirk, not known original behaviour: MBBSEmu's `FormatPrintf` (v1.0-alpha-090826,
    `HostProcess/ExportedModules/ExportedModuleBase.cs`) parses the `0` flag into `LeftPadZero` and never
    reads it; zero padding there comes only from a precision (`%.2d`) [C, MBBSEmu source]. `spr` is an MBBS
    runtime export, not in the recovered source, so what the original runtime printed is [I]: `00`, as a
    Borland `sprintf` gives; cause unverified. A bank with no book shows 0; a shop that is not a bank, or no
    shop, shows nothing. The all-banks list has no heading line, only the balance pairs. [C]
    `LJNDISP.C:4985-5012`, `LJNBUFF.C:4119-4161`
  - `BORROW` and `REPAY` are empty stubs returning 0 (handled as unknown words; `borrow skiff` still
    works as an exit). The BBS-credit loan code is never called. `MAXIMUM_BANK_BALANCE` caps a book's
    balance when it is read from disk (0 = no limit). [C] `LJNCMDB.C:5011-5040`, `LJNBUFF.C:4038`

## Coins from monsters
- A monster's coins are rolled when it spawns: each coin type with a nonzero maximum in its
  record gets `lngrnd(0, max+1)`, so about half the maximum on average. [C] `LJNMONST.C:697-714`
- For money, rank areas by **average coins per monster hit point** (kill time scales with HP).
  From the stock data [C data, I ranking]: the Silvermere slums average about 45 copper on
  about 45 HP (≈1 copper/HP; half-ogre bodyguard 65c/60 HP but hits 36). The Darkwood keep's
  bandits (9:749 Forest Keep, 9:776 Keep Wall Path, 9:807 Hallway; Chaotic Evil, AC 20, hit 12)
  average about 110 copper on 55 HP (≈2 copper/HP) and give 125 exp, so a party of four L7s
  makes about twice the slums' money and exp per kill-time there. 135 steps on foot from the
  Town Gates; 9:725 Keep Wall and the cells/dungeon are dark (−100/−200).
- The richest single monster at low level, the Graveyard gravedigger (up to 10 gold, 15 silver,
  50 copper, about 6 gold average), is no money route: one alive at a time, back an hour after
  its kill (`maxalive` 1, `regenhrs` 1), and Neutral. [C data]

## Prices
- An item record has `cost` in the item's `currency` (offset 0x421: 0 copper, 1 silver, 2 gold,
  3 platinum, 4 runic). [C] `MAJORMUD.H:404-409`, `struct item`
- Buy price in copper = `cost × (100 + markup)/100 × (110 − Cha/5)/100`, each `/100` truncating
  on its own, the markup first (markup is at shop offset 0xD1). Cha 30 → 104%, 50 → 100%,
  85 → 93%: a 200-copper item costs 208 at Charm 30. Prices over 100,000 copper are worked in
  hundreds of copper, so their last two digits are 00. The Charm division is unsigned, which only
  matters for a Charm above 550. [C] `LJNUTIL.C:6081-6094 buy_item` (the Charm formula is :6091)
- Sell (and APPRAISE) price = cost in copper × (Cha/2 + 25)%: 40% at Cha 30, 50% at Cha 50.
  [C] `LJNUTIL.C:6323 sell_item`

## Shops
- Shop types: General, Weapons, Armour, Items, Spells, Hospital, Tavern, Bank, Training, Inn,
  Specific, Gang Shop, Deed Shop. Most real stores in the stock data are "Specific" (10).
  [C] `LJNTABLE.C`
- Each shop has up to 20 items with `maxqty`, `qty` (live stock), restock time, number and %.
  **A `maxqty` of 0 means the shop only resells what players sold it** ("Recycler" shops in the
  Junkyard), so ignore those items. **A live `qty` of 0 means out of stock**: "You cannot buy X
  here!" [C]
- Live stock is readable from the running server's database (`WCCSHOPS.db`). [C]
- LIST shows each slot in stock as `%-29.29s %-8d %5d %-8s`: the price is the item's cost plus
  the shop's markup **in the item's own currency, without Charm** (BUY then applies Charm), or
  `Free`. A shop with nothing in stock (a hospital, a trainer) prints nothing at all. LIST with
  a word after it is not the LIST command. [C] `LJNDISP.C:4204-4246`, `LJNCMDA.C:2228-2240`
- **`(Too powerful)` is only for scrolls**: `user_can_use` clears `toopowerful`, and only
  `user_can_use_spell` sets it, when a scroll's linked spell suits the class's magery but its
  level is above the character's. The item record's own `level` field is never read for this;
  an item's "Minimum level" ability shows `(You can't use)`. [C] `LJNUTIL.C:9432`,
  `:9734-9739`, `:9785`; seen on "scroll of mend" in the Thornwyn transcript.
- BUY name matching works on the words of the shop's goods (a typed word must start one of
  the item name's words): an exact name wins, one match is bought, several give "Please be
  more specific" with the **second** match listed first. A name the shop does not trade in is
  "`x` is not a known item." even if it exists elsewhere. [C] `LJNUTIL.C:5118-5185`, `:6116`
- BUY refusal codes: 0 unknown, 1 "You cannot buy X here!" (no stock; the limited-item "no free
  copy" case, which would also take one copy off the slot, cannot happen in the original,
  because `remove_logical_from_room` always returns 1, LJNUSE.C:788-845), 2 "You cannot afford X.", 4-7 the gang house deed
  rules (4 is unreachable: a deed owner gets 5). [C] `LJNUTIL.C:5783-5787`, `:6114-6139`
- SELL puts the copy back on the shelf only while `qty < maxqty` (so a `maxqty` 0 slot never
  takes it); the money is always copper, then changed up into the fewest coins. Gang shops buy
  nothing; a worn cursed item sells only when a second copy is carried. [C]
  `LJNUTIL.C:6247-6342`
- Hospital: `buy healing` costs 2 copper per missing hit point; `buy curing` (or `cure
  poison`) 25 silver, or 15 silver when you were not poisoned (charged anyway). [C]
  `LJNUTIL.C:5799-5874`. `buy curing` when not poisoned prints `You hand over <coins> and find that
  you were not poisoned!` (blue), and `You do not have sufficient funds to buy curing!` (red) when the
  15 silver is not there; a real cure prints `... and your poisoning is cured.` and ends every
  poisoning spell on you. [C] `LJNUTIL.C:5815-5874`
- Restocking: at start-up a slot with no restock time is topped up once; a timed slot's first
  restock comes `genrdn(1, max(2, regentime))` minutes later: 1 to `regentime` − 1 (the top
  is excluded), exactly 1 when `regentime` is under 2 (LJNRSTK.C:191). Then every 21st slow cycle (630 s) each due slot
  below its ceiling gains `regennum` copies when a 1-99 roll is below `regenpct`, and is next due
  a full `regentime` later, whether or not the roll succeeded. Gang shops never restock. A slot with
  `regentime` 0 is topped up once at start-up (same roll, same ceiling) and never again. A stock above
  its `maxqty` is cut back to it at start-up. Only shops 0-199 are set up at start-up
  (`NRESTOCK` 200, so a shop numbered 200 or more is never restocked), and the due-slot pass looks
  at only the first 1000 queued slots, newest first, so with more timed slots than that the first
  queued are never restocked (sic). [C] `LJNRSTK.C:145-280`, `:224-246`,
  `LJNBKGND.C:629-633`, `LJNBUFF.C:66`, `:579-586`

## Gang shops
Shop type 11 inside a gang house room; the house is the room's `ganghouse` number. [C]
- **Who may run it:** STOCK, UNSTOCK and MARKUP need a carried "Gang Shop Item (controller)" whose
  value equals the room's gang house. STOCK tests `item_has_ability` as well; UNSTOCK and MARKUP read
  the value only, so an empty pack slot reads 0 (what that does in house 0 is not checked). [C]
  `LJNCMDA.C:1626-1633`, `:1935-1940`, `:2144-2149`
- **Limits:** only the first 10 of the 20 slots are used, at most 20 copies in a slot, an asking price of
  0 to 9999 (clamped, `free` or `0` = free, the currency word after the price names the coin; with no
  price the item's own cost and currency), a markup of 0 to 1000 percent (anything not a number is 0).
  Limited items, loyal items, undroppable items and a worn cursed item with no second copy are refused
  (`You may not stock limited items!`, `You may not stock that item!`). In the record the price sits in
  `regentime`, its currency in `regennum`, and the stocker's user-id as a string in `maxqty`. [C]
  `LJNCMDA.C:103-106`, `:1555-1580`, `:1660-1800`, `:2155-2172`
- **Takings:** every sale pays the user-id stored in the shop into that player's bankbook at bank 8
  (`GANGBANK`). With no book, or a balance that would wrap, the money is lost; only a debugging sysop
  is told. A gang shop buys nothing from players. [C] `LJNPRMPT.C:39`, `:730-790`, `LJNCMDA.C:1806`
- `UNSTOCK ALL` drops every copy on the floor, and a copy that finds the floor full is lost (sic). [C]
  `LJNCMDA.C:1942-1975`

## Training fees
- `TRAIN` costs **level × 5 silver × (100 + trainer markup)%**, worked in whole silver (level 10
  at 25% markup is 62 silver, not 62.5). The level is the one you have before training. Keep it
  in reserve when shopping. [C] `LJNUTIL.C:8327-8328 train_level`

## Buying policy (what worked)
- Only buy **meaningful** upgrades: an armour score (AC + 3 × DR) gain of at least 15. Value a
  weapon's average damage at about 10× armour points. The copper ring (AC 2) wasn't worth 200
  copper. [C]
- **Save for the best item affordable within about twice your spendable money**, rather than
  buying many small gains. [C]
- Skip shields while wielding a two-handed weapon. Anti-Magic classes (Witchunter) can't use
  items with the Magical ability (28). Skip missile weapons, which need ammunition. Respect
  `classes` restrictions (item offset 0x324). [C]
- Afterwards, sell replaced gear if it's worth 50 copper or more; otherwise drop it. [C, John]

## Encumbrance
- Capacity y = Str × 48, plus 36 per point over 100 (Str 79 → 3,792), times (100 + Alter
  Encumbrance)%. [C] `LJNUTIL.C:4743`, `:9007` Percent z = (items + coin weight) × 100 / y,
  truncated; the word is None up to 16%, Light over 16, Medium over 33, Heavy over 66. On the
  `Encumbrance:` line of `i` the word is grey, green, brown and red respectively. [C]
  `LJNUTIL.C:9051`, `LJNDISP.C:4057-4064`
- The capacity read is the record's `maxwgt` (set from Str by the secondary stats), never 0;
  without a character it is 800. The percent is stored in the record (`encumpct`) whenever it is
  worked out. [C] `LJNUTIL.C:9007-9029`, `:9043-9056`
- `Encumbrance: x/y - None [z%]` in `i`. **Staying under 33% gives about +12 to-hit and +8
  dodge**, worth more than 20 CP of stats. [C] `notes/combat.md`

## Free starter gear (Newhaven)
- Weapon Shop: quarterstaff (2–12, two-handed, AC 10) and club (3–8). Armour Shop: the padded
  set (vest AC 60, pants, helm, gloves and boots AC 10 each). General Store: torch, and a
  lantern for 2 gold. [C]

## Carrying and wearing items
From the silvermere T-030 port of the item code (LJNUTIL, LJNCMDB, LJNSUPP, LJNFIND, LJNDISP).
- The pack has 100 slots, the key ring 50, the worn list 20. A worn or wielded item stays in its
  pack slot; **every copy of a worn item counts as worn**, so with one helm on, `wear helm` on a
  second copy says "You do not have helm left unequipped." [C] `LJNFIND.C:641-660`, `LJNCMDB.C:4846`
- Picking up fails with "You cannot carry that much!" when item + load + coin weight would pass
  capacity, or no slot is free. [C] `LJNUTIL.C:5345-5352`, `:5391-5395`
- `get <coin>` takes at most three coins per unit of free capacity; an amount must be 1 to
  99998. With no room left, `get 5 gold` reports "You picked up 0 gold crowns". [C]
  `LJNCMDB.C:843-847`, `:851`
- A room floor holds 17 item slots (15 more hidden). An uncharged item dropped where the same
  item already lies stacks on that slot; charged items (torches) take a slot each. A full floor
  refuses: "There is no room to drop X here." [C] `LJNUTIL.C:6811-6851`, `LJNCMDB.C:3497`
- `stash <item>` / `hide <item>` / `stash <n> <coin>` put things in the room's hidden lists;
  `get` finds a hidden item only after a search has revealed its slot. `take` does nothing in
  1.11p (no routine), and there are no containers. [C] `LJNCMDB.C:3727-3888`, `LJNUTIL.C:6928-7006`,
  `LJNCMDS.C:1092`
- A worn cursed item cannot be dropped, given or hidden unless a second copy is carried. [C]
  `LJNCMDB.C:3452-3471`
- `equip` alone lists the inventory (it does not wear everything); `wear`, `wield`, `ready` and
  `arm` with a name all run EQUIP, which readies a weapon and wears anything else. [C]
  `LJNCMDS.C:502-536`, `LJNCMDB.C:4822-4845`
- Rings and bracelets share two slots each; a third ring takes one off. Anything worn "Worn"
  clashes with every new item. A cursed item in the way stops the new one. [C]
  `LJNUTIL.C:6501-6608`
- No off-hand item with a two-handed weapon readied, and no two-handed weapon with an off-hand
  item worn. [C] `LJNUTIL.C:6609-6621`, `LJNSUPP.C:2368-2385`
- Readying prints "This weapon feels heavy in your hands." when the weapon's Str requirement is
  above your Str, else "You are extremely quick and deadly with this weapon." when a normal blow
  would cost under 200 energy at 66% encumbrance or less (the fast-blow crit bonus). [C]
  `LJNSUPP.C:2417-2424`, `LJNCOMBT.C:891-904`
- Who may use an item: the class and race lists (a list naming anyone must name you; being named
  overrides weapon and armour type), Min/Max Level *abilities* (the item's level field is not
  checked), Anti Magic against Magical, the alignment abilities by legal level, the class's
  weapon type and armour weight (one-handed classes get no off-hand items), Class Item
  Inclusion. A "Limited" weapon class may use only items 100 (quarterstaff) and 68 (the third
  exempt number is 0, which names no item). [C]
  `LJNUTIL.C:9418-9740`, `LJNGAME.C:1030-1032`
- Only one light may be lit ("You already have something lit!"); `i` shows it as
  `torch (Readied/<uses/10>)`. A burnt-out light needs recharging before it can be lit again.
  [C] `LJNCMDB.C:4010-4013`, `:4042-4047`, `LJNDISP.C:3978`
- `keys` lists each kind once, newest kind first, a repeat as ` 2 iron keys` (an `s` added to
  any name) with a blank before every entry. [C] `LJNDISP.C:4495-4605`
- `give <n> <coin> to <player>` gives only what the receiver can carry (three coins per unit of
  free capacity); a player with receive off refuses every offer. [C] `LJNCMDB.C:4415-4476`

## Limited items
- An item with a `limit` keeps a count of copies in play (`item.active`) and one ownership
  record per copy (player, room or monster). At the limit, picking one up fails with "A strange
  force stops you from getting this item." Dropping moves the copy to the room, so another
  player can then take it. [C] `LJNUTIL.C:5336-5339`, `LJNUSE.C:730-1000`
- A limited copy sold to a shop stays in play: it is recorded as lying in the shop's room, and
  when every allowed copy is already in play the shop gives it up off the shelf. Buying it
  takes that record back; a buyer who cannot pay gets the record put back. The shop's own
  stock and restocks are not recorded (only the database reindex records shelf copies). A
  copy dropped on the shop's floor has the same kind of record as a shelf copy. [C]
  `LJNUTIL.C:6095-6100`, `:6355-6368`, `LJNRSTK.C:259-268`, `LJNUPDT.C:2192-2233`

