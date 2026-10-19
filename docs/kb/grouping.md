# Grouping, party play and multi-character boxing (MajorMUD 1.11p, stock data)

Tags: **[C]** read in the source (`WCCMMUD-1.11p-source/SRC`) or the game data (`mmud_data`), **[I]** inferred,
**[F]** forum or web. Monster stats are `hp/exp/maxhit`, where exp is `exp × level`, the amount a kill pays
(LJNCOMBT.C:1139). `N` = room `maxmonsters`. `rg` = regen minutes after a kill. Steps = walk from Town
Square 1:224 (`—` = no plain walk within 400 steps: keys, hidden exits or a map change). Related:
`kb/aoe-grinding.md` (spawn and AoE rules), `kb/classes/*.md` (per-class spells), `test/play/corpus/bosses.md`.

## 0. The six facts that drive everything [C]

1. **Experience goes only to whoever is in autocombat against the dying monster, split equally.** Party
   membership, level and level difference play no part. No range limit, no penalty, no leader bonus
   (`distribute_experience`, LJNACMB.C:949).
2. **Monsters pick targets at random among the players they can see.** Rank, damage dealt, healing and
   "threat" don't count. There is **no taunt, guard or protect** (`GUARD` is a stub, LJNCMDA.C:2820-2826;
   `DEFEND` and `HUNT` fall through, LJNCMDS.C:1095-1096). The only way to avoid being targeted is not being
   seen: hidden (only See Hidden monsters see you), or sneaking against a monster without See Hidden
   (`monster_can_see_user`, LJNBKGND.C:2631-2652; `do_monster_combat`, :2683-2942).
3. **Casting any spell ends your autocombat** (LJNCMDB.C:536-660). A healer who stops to heal is out of the
   exp split until they re-engage with `a <monster>`.
4. **Party = INVITE + FOLLOW.** At most **6 people**: the leader plus `NGROUP` = 5 slots (MAJORMUD.H:39).
   Pending invitations use slots too.
5. **Party-area spells reach only party members, and attack-area spells never hit them** (LJNSPELL.C:880-955).
   Being partied is what makes AoE safe around friends.
6. **More players in a Normal or Arena room means more spawns.** Monsters spawn while monsters < players
   (LJNBKGND.C:2411). Lairs fill to `maxmonsters` whatever the head count.

## 1. Commands [C LJNPARSE.C, LJNCMDS.C, LJNCMDA.C]

| Command (abbrev.) | Effect | Notes |
|---|---|---|
| `invite <name>` | Lets that player follow you. Writes them into your `group[]` and sets **you to front rank**. | Target must be in the room, visible, and not have you on `forget`. They see "*X has invited you to follow him.*" |
| `uninvite <name>` | Removes them: they stop following and the slot is freed. When nobody is left, your rank is reset. | "*X has been removed from your followers.*" |
| `follow <name>` / `join <name>` (`fo`, `jo`) | Join the leader's party (needs an invitation). Clears your own `group[]` except the leader, then links every member to every other member. | "*You are now following X*" / "*You must be invited first!*". You can't follow monsters ("Not implemented yet"). |
| `follow` (alone) or `leave` | Stop following. | "*You are no longer following X.*" A leader can't drop out except by uninviting everyone; the leader's death or logoff ends the party (`stop_following(-1)`). |
| `party` | Lists the members with class, **M/K% and H%**, P (poisoned), R (resting) / M (meditating), and rank. | **This is the client's cheapest status feed for the whole group** (LJNDISP.C:5071). |
| `frontrank` / `midrank` / `backrank` | Battle-line position. Front: +15 AV, −10 `defalign` (the defence-vs-alignment term the hit formula adds to DV; DV itself is untouched). Back: −10 AV, +15 `defalign`. Because a `break` is missing, **front actually gets +5 AV and +5 `defalign`**. Mid: 0. A jumpkick keeps its AV. | LJNCOMBT.C:834-849. The leader can't go to the back rank. The last front-ranker can't leave the front. Back may not outnumber front. Changing rank ends resting. |
| `join <1-255>` / `join 0` | Broadcast channel (the in-game "group chat"). `'text`, `-text` or `br text` sends to the channel; `'` alone lists who is on it. `leave` (with no leader) leaves it. | Channels are separate from parties. Profanity is never checked on them (sic). |
| `/name text` (telepath), `>name text` (say-to in the room) | Private or directed talk. | Each delivered message counts against the target's page allowance (`pagecnt`), which resets every 3-s medium cycle (default 3, `set maxpages N` up to 255; the check is `pagecnt <= pagemax`, so N+1 get through, LJNCMDA.C:4554). A refused telepath shows `--- Telepath Not Sent ---` to the sender. **Say-to is different:** it is refused (`--- Message Not Sent ---`) only when the target is in another room or map, or hidden; blocking telepaths and the forget list don't stop it, and the allowance isn't checked. Fine for bot coordination at a few per cycle: see `plan-grouping.md` §2 and §1a below. [C] |
| `share <amt> <currency> [with room\|party]` | Splits coins evenly. **Party is the default.** The sharer keeps the remainder. | Hidden players count only if you see hidden. Players with no-receive set are skipped. |
| `aid <name>` | A player at ≤0 HP **regains 1 HP per slow tick instead of bleeding 1**. | LJNBKGND.C:1624-1645. |
| `drag <name> [dir]` | Carries a downed (≤0 HP) player along. | **Not allowed while you are a party follower** ("*You may not drag someone while in a party!*"). The leader can drag. Not out of the Colliseum. |
| Gangs: `create gang <name>`, `invite member <n>`, `uninvite member <n>`, `join gang <name>`, `leave gang`, `disband gang`, `promote`/`demote`, `gb text` / `broadgang` | Persistent guild. Kills also add to the gang's exp total. | **Gangs have no combat effect.** [C] Membership is only the gang name in your character; the gang record counts members. `create gang` (or `guild`) needs 100,000 exp (or any billions), no gang, a name of at most 19 printable characters, not `None`, not taken in any case; a disbanded gang's name stays taken until its record is deleted (LJNCMDA.C:5002-5079). Leader **or lieutenant** may `invite member` and `uninvite member`; only the leader may remove a lieutenant, nobody can uninvite the leader, a lieutenant who tries is told `Such insolence ...` (LJNCMDA.C:4022-4087, :4207-4308). `uninvite member` works on members who are off line (their record is edited, LJNGANG.C:313). No gang size cap is checked on invite. Only the leader may `promote`, `demote` and `disband gang`; the leader may not `leave gang` (LJNCMDA.C:2447). An off-line member gets the rank change and a notice at next logon. `disband gang` asks `Are you sure you want to disband <gang>? `; only a one-word answer starting with Y disbands (LJNGAME.C:4771-4809); members off line lose the gang at their next logon. Invitations live in memory only: lost when the BBS restarts, dropped when the invited player leaves the game, and all of a player's invitations go after any `join gang`, right or wrong (LJNGANG.C:432-489). `gb` alone lists the members (online only under `set gang online`). `leave gang` is announced to the **room**, not the gang (LJNCMDA.C:2435). The top-gangs list shows a gang's exp only for a debugging sysop or with `EXPTOPGANG`; for a gang past the exp cap it adds `(%d times)` with `overflows+1` and `+<overexp>` (LJNDISP.C:390-399). Gang shops (STOCK, UNSTOCK, MARKUP, takings): `kb/economy.md`. |
| Stubs (return 0): `guard`, `defend`, `hunt`, `dismiss`, `play`, `take`, `shoot`, `identity` | — | Don't build features on them. |

