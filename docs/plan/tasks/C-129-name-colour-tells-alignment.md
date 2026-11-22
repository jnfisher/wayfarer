---
id: C-129
title: "The colour of a name in `Also here:` tells the bot its alignment, and settles a name with two records"
tier: A
size: S
wave: 20
depends: [C-090]
crate: wayfarer-parse, wayfarer-brain
touches:
  - crates/wayfarer-parse/src/text.rs (the line cutter keeps the colour in force at each name)
  - crates/wayfarer-parse/src/helpers.rs (`Present` gains the colour hint)
  - crates/wayfarer-parse/src/typed.rs (`AlsoHere` carries it)
  - crates/wayfarer-brain/src/policy.rs (`hostile` resolves a name by the hint)
  - crates/wayfarer-brain/src/fight.rs, crates/wayfarer-brain/src/modes.rs (the hint beside `here`)
  - crates/wayfarer-brain/tests/fight.rs
  - docs/brain.md "Fight here" (Worth fighting), docs/kb/combat.md, docs/kb/changelog.md
---

## Goal

Asa stood in the Warped Asylum (9/1183 to 9/1254, silver-mirror, 2026-11-22 09:56) with a
crazed lunatic hitting her for five or six a round and did not fight back: "Fight: in combat",
then at once "Fight: looking for the next" and "Waiting here", while her blessing round (blur,
magic armour, mageshield, arcane attunement) cancelled each attack spell John queued by hand
(`sbol c` → `*Combat Engaged*`, the blessing → `*Combat Off*`). The stock data has two
monsters named crazed lunatic: 262 (Silvermere slums, Neutral, 250 exp) and 495 (the
asylum, Neutral Evil, 275 exp). `hostile` wants every record a name finds to be huntable
(`kb/pitfalls.md` 1, after a good-aligned twin cost alignment), so the name is refused
wherever it is met, and nothing says so: this is not a monster rule, so no "Ignoring" notice.

The game had already told her which one it was. Every name after `Also here:` is coloured by
the monster's alignment, and the lunatic came in bright magenta (`ESC[1;35m`): the evil
record. When this card is done the parser keeps that colour as a hint on each name, and a
name whose records disagree is settled by it; a name with no hint behaves as today.

## Inputs

