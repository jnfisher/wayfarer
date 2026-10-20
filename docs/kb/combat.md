# Combat

For the full formulas, see `test/play/notes/combat.md` (with source references).

## What to attack
- **Only evil-aligned monsters**: Evil, Chaotic Evil, Neutral Evil, Lawful Evil. The monster
  record's `alignment` field (offset 0xA9) is the index into Good, Evil, Chaotic Evil, Neutral,
  Lawful Good, Neutral Evil, Lawful Evil. [C]
  - Good/Lawful Good monsters (guards, Templars, townsfolk) cost alignment. With evil warnings
    on, the game refuses: "you must turn off your evil warnings". [C]
  - Neutral monsters cost no evil points and are not refused for their alignment: only Good and
    Lawful Good ones charge (LJNCOMBT.C:2435-2441). "Your weapon has no effect against this
    monster!" is the *magic* check: the monster's Magical ability exceeds the player's Hit
    Magical plus the weapon's Magical, and bare hands never count (LJNCOMBT.C:2543-2582). The
    ferryman and the like are refused because they are Magical, not because they are Neutral. [C]
  - In the stock data: 688 Chaotic Evil, 164 Neutral, 84 Lawful Good, 71 Good, 45 Neutral Evil,
    34 Evil, 15 Lawful Evil. [C]
- Also skip `montype` 2 (NPC) and monsters worth 0 exp. If a name matches several records,
  require **all** of them to be evil. [C]
- In-game, monster name colour shows alignment. [F, John]

## What attacks you [C LJNBKGND.C:2683-2942]
- Monsters of the four evil alignments attack players they can see; Good, Lawful Good and Neutral
  ones never start a fight; guards go for outlaws (40+ EP), Lawful Evil monsters leave outlaws alone
  until fought. Full rules in `grouping.md` §4.
- **(sic) The monsters' round visits each monster once per player in its room** (the outer loop is
  over players, LJNBKGND.C:2697-2732), so with three players in a room each monster gets three
  turns. Only the first strikes (no energy left after it), but each turn that picks a target
  raises that player's `attackers` count, which lowers the odds for the next pick by 5% each.
  An ordinary hostile monster always picks while it sees someone. Guards and Lawful Evil
  monsters may pick nobody (`grouping.md` §4).
