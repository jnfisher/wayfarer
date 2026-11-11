---
id: C-096
title: "Monster rules are the player's, over defaults from MegaMud's list; a stalemate makes no rule"
tier: B
size: S
wave: 16
depends: [C-094]
crate: wayfarer-world, wayfarer-brain
touches:
  - crates/wayfarer-world/src/monsters.rs (`DEFAULT_RULES`)
  - crates/wayfarer-brain/src/rules.rs, session.rs (`learn`), fight.rs
  - crates/wayfarer-brain/tests/rules.rs
  - docs/brain.md "Monster rules", docs/kb/megamud-formats.md, docs/kb/changelog.md
---

## Goal

The bot never decides on its own, for the session, that a monster is not to be fought. The
monster rules are the player's, over a sensible default list. Where wayfarer's own test (evil,
not an NPC, worth experience) differs from MegaMud's shipped relationships, that list follows
MegaMud.

John (2026-11-11): "I think this automated skipping is a problem and I'd rather have it be
manual with a sensible starting default. we should never be "skipping" a regular monster we're
grinding like a nomadic shaman".

## Inputs

- MegaMud v2.0's `Default/Monsters.md`, decoded: byte 37 is the Relationship
  (`kb/megamud-formats.md`). 803 Enemy, 195 Friend, 128 Avoid, 2 Flee.
- Compared with wayfarer's `huntable` test on the stock world, only about 21 of MegaMud's
  non-Enemy names would be fought: the ten gem guardians (Friend), the giant roc (Avoid), the
  gigantic black ooze and huge gruesome creation (Flee), and eight names with both an Avoid
  and an Enemy record.
- The repository keeps MegaMud's databases out (`data/megamud/PROVENANCE.md`). The defaults
  are wayfarer's own list, citing MegaMud, not a copy of its file.

## Behaviour

- `DEFAULT_RULES` gains *ignore* for the huge gruesome creation (32,000 HP for 50 exp) and the
  ten gem guardians (1 exp for 350 HP). The giant roc and the other bosses MegaMud avoids are
  left to the player. The names with mixed records are left alone.
- A stalemate makes no rule. Fight skips the monster while the character stays in the room
  (C-094), the room is avoided for 30 minutes as before, and the notice says how to ignore it
  for good. `RuleBook::learn` is gone; the store already drops learned rows (C-091).
- The only automatic exclusions left: a game refusal or an unanswered attack, for that room
  only (C-094), and the stalemate above.

## Done when

- `rules.rs` (unit): `layers_go_defaults_then_the_player`; the gem guardians are ignored by
  default.
- `tests/rules.rs`: the stalemate tests show no rule and the new notice.
- fmt, clippy `-D warnings`, nextest.

## Notes for the agent

- Needs John: whether the giant roc, Kai Master, ancient sand dragon or storm giant king should
  be default *ignore* or *avoid* too. MegaMud avoids them; they are worth a lot of exp.
