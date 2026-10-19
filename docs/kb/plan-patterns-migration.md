# Plan: move the bot's screen parsing onto the pattern registry

**Goal:** `test/play/progress.py`, and its small cousin `grind.py`, stop carrying their own
regexes. Every screen read goes through `test/play/patterns.py`, which turns text into ordered
`Event`s with named fields. The catalogue behind it is `screen-patterns.md`.

**Status:** plan only. `progress.py`, `bridge.py` and `mmud_data.py` were not touched, because
two bots are running on them. Line numbers are from 2026-10-13 and will drift; search for the
quoted code.

## Why bother

The audit in screen-patterns.md section 6 found these misreads. Each is fixed by the event noted
next to it.

- Titles glued behind a mana or resting prompt are lost: `prompt` peeling plus `room_title`.
- Trap doors up and down aren't seen as exits: `parse_exits`.
- Real death text is never matched: `you_killed` / `drops_unconscious` / `lives_left`.
- `ATTACKED_RE` gives false positives and misses some attacks: the `mon_*_you` events.
- "is no longer lit!" is never noticed: `light_out` / `item_destroyed`.
- Kai is ignored in `st`: `st_mana` with `mtype`.
- `cleanup()` never drops junk a shop refuses: `sell_refused`.
- Most move refusals are missed: the movement refusal events.
- `record_attack()` counts non-attacks: `you_hit` / `you_miss` / `you_dodged` / `you_glance` /
  `spell_cast_you` / `cast_fail`.
- `here()` drops capitalised monsters: `parse_also_here` plus the monster list.

The statline can also be configured to forms the bot can't read (screen-patterns.md section 2).

## Use sites and their replacements

`ev(name)` below means "the events of that name in the classified response". `reg` is one
`patterns.Registry.load(world=self.w)` built at start-up. It takes about 0.5 s and shares the
World already loaded.

### Constants (progress.py:30-35)

| Current | Replacement | Behaviour change |
|---|---|---|
| `HP_RE` `\[HP=(-?\d+)` | `prompt` event, `fields["hp"]` (via `PromptParser`) | Works for std, custom and generic `[...hp=...]:` prompts. Never matches room text. |
| `MA_RE` | `prompt.fields["mana"]` (`mtype` MA/KAI) | Also custom `%m`. |
| `NOMANA_RE` | `no_mana` (`what` = mana/kai) | none |
| `ATTACKED_RE` | any of `mon_hits_you`, `mon_miss_you`, `mon_dodge_you`, `mon_deflect_you`, `attacks_you`, `spell_cast_on_you` (hostile caster only), `combat_engaged` | No more false hits on `... creeps in after you!` (now `monster_follows`) or friendly heals. Catches `X stabs you for N damage!`. |
| `COMBAT_END_RE` | `combat_off` | none |
| `DARK_RE` | `room_dark` (and `blind`) | `barely visible` / `dimly lit` are `room_light`, not dark. Same as today, since DARK_RE didn't match them either. |

### Bridge plumbing

| Site | Change |
|---|---|
| `Bot._seen()` progress.py:79-98 | Classify `r["text"]` once per response, store `self.events`, and update HP, mana and state from the **last** `prompt` event. Raise `Died` on `you_killed`, or on `drops_unconscious` with `who == self.firstname`. Also accept HP ≤ 0 from the prompt, as now. |
| `bridge.Session.collect()` bridge.py:140-162 | (Later, when the bots can be restarted.) Return the ANSI text too, as `"ansi": new` next to `"text"`. This is backwards compatible. `raw_to_lines()` then splits on CLRLINE, which is more accurate than peeling prompts out of plain text. Until then, classify the plain text: the registry peels glued prompts. |
| `bridge.plain()` bridge.py:57 | Keep it for the transcript. The registry doesn't depend on it. |
| `until=` regexes, e.g. `until=r"\[HP=-?\d+\|\*Combat Off\*"` (progress.py ~447) and `until=ATTACKED_RE.pattern` (~763, ~1323) | `until` is matched in the bridge against plain text, so the strings stay. Generate them from the registry instead: `patterns.STD_PROMPT`'s HP part, and `reg.byname["combat_off"].rx.pattern`. For "attacked", the simplest robust choice is "until the next prompt", then classify. |
| Echo | Pass the command we send: `reg.classify(t, sent=[cmd])`. The echo then becomes an `echo` event instead of text. |

