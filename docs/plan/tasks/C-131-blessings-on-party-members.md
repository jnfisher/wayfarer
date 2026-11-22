---
id: C-131
title: "Blessings on party members: a character keeps a lasting spell on another of ours, picked per spell in the Blessings panel"
tier: A
size: M
wave: 21
depends: [C-047, C-043, C-109]
crate: wayfarer-world, wayfarer-brain, wayfarer-store, wayfarer-proto, ui
touches:
  - crates/wayfarer-world/src/spells.rs (`BuffDef::on_another`: target 2 goes on a player)
  - crates/wayfarer-brain/src/bless.rs (a kept spell is a spell and a target; `sped riley`; the target's lines)
  - crates/wayfarer-brain/src/party.rs (`Mate.buffs`, `MemberInfo.buffs`: the lasting spells a member believes on itself)
  - crates/wayfarer-brain/src/session.rs (`Felt`: every lasting line a session sees, not only its ticked spells; `set_blessings` with targets)
  - crates/wayfarer-brain/src/brain.rs, crates/wayfarer-store/src/lib.rs (the chosen targets per spell, kept with the blessings)
  - crates/wayfarer-proto/src/lib.rs (`BlessingSpell.others`, `BlessingsSave.on_others`)
  - ui/src/BlessingsPanel.svelte, ui/src/lib/blessings.ts, ui/tests/blessings-panel.test.ts
  - docs/brain.md "Spells" (Blessings), docs/protocol.md, docs/web-ui.md, docs/kb/spells.md, docs/kb/changelog.md
---

## Goal

John wants Asa (Mage) to keep speed on Riley (silver-mirror, 2026-11-22). Blessings (C-047) cast
only on the caster or the whole party: `Kept` is one belief per spell, `step()` sends the bare
short name, and the panel ticks spells with no target. The game lets a Self-or-Single spell
(target 2: speed, blur, frenzy, resist fire and cold, bless, divine favour, protection from
evil, alertness, rapid healing, the Bard's songs of might, agility, quickness, heroism) go on
another player with `sped riley`; a Self-only spell (target 1: mageshield, magic armour,
ethereal shield, barkskin) is refused with "You may not cast that spell on a user!".

When this card is done the Blessings panel offers, beside each Self-or-Single spell a
character knows, the other characters of the gateway as targets; the bot casts the spell on a
ticked target when it stands in the same room and the spell is not believed on it, keeps it up
as it keeps its own, and the panel shows per target whether it is on. Targets are our own
characters only: their sessions run in the same brain, so the target's own lines ("You feel
fast!", "You slow down.") say when the spell is on it.

## Inputs

- `docs/plan/tasks/C-047-blessings.md` (what Bless does today) and `docs/brain.md` "Spells"
  (Blessings, Buff items, the "Not done" line: "Buffs on other members").
- `crates/wayfarer-brain/src/bless.rs`: `Kept { def, on_until, retry_at, fails }`, `set`,
  `due`, `step` (:473 "A spell on the caster or the party takes no target word"), `outcome`
  (a prompt is success), `on_event` (`CastFailed`, `NoMana`, `AlreadyCast`; a line equal to
  `def.wear_off` turns off, equal to `def.felt` turns on "whoever cast it"; `YouKilled` turns
  all off; the status screen check `check_at`/`screen_seen`, `spells.bless_check_seconds`).
- `crates/wayfarer-brain/src/fight.rs` `cast_step` (:144): a heal on a member is
  `format!("{} {who}", s.short)` ("mihe lauren"), narration "casting X on Who"; `heal_mates`
  (:579) and `View::heal_for` (`modes.rs:680`) build the list from `party.mates()`.
- `crates/wayfarer-brain/src/party.rs`: `Mate { name, hp, missing, flee, until, short, resting,
  hp_rate, mana_wait }` (members in the caster's room with HP known, :1395-1419), `MemberInfo`
  (`session.rs` `party_info()`), the coordinator's `PartyRole`.
- `crates/wayfarer-brain/src/session.rs`: `known_buffs()` (the `spells` screen rows through
  `spells.buff(class, short, name)`), `refresh_blessings()`, `set_blessings()`,
  `blessings_status()`, `RESYNC_COMMANDS` (`st` is read at every re-sync).
- `crates/wayfarer-world/src/spells.rs`: `BuffDef { target, felt, wear_off, rounds(level),
  on_party() (10 | 13) }`; `is_lasting` takes targets 1, 2, 6, 10, 13.
- The data (stock export): 42 lasting player spells of target 2, 46 of target 1, 22 of target
  13; none of 6 or 10. Speed is record 57 (Mage 12, 12 mana, 20 rounds; energy ×85%); its
  descriptive message 8537 is ["You slow down.", "", "You feel fast!"] and its cast message 8462
  "You cast %s on %s." / "%s casts %s on you." / "%s casts %s on %s!". Bless's cast message 16 is
  "You cast %s on %s!" / "%s casts a spell on you!".
- `docs/kb/spells.md` "Casting": the target is checked as "You do not see %s here!" before
  anything else; a copy already on the target is renewed only when the engine allows, otherwise
  "You attempt to cast %s, but fail." (the failed-roll line, no "already affected" line of its
  own); every cast ends the caster's auto-combat and rest. `screen-patterns.md` `bad_target`:
  "You may not cast that spell on a user!". `kb/changelog.md` :189: "You cast bless on Aldric!"
  reads as `mon_hits_you`; `typed.rs:895-907` makes it `SpellCast` only with damage.
- Anti-Magic (`spells.md` :239): a Witchunter resists a friendly spell at mr/2 %, capped at 98.
- `crates/wayfarer-proto/src/lib.rs` `BlessingSpell { short, name, mana, chosen, on }`,
  `Blessings`, `BlessingsRequest`, `BlessingsSave { session, spells }`;
  `crates/wayfarer-store/src/lib.rs` table `blessings(session, spells TEXT, updated_at)`;
  `ui/src/BlessingsPanel.svelte`, `ui/src/lib/blessings.ts` (`chosenOf`, `toggleSpell`,
  `changed`, `spellLabel`), `ui/tests/blessings-panel.test.ts`.

## Behaviour

- **The panel.** Each character's box lists its lasting spells as today. A spell the game lets
  go on a player (target 2) has, under its own tick, one tick per other character of the
  gateway ("on Riley", "on Sasha"...; every configured character, not only the party of the
  moment, since parties come and go), each with its own ● when the brain believes the spell on
  that character. A Self-only or party spell shows no targets. Save sends the targets with the
  spells; the saved choice outlives restarts as the blessings do.
- **When it casts on a member.** The target is in the caster's room (a `Mate`: same room, HP
  known), the spell is not believed on the target, the cast leaves the mana at or above
  `spells.bless_mana_above`, and Bless's turn has come as today (after its own spells; in a
  fight only with `spells.bless_in_combat` and `may_bless`; out of one when Recover yields). A
  target in another room, lost, or not running is skipped without a word and without a failure
  count: it is looked at again when it stands beside the caster. The caster's own spells come
  first, then the members' in panel order.
- **The cast.** `sped riley`: the short name and the target's first name in lower case, as a
  heal on a member is sent. Narration "Blessing: casting speed on Riley". A prompt with no
  refusal counts as cast, as today, and the spell is believed on the target for
  `rounds(caster level)` × 5 s.
- **What the target sees decides.** The target's session reads its own lines: "You feel fast!"
  (the spell's `felt`) means on, "You slow down." (`wear_off`) means off, its death means all
  off, and the status screen it reads at a re-sync (and at the blessing check, when it runs
  one) lists what is on. Those beliefs travel through the party coordinator to the caster, so
  the caster does not cast speed twice on Riley, two casters do not both cast it, and a brain
  restart does not re-cast what Riley still feels. Without word from the target (a member whose
  session is not running), the caster's own timer stands.
- **Refusals.** "You attempt to cast speed, but fail." is a failure as today (retry after a
  round, three in a row hold the spell on that target for five minutes): it is the line for a
  copy already on the target too, so the target's belief is read first and the hold stops a
  loop. "You do not see riley here!" marks the target away (no failure count; the mates list was
  stale). "You may not cast that spell on a user!" drops that target for the session and says
  once "Blessing: speed cannot be cast on another player"; the data should keep this from
  happening (target 2 only), so it is a guard, not a path. A Witchunter target resists at
  mr/2 %: the failures and the hold take care of it, and the panel's class note says so.
- **Nothing else changes**: the mana line, the in-combat setting, the boss-mode silence, buff
  items (self only), the regen tier (self only).

## Design notes

- `spells.rs`: `BuffDef::on_another(&self) -> bool { self.target == 2 }` beside `on_party`;
  `Spells::buff` is unchanged (it never looks at the target).
- `bless.rs`: `Kept` gains `target: Target` (`Me`, `Member(String)` with the lower-case first
  name), so a spell ticked for self and for Riley is two `Kept`s. `set(defs)` takes
  `Vec<(BuffDef, Target)>`. `due()` skips a `Member` whose name is not in `v.party.mates()` and
  one whose mate's `buffs` has the spell. `step()` sends `format!("{short} {name}")` for a
  `Member`, the bare short name otherwise. `turn_on`, `failed`, `held` and the narration take
  the target. `on_event`: `felt`/`wear_off` lines keep their meaning for `Me` only (the caster
  seeing "You feel fast!" is its own copy); `CastFailed` within a round of a member cast fails
  that `Kept`; a `TargetMissing` (`You do not see riley here!`) marks it away; a `BadTarget`
  ("You may not cast that spell on a user!") removes the target with the one-time notice.
- The target's side, `session.rs`: a small `Felt` tracker (or `Bless::seen`) records, for every
  lasting spell of the pack the class could carry (not only ticked ones), the spell short
  believed on from its `felt` line and the status screen, off from its `wear_off` line, death
  and a status screen without it. The status screen's spell list is already read for
  `screen_seen`. `party_info()` puts the set in `MemberInfo.buffs: BTreeSet<String>`; the
  coordinator copies it to `Mate.buffs`. The caster's `due()` reads it; when the mate has no
  word (empty, never seen a screen) the caster's `on_until` timer stands.
- Storage and protocol: `BlessingsSave { session, spells: Vec<String>, on_others:
  BTreeMap<String, Vec<String>> }` (spell short → first names; absent means none, so old
  clients still work); the store's `spells TEXT` keeps a JSON array for the self list and
  gains a sibling column or a JSON object `{ "spells": [...], "on_others": {...} }` with a
  migration that reads the old array as self only. `BlessingSpell` gains `others:
  Vec<BlessingTarget { name, chosen, on }>`, filled only for `on_another()` spells, listing
  every other character of the gateway (`brain.sessions` names; the first name in the game's
  case) with `on` from the target's `MemberInfo.buffs` or the caster's timer.
- UI: `lib/blessings.ts` gains the targets in the draft (`toggleTarget(spell, name)`,
  `changed` compares them too); the panel renders them indented under the spell, hidden for
  spells with none; the ● per target. Keyboard and labels as the spell rows.
- Not in this card: buffing a party member who is not one of ours (no word back; the timer
  alone would do, but the name list would have to come from the `par` screen); party-wide
  spells on a member's say-so; a target's own request ("Riley wants speed"); the Witchunter's
  refusal of buffs as a setting.

## Done when

- Asa's panel shows speed with "on Riley" (and the other characters) under it; ticking Riley
  and saving sends `blessings_save` with `on_others: { "sped": ["riley"] }`, kept across a brain
  restart.
- In a brain test with Asa and Riley in one room, Asa sends `sped riley` after her own
  blessings, says "Blessing: casting speed on Riley", and does not send it again while Riley's
  session has seen "You feel fast!" and not "You slow down."; after Riley's "You slow down." she
  casts again; with Riley two rooms away she sends nothing and says nothing.
- "You attempt to cast speed, but fail." three times holds speed-on-Riley for five minutes;
  "You do not see riley here!" marks Riley away without a hold; "You may not cast that spell on
  a user!" drops the target with the one-time notice.
- A Self-only spell (mageshield) shows no targets in the panel; a party spell (holy aura) none.
- `docs/brain.md` Blessings says so and the "Not done" line loses "Buffs on other members";
  `docs/protocol.md` and `docs/web-ui.md` have the fields; `docs/kb/spells.md` "Casting" gains
  the player-target rule (target 2 only; target 1 gives "You may not cast that spell on a
  user!"; the cast lines of speed and bless) tagged [C data] and, once a live cast is seen,
  the exact lines [F]; `docs/kb/changelog.md` has the line; CI green.

## Tests

- `bless.rs` unit tests: `a_member_target_is_cast_with_its_name`,
  `a_member_not_in_the_room_is_skipped_without_a_failure`,
  `a_member_who_feels_the_spell_is_not_cast_on_again`,
  `the_wear_off_seen_by_the_member_makes_it_due_again`,
  `a_user_refusal_drops_the_target_once`.
- `tests/bless.rs` (brain, two sessions): `speed_is_kept_on_a_party_member_from_its_own_lines`,
  `two_casters_ticked_for_the_same_member_cast_once`.
- `wayfarer-store`: `blessings_with_targets_survive_a_reload_and_an_old_row_reads_as_self`.
- `ui/tests/blessings-panel.test.ts`: `a_self_or_single_spell_lists_the_other_characters_as_targets`,
  `ticking_a_target_saves_it`, `a_self_only_spell_has_no_targets`.

## Notes for the agent

- The cast's confirmation line to the caster ("You cast speed on Riley.") reads as
  `mon_hits_you` today (changelog :189) and becomes `Other`; do not build on it: a prompt with
  no refusal is the success, as C-047 did. The target's "Asa casts speed on you." is likewise
  not a signal; the target's `felt` line is.
- The first name in lower case is what the game takes (`sped riley`), as `cast_step` sends for
  heals; the panel shows the name as the game spells it.
- `Mate` is built from members in the same room with HP known: a member whose HP the brain does
  not know yet (just logged in) is not a target until it is; that is the right order anyway
  (the member's belief about its own buffs needs a status screen or a line).
- Energy, not casts: speed is ×85% energy use, so it is for the fighters; the panel does not
  judge who should get what, the player does (goal 2).
- The KB's `classes/gypsy.md:148` says ethereal shield can go on a fighter; the data says target
  1 (Self). Trust the data; note the conflict in the KB with a [C data] tag when the card
  touches `spells.md`.
- Needs John: whether a target list should also offer the game party's strangers by name from
  the `par` screen (left out above).