- **Not every monster swings every round.** A monster strikes only with a full round of energy
  (LJNCOMBT.C:3235), and the 5 s refill reaches only monsters in the active cache. On a realm whose
  cache is full, evicted monsters skip rounds silently (`server-behaviours.md`, "A full monster
  cache") [C mechanism; I for the observed size of the effect].

## Monster spell attacks [C LJNMONST.C:3235-3560, :4922-5116; LJNCOMBT.C:3387-3430]
- **Four ways a monster casts.** (1) An attack slot of type `MATK_SPELL` (2): `atkaccs` is the
  spell number, `atkmins` the cast chance in percent, `atkmaxs` the cast level, `atkenergies`
  the cost. (2) `hitspells[slot]` after that slot's normal blow landed (LJNCOMBT.C:3703). (3) One
  `spells[]` entry after the round while the fight goes on (:3894). (4) `createspell` when
  generated (LJNMONST.C:917-923) and `deathspell` when killed (:1090-1091), both at level 1 with
  no player.
- **One roll, then everything.** `monster_cast` draws `genrdn(0,100)` first (:3269); the cast
  works when it is below the chance (`atkmins[slot]`; 100 for a cast from no slot, i.e. a hit
  spell or after-round spell, :3316-3319). Hit and after-round spells get no level bonus on
  strength or duration (:3509-3522, :3539-3542).
- **Energy.** Too little energy for `atkenergies`: nothing happens but the function returns 1
  (:3386), so the round does not fall back to a normal blow. A landed cast pays the full cost
  (:3499); a failed cast roll or a resist pays half, at least 1 (:3428, :3451); a free cast pays
  nothing. Only a return of 0 (no spell record, a refused lasting add with the energy given back
  :3579-3590, an area spell that did nothing) lets the round strike with slot 0 when slot 0 is a
  normal attack (LJNCOMBT.C:3418-3430). Casting never costs the monster mana.
- **Round loop.** Up to six picks per call (LJNCOMBT.C:3387 `rounds <= 5`), but only the first
  has the energy at the stock 1000 per attack: later spell picks are "too tired" (1), later
  blows are misses. A cast that kills the player returns 2 and ends the fight.
- **Resist.** Spell Immunity above the spell's level resists (:3354); a worn item that negates the
  spell resists; else, only for a cast whose roll succeeded and only for `SRES_ALWAYS` (or
  `SRES_ANTIMAG` with Anti Magic), `genrdn(1,100) <= min(mr/2, 97)` (:3398-3407). Texts (colour
  cyan): `You resisted <monster>'s cast of <spell>.` (:3415, the monster name as in the record,
  no "The"), room `<Player> resisted <monster>'s cast of <spell>.` (:3421), fail `The <monster>
  attempted to cast <spell> at you, but failed.` (:3437), room `... at <Player>, but failed.`
  (:3444). A landed cast uses the spell's WCCMSG triple with the monster as caster
  (`monster_display_spell_success`, :1589-1715, light red).
- **Target kinds.** `monster_cast` handles Single, Self or Single, Monster or Single and Any
  against the player; every other kind goes to the area cast with level `atkmaxs[slot]` (and,
  (sic), `atkmaxs[-1]` = `atkmins[4]` for a cast from no slot, :3313-3329). By number
  (`monster_cast_spell_by_number`, :5027): Self, Self or Single, Monster cast on the monster
  itself ("itself" to the room, a lasting spell goes into its own five slots, Poison adds to its
  poison, Summon makes the monster); Single and Monster or Single only when a player is
  fighting it; area kinds, Any and Item to the room (:5040-5075). Create and death spells have no
  player, so a Single-kind one does nothing.
- **After-round pick** (:5082-5116): no roll unless the monster, a player and `spells[0]` exist;
  one `genrdn(0,100)` against the cumulative `spellpcts[]`, first slot above the roll, cast at
  `spelllvls[slot]` (read as a signed `char`); a roll above every slot casts nothing.
- **Stock data** [C, WCCKNMSR]: no kobold shaman exists; the orc shaman (481) casts `harm` (12)
  in attack slot 1 at 95%, level 8, 1000 energy, and after the round `curse` 50% / `minor
  healing` to 100%; `harm` is `SRES_ALWAYS`, so a player's MR resists it at `mr/2` percent.
- Effects on the player: see `spells.md`; silent instants are Alter Hunger/Thirst, the death of
  the player is credited to nobody for 0 experience (:3763-3777).

## Evil points and the legal levels [C LJNEVIL.C]

`player.evil` places a character in one of eight legal levels (`get_legal_level`,
LJNEVIL.C:120-145; names :94-96, colours :83-92):

| Evil points | Level | Colour | Effects |
|---|---|---|---|
| below -200 | Saint | white | x3 cost to an attacker |
| -200 to -51 | Good | white | x2 cost to an attacker |
| -50 to 29 | Neutral | cyan | |
| 30 to 39 | Seedy | grey | free victim (`EVIL_VICTIM_LIMIT` 30) |
| 40 to 79 | Outlaw | red | `*` after the name for everyone (LJNDISP.C:2567-2581); rebirth in `evilreincroom` (142) (`OUTLAWPTS`/`EVIL_OUTLAW` 40) |
| 80 to 119 | Criminal | brown | a Lawful Evil monster's free attack on entry spares the player (LJNMONST.C:4692); a guard keeps hitting a downed player (LJNCOMBT.C:3330) |
| 120 to 209 | Villain | yellow | |
| 210 and more | FIEND | bright red | any player may attack them, whatever the level range (`pvp_level_range_ok`, LJNSUPP.C:967) |

`add_evil_points(attacker, victim, points, ticks, kind)` (LJNEVIL.C:249-417) runs before every evil
act: attacking a Good or Lawful Good monster (10 points, LJNCOMBT.C:2439), attacking a player (10,
LJNCOMBT.C:1725), robbing a player (1, LJNUTIL.C `rob_user`), evil spells (10, `EVIL_SPELL_PTS`
LJNSPELL.C:48). Every timer runs 11 rounds.

- **Tournament play**: nothing is checked or charged (:270-272).
- **A player victim with 30+ points is free**: no refusal, no points, but a timer with 0 points
  starts (:300, :366-367).
- **Refusals, in this order** (the act does not happen; nothing else is printed): warnings on
  (`set warning`, `PS_EVILWARN`) -> "To do this action, you must turn off your evil warnings."
  (:302-307); more than 300 points -> "You have progressed too far to the evil side to do this
  action." (:310-315); lawful (`player.lawful` -10, option ENABLAWF) -> "You have chosen a way of
  life which does not allow this action." (:318-323). ROB checks warnings and lawful itself first,
  with different wording: "You have chosen a way of life which prevents this action."
  (LJNUTIL.C:9107-9110).
- **Otherwise** "A dark cloud passes over you" in dark grey (:326-328), then the points: a lawful
  victim x3 (or 10 for a 0-point act), a Saint x3, a Good victim x2 (:335-354; the rules text says
  x2 for Saints, the code x3); a good attacker lands on at least 10 (:356-358); the total is not
  raised from 30000 (:360-363). A changed legal level takes off worn items the new level forbids
  ("Your <item> has been removed.", LJNUTIL.C:232-280).
- **Points kept at death**: a character deleted for good leaves `evil * EVILKEEP / 100` to the
  account's next character, or all of them if the last death was today (LJNEVIL.C:728-733).

## Attack modes
- `a <target>` is a normal attack. `bash <target>` is BASH: ×1.1, then ×3 damage, monster DR
  subtracted before the ×3, −15 to-hit, no crits, double energy capped at a full round. Typing
  it once keeps auto-combat bashing. [C] `notes/combat.md §2`
- **For heavy, slow weapons BASH is close to free.** A Witchunter at level 1 bashing with a
  quarterstaff did 39–42 damage per hit, against 2–12 for a normal attack. [C]
- Use a normal attack against high-armour monsters. Exact per blow (LJNCOMBT.C:1254-1398):
  BASH wins when `3·(avg₁₁ − DR)·(hit(AV−15) − 1) > (avg − DR)·(hit(AV) − 1)`, with `avg₁₁` the
  mean of the +10% range scaled end by end in integers, the integer hit chance and, in sustained auto-combat, the attack side times its blows per tick
  (≈1000/cost). For a level-1 Kang Witchunter with a quarterstaff (AV 58) that is AC ≤ 32 on a
  fresh round and AC ≤ 29 sustained. [C] `notes/combat.md §2` Recomputed from the C with the
  integer arithmetic (attack 605, bash capped at 1000, damage 2-14 and 2-15 after the +10%): the
  thresholds stay 32 and 29 for monster DR 0 to 3 (DR 5 moves the fresh one to 33). That is a
  proven value for this one character and weapon. The "about 0.45-0.6 × AV" rule for other
  levels and the DPR table in the notes are a model [I]: unproven, and the table's damage
  roll runs one too high (the C rolls `min..max`, not `max+1`).
- **Fast weapons crit more.** A blow whose energy cost comes out under 200 (after the
  encumbrance factor, before the floor of 200) adds `min(20, 200 - cost + (Agi-50)/10)` to the
  critical chance, halved from 33% encumbrance, nothing above 66% or with a Str shortfall
  (LJNCOMBT.C:892-901). The Kang quarterstaff at 18% load and Agi 42 costs 238 at level 16
  before the load factor and 199 after it: +1 at level 16, +9 at 17, +17 at 18, the full +20
  from 19 (recomputed from `compute_energy_used`, LJNCOMBT.C:221-250, with the fixture's
  quarterstaff, speed 1200, energy 0). [C]
- **Monster dodge and crits.** A monster dodges only through a Dodge ability (34) on its record
  (the sum over its ability slots) and never lands a critical (LJNCOMBT.C:1148-1149). 167 of the
  1,101 stock monsters have the ability (decoded from the WCCKNMSR fixture: giant bat 20, dark
  monk 15 and 25, wererat 50, quickling 65, pixie and leprechaun 68, Aldreth 100; the giant rat,
  kobold thief and acid slime have none). The dodge is rolled only after the blow has hit and
  its damage is rolled: it happens on `dodge*10/(AV/8)` percent of those blows (AV the
  attacker's attack value, integer division, capped at 95, none for AV 8 or less; a fifth of
  that against a backstab), and `genrdn(0,100)` must be under it (LJNCOMBT.C:1378-1404). Against
  AV 58 a Dodge 15 monster dodges 21% of the blows that would land, and a Dodge of 67 or more
  hits the 95% cap. A dodged blow does no damage, whichever move it was. [C]
- **Monster special attacks and buffs** are monster spells (the section above): slot attacks of
  type `MATK_SPELL`, `hitspells`, after-round `spells[]`, create and death spells. The dark monk
  (monster 228: Dodge 15, AC 30, DR 2, 125 HP, 525 exp) has five plain attack slots
  (4-14, 5-18, 9-30, 6-20, 4-14) plus, in the stock record, the after-round spells 36 "way of
  the swan" (75%) and 38 "way of the tiger" (to 100%), and the hit spells 318 "knockdown" and
  324 "power punch" on slots 4 and 5 [C, WCCKNMSR/WCCSPELS fixtures]. What those spells do to
  the monk or to you is not decoded here: the spell ability values in the fixture look
  misaligned, so treat the magnitudes as unknown. [I]
- The "Defence vs Good/Evil" abilities count only against a foe of another alignment group,
  and there Neutral Evil monsters are in the *neutral* group (LJNCOMBT.C:287-312, 389-397);
  the aggression rule above is a different test. [C]
- Every class in the stock data has Bash (ability 31). SMASH (×5) needs ability 32, and a
  Witchunter can only keep it from level 20. [C]
- Race and class abilities feed combat through `update_dynamic_stats` (LJNABIL.C:1230):
  "Alter Defence Value" (2) adds to the player's DV (a quarter of it against a backstab) and
  "Alter Damage Resistance Value" (7) to DR in tenths, a tenth of which comes off every hit
  (LJNCOMBT.C:754-761, :1371). Kang: Defence 5 is +5 DV (`acbonus`, LJNABIL.C:1061-1063), Damage Resist 10 absorbs 1 point per
  hit (`drbonus`, :1070-1072). Abilities add up over own slots, spells, race, class, worn items, the weapon and
  non-wearable pack items; a spare weapon or an unworn ring in the pack gives nothing
  (LJNABIL.C:321-514). [C] `notes/combat.md §4`

## How a fight runs [C LJNBKGND.C:340-424, LJNACMB.C, LJNCOMBT.C:2340-2830]
- **The round.** Every 5 s (`secenergy`, LJNGLOBL.C:194) each player gets `maxenergy` added to what
  is left, each monster the same; then the backstabs run, then (60% / 40%, LJNBKGND.C:382-392) the
  players' auto-combat and the monsters' round in that order or the reverse; at the end every
  player may cast a general spell again (`PS_CANCAST`) and a player who was attacked this round is
  "spotted" for the next. Outside auto-combat the energy is cut back to one round; in auto-combat
  the remainder carries over, except that a player whose player-target has left the room and who
  still holds a full round gets nothing added (LJNBKGND.C:438-445).
- **Typing an attack strikes no blow.** `a`, `bash` or `smash` on a monster outside auto-combat only
  engages: `*Combat Engaged*` to you, `<Name> moves to attack <monster>.` to the room (brown, line
  cleared first; a backstab says nothing to the room), your energy goes to 0, and the monster may
  start to follow you (never a charmed monster or an angel; always a Neutral, Good or Lawful Good
  one; any other on `genrdn(1,100) < followpct`; a guard already following someone stays with
  them). The first blow comes with the next energy round (LJNCOMBT.C:2420-2642). So an `a` typed
  into a fight that is already running prints `*Combat Off*` first and cuts energy above one round
  (LJNCMDA.C:332-336, LJNACMB.C:871-883), then the new attack zeroes it: the carried remainder is
  lost (pitfall 31). The blow loop only runs inside auto-combat, so the `You must rest before
  you may attack again.` branch of this function (no `\r`) cannot be reached (LJNCOMBT.C:2778-2780). [C]
- **Guarded monsters.** Attacking a monster with the Guarded by Monster ability makes it and every
  guard of that kind in the room start following you, and the last guard found takes the fight:
  `<guard> moves to protect <monster>` (LJNCOMBT.C:2496-2536). The "required weapon" and Magical
  refusals come before `*Combat Engaged*` (:2461-2582). [C]
- **"In auto-combat" means "queued".** A player is in auto-combat while his channel sits in the
  ring queue for the next round (`is_inside_autocombat`, LJNACMB.C:183). Each round takes the
  player off; only an attack or spell that carries on puts him back (`engage_autocombat` stores
  the move in progress and appends, :844-848). A round that does not re-queue him is his last. The
  `*Combat Off*` notice appears only where the code calls `display_autocombat_broken` (no HP left,
  afraid, a player target gone from the game, a monster target gone); a round that just returns,
  such as an attack in a protected room, lets the fight lapse in silence (LJNACMB.C:661-677,
  LJNCOMBT.C:2385-2405). [C]
- **A repeat of ATTACK / BASH / a spell** while queued is the command path again, so `*Combat
  Engaged*` and "moves to attack" appear again (`restart_autocombat`, LJNACMB.C:470-482).
  `BREAK` is the deliberate stop that also tells the room: you get `*Combat Off*`, they get
  `<Name> breaks off combat.` (LJNCMDS.C:748-752, LJNACMB.C:152-165); other stops show you only
  `*Combat Off*`. [C]
- **A target that leaves.** A monster target that is gone or elsewhere ends the fight at the next
  energy round with `*Combat Off*` and no room notice (`validate_auto_combat`, LJNACMB.C:418-440).
  The same when the character is the one who leaves: fleeing up the stairs out of the Newhaven
  Arena mid-fight, the new room showed, then `*Combat Off*` after its prompt (silvermere,
  wayfarer C-028 live check). So a bot that flees is out of combat within a round, without a
  `break`. [C]
  A player target that leaves only sets `ST_TARGETGONE` and zeroes `pvpcnt`; auto-combat goes on
  until he returns or you break. [C]
- **SMASH knock-down.** `str + level > monster level + magicres + 30` knocks the monster down
  (`The <monster> is smashed to the floor defenseless!`, light red, to you and the room) for
  `smashrounds` (15), with -35 on its AV and DV (`smashpenalty`, LJNGLOBL.C:184-187). The 15 is a
  counter, not 15 rounds of yours: it drops by 1 on every fast monster pass (about once a second,
  LJNBKGND.C:1420), at the start of each of your rounds against it (LJNCOMBT.C:2676-2703), at the
  start of each of its own attack rounds against a player (:3473-3500) and when it tries to walk
  (LJNMONST.C:1242). It is up after about 15 s idle and sooner in a fight, with `Slightly dazed
  the <monster> rises from the floor.`; every new knock-down resets the counter to 15 (:3071). So
  a monster smashed every round (5 s) never gets up. [C]
- **Who sees the misses.** The room sees a plain miss, a dodge or a glance only if its message
  level is high (`TRN_HIGH`); hits go to everyone. A glance (armour took all the damage) and a
  miss both use the *first-person* miss verb in both views (`Your swipe at X hits, but glances
  off its armour.` / `Bob's swipe at X hits, ...`). A monster you attack that dodges shows
  `<Name> <miss verb> <monster> but it dodges.` to the room, with no prompt on your own line
  (LJNCOMBT.C:3085-3128). [C]
- **Monster-on-player texts** come from the attack slot's WCCMSG records, each with two extra
  `"*"` arguments and the first letter upper-cased (:3666 and the like): a hit uses text 1 / 2,
  a deflect (armour took everything) the slot's deflect record text 1 / 2, a miss the miss record
  text 2 / 3, and a dodge deflect text 3 to you plus miss text 1 to the room when the slot has
  both records; a slot without them gets built-in texts (`X's <verb> hits you, but your armour
  deflects.`, `X <verb> you with <weapon>, but you dodge!`, `X <verb> you with <weapon>.`)
  (LJNCOMBT.C:3717-3871). `<Name> appears!` before a monster's blow means it was not listed in
  its room (`confirm_monster_in_room` failed, :3341-3347), not that it was hidden. [C]
- **Experience share.** On a kill the killer always counts as 1, whether or not he is queued, plus
  every other queued player fighting the same monster (or a queued caster with no target in the
  room); `exp /= count` drops the remainder (minimum 1). Each sharer and the killer then has his
  auto-combat ended (`*Combat Off*`, no room notice), except the no-target casters, who get the
  share and keep fighting (`distribute_experience`, LJNACMB.C:949-1045). [C]
- **Backstabs** are run before anything else each round, and a backstab entry that did not strike
  is re-queued behind the round marker (LJNACMB.C:1160-1185, LJNBKGND.C:382-392). [C]
- **A downed player.** Once you are at 0 HP or less, Good and Neutral monsters (and any unknown
  alignment) leave you alone; Evil, Chaotic Evil, Neutral Evil, Lawful Evil and (sic) Lawful Good
  ones keep hitting you, except guards, which stop unless you have 80 or more evil points
  (LJNCOMBT.C:3318-3335). [C]
- **Free attack on leaving** (rules in `navigation.md`): the monsters are tried in slot order and
  the loop stops at the first one that strikes, and also at once if you already have an attacker
  this medium cycle (`attackers` is 1 or more), so a room you are already fighting in gives no
  extra free blow (`give_monsters_a_free_attack`, LJNMONST.C:4652-4720). [C]
- **A Flee spell on a monster**: each round the lasting effect has `genrdn(0,100) < value` to
  make the monster leave by a random usable exit
  (`pick_valid_random_direction`: the first usable exit, each later one replaces it on 40%;
  keys, doors, gates, action, spell-trap and block-guard exits count, text, toll, hidden, trap,
  class, level and the other gated ones never do; LJNMISC5.C:189-245, LJNSPELL.C:9419-9424).
  A player's Summon spell creates its monster with the room's spawn limits ignored and makes it
  a friendly, charmed follower of the caster (LJNSPELL.C:3757-3765). [C]

## After a kill, a death, a robbery [C]
- **A monster dies** (`check_kill_monster`, LJNMONST.C:1030-1122): the killer always gets a
  line-clearing block with one `N <coin> drop to the ground.` per currency it carried (a blank line
  when there are none, which is the blank before the death text in the captures); the death spell
  goes off at level 1 with no player; the room sees WCCMSG death text 3 (`msgs[2]`); its carried
  items are left in the room, or in a neighbour when it is full (`dispose_of_item_in_room` tries
  the visible slots twice and never the hidden ones (sic), and follows non-remote, non-map-change
  exits up to depth 5; LJNUTIL.C:4168-4228).
- **Damage shield on the attacker**: `You took N damage!` when a PvP victim's shield hits you
  (LJNCOMBT.C:1987, :2041); a monster's ends in `.` (:2867, :2916).
- **ROB on a player** (`rob_user`, LJNUTIL.C:9079-9230): a lawful or warning-on character gets
  `You have chosen a way of life which prevents this action.`; a roll
  above `thievery + 10` (`genrdn(1,100)`) is a clumsy attempt the victim feels. With nothing named
  the target is coins about half the time (a random
  currency copper to platinum, `genrdn(CUR_COPPER, CUR_RUNIC)` never gives runic, :9195) or an
  item otherwise: each filled slot is picked at 50% and the last pick stands, but (sic) only if the
  item *has* Loyal Item (:9205-9213). The guilt line has no full stop (:9136). ROB on a monster
  (`rob_monster`) was never written: it only refuses a forced-lawful character
  (LJNUTIL.C:9378-9395), and `monster_rob_user` (LJNMONST.C:4560) always returns 0. `KILLBLOW`
  is not a command (`cmd_killblow` returns 0, LJNCMDA.C:534-540). [C]
- **Death in a safe Colliseum** (`arenasafe`): you get back the HP you had on entering (`arenahp`)
  and full mana, keep items and lives, and wake in the room's own death room if it has one, else the reincarnation room (the evil one at 40+
  evil points, LJNUTIL.C:4329-4330, :4391); you still see
  `But, because you were in a colliseum, you have been saved.` and `You have N lives left.`
  (LJNUTIL.C:4400-4416). [C]