### Login (progress.py:100-121, `enter_realm`)

| Current | Replacement |
|---|---|
| `Username:\s*$` | `bbs_username` |
| `Make your selection` | `bbs_menu_prompt` |
| `\[MAJORMUD\]:\s*$` | `mud_menu_prompt` |
| `press enter\|more\?\|\(N\)onstop\|continue` | `bbs_more` [I]. Drop "continue", which matched any text containing the word. |
| `HP_RE` on the last line | the last event is `prompt` |
| **New:** right after entering, send `set` and read `settings_statline`. If it isn't `Full`, send `set statline full`, or set the custom template and give it to `PromptParser`. | |

### Room reading

| Current | Replacement | Behaviour change |
|---|---|---|
| `room_name()` progress.py:124-135 | last `room_title` event before the last `room_exits` | Titles behind any prompt form are found. |
| `observe()` progress.py:137-155 (exit words filtered by `md.DIRS`) | `room_title` plus `parse_exits(ev("room_exits").fields["exits"])` | `trap door above/below` become up/down. Secret passages and custom hidden-exit texts become exits too (kind `secret`/`custom`); `fits()` should count them only against Hidden exits. `NONE!!!` gives an empty set. |
| `update()` refusal check progress.py:170-171 | any event in a `MOVE_REFUSED` set: `no_exit`, `door_closed`, `door_closed_drag`, `need_spell`, `need_item`, `toll_short`, `not_permitted`, `exit_forbidden`, `exit_level_low`, `exit_level_high`, `exit_alignment`, `exit_combat`, `exit_unhealthy`, `exit_timed`, `exit_purchase`, `cant_move`, `stunned`, `too_heavy` | More refusals are recognised, so the bot doesn't "follow" an exit it never took. |
| `here()` progress.py:211-214 | `parse_also_here(ev("room_also_here").fields["who"])`. Monsters are entries whose name resolves through `split_monster()` against the monster names. Players are capitalised names that don't. | Named NPC monsters are kept. `*`, `(Hidden)` and ` (Charmed)` are stripped (a charmed monster should be skipped). |
| `where()` progress.py:290-297, `DARK_RE` | `room_dark` / `blind` → `check_light()` | Same, and needs the statline FULL guarantee from login. |
| `step()` progress.py:313 `(door\|gate) is closed` | `door_closed` (`what` = door/gate) | none |
| `step()` progress.py:320, 326 | `room_dark`; attacked as above | as above |
| `flee()` progress.py:712 | `door_closed` / `door_locked` | none |
| `_raw_step()` progress.py:723 | `room_title` present | none |
| `recover_death_pile()` progress.py:680 `You notice (.*?) here\.` | `room_items.fields["items"]`, split on ", ". Coin entries: `parse_carrying`'s coin regex. | Wrapped lines are joined by the registry. |
| progress.py:685 `you (took\|picked up\|get)` | `took_item` / `picked_up_coins` | none |

### Combat