## 1a. Talk, channels and the forget list [C]

Confirmed from the source for the silvermere T-045 port (silvermere#49).

- **Telepath** (`/name text`, `WHISPER name text`; `cmd_whisper` LJNCMDA.C:4502). The name may be an
  abbreviation of any word of a first name; a full name wins outright; an abbreviation fitting several
  players gives the "Please be more specific" list and the prompt, and nothing is sent (`get_user_number`
  LJNUTIL.C:7765, LJNCMDA.C:4534). Delivered only when the target has a page left (`pagecnt <= pagemax`,
  so pagemax+1 per 3-s period), doesn't block telepaths (`PS_NOTELEPATH`) and hasn't forgotten the sender
  (LJNCMDA.C:4554-4556). The sender sees only `--- Telepath Sent to X ---` or `--- Telepath Not Sent ---`,
  never which check failed (:4582, :4587). A sender who blocks telepaths may not send one (:4541).
  `TELEPATH name` with no message prints `What do you really mean?` but **has already used one of the
  target's pages** (sic, :4557, :4573). The host's `/P`, `/#`, `/SYS` don't exist in-game: `/P x` is a
  telepath to whoever "P" names.
- **Say-to** (`>name text`; `cmd_direct_message` LJNCMDA.C:4621). The target sees `X says (to you) "…"`,
  everyone else in the room `X says (to Y) "…"`. **Refused only when the target is in another room or map,
  or hidden** (LJNCMDA.C:4668-4669, `--- Message Not Sent ---` :4713-4714). `PS_NOTELEPATH` and the forget
  list play no part. A mute player gets `You may not direct any messages right now.` (:4662). It adds one
  to the target's `pagecnt` but never checks it (:4671).
- **Gossip and auction** (`cmd_gossip` LJNCMDB.C:5592, `cmd_auction` :5661). Separate on/off switches
  (`gossip`/`auction` alone toggles), separate keys and separate allowances (`gossmax`/`auctmax`, default 3,
  `set maxgossip N` / `set maxauction N`). The line goes out as **two** `tell_game` calls: `X gossips: `
  first, then the text in the `COL_BROADCAST` colour with the prompt (:5639-5642, :5707-5710). **Each call
  counts against every listener's allowance** (LJNTELL.C:218-236, `count <= max`), so one gossip costs a
  listener 2 counts: at the default 3 a listener hears **two** whole gossips per period, not four. With an
  even max (e.g. 4) the third gossip's `X gossips: ` arrives without its text. The speaker is a listener too
  (`tell_game(-1)`), so their own gossips use up their own allowance, and they get a second prompt after the
  line. With the channel switched off, your own gossip is refused with `You may not use gossip while your
  gossip is turned off.` / `Type 'gossip' to turn gossip on again.` (:5618-5619).
- **Channel broadcast** (`'text`, `-text`, `br text`; `cmd_broadcast` LJNCMDA.C:2737). Goes to everyone on
  your channel, you included (with a prompt, then the command's prompt again). `' hi` (a space after the
  quote) broadcasts `" hi"` with the space (the text is `margv[0]+1` after `rstrin`, :2766-2768). Off any
  channel the line isn't a command and becomes speech.
- **Forget list** (`forget <name>` toggles, `forget` alone lists newest first; LJNFORGT.C). Per session, in
  memory, cleared at logoff and hang-up. Holds up to max(`nterms`, 20) names, stored as typed and never checked
  against players. A name of 11 bytes or more is not a command and falls through to speech (:219). The add
  and remove lines end in `\n` rather than `\r` in the source (:161, :181). It stops telepaths and
  invitations from that first name, not say-to.
- **Say and yell** (LJNPEND.C:500-597). `.text` says; `"text` yells to your room and to every room one exit
  away, heard there as `Someone yells from the <reverse direction>` or `from above`/`from below`. Nobody
  hearing it gives `Your command had no effect.` unless `talk fast` is set. `set talk brief`
  (`PS_BRIEFTALK`) only changes your own echo to `Well Spoken...` (:582-587).
- **GOSSKEY/AUCTKEY are gag keys in the source:** *holding* the key sets `hasgoss`, and `hasgoss != 0` is
  refused with `You may not use gossip - please contact your sysop.` (LJNGAME.C:3005-3014,
  LJNCMDB.C:5611-5613). Sysops and game operators are never gagged.

## 1b. PvP evil timers and FORGIVE [C LJNEVIL.C]

- An evil act on a player starts a timer attacker -> victim (one per pair, list oldest first,
  LJNEVIL.C:535-575). It counts down one round per slow update of the **attacker** (every 30 s,
  `slow_update_character` LJNBKGND.C:1574), so 11 rounds is 5.5 minutes (:627-644).
- While it runs: the victim strikes back for free, and the attacker's further attacks on the
  same victim cost nothing (`should_give_evil` :200-220); an attack replaces a robbery timer; a
  robbery always replaces the pair's timer, keeping the star of a robbery the victim saw
  (:279-297).
- **The star**: `Also here:` puts `*` after a player who has a timer either way with the viewer,
  or 40+ points, unless that player robbed the viewer unseen (`KIND_NOSTAR`, the failed-quietly
  and successful robbery paths) (LJNDISP.C:2567-2581, LJNEVIL.C:492-510). WHO shows no star.
- **Retaliation period**: while the attacker holds a timer that charged points (not one on a 30+
  victim), they may not enter a protected room or the safe arena ("You may not enter that room
  during a retaliation time-period.", LJNUTIL.C:2582, :2610) nor SYSOP GOTO ("You may not teleport
  during a PVP retaliation period.", LJNCMDB.C:1827) (`is_in_retaliation` :997-1008).
- **FORGIVE <name>**: only the victim can forgive; the attacker's timer goes and the points it
  charged come off again. "The gods have forgiven you for your action." to the attacker, "The gods
  have forgiven him for his action." to the victim; with no timer "The gods refuse to forgive him
  for his actions." (LJNCMDA.C:5316-5374, LJNEVIL.C:939-988). (sic) With two or more timers
  running, forgiving one after the first frees the wrong timer (:982): heap corruption in the
  original.
- (sic) An expired first timer of the list is only freed once it is the only one; until then it
  keeps counting and its signed byte wraps to 127 (:589-617).
- Timers are kept by channel and are not cleared when a player leaves.

## 2. Moving as a party [C LJNUTIL.C:2711-3800, LJNBKGND.C:2063-2310]

- Followers take the leader's direction in the same fast cycle (`move_user(...,3)`). Each follower runs the
  **full exit checks on their own**: class or level exits, locked doors, traps, encumbrance over 100%. A
  follower who can't pass, is held or stunned, or is at ≤0 HP **silently drops out of the party line**
  (`plr->leader = -1`, "*You are no longer following X.*"). **Text exits work for followers** (how 3 counts
  like typed words).
- Followers see "` -- Following your Party leader <dir> --`". Their arrival isn't announced to neighbouring
  rooms.
- **Every member who is seen leaving gives the room's monsters a free attack** (one room roll against each
  monster's followpct). Retreating as a party costs N free-attack chances, not 1.
- **A sneaking follower stays sneaking** through party moves. A ninja or thief can trail the tank and then
  backstab [C]. A sneaking or hidden member isn't targeted by monsters without See Hidden, and not at all on
  the round they arrive (ST_JUSTARRIVED) [C].
- Moving ends resting and meditating for everyone who moves.

## 3. Experience, loot, healing and death [C]

| Topic | Rule | Source |
|---|---|---|
| Who shares a kill | (a) the killer, always; (b) **everyone in autocombat with that monster as target**; (c) everyone in autocombat **with no target** (an AoE spell being recast) in the same room. | LJNACMB.C:927-1045 |
| Split | `exp / count`, integer, at least 1 each. **Equal, whatever the level.** A level-1 alt that lands one `attack` on a kobold king gets the same share as the L20 tank. | same |
| Not counted | Anyone fighting a *different* monster. Healers or buffers whose cast ended autocombat. Followers who are just standing there. Party members in another room. | same |
| Training cap | Exp past (needed for level + `xtralvl`) is refused: "*You have progressed too far without training…*". Every boxed alt must keep training. | LJNUTIL.C:1635 |
| Class/race exp% | Changes only how much exp a level needs, not the share. With equal shares a Thief (−20%) out-levels a Ranger or Mystic (+150%) on the same kills. | LJNPRMPT.C |
| Loot | When a monster dies its coins and items drop on the floor: **first `get` wins**. No loot rules. Split coins afterwards with `share`. | LJNMONST.C remove_active_monster |
| Heals on others | "Self or Single" spells: minor/major/greater healing, godheal (Priest line), mend/regeneration (Druid, Ranger). Party rains: healing rain L10, major L17, greater L24 (Priest only), divine mercy L30 (Priest), rejuvinating field L32 (Druid HoT). Per-class numbers are in `kb/classes/*.md`. | WCCSPELS target 2/13 |
| Party buffs (Full Party Area) | Chant L6 (+AV), holy/unholy aura L9 (Priest), song of valour L2 (+5 AV), song of life L8 (**+50% party regen**), song of traveling L9, bard sageness/knowledge/force/swiftness L24-30, soothing L35 (+75 regen), battle/elements L40; mass frenzy L20, wind of haste L22, shimmering mirage L38 (Mage; no shop sells the scroll, item 2012 is a 1% drop of the rakshasha sage, room 12:2247 [C, stock data]). | WCCSPELS target 13 |
| Regen | No group regen bonus apart from songs of life and soothing. Each member `rest`s (or `meditate`s) separately. Resting is refused while anything is attacking you. | LJNBKGND.C:2104, 1651 |
| Downed member | At ≤0 HP they bleed 1 HP a slow tick unless aided. `aid` turns that into +1 HP a tick. The leader may `drag` them out. A downed follower drops out of the party line. | LJNBKGND.C:1624 |
| Friendly fire | Attack-area spells (fireball and the like) skip the caster, their leader, their followers and their `group[]`. **Non-party players in PvP level range in the room *are* valid targets** (evil act). Monster area spells hit everyone. | LJNSPELL.C:830-955 |
| AoE engage | `cast fbal` outside combat → "*Combat Engaged*", and auto-combat recasts it each round. That caster then shares **every kill in the room**, melee kills included. | LJNSPELL.C:2097 |

## 4. Aggro in detail [C LJNBKGND.C:2683-2942]

Every energy round (5 s), after the backstabs, a `genrdn(0,100) < 60` roll puts the players' autocombat
before the monsters' round, else after it (LJNBKGND.C:382-392). In the monsters' round, for each player
(in the `mappings[]` order, reshuffled every slow cycle, LJNBKGND.C:625), each monster slot of that
player's room in order takes a turn:
- **Hostile alignments** (Evil, Chaotic Evil, **Neutral Evil**, Lawful Evil): it walks the visible players
  in that order. Each one is attacked with chance `(50 − 5 × attackers)%` (`genrdn(0,100) < 50 -
  attackers*5`, :2918). If none is picked, the last one seen is attacked (:2928-2934), so a lone visible
  player is always attacked. Lawful Evil monsters skip players with 40+ EP unless that player is fighting
  them, without a roll (:2909-2914).
- **Good, Lawful Good and Neutral monsters never start a fight** (:2894-2896). Once attacked, they fight back
  through the attacker's autocombat.
- **Guards** (`MTYPE_GUARD`, :2829-2878): the first visible player already fighting it; otherwise a guard
  that is not Lawful Evil attacks players with ≥40 EP at `(100 − 5 × attackers)%` each, never anyone under
  40; a Lawful Evil guard does the same to players under 40 and leaves outlaws alone. No fallback.
- **Followers** (`actmon.following`, :2733-2807): a friendly guard attacks the first visible player fighting
  its master; a charmed monster fights its master's autocombat target (`do_charmed_monster_combat`,
  LJNACMB.C:1086; if the master attacks the pet itself the charm breaks); any other friendly follower that is
  not Good/Lawful Good/Neutral attacks the first visible player who is not its master; an **unfriendly
  follower attacks its master** whatever its alignment. A master who is offline: nothing.
- **Hunters** (`actmon.hunting`, :2809-2825) walk one step along the hunted monster's trail (unless
  confused), or attack it when the trail gives no step (sic: also when it is not in the same room).
- `attackers` is raised **before** each attack and zeroed by the medium cycle (`medium_update_character`,
  LJNBKGND.C:1919) every 3 s (`secmed`, LJNGLOBL.C:106), so it counts this round's picks (and free attacks).
- **(sic) A monster is visited once per player in its room.** With N players in a room, every monster gets
  N turns a round. Only the first can strike (the strike spends its energy and `attack_monster_user` needs a
  full round of energy, LJNCOMBT.C:3232-3235), but each later turn runs the target choice again and, when it
  picks someone, raises that player's `attackers`. An ordinary hostile monster always picks someone while it
  sees a candidate (the `lastseen` fallback). A guard can pick nobody (no fallback: nobody fighting it and
  no one of the alignment it goes after). So can a Lawful Evil monster whose visible players are all
  outlaws not fighting it. An unfriendly follower picks its master only while it sees him. So in a party
  the −5% term grows with the party size, not just with real attacks.
- So **every visible member takes about 1/N of the hits**, spread a little by the −5%/attacker term. A robe
  caster in a 3-person party eats about a third of the damage. "Tanking" means **high DR and HP on everyone
  plus fast heals**, not holding aggro [I].
- The only real protection: stay **unseen** (sneak or hide; a rogue scouting ahead), or put a **charmed or
  friendly-guard pet** in the fight (it attacks your attackers but doesn't draw hits).
- (sic) `monster_can_see_user` spares a sneaking player on the round he arrived (`ST_JUSTARRIVED`,
  :2643-2649), but the energy round clears that flag for everyone first (`energy_update_character`, :436), so
  in practice a See Hidden monster sees a sneaker at once.

## 5. Spawns with several players [C LJNBKGND.C:2378-2560]

| Room type | Solo | Party of P |
|---|---|---|
| Normal | 5% per 5-s cycle **while monsters < 1** | 5% per cycle **while monsters < P** (room cap 15). Each neighbour also rolls 5% per exit while its monsters < P. **A 4-party roughly quadruples the standing monster count in plain hunting grounds.** |
| Arena | 90% while monsters < 1 | 90% while monsters < P |
| Lair | Fills to `maxmonsters` | Same. A group gains nothing extra but can clear big-N lairs safely. |
| All | `regendelay` after any kill in the room (0 → 5 min) | Same. Rotate lairs. |

## 6. Class roles [C class data: armour type, combat, magery; I roles]

Armour: 9 plate, 8 scale, 7 chain, 6 leather, 2 padded, 1 robes. Exp% = the class's extra exp needed per level.

| Class | Armour / combat / exp% | Group role | Brings |
|---|---|---|---|
| Warrior | 9 / 6 / 0 | Front-line damage, DR soak | BASH, platemail. Cheapest to level. |
| Witchunter | 8 / 7 / +30 | Front-line damage, anti-caster | Highest combat value, Anti-Magic (MR). No heals, and **resists friendly heals and buffs at MR/2 %** (LJNSPELL.C:1130, 3095), so it's a poor fit for a healer-based group. Best solo, or as off-tank/DPS with its own regen (rest, healer shops). [C] |
| Paladin | 9 / 6 / +120 | Off-tank + backup healer | Minor/major healing, healing rain L10, chant, turn undead. |
| Cleric | 9 / 5 / +120 | **Main healer who survives being targeted** | Healing rain L10, major rain L17, greater healing, divine favour, black wind. |
| Priest | 1 / 3 / +40 | **Strongest healer**, fragile | Greater rain L24, godheal L22, divine mercy L30 (party 25-65+), auras, turn undead. Needs heals on itself first. |
| Missionary | 6 / 4 / +120 | Healer-rogue | Healing rains, chant, cleansing, turn undead, picklocks and traps. |
| Druid | 6 / 4 / +90 | AoE + secondary healer | Swarm L12 → earthquake L28, mend/regeneration, rejuvinating field L32. |
| Mage | 1 / 3 / +40 | **Primary AoE** | Dancing blades L11, fireball L15, mana storm L23, **spirit horde L28**, mass frenzy/haste. |
| Warlock | 7 / 5 / +85 | AoE + melee | Fireball L15, mana storm L23, chainmail. |
| Gypsy | 6 / 4 / +120 | AoE + rogue | Mage AoE line, picklocks, traps, debuffs. |
| Bard | 6 / 5 / +110 | **Party buffer** | Valour, life (+50% regen), traveling, shockwave L19 AoE, picklocks. |
| Ninja | 2 / 5 / +85 | Opener and scout | Sneak + backstab, then melee. Unseen until it strikes. |
| Thief | 6 / 4 / −20 | Scout, locks, opener | Backstab, picklock, traps. Fastest-levelling alt. |
| Mystic | 1 / 5 / +150 | Melee damage | Unarmed crits, backstab opener. Fragile early. |
| Ranger | 6 / 6 / +150 | Melee + self-heal | Mend, barkskin, stealth. Shares the Druid's swarm. |

## 7. Recommended compositions (also the boxing builds) [I from §0-§6]

Design rules: (1) everyone eats about 1/N of the hits, so **avoid robes before ~L15 unless a plate healer
backs them**. (2) One AoE caster turns lairs into time multipliers (`aoe-grinding.md`). (3) The healer
must re-`attack` after each heal or it falls behind on exp. (4) For boxing, pick classes with similar exp%
so the alts level together.

| Size | Band | Composition | Why |
|---|---|---|---|
| 2 | 1-10 | Warrior + Cleric (or Paladin) | Two plate bodies. The cleric's minor healing on the warrior. Both melee the same target. |
| 2 | 10-25 | **Cleric + Mage** (or Cleric + Druid) | Classic pair. The Mage AoEs lairs (fireball L15, mana storm L23). The Cleric's healing rain/major rain keeps both up and it absorbs half the hits in plate. |
| 2 | 15+ | Warrior/Paladin + Priest | Melee killer plus the best heals. Priest in robes: keep it back rank (+15 DV). (Not a Witchunter: it resists the Priest's heals at MR/2 %, so the higher its Wil, the more heals are wasted.) |
| 3 | 1-15 | Warrior + Cleric + Thief | Thief backstab opener, picklocks, scouting. Thief exp −20% → it keeps level with the others. |
| 3 | 15-30 | **Warrior/Paladin + Cleric + Mage** | DR front + rains + AoE: clears N=5-10 lairs (Darkwood Clearing, Smuggler Den, nomads). |
| 3 | 28+ | Paladin + Priest + Mage | Spirit horde, divine mercy, double healing. |
| 4 | 10-25 | Warrior + Cleric + Mage + Bard | Bard valour (+5 AV all) and life (+50% regen) cut downtime, which is the real limit [I]. |
| 4 | 20+ | Warrior + Cleric + Mage/Druid + Ninja | Ninja scouts unseen (find the boss's spawn state, safe pulls) and opens with backstab. |
| 5-6 | 25+ bosses | 2 front, 2 healers (Cleric + Priest), 1-2 damage | Goru-Nezar, Champion of Blood, giants. Forum sizes: 4-5 at L25+ (t=6), "about six L20s" for the Blood Champion (t=11) [F]. |

Avoid: two robe casters with no plate healer; Ranger and Mystic boxed with a Thief or Warrior (exp% spread
of 150-170 points) [I].

## 8. Group grinding spots by level band [C room/monster data; I ranking]

Exp/head = clear exp ÷ party size. "Why group" = what a solo player can't handle.

### L1-10
| Map:room | Area | Type, N, rg | Pool / boss | Steps | Why group |
|---|---|---|---|---|---|
| 1:2617 (+1:2637 N4, 1:2633/2647 N3) | Graveyard edges | Lair 5, rg 1 | skeleton 43/55/11, zombie 62/70/14, giant bat 35/40/5 | 34 | Five undead at once: a solo L5 takes ~5×11. A duo splits the hits. |
| 1:824, 830, 2618, 2635, 2640 | Graveyard gravediggers | placed, 1 h | **gravedigger 130/1500/30** (Neutral: you start the fight) | 29 | 30 max-hit vs L5-8 HP. Two melee + healer kill it fast. Five of them per hour. |
| 9:1 → 9:146 | Slimy Sewers + **white jelly** | Normal rooms; boss 3 h | slimes 30-55 hp / 60-125; **white jelly 325/2250/18** | 51 (search w 1:619) | Normal rooms: more spawns with more members (§5). Jelly is a duo kill at L8. |
| 1:382 | Strange Mansion, spectral mage | placed, 1 h | **200/1200/20** ac40; ancient sword 25% | 35 | ac40 hard to hit solo pre-L8. |
| 1:1309 | Crypt mummy (bone key) | Lair 1 | 100/200/20 | 67 | The whole party can take the L10 quest route together, but class exits split it: each member takes their own class door (§2). |

### L10-20
| Map:room | Area | Type, N, rg | Pool / boss | Steps | Why group |
|---|---|---|---|---|---|
| **1:2707** | Darkwood Forest, Clearing | **Lair 10**, rg 5 | dark goblin archer 73/80/12, dark goblin 90/150/21 | 54-58 | 10 monsters ≈ 130 dmg/round worst case. Solo needs 250 HP. Cleric + Mage at L15 (2 fireballs, rains) is the textbook duo. Clear ≈ 1,150. |
| 9:218, 219, 222 | Smuggler Den, Sleeping Quarters | Lair 5 ×3, rg 5 | thief 55/125/12, smuggler 70/150/12 (**Neutral Evil: attacks**) | 60 (hidden d at Docks) | Three 5-packs in one area: rotate. Boss smuggler 180/500/15 at 9:223. |
| 6:834 (+6:754 Lair 3) | Kobold caves, Fancy Chamber | **Normal, cap 15**; king 3 h | kobold warrior 50/150/13 spawning per head; **kobold king 250/2250/16** | 207, light −200 | Normal room: a 4-party gets 4 warriors at once plus the king. Low hit = safe for weak alts. |
| 10:43, 66, 67, 72, 80 | Frozen Caverns nomads | Lair 3, rg 0 → 5 | nomadic tribesman 130/365/19, huntsman 130/355/16, shaman 110/365/16 | 88 (hidden e 1:600, n 10:3) | Best exp/hp in band. Group can hold 2 lairs at once. |
| 10:218 → 10:245 | Large yeti → Ice Sorceress tower | yeti 1 h; sorceress 1 h | **large yeti 380/4500/45** (glass key 100%), ice golem 300/1600/45, **ice sorceress 550/9500/25** | 148 / 171 | Yeti hit 45 at L15 = group kill. Each member must do the throne step himself for the +1 AC (room clear, level 15+, 750,000 exp; `navigation.md` "Text blocks") [C]. |
| 2:40 | Orc Mansion, orc warleader | Lair 2 elite orc guard 100/275/**70**; boss 1 h | **orc warleader 175/2500/27** ac50 | 50 | Elite guards hit 70: plate front + heals. |
| 2:1122 | Gnoll Encampment | Lair 2 | gnoll scout 100/325/30, axeman 130/375/48; **gnoll chieftain 220/3500/60** | 107 | Hit 48-60 at L16-20 needs a healer. |
| 1:1952 | Ancient Crypt tomb | Lair 2, light −175 | skeletal warrior 130/300/60, moaning spirit 128/375/20; **banshee 250/4000/36** (24 h) | 50 | Undead: Priest/Cleric/Paladin turn undead + melee. |

### L20-30
| Map:room | Area | Type, N, rg | Pool / boss | Steps | Why group |
|---|---|---|---|---|---|
| 6:872 | Ancestral Tomb | undead | tortured spirit 150/525/15, barrow wight 180/725/15 | (priest.md) | Low hit: the ideal place to power-level a weak alt (equal shares). |
| 3:35 / 3:37 | Foul / Witch's Cave | Lair 4 / 3, **rg 20** | vampire bat 235/1100/18 | 60-62 | Clear 4,400. 20-min regen → rotate with 3:6 Mossy Cave and 1:2362 Rocky Waterfall. |
| 3:593, 3:606 | Ruins, dragon serpents | Lair 3, light −175 | 190-235 hp / 650-1100 / 18-21 | 123-125 | Avoid 3:560 Plaza (pool may include crimson mist 800/300,000) unless the full group is ready. |
| 15:139 (+15:137 Normal) | Orc Barracks | Lair 5, rg 2 | orc captain 300/2250/50, **ogre gladiator 400/1500/150**, lieutenant 180/950/70, warrior, fanatic | (hidden s 7:13) | Hit 150 one-shots casters: solo players avoid it. A plate front + 2 healers farm it. Fanatic MR 95: melee them. |
| 9:930 | Shadow Fist, dread mystic | Lair 2 (Lawful Evil) | **dread mystic 360/9250/50** (3 h) | black serpent key | Forum solo ~L25 (t=51). A group does it at ~L20. |
| 9:870 | Leo the Quick | boss 2 h | **240/10000/80** | 117 | Hit 80. |
| 1:1865 | Minotaur chieftain | boss 24 h | **275/5000/100** | 43 | Hit 100. |
| 3:668 | **Goru-Nezar** (good L20 quest) | boss 24 h | **1000/20000/35** ac60 dr7 | map 3 | **4-5 players at L25+ (t=6) [F]**. |
| 7:1285 | **Champion of Blood → Greater Hellion** | boss 85 h | **600/60000/100** | 3 Black Fortress keys | **About six L20s (t=11) [F]**. |
| 3:543 | Woodelf Lord (evil L20 quest) | boss 2 h | **825/40000/40**, Lawful Good (+EP), 10 woodelf guards | map 3 | Hall full of wardancers (hit 80). |

### L30+
| Map:room | Area | Type, N, rg | Pool | Steps | Why group |
|---|---|---|---|---|---|
| 6:409 (+6:415, 6:425 N4 adolescents) | Misty Bog | Lair 4 ×34 rooms | baby/young/adolescent black dragon 320-480 / 2800-6000 / 35-60 | 138-195 | 34 lair rooms: a party can chain-clear. Clear ≈ 17,000. |
| 1:3094 (6 rooms), 1:3111 | Large Black Cave (green dragons) | Lair 5, light −175 | baby green 400/3200/38, hatchling 315/2450/38 (+ young/adolescent at 1:3111) | 138-148 | Five 400-hp dragons need AoE plus 2 melee. |
| 12:101 (8 rooms), 12:80 (6) | Fortress Courtyard / White Stone Palace | Lair 6 | rakshasha 300/4000/40 (spell immunity 10), rukh 450/8000/60 | 338-348 | Open with `attack`, not a hand-cast. N=6 → group. Clear 24-36k. |
| 12:2131 / 12:2099 | Saracen Living Quarters | Lair 6 | saracen raider 250/1500/**90**, leader 350/4000/80 | 395-398 | Six hit-90s: 2 plate front + healer. |
| 16:1183-1185, 16:1160 | Storm Fortress | Lair 3-5 | **storm giant 750/9500/150** | — | Endgame. 5 × 150-hit giants. |
| 17:2043 / 17:2088 | Mausoleum (vampires) | Lair 4 | vampire minion/acolyte/magus 500-600 / 5500-12000 / 40-50; elder 1000/16000/65 | — | Low hit for the exp: good for a mixed-level party. |
| 17:1957 (40 rooms) | Library | Lair 9 | animated books 350 / 5000-7000 / 25-30, **MR 150** | — | Melee party: AoE does −50%. |
| 17:2728 (89 rooms) | Dying Fields | **Lair 15**, rg 15 | bone warriors 450-650 / 4000-6500 / 55-150 | — | 15 at once: full 6-party only. |
| 16:24 / 16:53 / 17:2674 | Tasloi chief / massive fanged spider / crowned spider | bosses in lairs | 1500/450,000/110; 1400/150,000/75; 2600/1,200,000/140 | — | Raid bosses. |

Caveat [C]: `mmud_data.spawn_pool` over-includes for rooms with a `genmonster`. The bosses are `specmon`
placements (see corpus `bosses.md`), not pool members.

## 2a. Party quirks confirmed in the source [C] (silvermere T-043)

- **A follower refused by an exit drops out silently.** The level, class, door, alignment… refusal is the only
  line he gets; the fast cycle then sets `leader = -1` without `stop_following`, so there is **no** `You are no
  longer following X.` and he stays in the leader's list (LJNBKGND.C:2090-2092, :2295-2297). That line comes
  only when he is held, stunned, too heavy, or knocked down by a free attack (LJNUTIL.C:2795, :2803, :2849, :3762),
  or when the leader leaves, dies, uninvites him or disbands. A client must treat a missing ` -- Following your
  Party leader` line as a drop.
- **A rank change is announced to every member.** `change_user_rank` sends `X just moved to the … rank in your
  group.` with `tell_user` per member; `tell_user` sends `prfbuf` and only rewinds `prfptr` (LJNTELL.C:695), so the
  text stays and every party member reads the same bytes: the dispatcher's pending colour reset (ANSI_NORMAL,
  LJNCMDS.C:213), `CLRLINE`, the line. The mover's own `You have moved to the … ranks of your group.` overwrites the
  buffer and comes without the reset; when nobody qualifies (a leader with only invitations) the mover gets the
  announcement line before his own (LJNUTIL.C:7955-7975) [C, silvermere T-092 (#103), reviewed against the C].
  (Corrects an earlier note here that said only the first member reads it.)
- **Rank counts leave the mover out.** Back is refused when the others in the front are fewer than the others in
  the back (`front < back`), so front 1 / back 1 still lets a third member go back (LJNUTIL.C:7929-7947, :7990).
- **A follower above or below a Level exit's range is refused for himself** (`You have progressed too far to
  go through this exit!` above the maximum, `You have not progressed far enough to go through this exit!`
  below the minimum, LJNUTIL.C:3553-3568) and drops out as above.
- **A new character has an empty trail.** The start room is written into the record directly, not through
  `set_user_location`, so it never enters the trail (LJNUTIL.C:1385, :1425-1430, :1850). A follower trails the
  leader only if the follower's room is at trail index 1 or later in the leader's trail (`direction_player_
  travelling`, loop from `i=1`, LJNUTIL.C:7433-7440, called from LJNBKGND.C:2089), so a follower standing in
  the start room does not follow the leader's first step off it, **if the follower is on a higher channel
  than the leader**. A follower on a lower channel is moved by the leader's own update (`for i < unum`,
  `move_user(i, plr->movedir, 3)`, LJNBKGND.C:2291-2302) with no trail check, so it does follow the first step [C].
- **`par` always ends with your own row**, even after `You are not in a party at the present time.`, and that
  case resets your rank to Midrank (LJNDISP.C:5172-5203; capture `status-screens`).
- **`par` HP and mana are `%3.3s` of the percentage**: -100% and below show three characters (`-10`).
- **`disband party` by a follower** takes him off every list but leaves him following (he still marches);
  `follow` by a leader likewise empties his list while his followers keep following him (LJNUTIL.C:7750,
  LJNCMDA.C:2553-2559).
- **`invite` of someone already listed, or into a full list,** still prints `You have invited X to follow you.`;
  only the invitee's line is skipped (LJNCMDA.C:4132-4141).
- **`share` with the party reaches members anywhere**, not only in your room; `with room` is the room.

## 9. Client implications (multi-character / boxing) [C messages; I design]

Each character is its own connection and BBS account (one MajorMUD character per BBS user record [I]).
MegaMud ran one client instance per character, coordinated by telepathed `@` commands (`@wait`, `@ok`,
`@party`, …) and room text. The bot design that copies this is `plan-grouping.md` [F].

| Concern | What to implement |
|---|---|
| Formation | Leader: `invite A`, `invite B`… Each alt on "*X has invited you to follow*" → `follow X`. Confirm "*You are now following X*". Send `frontrank`/`backrank` (casters back, plate front). Keep a slot map (max 5 followers, invitations included). |
| Movement sync | The leader moves only when every member is **in the room, HP above threshold, not resting or meditating, encumbrance ≤ 66%**. After each leader step, expect " -- Following your Party leader <dir> --" from each follower within one fast cycle. A missing line, or "*You are no longer following*", "*can't seem to move*", "*too stunned*" or "*too heavy*" means that member dropped: stop, regroup (walk them over, `follow` again). Class, level and locked exits split parties, so route planning must use the most restricted member. |
| Target calling | All alts `a <same monster>` (same name and index). **Exp is shared only by those engaged on the dying monster.** After each kill ("*Combat Off*", "*You gain N experience.*"), re-target together. AoE caster: keep it in no-target autocombat so it shares every kill. |
| Healing triggers | Poll `party` (H% and M% per member) or each alt's prompt. Single heal at ~60% HP for the most-hit member; party rain when ≥2 members are below ~70%. **After every heal, re-issue `a <target>`.** Heal yourself first if you're a robe healer. |
| Mana/rest | Party-wide rest when the healer's or AoE caster's mana < ~2 casts or anyone's HP < 50%. Bard: cast song of life before resting. Wait for every member's R/M flag. Resume when everyone is above threshold. Any action ends rest, so freeze the movement queue while resting. |
| Downed member | On ≤0 HP: someone `aid`s right away (stops the bleed). The leader can `drag` (followers can't). The party retreats only when the downed member is up or being dragged. |
| Loot | Assign one looter (`get all`/`get <coin>` right after a kill), then `share N gold` periodically. Each alt's `wealth` limits matter for training costs. |
| Exp balance | Watch each alt's exp-to-next. Train before the `xtralvl` cap ("*progressed too far without training*"). Match exp% classes. |
| Sneak scout | Thief or ninja sneaks one room ahead (not following, since following drags the scout along), reports the room's monsters, returns. Or sneaks as a follower and backstabs on arrival. |
| AoE safety | Make sure everyone in the room is in the party before an attack-area cast. A stranger in PvP range gets hit, and the caster takes evil points. |
| Retreat | Every member leaving takes a free-attack roll. Retreat the leader first (followers auto-follow), and do it before HP gets critical. |

### Screen messages to parse (exact strings, `%s` = name) [C]
| Event | Text | Source |
|---|---|---|
| Invite received / sent | `%s has invited you to follow %s.` / `You have invited %s to follow you.` | LJNCMDA.C cmd_invite |
| Joined / leader told | `You are now following %s` / `%s started to follow you.` | cmd_follow |
| Not invited | `You must be invited first!` | cmd_follow |
| Left / dropped | `You are no longer following %s.` | LJNUTIL.C stop_following |
| Removed | `%s has been removed from your followers.` | cmd_uninvite |
| Party move | ` -- Following your Party leader %s --` (direction name) | LJNUTIL.C:3789 |
| Can't move | `You can't seem to move anywhere!` · `You are too stunned to move anywhere!` · `You are too heavy to move!` | move_user |
| No drag in party | `You may not drag someone while in a party!` | move_user |
| Party list | `You are following %s.` · `The following people are in your travel party:` · `  <name 30> (<Class>) [M:nn%] [H:nn%]PR - Frontrank` (or `[Invited]`) · `You are not in a party at the present time.` | LJNDISP.C:5071 |
| Rank | `%s just moved to the front rank in your group.` · `You have moved to the front ranks of your group.` · `You may not enter the backrank of your own party.` · `You are the only one in the front rank.  You may not leave.` | change_user_rank |
| Channel | `You just joined channel %d.` · `%s just joined your channel (%d)` · `Broadcast from %s "%s"` · `You just left group %d.` | cmd_join, cmd_broadcast, cmd_leave |
| Combat / exp | `*Combat Engaged*` · `%s moves to attack everyone in the room.` · `*Combat Off*` · `You gain %s experience.` | LJNSPELL.C, LJNACMB.C, LJNUTIL.C:1777 |
| Training cap | `You have progressed too far without training to acquire anymore experience!` | add_experience |
| Aid | `You have aided %s, %s wounds are now healing.` · `%s has aided you.` · `%s is in no need of assistance.` | cmd_aid |
| Drag | `You are now dragging %s.` · `%s is too healthy to be dragged.` · `You are no longer dragging %s.` | cmd_drag |
| Share | `You gave %s %d %s` · `%s gave you %d %s` · `You do not have enough to share!` | LJNSUPP.C share_currency |
| Gang | `Gang leader %s has invited you to join %s.` · `You have left %s.` · `%s has exiled you from %s.` | cmd_invite, cmd_leave, cmd_uninvite |

## 10. Open questions [I]
- ~~Whether `follow` re-links parties correctly when a member rejoins mid-session.~~ **Answered [C]:** yes.
  `cmd_follow` clears the follower's own list except the new leader, and `add_to_group(leader, follower, 1)`
  re-links every slot of the leader's list to every other each time (LJNCMDA.C:2553-2562, LJNUTIL.C:7881-7895).
  A follower who dropped out at an exit (below) is still in the leader's list, so he can `follow` again without
  a new `invite`. `stop_following(leader, -1)` (leader's death LJNUTIL.C:4332, logoff/hang-up/exit
  `cleanup_when_user_leaves` :10473, `disband party`, teleports) tells every follower `You are no longer following
  X.` with a prompt, never the leader, then clears the leader's list (LJNUTIL.C:7730-7750). (silvermere T-043)
- MBBSEmu's concurrent-login limit (`nterms`) for boxing more than 2-3 characters isn't checked.
- Web search (mudinfo, breakintochat) found no MajorMUD-specific party documentation; MegaMud's party protocol is in `plan-grouping.md` §1. Everything above
  comes from the source and the data, plus the boss forum notes already in `corpus/bosses.md`.

## Witchunters in groups (John, 2026-10-13; confirmed in source)
- Anti-Magic resists **friendly** heals and buffs at MR/2 % (capped at 98%), the same as hostile
  spells (LJNSPELL.C:1130-1137; the 75% cap is the damage cut, :3099-3106; see `spells.md`
  "Resistance"). A Wil-stacked Witchunter wastes up to half the healer's casts.
  They also can't use shop healing potions (all Magical, ability 28).
- So treat a Witchunter as a **self-sustaining soloist**: rest and healer shops are its
  recovery. In a group it fits only where heals aren't the plan: short fights, a Bard's
  non-spell regen if any (unverified), or a second melee alongside a Paladin or Cleric who heals
  the others.