- Asa's line, `logs/asa/output.log` seq 672060:
  `ESC[1;36mWarped Asylum\r\nESC[0;35mAlso here: ESC[1;35mcrazed lunaticESC[0mESC[0;35m.\r\n`
  (hand-copy it into the test; the host's logs are not fixtures).
- The game's rule, quicksilver `crates/silvermere-engine/src/display/room.rs` `monster_colour`
  (mirrors `LJNDISP.C`): Good and Neutral names in `ANSI_CYAN` (`ESC[0;36m`); Lawful Good in
  `ANSI_GREY` (`ESC[0;37m`) for a viewer under 40 evil points, else the here-names colour;
  Lawful Evil the other way round (here-names colour under 40, grey at 40 or over); Evil,
  Chaotic Evil and Neutral Evil in the here-names colour. The here-names colour is palette row
  8 (`COL_HERENAMES`): `ANSI_LTMAGENTA` in palettes 0 and 3, **no colour at all** in palettes
  1 and 2, where the name simply continues in the `Also here:` label's colour. So cyan and
  grey are fixed by the game; "anything else, or nothing" is the evil case.
- `crates/wayfarer-parse/src/text.rs`: the line cutter drops every ANSI sequence but
  `ESC[79D ESC[K` before any pattern sees the line; `helpers.rs` `parse_also_here` and
  `Present { name, player, flags }`; `typed.rs` `"room_also_here"` →
  `G::AlsoHere { players, monsters }` (names only).
- `crates/wayfarer-brain/src/policy.rs` `hostile` (all records huntable), `is_one_of`,
  `quiet`; `crates/wayfarer-world/src/monsters.rs` `huntable`, `is_evil` (alignments 1, 2, 5,
  6 of Good, Evil, Chaotic Evil, Neutral, Lawful Good, Neutral Evil, Lawful Evil), `by_name`;
  `fight.rs` around `hostile(&v.atlas.monsters, n)` (the target filter); `modes.rs`
  `View.here: Vec<String>`.
- The stock data: 17 names have records that disagree on huntable; colour separates 10 of
  them (ancient sand dragon, Argak the Grey, Choira Pyromancer, cloaked figure, crazed
  lunatic, Enigma Lord, giant roc, Kai Master, Lallim Whitemane, Sharh'kur); the other 7
  differ only on exp or montype (the chess pieces, the black and white guardsmen, Nahr), where
  colour cannot help and today's rule stands.
- `docs/kb/combat.md` line "In-game, monster name colour shows alignment. [F, John]";
  `kb/pitfalls.md` 1 and 47 (the colour reset riding on sends).

## Behaviour

- Each monster name of a room view carries a hint: *good or neutral* (cyan), *lawful* (grey),
  *evil* (any other colour, or none of its own). Names that arrive by other lines (a monster
  walking in, a blow heard, a cast) carry none until the next room view; the fight code already
  looks after each engagement, so the hint comes within a round.
- `hostile(name)` with a hint: of the records the name finds, keep those whose alignment fits
  the hint (good or neutral: alignments 0 and 3; lawful: 4, or 6 for an outlaw viewer; evil: 1,
  2, 5, and 6 for a viewer under 40 evil points, 4 for an outlaw). The name is hostile when at
  least one record is kept and every kept record is huntable. No record kept (a colour the data
  does not explain): today's rule over all records. No hint: today's rule.
- The outlaw case: with the character's evil points known to be 40 or more, grey means Lawful
  Evil and the here-names colour may be Lawful Good; the filters above say so. Unknown evil
  points count as under 40 (every character of ours).
- The target filter in Fight, `quiet` (between fights) and the loop's "a room spawns" test all
  go through `hostile`, so they all gain the hint. Monster rules are unchanged: *ignore* still
  wins over any colour.
- Nothing is narrated when the hint settles a name; when a name is refused because its records
  disagree and no hint is at hand, the bot says once per room "Not fighting NAME: two monsters
  share the name and the room did not show which", so the silence Asa fell into has a reason on
  the tile.

## Design notes

- `text.rs`: the cutter already walks every ESC sequence to drop it; let it record, per output
  line, the SGR sequences it dropped with the offset in the plain line where each applied
  (`Vec<(usize, String)>`), on the `Line` it hands up. Only the `room_also_here` handler reads
  them; every other pattern sees the plain text as today.
- `helpers.rs`: `Present` gains `colour: Option<NameColour>`; `parse_also_here` takes the
  spans and classifies the SGR in force at each name's first character: `0;36` → GoodOrNeutral,
  `0;37` → Lawful, any other (`1;35`, the label's `0;35`, or nothing since the reset) → Evil.
  A `1;30` or other grey spelling is not sent by the game: do not widen the match.
- `typed.rs`: `G::AlsoHere { players, monsters: Vec<SeenName { name, colour }> }` or a parallel
  `colours` vector, whichever keeps the proto's `here` lists as plain names (the UI shows names).
- `modes.rs`: beside `View.here` keep `View.colours: BTreeMap<String, NameColour>` from the
  last room view, cleared with `here`; `policy::hostile` takes `Option<NameColour>` and the
  viewer's evil points (`Traveller.evil`, already read from the status screen) through a small
  `Viewer` struct so `fight.rs`, `quiet` and the spawn test pass the same thing.
- `monsters.rs`: `huntable` stays; add `fits(alignment, hint, outlaw) -> bool` beside `is_evil`
  with the alignment table in its comment.
- Not in this card: using the colour to learn a monster's alignment when the pack lacks the
  record (a renamed or new monster); the hint only chooses among records the name finds.

## Done when

- Asa's line parsed gives `crazed lunatic` the hint Evil, and `hostile` is true for it against
  the stock pack; the same name with `ESC[0;36m` gives GoodOrNeutral and `hostile` false; a
  palette-1 line (no colour of its own on the name) gives Evil.
- The giant roc (two Neutral records at 60,000 exp, one Chaotic Evil at 12,000) in cyan is not
  hostile; in magenta it is, and `is_first` still treats it as a boss.
- An outlaw viewer (evil 40) with a grey name keeps the Lawful Evil record.
- A brain test replaying Asa's room view and the lunatic's blows has the bot send `a lunatic`
  on the next tick, with no blessing cast in between.
- `docs/brain.md` "Fight here" says so under "Worth fighting"; `docs/kb/combat.md` replaces the
  [F] line with the rule and its colours, tagged [C] with the quicksilver file (it mirrors
  `LJNDISP.C`), and `docs/kb/changelog.md` has the line; CI green.

## Tests

- `wayfarer-parse` unit tests: `also_here_names_carry_their_colour` (Asa's line),
  `a_name_in_cyan_is_good_or_neutral`, `a_palette_without_a_names_colour_still_marks_evil`,
  `a_colour_reset_riding_on_the_send_does_not_move_the_hint` (pitfall 47).
- `policy.rs` unit tests: `a_name_with_two_records_is_settled_by_its_colour`,
  `an_unexplained_colour_falls_back_to_every_record`, `an_outlaw_reads_grey_as_lawful_evil`.
- `tests/fight.rs`: `a_monster_whose_colour_says_evil_is_fought_though_its_name_has_a_neutral_twin`.

## Notes for the agent

- The here-names colour is a per-player palette setting, so never match magenta: match cyan
  and grey, and call the rest evil.
- Players are listed before monsters in the same line, in `COL_HERENAMES` too, with a star for
  outlaws; `Present.player` already separates them by the capital and single word, keep that.
- `kb/pitfalls.md` 47: the `ESC[0;37;40m` reset rides on whichever send comes first and may sit
  at the front of the line; it is not a name's colour. Only the SGR in force at the name's
  first character counts.
- The two-record silence was found on Asa (Mage, level 22) and would hit any class; the depraved
  murderer in the same asylum has two records too, both Evil, so it fights fine.