| Current | Replacement | Behaviour change |
|---|---|---|
| `monsters()` / `fight_all()` attacked tests progress.py:234, 415, 453 | attacked events (above) | as above |
| progress.py:433 `no effect\|You can't attack\|not here\|don't see` | `no_effect_weapon`, `wrong_weapon`, `target_missing`, `evil_warning`, `too_evil`, `lawful_block`, `guilt` | `evil_warning` now marks the name as "good-aligned, skip" instead of "immune". |
| progress.py:440-441 NOMANA strip | `no_mana` | none |
| progress.py:457 kills `You gain \d+ experience` | count `exp_gain`, and sum `fields["exp"]` too | none |
| `record_attack()` progress.py:563-565 | actions = `you_hit` + `you_miss` + `you_dodged` + `you_glance` + `spell_cast_you` + `cast_fail` + `cast_resisted` + `no_effect_on`; damage = the sum of `dmg` from `you_hit` + `spell_cast_you` | Fixes counting `You took N damage`, `You have been killed!` and similar as attacks. **The stored attack stats (`ATTACKS` json) shift slightly. Reset them or tag them with a version.** |
| `loot()` progress.py:648-650 | `coins_drop.fields["coin"]` → `get <coin>` → `picked_up_coins` | Also handles the sysop-named currency (`runic coin`). |
| `rest()` progress.py:763-776 (prompt `(Resting)` check) | `prompt.fields["state"] == "Resting"` | Works for both standard layouts and custom `%r`. |
| `check_light()` progress.py:785-793 | burn-out: `light_out` / `item_destroyed` (this item) / `not_lit`. Lit: `light_ok`, or `light_fail` with "already have something lit". | Catches "is no longer lit!". |

### Status, inventory, shops

| Current | Replacement | Behaviour change |
|---|---|---|
| `status()` progress.py:255-280 | `st_name` (lives, cp), `st_race` (race, exp), `st_class` (cls, level), `st_hits` (hp, maxhp, ac, dr), `st_mana` (mtype, mana, maxmana), `st_str`/`st_int`/`st_wil`, then `exp` (exp, level, left, total, pct) | Kai is read. Multi-word race and class names (`Gaunt One`, `Half-Ogre`) are kept whole. |
| `read_spellbook()` progress.py:527 | `spell_row` events (after `spells_columns`): level, mana, short, name | Row parsing is limited to the table. |
| `carried_items()` progress.py:846-866, `inventory()` 874-879 | `parse_carrying(ev("inv_carrying").fields["items"])` gives coins plus `(name, slot, count)`; `inv_wealth.fields["copper"]` | Slot names are exactly `wornlocs[]` plus `Two handed`, `Weapon Hand` and `Readied/N`. The normalising code goes. |
| `carried_unused()` progress.py:1072-1080 | the same parse, with `slot is None` | none |
| `equip_carried()` progress.py:986, 991 `now holding` / `now wearing` | `wield_ok` / `wear_ok`. Failures: `weapon_forbidden`, `offhand_2h`, `wear_fail`, `not_have_any`, `weapon_heavy` (a warning, not a failure). | The failure reason becomes available. |
| `ensure_light()`, `learn_spells()`, `shop()`: `"bought" in t` progress.py:836, 1031, 1058 | `bought` (item, price) or a failure: `buy_not_here`, `buy_poor`, `buy_unknown`, `not_in_shop` | `nobuy` gets a reason. |
| `learn_spells()` progress.py:1035 `learn the spell` | `learn_scroll` / `already_known` | none |
| `heal_at_healer()` progress.py:750 | `healed_paid` / `heal_poor` / `healer_done` | It stops on "no funds" instead of on any text without "heal". |
| `cleanup()` progress.py:1093-1096 | `sold` → done; `sell_refused` / `not_in_shop` → drop it | Fixes the never-drop bug (both branches `continue`). |
| `train()` (after progress.py:1100) | `trained` (level), `train_row`, and failures `train_no_exp`, `train_no_money`, `train_wrong_room`, `train_level_band` | none |
| `try_goto()` progress.py:350-354 | `you_say` with `msg` starting "sysop" → no key. `no_autocombat` / `exit_combat` → refused. | none |
| `open_way()` progress.py:366-371 | `door_locked` → bash. Success: `bash_ok` / `bash_through` / `door_already_open`. Retry: `must_wait`, `bash_fail`. | none |

### grind.py

`grind.py` is the older single-purpose loop. The same replacements apply:

| Site | Replacement |
|---|---|
| `HP_RE` :22 | `prompt` |
| `Also here` :52 | `room_also_here` |
| `lunges\|attacks\|into the room` :93 | attacked events plus `monster_arrives` |
| `Combat Off\|You gain` :109 | `combat_off`, `exp_gain` |
| death :121 | `you_killed` |
| coins :132-134 | `coins_drop`, `picked_up_coins` |
| `Exp:` :139 | `exp` |

Or retire `grind.py` if `progress.py` covers it.

## Order of work

1. **Freeze a replay corpus.** Copy the current `logs/*/transcript.txt` and `raw.bin` into
   `test/play/corpus/screens/`, so tests don't move while bots keep writing logs. Record the
   self-test baseline: 0.2% unclassified and the per-event counts.
2. **Golden tests for patterns.py** (new `test/play/test_patterns.py`, stdlib `unittest`):
   - One case per static pattern from its example. The self-test already checks examples
     against their own pattern; turn that into a test.
   - Prompt forms: std, mana, kai, resting in both positions, debug suffix, off, custom
     (including `%n` and `%%`), generic.
   - `parse_exits` with trap doors, secret passages and NONE!!!; `parse_carrying`;
     `parse_also_here`; `_fix_monster`.
   - Replay assertions on the frozen corpus: at most 0.5% unclassified, and fixed counts for key
     events (`exp_gain` = the number of `You gain` lines, `room_title` = `room_exits`, every
     `combat_engaged` closed by a `combat_off`).
3. **Shadow mode (no behaviour change).** Add `reg.classify()` to `Bot._seen()` and log any
   disagreement between old and new for HP, mana, attacked, combat off, dark and death. Run one
   bot for a session and review the disagreements. They should all be cases from the "Why
   bother" list.
4. **Swap in this order**, smallest blast radius first, replaying the corpus after each:
   1. prompt (HP/MA/state) and death detection
   2. combat (attacked, combat off, kills, no mana, `record_attack`, after resetting the attack
      stats)
   3. loot and inventory (`carried_items`, `loot`, `cleanup`, `equip_carried`)
   4. room reading (`room_name`, `observe`, `here`, `update` refusals). This is the riskiest,
      because position tracking depends on it. Replay movement from transcripts through `update()`
      with a fake bridge and check the key sequence against the logged one.
   5. shops, training, healer, light, login
5. **Login hardening:** read `set` and force `statline full`. Add a `--statline` option for a
   custom template, passed to `PromptParser`.
6. **Bridge:** when both bots can be restarted, add `"ansi"` to the bridge responses and switch
   classification to `raw=True`. CLRLINE splitting replaces prompt peeling. Re-run the corpus
   test in `--raw` mode as well.
7. **Delete** the old constants and helpers, then update `screen-parsing.md` to point at the
   registry.

## Test strategy

- **Unit:** `test_patterns.py` (step 2). It runs in under 2 s.
- **Replay:** a fake bridge (`FakeBridge(lines)`) that feeds a transcript to `Bot` one response
  at a time, split on the `>>> 'cmd'` markers, and checks what the bot does:
  - the commands it would send
  - its HP, level and kill counts
  - its room key after each move
  - Compare old and new parsers on the same stream. Differences must be explained by the audit
    list.
- **Live:** shadow mode (step 3) on one bot for a full session before each swap lands.
- **Regression gate:** `python3 test/play/patterns.py` must show no rise in unclassified lines
  on the frozen corpus.

## Risks

- **Room-title collisions.** A room name that is also a message line would be misread as a
  title. None were seen in 40k lines, but `room_title` should still require a following
  `room_exits` within 8 lines before position tracking trusts it. That is the same rule as today.
- **The generic `wccmsg` role** can shadow an intended static pattern if a WCCMSG text
  duplicates a source string. Static patterns have priority 50 and wccmsg 70, so static wins.
  Keep that ordering.
- **Speed.** Classification takes 11 µs per line, which is negligible. Registry load takes
  0.5 s; load it once.