- **Cosmetic roll**: `pick_random_attack_move` rolls `genrdn(0,100)` before every blow to choose a
  "move" 9-17 and a percentage 75-125, but no code reads either (LJNCOMBT.C:107-116, :1444-1535). [C]

## Danger estimation (pre-fight)
- Per monster: largest single hit (`atkmaxs` over its attack types), HP and AC.
- **Area safety rule:** skip areas where any spawn hits for more than a quarter of your max HP,
  or has more HP than you. [C] `progress.py Bot.safe`
- **Group rule:** if the expected hits from everyone hostile in the room (about 0.6 × max hit
  each) exceed half your current HP, back off. [C] `Bot.too_many`
- Rooms can hold up to 15 monsters (`maxmonsters`). Two at once took a level-1 Kang from 48 to
  16 HP. [C]

## Fleeing
- Below 30% HP in combat: leave by the first normal exit and rest. [C]

## Choosing the attack (bash / attack / spell)
- Options: `bash`, `a`, and each affordable single-target attack spell. If a heal spell is
  known, mana for two heals is held back.
- Model: melee = weapon average less monster DR, × ~60% hits (−15% for BASH), less an AC
  penalty; BASH ×1.1 then ×3. Spell = damage at caster level × ~72% cast success × (1 − MR/200).
- Measured: damage per action per mode, per character; after 15 actions the measured average
  replaces the model. 15% exploration of under-sampled modes. [C] `progress.py attack_options`
- Low-level casters usually do better bashing with a staff than with their first nuke, and it
  saves mana for heals (John's hunch, confirmed by the model).

## Auto-combat etiquette (John, observed)
- Re-issuing `a`/`bash`/a spell while auto-combat runs **restarts it** ("*Combat Off*" then
  "*Combat Engaged*"). The swing timer resets, so the first round has fewer swings and the
  fractional energy carried into the next round is lost. Issue one command per target and let
  auto-combat run.
- But don't stand idle while something hits you: if attacked and not engaged, engage at once.
