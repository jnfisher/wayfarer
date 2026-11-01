---
id: C-067
title: "Rest and meditate by the 1.20 rules, and the regen buff kept up"
tier: B
size: M
wave: 12
depends: [C-064, C-066]
crate: wayfarer-brain, wayfarer-settings
touches:
  - crates/wayfarer-brain/src/{recover.rs,regen.rs,modes.rs,bless.rs,party.rs,session.rs}
  - crates/wayfarer-settings/src/lib.rs (only if a setting is needed)
  - docs/brain.md, docs/kb/quicksilver-1.20.md, docs/kb/changelog.md
---

## Goal

On a 1.20 server the bot rests and meditates the way 1.20 works: a caster with Meditate meditates
for HP and mana alike, one without rests for mana too, a rest that ends by itself at full bars
is not mistaken for an interruption, and recovery estimates use 1.20's rates. A caster keeps its
best regen tier up. On 1.11p and unknown, nothing changes.

## Inputs

- `docs/plan/quicksilver-1.20/handoff.md` 3.7, 5.1, 5.2, 4.2 (tiers, two-cast upgrades), 3.8 (the
  "when it ends" lines), 6.7 and 6.10 (regen summed).
- `crates/wayfarer-brain/src/recover.rs`, `regen.rs` (rates per hour), `modes.rs`, `party.rs`
  (party rests, C-045), `bless.rs` (blessings, C-047/C-056).
- The settings `health.rest_*` (C-051 made resting for mana off by default).

## Behaviour

- **Which to use** (1.20 only): a character with Meditate meditates whatever is missing (HP,
  mana or both), and never switches to `rest`. Without it, it rests. Today's rule "rest for mana
  only if `health.rest_mana_below` > 0" stays; a 1.20 rest refills mana along the way anyway.
- **The end of a rest.** On 1.20 a rest ends silently when HP and mana are full: the `(Resting)`
  marker vanishes. The bot checks the bars before re-sending `rest`: full bars mean the rest is
  over; bars not full mean something interrupted it and `rest` is re-sent as today. Meditate ends
  with `You awake from deep meditation feeling stronger!` only at full HP and mana.
- **`Meditation will not help at this time.`** on 1.20 means both bars are full, never "use rest
  instead".
- **Estimates.** On 1.20, `regen.rs`'s mana rate is ×3 resting and ×5 meditating
  (`rest_mana_mult`, `meditate_mana_mult`), meditate also giving rest's HP; plus the known regen
  tier and race bonus to Alter Mana Regeneration (C-069 supplies the race numbers; until then
  race 0). The party rest (C-045) uses these estimates.
- **The regen buff.** On 1.20 a caster whose family has a tier it knows (C-066) keeps the highest
  one up, as a blessing the bot adds by itself (not one the player has to tick), cast at the mana
  and times `bless.rs` already uses. Re-cast on the family's "when it ends" line. Upgrading: the
  first cast of a new tier while an old one is up removes the old one and ends there; the bot
  casts once more to land the new tier, then believes it on.
- **A setting** `spells.keep_regen_buff`, on by default, turns the upkeep off. (Only on 1.20; it
  is shown greyed with "1.20 only" otherwise, if the Settings panel can do that cheaply.)

## Design notes

- Branch on the rule set in one place each (the rest decision, the end-of-rest check, the rate
  function), not throughout.
- The regen buff reuses `bless.rs`'s machinery with a synthetic entry; the two-cast upgrade is a
  small state: after a cast of tier N while tier M < N was believed on, expect it off and cast
  again.

## Done when

- On 1.20: a Mage with Meditate below full HP and full mana meditates (and the start is accepted);
  a Warrior's rest ending silently at full HP is not re-sent; a Priest without Meditate rests for
  mana; a Mage keeps arcane attunement up and re-casts it on `Your attunement to the mana fades.`;
  learning greater attunement makes two casts.
- On 1.11p the same scenarios run exactly as before (the existing tests unchanged).
- `docs/brain.md` "Resting" says what changes on 1.20; KB section "Rest and meditate" ([Q]).
- fmt, clippy and nextest pass.

## Tests

- The rest decision per rule set and class, with and without Meditate.
- The silent end at full bars vs an interruption below full.
- `regen.rs`: the ×3 and ×5 rates, a regen tier's %, per rule set.
- The regen buff: kept up, re-cast on the fade line, the two-cast upgrade, the setting off.

## Notes for the agent

- Don't train stats, ever (CLAUDE.md "Rules"); nothing here goes near `train`.
- A class with no mana counts as full on mana (handoff 5.1).
