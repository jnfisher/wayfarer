---
id: C-136
title: "An attack names the monster it means; a monster the weapon cannot hurt is fought with the spell; a fight in the room holds off a rest and still gets the heals"
tier: A
size: S
wave: 25
depends: [C-091, C-133]
crate: wayfarer-brain
touches:
  - crates/wayfarer-brain/src/policy.rs (`target_word`, `names`; tests)
  - crates/wayfarer-brain/src/fight.rs (every attack, backstab and cast line; `refused`; `weapon_proof`; `heal_by`)
  - crates/wayfarer-brain/src/modes.rs (`Presence::fighting`, `fight_here`)
  - crates/wayfarer-brain/src/recover.rs (no rest while a fight goes on here)
  - crates/wayfarer-brain/tests/fight.rs, tests/spells.rs, tests/party.rs
  - docs/kb/combat.md "Naming a monster", docs/kb/changelog.md, docs/brain.md "Fight here"
---

## Goal

John (silver-mirror, 2026-11-28): "is the adult red dragon not a priority target over the
other stuff?" It was: in the Volcanic Chamber all six bots opened on the adult red dragon. The
room held, in order, `adult red dragon, young red dragon, red dragon hatchling, fat baby red
dragon, nasty young red dragon`. Rocco, Riley, Sasha and Penny sent `bash dragon`, `a dragon`,
`bs dragon` and got `Your weapon has no effect against this monster!` (fair: their weapons
cannot hurt the adult). Each then picked the young red dragon and sent `a dragon` again, which
the game gave to the adult, the first "dragon" in the room. "No effect" came back, the bot
skipped the *young* dragon, and the same happened with the fat baby and the nasty young one.
Only `a hatchling` landed. Five seconds in, the four melee bots had skipped every dragon but
the dead hatchling and stood in boss fight mode, "Waiting here", while the dragons burned Lexi
(hung up at 6%) and Asa (−45%), then Rocco (3%). The same thing happened at 09:31:53 with the
first adult.

Two more faults made it worse. Rocco lay down to rest at 09:35:12 ("Recover: resting to 85% in
Volcanic Chamber"), since nothing attacked *him*. Sasha, a healer with nothing she could hit,
stood by: out of combat, heals were Recover's, and Recover heals only in a quiet room.

When this card is done:

- the word an attack sends names the monster the bot chose;
- a monster the weapon cannot hurt is fought with the attack spell if there is one;
- no rest starts while a fight goes on in the room;
- a character out of the fight still heals itself and the party there.

## Inputs

- The game's matcher, `ljnsame` (`mmud-exploration/WCCMMUD-1.11p-source/SRC/LJNUTIL.C`,
  about line 4016). A typed name matches a monster when it is the start (kernel `sameto`,
  case ignored) of the name from any word on. The typed text may run to several words. The
  room's monsters are tried in slot order, which is the order of `Also here:`, and the first
  that matches is the one attacked.
- `crates/wayfarer-brain/src/policy.rs` `attack_word` (the name's last word, used for every
  attack).
- `crates/wayfarer-brain/src/fight.rs`: the attack, backstab and spell lines, `refused`,
  `skip`, the in-fight heals (`heal`, `heal_mates`).
- `crates/wayfarer-brain/src/modes.rs` `Presence` (`here` in room order; `seen_fighting`).
- `crates/wayfarer-brain/src/recover.rs` `step`; `Bot` mode order in `modes.rs`.
- The live log: silver-mirror `logs/{rocco,riley,sasha,penny,lexi,asa}/output.log` and
  `notes.log`, 2026-11-28 09:31:50 to 09:35:46.

## Behaviour

- **The word picks out the target.** It is the name's last word, as before, when no monster
  listed before the target in the room answers to it. Otherwise it is the shortest run of
  whole words of the name that none before it answers to, on a tie the later one. In the log's
  room: adult `dragon`, young `young`, hatchling `hatchling`, fat baby `fat`, nasty young
  `nasty`. With the same name twice, the last word: the first of the two is as good. Melee,
  `bs`, spell lines, the weapon after a spell, a switch to a boss and the restart for
  attacking last all use it. A blind try (C-133) is matched against the word it went with.
- **The weapon has no effect.** The monster is fought with the attack spell while there is
  one to cast (no hold after a refused spell, the setting on). That holds whether or not the
  bot was sneaking for a backstab, and whatever `spells.max_casts_per_monster` says: the
  weapon never takes over from the spell on it. With no spell, or a second "no effect", it is
  left alone in the room as before. Narrated "Fight: adult red dragon with meteor swarm (the
  weapon has no effect)".
- **A fight in the room.** A monster here seen fighting anyone (`mon_hits_other`,
  `mon_miss_other`, `other_hits`, `attacks_other`, `spell_cast_room`, an intercept) marks a
  fight here until it dies or leaves.
  - No rest starts, nor a walk out to one, while there is one. Narrated "Recover: not resting
    while young red dragon fights here". A rest under way is left alone.
  - Out of combat with nothing here it can fight, Fight still heals while there is one: itself
    under `spells.heal_in_combat_below_hp`, a member under the heal-a-member line in a fight,
    as when engaged. No `*Combat Off*` follows such a cast.
- **Choosing is unchanged.** The adult first was right, and so was skipping a monster the
  weapon cannot hurt when there is nothing else to hurt it with.

## Design notes

- `policy::names(typed, name)` is `ljnsame`; `policy::target_word(name, room)` takes
  `Presence::here` in order. `Fight::word` builds the word for a line and keeps it in `sent`,
  for the speech matcher.
- The card first had a refusal blamed on "the monster the word reached". Built from the same
  room list, that is always the target, so it was dropped. A room order gone stale (an
  arrival in an earlier free slot) is not something the bot can see.
- `Fight::weapon_proof`: cleared with the skips on leaving the room. `pick` counts it as
  skipped when no spell is ready.
- `Presence::fighting`: filled by `seen_fighting`, trimmed to `here` at each room view,
  cleared by `moved_unseen`; `fight_here()` is the first still here.

## Done when

- In the log's room each melee bot that gets "no effect" from the adult goes on to the young
  red dragon by `young` and hits it, rather than skipping every dragon.
- A caster's spell goes on at a monster its weapon cannot hurt.
- A bot does not lie down to rest beside its party's fight, and a healer out of the fight
  heals the member being hurt.

## Tests

- `policy.rs` unit: `the_word_picks_out_the_target_from_those_before_it`,
  `a_lone_name_keeps_its_last_word`, `matching_is_from_any_word_start_like_the_game`.
- `tests/fight.rs`: `no_effect_on_the_first_does_not_skip_the_others` (`a rat` no effect,
  then `a fat`), `the_attack_word_picks_out_the_target_from_those_listed_before_it`
  (`a large`), `no_rest_starts_while_a_fight_goes_on_in_the_room`.
- `tests/spells.rs`: `a_monster_the_weapon_does_nothing_to_is_fought_with_the_spell`.
- `tests/party.rs`: `a_healer_out_of_the_fight_heals_a_member_fighting_in_the_room` (fails
  with the heal taken out).

## Notes for the agent

- KB: `docs/kb/combat.md` "Naming a monster" and a changelog line.
- Not this card: a melee character left with nothing it can hurt (only the adult) still
  stands by. Nothing in the game lets it help but heals, which it now casts.
