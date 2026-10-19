# Ninja (WCCCLASS 7): progression strategy

Scope: MajorMUD 1.11p, stock "Realm of Legends" data, MBBSEmu. Tags: **[C]** source/data
(file:line under `WCCMMUD-1.11p-source/SRC/`, or record field), **[I]** inferred/modelled,
**[F]** forum/web. Monster stats `hp/exp/maxhit`. Prices include markup, not Charm. `1p` = 100 gold.
Steps from Town Square 1:224. Shared: `test/play/notes/combat.md`, `test/play/corpus/leveling.md`.
This file holds the **stealth/backstab reference** that `thief.md`, `mystic.md` and `ranger.md` point to.

**One-line path:** Dark-Elf, Agi-first. Sneak, **backstab** the opener with a 1H blade (one-shots
most L1-15 monsters), then finish with ATTACK; a 25 Dodge class base plus Agi makes you very hard to
hit in light padded gear. Arena → Graveyard → Slimy Sewers/Crypt → L10 crypt quest (ebony ninjato) →
kobolds/bandits → wolves → Ancestral Tomb. Perfect Stealth at L20.

## 1. Class facts [C: WCCCLASS rec 7]
| Field | Value | Meaning |
|---|---|---|
| exp | **+85%** | L2 = 10×(185+race) (Dark-Elf 2,300; Goblin 2,200) |
| basehp / hplevel | 4 / 4 | +4 HP/level + ~2.5 random: fragile |
| combat | 5 | Normal-attack AV ~10 below a Warrior at L10 |
| magery | none | |
| weapons | 8 = All | 2H allowed, but **backstab needs a 1H weapon** for repeats (LJNACMB.C:486-498) |
| armour | **2 = Padded** | Plus anything that lists class 7 (ninja robes/armour, tabi, mask, kote…) — a class-named item bypasses the type check (LJNUTIL.C:9550) |
| abilities | 31 Bash, **34 Dodge 25**, **58 Crit +10**, 103 Class Stealth, 37 Picklocks, 40 Find Traps, 38 Tracking | |
| Perfect Stealth | keepable from **L20** | sneak never fails (LJNCMDA.C:2900, LJNUTIL.C:705) |
| Alignment-quest reward | backstab min/max +10, stealth +2 | LJNUTIL.C:504-510 |
| L11 trainer | "Super Rogue Trainer", shop 57 at **1:2273** | crypt quest (§7) |
| L1-10 class shop | **Ninja supply shop, 1:2308** (up from Ninja Training Room 1:200; 15 steps; Block Guard exit at 1:197→1:198 [I: guild gate]) | |

### Stealth and backstab reference [C]
| Quantity | Formula | Source |
|---|---|---|
| Stealth | `Agi/4 + min(L,15)×2 + max(0,L−15) + Cha/6 + Int/8 + 20`; **+10 with class AND race stealth**, −15 with race stealth only; + "Stealth" item/spell abilities | LJNUTIL.C:4760-4800 |
| Sneak chance | `stealth` (−5 at >33% load, −10 at >66%; ×2/3 if spotted), **−1 per other player and per monster in the room**, cap 95. Perfect Stealth = 100%. Failed sneaks look like success unless a perception roll catches it ("You don't think you're sneaking.") | LJNSUPP.C:1483; LJNCMDA.C:2863 |
| Monsters vs a sneaker | A sneaking player is **not seen** by monsters (no aggro, no follow) unless the monster has **See Hidden (57)** and you did not just arrive | LJNCMDB.C:5195 `monster_can_see_user`; LJNBKGND.C:1445 |
| Backstab allowed | `bs <mob>` / `backstab <mob>` while hidden or sneaking. Weapon must carry ability 116 (most 1H weapons do, even with a negative value); else it is a normal blow but still costs a full round. **Unarmed backstab is allowed** (Mystic). Repeats from autocombat only while still hidden: in practice **one backstab per fight, as the opener** | LJNCMDA.C:484-540; LJNACMB.C:486-510 |
| Backstab AV | `(stealth+Agi)/2` **+5 if class+race stealth** (−15 race only) `+ weapon/item BS-AV/2 + avbonus`; −10 if spotted | LJNCOMBT.C:719-731 |
| Monster DV vs backstab | `(ac/2 + 2×bsdefense)/2` | LJNCOMBT.C:1128 |
| Hit chance | **AV − DV** directly (clamp 10-99); target's dodge chance ÷5 | LJNCOMBT.C:1316, 1370 |
| Damage | `(2×wmin + 2L + stealth/10 + BSmin) .. (2×wmax + 2L + stealth/10 + BSmax)`, ×75% if race-stealth only, then **×(100+L)%**. wmax includes Str `(Str−50)/10`. No crits. Energy = full round, struck before everyone else | LJNCOMBT.C:797-816; LJNACMB.C:1144 |
| Picklocks | `(Agi+Int+10×L')×2/7` + Picklocks abilities; L' = L to 15, then 15+(L−15)/2. Pick chance = picklocks + exit bonus | LJNUTIL.C:4865; LJNCMDA.C:2960+ |
- Most early monsters have **bsdefense 0** (rats → kobold warriors → grey wolves → barrow wights). Exceptions: shade 20, dark priest 20, mummy 20, tentacled abomination 25, harpy 25, **moaning spirit 60, banshee 50**, gnoll scout/axeman 15, dark monk/mystic 15 [C WCCKNMSR 0x6E].
- See-Hidden monsters to avoid while sneaking: spectral mage, gorgon (Dragon's Teeth) [C monster abilities].
- Locked doors you can pick instead of keying (exit bonus): bone-key door 1:1309 n (0), copper-key door 1:2271 w (−20), black serpent doors 2:176 s (−80), black star door 1:1224 e (−99) [C exitpar3]. The last two need picklocks ~100+ (≈ L25 with +15 from items).

Worked example [I]: Dark-Elf Ninja L10, Agi 85 Cha 46 Int 55 Str 60, ebony ninjato (5-18, BS+10, stealth+5) + robes/tabi/mask (+5): stealth ≈ 94, BS AV ≈ 99. Kobold warrior (ac35, bsdef0): DV 8 → **91% hit**. Damage (10+20+9 .. 38+20+9)×1.10 = **42-73** → one-shots everything ≤ 50 hp, 2/3 of a wolf.

### Defence (why padded works) [C LJNCOMBT.C:410-418, 1378-1404]
Dodge = 25 + (Cha−50)/5 + L/5 + (Agi−50)/3 (+ up to 10 under 33% load). Monster's blow is dodged with `dodge×10/(AV/8)`%, cap 95. Dark-Elf L10 Agi 85, unencumbered: dodge ≈ 47 → **~50% of landed blows dodged** by an AV-75 monster. Keep encumbrance < 33% always.

## 2. Race choice [C WCCRACE / I ranking]
| Rank | Race | exp (total) | Agi | Str | Hea | Extras | Why |
|---|---|---|---|---|---|---|---|
| **1** | **Dark-Elf** | +45 (230%) | 50-120 | 40-90 | 30-90 | **Race stealth** (+10 stealth, +5 BS AV), **night vision 80**, Crit +1, 3rd-AV +3 | Best stealth+vision combo; Int 50-120 also feeds stealth (/8) and crit base (/10). Sees in −230 rooms. |
| 2 | Goblin | +35 (220%) | **55-125** | 30-70 | 40-100 | Race stealth, NV 65 | Cheaper exp, higher Agi cap, better Hea; Str 70 cap hurts normal-attack damage. |
| 3 | Nekojin | +50 (235%) | **60-130** | 40-100 | 30-80 | Race stealth, NV 50, Tracking, fire def +10 | Highest Agi/Cha start (dodge, stealth); Hea cap 80 = lowest HP. |
| alt | Halfling | +25 | 60-150 | 20-60 | 40-100 | race stealth, Dodge 10, **−1 HP/level** | Great backstabber, 3 HP/level is too fragile for a padded class. |
Human/Half-Orc (no race stealth, no NV) lose 10 stealth and 5 BS AV and need a light.

## 3. Stats / CP plan [I, breakpoints C]
Start (100 CP): **Dark-Elf Str 58, Int 50, Wil 30, Agi 75, Hea 46, Cha 46 (99)**; Goblin Str 55, Int 45, Wil 40, Agi 78, Hea 50, Cha 46 (100).
Agi is the ninja's Str: it feeds stealth (/4), BS AV (/2 directly + /8 via stealth), dodge (/3), AV (/6), energy and crit base (/20). Breakpoints: dodge every 3 Agi above 50, stealth every 4.
1. Cha to 46-50 (removes the −4…−1 dodge penalty; Cha/6 stealth). 2. **Agi** up every level (3-4 CP/pt at 70-90 above min… budget it). 3. Str 60 / 70 (+1 max dmg each, which counts ×2 in backstab). 4. Hea 50-60. 5. Int 60 (stealth /8, crit base). Wil: none.
- Max HP Dark-Elf Hea 50: ~90 at L10, ~160 at L20. Treat fights you can't open with a backstab as optional.

## 4. Attack style [C §1, combat.md §2]
1. `sneak`, walk in (unseen), `bs <mob>` — the opener lands first in the round.
2. Then **ATTACK** (the autocombat entry falls back to normal blows). With Dodge 25 + crit 10 the Ninja's sustained attack is crit-driven; BASH forbids crits. Fast-weapon bonus (+up to 20% crit) applies once the blade's cost is < 200 (about L18-20 with a ninjato at Agi 90) [C LJNCOMBT.C:887].
3. BASH only pre-L10 with a slow weapon vs low-AC mobs, or never.
4. After the kill, `sneak` again before the next room (combat ends sneaking). Rest while sneaking/hidden in a cleared room.
- Group fights: avoid rooms with 2+ monsters before L10 (sneak −1 per monster; the second one gets free swings).

## 5. Gear by level band [C; Ninja supply shop 1:2308 mk100 unless noted]
| Band | Weapon (1H for backstab) | Armour (type ≤ 2 or class 7) | Notes |
|---|---|---|---|
| L1-3 | club 3-8 (free); dagger 1-5 BS+10 2g / **shortsword 2-9 BS+5 12g** (1:355) | padded set (free 1:2142) | |
| L3-10 | **ninjato 3-13** 2p (cls 7) or broadsword 3-13 26g (1:355); **ancient sword 4-19 BS+10, wt 0** (spectral mage 1:382, 25%) | **black ninja robes ac7 st+2 50g**, hooded mask ac1 st+1 20g, black tabi ac1 st+2 40g; skullcap ac2 6g (1:2154); travelers cloak 4g (1:312) | Ancient sword: best pre-L10 backstab blade in the data. |
| L10 quest | **ebony ninjato 5-18**, 1H sharp, s40, acc 6, BS+10, Stealth +5, Magical 2, Ninja-only, limit 1 | | |
| L10-20 | serrated battle axe 5-16 4p (1:307), black scimitar 3-16 1.4p (Slum 1:2324); katana 7-17 crit+2 (master assassin 7:1283, 2%) | **ninja battle armour ac11 dr1.1 wt320 10p**; golden armbands 2.4p (1:290); **adamantite bracers ac2 dr0.5 L15 70p** (6:1337); iron crown ac2 (kobold king 100%); elven cloak st+5 L15 62.5p (3:541); **shadow cloak** st+10 BS+15 L15 150p (Rhudaur Thieves' Guild 2:2561 / Dreary Shop 7:1136); golden snake earrings BS+5 6p (1:290); fingerbone bracelet BS+5 11p (Slum 1:2324) | BS-AV items count ½; Stealth items count fully and ×½ again in BS AV. |
| L20-30 | sai 5-16 2H BS+10 L20 2p (not for repeats); **adamantite ninjato 7-21** BS+10 crit+3 L25 (Ebon Blade assassin, 1%); javelin 7-22 L25 (Lost City) | kote (arms) ac1.8 st+5 L20 10p (16:318); gunsen off-hand ac2.5 L20 14p (16:318); midnight sash st+5 crit+2 L25 20p; black silk robes ac9 dodge+5 L20 (Jorah 1:3303) | |
| L35-50 | Ninja-only: black dragon hood/tabi/robes (Shadow Master 17:581/807/811), dragonfang ninjato 13-35 L50, ruby ninjato 10-25 L40, death's shadow (back, BS+25 L40) | | |
Utility: smoke bomb (cls 7, L8) 50g at 1:2308 [effect: spell 305, not traced]; lantern only if not Dark-Elf/Goblin.

## 6. Grinding by level band [C corpus leveling.md / I fit]
Stealth fit = single-spawn rooms, bsdef 0, no See Hidden, hits you can survive if the backstab misses.
| Levels | Area (anchor) | Monsters (bsdef) | Access | Ninja notes |
|---|---|---|---|---|
| 1-3 | Newhaven Arena 1:2150 | ≤ 25 hp (0) | maxlevel 3 | Backstab one-shots everything |
| 3-6 | Graveyard 1:817 | skeleton 43/55/11, zombie 62/70/14 (0) | none | Gravediggers 130/1500/30 (Neutral, 1:824/830/2618/2635/2640): sneak-backstab opener ~50 then finish — **best early exp** |
| 5-10 | **Slimy Sewers 9:1** | slimes 30-55 hp (0) | `search w` 1:619; light −200 (NV) | One backstab per slime |
| 6-10 | Crypt L1-3 1:835 | ghoul (0), wight (0), shade (20) | hidden exits; L10 quest route | |
| 6-10 | Strange Mansion 1:63 | vision/spectre/astral shade (0) | 31 steps | **Spectral mage has See Hidden** — fight it head-on or skip; it drops the ancient sword |
| 10-15 | **Kobold caves 6:752/760** | kobold 30/80/9, kobold warrior 50/150/13 (0) | 186 steps | One-shot kills; king 250/2250/16 (6:834) |
| 10-15 | Smuggler Den 9:161; Darkwood bandits 9:720 | thief/smuggler 55-70 hp (0); bandit leader 100/275/18 | hidden `d` at Docks | Cash for the 10p armour |
| 15-20 | **Frozen Caverns 10:35** | grey wolf 125/375/21, guardian 145/500/22 (0) | `search n` 10:3; light −175 | BS ~60-80 + 3 rounds |
| 15-20 | Rhudaur Dark Alleys 2:1808 | cutpurse 110/275/15, depraved murderer 130/375/15 (0) | black serpent key (dark monk 20%), light −200 | Hits ≤ 15: ideal for low-HP sneaks |
| 20-30 | **Ancestral Tomb 6:872** | tortured spirit 150/525/15, barrow wight 180/725/15 (0, ac 50-60 → BS DV 12-15) | hidden `w`, light −200 | Normal attacks miss ac60; backstab ignores most of it |
| 20-30 | Shadow Fist Temple 9:913 | dark monk 125/525/30 (15, dodge 15), dark mystic 150/775/36 | black serpent key; t51: non-hostile to evil chars [F] | |
| 30+ | Quaggoths 6:1811, Black Cave 8:1 | see corpus | | |
Skip: Orc Mansion/Labyrinth (hit 70), gnoll axeman (48), Ancient Crypt (moaning spirit bsdef 60, skeletal warrior hit 60).

## 7. Class quest: L10 crypt → ebony ninjato + L11 [C rooms/exits]
From 1:1309 (mummy 100/200/20, bsdef 20; bone key 100%, or `picklock n`): `use bone key n` → 1:1377 → `n` ×4 → 1:1422 → **`w`** (Class 7) → 1:1429 → `w` → 1:2263 → `w`,`sw`,`w` → 1:2266 → **`w` is a locked door** (bonus −50): `picklock w` (chance picklocks−50: ~15% at L10, retry) or `bash w` (Str−50 %, 25% chance of 1-4 self-damage) → 1:2267 → `n` → 1:2268 → **`n` Trap exit (50)**: `search n` / `disarm trap n` first (Find/Disarm Traps) → 1:2269 → `w` → 1:2270 → **`w` Trap (70)**: disarm → 1:2271 owlbear **93/225/28** (copper key 100%) → `use copper key w` (or pick, −20) → Marble Chamber 1:2272 → **`ne`** (Class 7) → 1:2276 → `ne` → **Large Tomb 1:2273**: ninja spirit, ebony ninjato placed (limit 1). `train` (shop 57, mk 500), `go portal` → 1:834.
- Trap semantics (damage vs throw) not traced; the exit params are 50 and 70 [C exit data / I effect].

## Group role
Scout and opener: a sneaking follower stays sneaking through party moves and isn't targeted by monsters without See Hidden, so it backstabs on arrival, then melees. Padded armour: don't stand visible in big pulls. See `kb/grouping.md`.
