# Recovery: resting, healing, light, death

## Resting
- `rest` regenerates HP: `You are now resting.`, and the room sees `<name> stops to rest.` (only
  when you were not already resting). It also ends auto-combat, hiding, sneaking and meditating;
  a poisoned player gets `You are too sick to rest!` (LJNCMDS.C:952-985). Observed at level 1:
  20 HP in about 100 s. [C]
- **The numbers** (`slow_update_character` LJNBKGND.C:1656-1658, rest block :2104-2147): one
  regeneration unit is `max((level+20) * Hea / 750, 1)` (integer division). Every slow tick
  (30 s) a character below full HP gets one unit, and a resting character gets **three
  units more every 21st second** (the rest counter runs 0..20), each scaled by the "Alter
  healing rate" ability as `(healrate+100)*amount/100`. At level 1 a unit is 1 HP up to Hea 71
  (21 * 71 / 750 = 1.99 truncates to 1; it is 2 from Hea 72), so a resting level-1
  character gains about 3 HP per 21 s plus 1 per 30 s, roughly **10 HP a minute**; a
  walking one gains 2 a minute. The earlier "+2 HP every 10 s" was an eyeball average of this
  (corrected 2026-10-16). Neither tick heals above max HP. [C]
- **What ends resting** is each command or event that clears the flag itself, not "any
  command": moving (`move_user`, LJNUTIL.C:2840; the direction commands LJNCMDS.C:274),
  attacking or being attacked (LJNCOMBT.C:1641, :1732, :1803, :3372; a monster spell,
  LJNMONST.C:3334), casting, REMOVE, UNREADY, BREAK, the rank commands, and almost every
  action command (LJNCMDA.C:170 says so). `look` (LJNCMDA.C:962-1345 has no clear), `st`, `health`
  and `i` do not clear it (LJNCMDS.C:307-345). A monster merely arriving in the room has no code that stops the rest: it
  ends when the monster attacks. [C] (the last point [I]: no line found, only the attack
  paths.)
- **Resting dominates time** in harder areas: in the slums a level-1 Kang spent most of its time
  resting, which is why the slums measured 920 exp/hr against the arena's 2,830. [C]
- Policy: rest before moving on when below 80%, rest to 95%, and flee and rest below 30% in
  combat. [C] `progress.py`

## Background cycles that touch your character [C]

The game runs three per-player cycles: **fast** every 1 s, **medium** every 3 s, **slow** every
30 s (`secfast`, `secmed`, `secslow`, LJNGLOBL.C:104-108; started at LJNGAME.C:1044-1048).
Everything below is from `slow_update_character` (LJNBKGND.C:1555-1725), the medium update
(:1905-2030) and the fast update (:2085-2240).

| Cycle | What happens |
|---|---|
| slow, 30 s | The per-tick move count resets; hunger and thirst each drop by 1 (no effect or message, see below); **poison**: `You feel ill.` (brown), then `poison` HP of damage, and a character who was above 0 and drops below 0 is knocked out (:1595-1606). Then exactly one of: at 0 HP or less and **unaided**, lose 1 HP (the bleed to `DEATHHP`: 30 s per HP, so −200 is 100 minutes away); at 0 HP or less and **aided**, gain 1 HP; otherwise below max HP, regenerate one unit (see Resting). `aided` clears once HP is above 0 (:1622-1668). Then mana, below. `sesstime` goes up by 1. |
| slow, mana | Below max mana: `stat * (level+20) * (magery level+2) / 1650`, scaled by `(manarate+100)*amount/100`. The stat is Int for a Mage, Wil for a Priest, (Int+Wil)/2 for a Druid and Cha for a Bard; a Kai always gets exactly 1 (:1675-1706). A class with no record gets 1. |
| medium, 3 s | The telepath, gossip and auction counters reset (:1918-1927), and so does the per-tick move count unless encumbrance is above 66% (`HEAVYLOAD`, LJNBKGND.C:40, :1925): a heavily laden character's move count is reset only by the 30 s slow tick (what the count limits is not traced here). A readied light burns one charge (see Light). Room spells fire every other pass. |
| fast, 1 s, resting | Every 21st second heal three units (see Resting). The prompt is redrawn. |
| fast, meditating | Every 15th second the same mana amount as the slow formula, **without** `manarate`; Kai get 1. When mana is full the character wakes: `You awake from deep meditation feeling stronger!` (:2152-2212). `MEDITATE` needs the Meditate ability (without it the command does nothing), is refused when poisoned (`You are too sick to meditate!`) and at full mana (`Meditation will not help at this time.`); the room sees `<name> kneels to meditate.` (LJNCMDS.C:991-1030). |
| fast, knocked down | After a smash the `pvptimer` counts down one per fast second (and per blow in combat); at 0 `Slightly dazed you get back onto your feet.` and the room sees `Slightly dazed <name> rises from the floor.` (:2227-2245). |

`REST` and `MEDITATE` do not simply end on "any command" (see Resting for what clears them).

## Healers
- Shops of type Hospital (5) heal for money: **`buy healing`**, and curing also exists.
  [C] `LJNUTIL.C:5799`
- Stock locations: Temple Healer 1:527 (Silvermere), Temple, Halls of the Dead 1:2189,
  Newhaven Healer 1:2190. [C]
- Policy: go to a healer if resting is interrupted three times while below 40% HP. [C]

## Light and darkness
- A room's `light` value (room offset 0x468), plus the light and room light of every player in
  the room and the character's Alter User Light, decides visibility. Ordinary light is capped
  at 900; Blinding Light adds past the cap. [C] `LJNUTIL.C:8818-8837`
- Bands: below −200 **pitch black**, −200 to −151 **very dark**, −150 to −101 **barely
  visible**, −100 to −1 **dimly lit**, 0 to 199 Regular Light, 200 to 899 Daylight, 900 and up
  `You are blind!`. Below −150 (`LIGHT_DARK`) you can't see ("The room is very dark - you
  can't see anything"). From −150 to −1 the room shows, ending with `The room is barely
  visible` or `dimly lit`. [C] `LJNUTIL.C:8839-8859`, `LJNSUPP.C:42`, `:478-481`
- The stock world has **11,122 rooms darker than −150**. In Silvermere's slums, 18 of 46
  rooms are −175. [C]
- Races without night vision (Kang, for example: its abilities are only poison immunity, DR and
  defence) need a light. [C]
- Light sources are item type 6 with the "Alter General Light" ability (54): **torch** +100
  for 800 charges (free in the Newhaven and Silvermere General Stores), **lantern** +175 for
  2,400 charges (2 gold). They burn out. Light one with `light <item>` → "You lit the
  torch." (the room sees "Bob lights his torch."), or `use <item>`, which does the same.
  Only one light at a time: "You already have something lit!". `light` alone →
  "The current light level is <Regular Light|...>". A burnt-out light →
  "You must recharge that before you may light it again." [C] `LJNCMDB.C:3962`, `:4004-4045`,
  `LJNCMDB.C:224-229`, `:274-277`
- A lit source burns one charge per medium-cycle pass (every 3 s, so the torch's 800 charges last 40 minutes and the lantern's 2,400 two hours); at 0 it goes (unless kept when empty).
  An item with a WCCMSG destroy message prints its text 0 to the player and its text 1 to the
  room, each only when not empty: the torch (WCCMSG 8603) has only "Your torch flickers and
  goes out.", so the room sees nothing. Only an item with no destroy message gets the standard
  pair, "<item> is no longer lit!" and, to the room, "<who>'s <item> just went out.". [C]
  `LJNBKGND.C:1929-1985`
- Policy: always carry a light; light it on entering a room darker than −150 or on seeing a
  darkness message; rebuy when it burns out. [C]
- `search` needs sight too: in a room too dark to see it prints only the darkness line and
  costs a tick, so a hidden exit out of a dark room cannot be found without a light (a pool
  character with no torch was stuck in Newhaven's Secret Passage, `navigation.md` "Hidden").
  Another player's light in the room counts. `light torch` with no torch carried is taken as
  speech (`You say "light torch"`). [C, silvermere `movement/search.rs`,
  `commands/inventory.rs`; live, wayfarer C-037, 2026-10-20]

## Death [C, observed 2026-10-13 with Thornwyn]
- **Unconscious between 0 HP and `DEATHHP`** (sysop option; **−200 in this configuration**,
  module default −10). An unconscious character can't act: spells typed by name come out as
  speech ("You say ..."), since casting by name needs HP > 0 (LJNPEND.C:430), and monsters keep
  hitting. At or below DEATHHP the character dies. An unconscious character nobody has helped loses 1 HP every slow tick (30 s), so the bleed from 0 to −200 takes 100 minutes; an aided one gains 1 HP per tick (LJNBKGND.C:1622-1668). [C] `LJNUTIL.C:4275`, `WCCMMUD.MSG`
- Dying costs one **life** (`st`: `Lives/CP: 8/2` after the first death) and **drops carried
  items** where you died (armour went from 11/0 to 0/0). You come back at a reincarnation room
  at full HP; a level-1 character came back in the Newhaven Healer. 0 lives = character
  deleted. [C, F]
- So **keep HP well above 0**: once below 0 nothing can be done. **Run when low, then rest
  somewhere quiet** (John): casters at 50% HP, melee at 30%. Escape into a room with no spawns,
  and never rest in a spawn room or arena.
- **Death pile:** the dropped gear stays in the room where you died; walk back after rebirth,
  clear the room, `get` each item, and re-equip. [F John, C observed]
- Disconnecting while in the realm is penalised on the next login. [C]

- **What a deleted character leaves behind** (last-life death, `SUICIDE` on the last life,
  `SUICIDE NOW` or `REROLL`; LJNUTIL.C:4590-4600, LJNMISC5.C:522-543): the account's next character inherits the
  palette, message level, option flags (except "name validated" and the sysop's "not in
  top ten"), chat limits, hair and eyes, statline mode and custom statline text, verbose,
  fast talk, and first and last name (LJNPERM.C:284-310). With **both** the player's "keep
  some experience" setting and the sysop's `KEEPEXP` on, and level > 1, it also starts with
  the larger of "experience half the level needs, + 1" and "a tenth of the experience"
  (LJNPERM.C:116-171); otherwise 0 (the distributed `WCCMMUD.MSG` ships `KEEPEXP NO` and `EVILKEEP 0`, so on a stock install nothing is kept: WCCMMUD.MSG:343, :452). Evil points carry over too: `evil * EVILKEEP / 100`, or all of them when the character
  dies for good on the same day the previous one did (`deathdate`, carried over by the
  profile; LJNEVIL.C:728-733, LJNPERM.C:284). [C]
- **Deleting the BBS account deletes the character** (`ljngame_dlarou` LJNGAME.C:6398, only
  when the `DELUSER` option is on: module default 1, LJNGLOBL.C:329, shipped `DELUSER YES`,
  WCCMMUD.MSG:484): the log line `Deleting Mud user <id>`, then `delete_offline_mmud_user`
  (:6411-6467) removes the evil points and the bank books **first** (so they go even if the
  account never made a character, :6425-6426), then, if a character exists, hands every
  inventory and key item back to the item files (limited items become available again),
  takes it out of its gang (one member fewer; the gang is marked disbanded when the character
  is the leader, :6455-6457) and finally deletes the WCCUSERS record. It reads the record from
  the file and never touches an online player's in-memory record, so it assumes the
  character is offline. [C] The sysop's limited-item cross-reference does the same removal
  for items only, printing `Removing limited items from <id>` and skipping an online player
  with `<id> is online - no action taken.` (LJNUPDT.C:2950-2964). [C]
- **SUICIDE, SUICIDE NOW and REROLL** (`cmd_suicide` LJNCMDB.C:5507, `commit_suicide`
  LJNMISC5.C:296; confirmed porting silvermere T-068, silvermere#43): [C]
  - Refused with `You may not suicide or reroll during a tournament.` (checked first,
    LJNCMDB.C:5515-5516) and, in a Colliseum room while the arena is safe, `You may not
    suicide or reroll in this room!` (:5529-5531).
  - `SUICIDE` takes at most one word after it and `REROLL` none; otherwise the line is not
    the command and falls through to speech (:5522). `SUICIDE NOW` matches as an
    abbreviation, so `SUICIDE N` deletes at once too (:5542).
  - With a suicide password set the game asks `Enter your suicide password:` (echoed as
    `*`); the check ignores case, and anything else, an empty line included, is
    `Invalid password specified.` and nothing happens (LJNCMDB.C:5549-5550,
    LJNGAME.C:4813-4821).
  - Plain `SUICIDE` with lives left **costs one life**, it does not delete: "After a LONG
    thought, you take your own life." then `You now have N lives remaining.`; the room
    sees `<name> took their own life!`. The character wakes at full HP and mana in the
    room's own death room if it has one, else the evil (40+ evil points) or the normal
    reincarnation room (LJNMISC5.C:318, :339-342, :565-591).
  - `SUICIDE NOW`, `REROLL` and `SUICIDE` on the last life **delete the character**
    (LJNMISC5.C:522): the line has no full stop ("...your own life") and the player is
    back at the `[MAJORMUD]:` menu (:555-559); `E` then starts a new character with the
    profile above. REROLL is a deletion, not a reset.
  - Either way the coins fall to the floor and the inventory and keys are dropped in the
    room; items marked "destroy on death", or that do not fit, give `Your <item> has
    returned to its rightful place.` Loyal and major-cursed items are kept on a life-lost
    suicide but dropped on a deletion; keys are always dropped (:331-468). Worn items,
    the weapon, spells and poison are cleared (:475-510).
  - Logging (`SYS_LOG_DEATHS` 1): a life-lost suicide logs `User <name> has committed
    suicide.` with the real name (:572-574). (sic) A deletion logs `User  has rerolled.`
    with a blank name, even for a suicide, because the log line comes after the record is
    wiped (:552-553).
  - (sic) On a life-lost suicide only the last carried item's Death text block runs
    (:357, :593-595); a deletion returns at :561 before it, so none runs.
- On login, a character whose stats are out of its race's range (more than 50 above a
  maximum, or below `max(minimum - 30, 5)`) is reset: sent to the reincarnation room (the
  evil one from 40 evil points), spells ended, stats back to the race minimums and the CP
  of every level returned ("Your stats have been reset due to an earlier bug.",
  LJNUTIL.C:881-985). Smash, Perfect Stealth and Meditate are removed from classes below
  their level gates on login and on save (LJNUTIL.C:691-731, LJNSUPP.C:2488-2536); See
  Hidden is removed from every race but the Gaunt One on login only
  (LJNUTIL.C:1268-1275). [C] The same reset runs for any record whose CP total is negative (`plr->cp < 0`, LJNUTIL.C:928-942); [I] a TWEAKed level above the character's experience, taken down with its CPs, would reach it (the cited lines do not show the level check).
  - **What sets `cp = -1`** (LJNUTIL.C:872-926): any stat more than 50 above the race maximum
    or below `max(minimum - 30, 5)`; a record older than version 22 (all pre-1.11p records; this
    also sets the reset flag `ST2_STATRESET`); and a **race or class change** (CHANGERACE, see
    `connection.md`; LJNSUPP.C:1642, :1693), and `SYSOP GOD <user> RETRAIN` (LJNCMDB.C:2397,
    answering `The next time <name> logs into MajorMUD they will have to retrain their stats.`).
  - **The reset itself** (:928-985): the message is three lines, `Your stats have been reset
    due to an earlier bug.` / `Please TRAIN STATS as soon as possible to re-apply your CPs` /
    `(Unfortunately, if any spells were cast on you they will be terminated.`; the character
    goes to the reincarnation room (evil one from 40 evil points) on map 1, stats are rolled
    back to the race minimums and the CP are given back per level: **10 for each of levels
    2-10, 15 for 11-20, then 20 for 21-30, 25 for 31-40** and so on (+5 per ten levels).
  - **Then the editor opens at once:** a character carrying the reset flag who presses `E`
    at the menu goes straight into the full-screen stat editor (LJNGAME.C:3914-3915). Leaving
    it with QUIT returns to the main menu with the flag still set (:2352, :2378-2384); SAVE
    clears it (:2344). See `connection.md` "Character creation" for the form.

## When the character is saved [C]

- **Periodic:** the "other" cycle runs every `SECOTH` seconds, rounded down to whole minutes
  with a floor of 60 (LJNGAME.C:762-767; the shipped `SECOTH 160` therefore saves every 120 s,
  WCCMMUD.MSG:124) and calls `save_characters`, which saves at most `BUFTOSAV` in-game players
  per pass (shipped 1, WCCMMUD.MSG:141; module default 5), looks at no more than 300 channels,
  and resumes from the channel after the last one saved, wrapping at the end
  (LJNBKGND.C:903-910, :2952-2970). With the shipped values **one player is saved every 2
  minutes**, so with N players on line each is written about every 2N minutes: a hard kill can
  lose far more than a minute of progress. [C]
- **Always saved at:** a clean exit (`ljngame_lofrou`, LJNGAME.C:5257-5266, and the end of the
  exit delay, LJNBKGND.C:2328-2329), a hang-up (before and after the penalty), `TRAIN STATS`
  (before the editor opens, LJNCMDA.C:4441), the editor's SAVE, and shutdown (`finrou`
  writes back every cached record and deletes the `WCCRECOV.FLG` flag file so the next start
  knows the shutdown was clean, LJNGAME.C:1160-1185). The game prints no notice to players at
  shutdown. [C]
- **Leaving the realm** (`x`/`quit`; `cmd_quit` LJNCMDB.C:688-720): refused at 0 HP or below
  (`You may not exit MajorMud while you are mortally wounded!`); otherwise auto-combat ends,
  the game prints `You will exit after a period of silent meditation.` (the `.` is a line of
  its own), and **6 command-delay ticks** later (the fast cycle prints `...` for each second still
  waiting, LJNBKGND.C:2366-2369) the
  character is moved to the room's hang-up room if it has one, saved, and shown the main
  menu; the game hears `<name> just left the Realm.` (LJNBKGND.C:2340-2347, unless
  invisible). While waiting, every line except `BREAK` gets `You may not perform any commands
  while waiting to exit!` and `BREAK` cancels the exit (LJNPEND.C:150-169). Only the exact
  word `x` is the quick exit at that stage; `quit` goes through the command table
  (LJNPEND.C:220, LJNCMDS.C:225). [C]
- **An attack cancels the exit** (`stop_users_exit`, LJNUTIL.C:9917-9940): the waiting
  character gets `Your meditation has been interrupted - you may not exit now!` and at once a
  prompt; the exit and the command delay are cleared, so `x` can be typed again straight
  away. It is called for **every** attack on the character, before the blow is rolled, so a
  miss interrupts as surely as a hit: a monster's melee round (LJNCOMBT.C:3374), a monster's
  spell (LJNMONST.C:3346), a player engaging or striking (LJNCOMBT.C:1793, :1805), a player's
  spell (LJNSPELL.C:5166), and `BREAK` (LJNPEND.C:170). A monster that keeps attacking
  therefore interrupts every six-tick meditation: the character can only leave once nothing
  attacks it for six ticks (it got away, or the attacker died or left). `x` ends the
  character's own auto-combat, so it does not kill the attacker meanwhile. [C] Seen on
  silvermere (wayfarer C-027 live check, the Newhaven Arena): a giant rat interrupted each `x`
  about 5 s after it was sent, every time. [C]

## Hanging up and the daily cleanup [C]

From the silvermere T-056 port (`ljngame_huprou` LJNGAME.C:5296-5459, `ljngame_mcurou`
:5505-6384).

**Hanging up in the realm** (the line drops, or the client is killed):

- Outside a safe room, `PUNHUP` decides. A safe room is a protected room while you are
  neither in auto-combat nor under attack, or a Colliseum room while the arena is in safe mode
  (`arenasafe`) (LJNGAME.C:5324-5327). "Under attack" (`is_being_attacked`, LJNACMB.C:235-247)
  means a **player** in the game is queued in auto-combat against you: a monster fighting you
  does not make a protected room unsafe, and does not count for `MEDIUM` either. Only the
  protected-room test looks at combat; a Colliseum room under `arenasafe` is safe even
  mid-fight (:5328). In a safe room nothing is taken and `ST_HUNGUP` is not set (no notice at
  the next entry); you are only moved to the room's hang-up room (:5326-5332). `HIGH` always punishes (the module default,
  LJNGLOBL.C:237; the distributed `WCCMMUD.MSG` ships `NONE`, WCCMMUD.MSG:272 in
  `dist/WCCMMUD_1.11p_DOS_MBBSEmu_v2.zip`), `MEDIUM` only in auto-combat or under attack,
  `LOW` only in player-versus-player combat, `NONE` never (LJNGAME.C:5336-5339).
- A punished hang-up costs `lo + genrdn(0, hi-lo+1)` hit points, `lo = maxhp*MINHUPHP/100`,
  `hi = maxhp*MAXHUPHP/100` (25 and 50 per cent by default, LJNGLOBL.C:281-282); at 0 HP or
  below you get `DEATHHP` added instead (:5346-5361). Then every **second** inventory slot
  (0, 2, 4, ...) is dropped to the floor, then every second key slot, up to `MAXHUPIT` (100)
  items (:5363-5430). Loyal Items stay; items flagged to be destroyed are lost, not dropped.
  The `MAXHUPIT` count goes up for each inventory item that left (a destroyed one counts; one
  the room had no space for is lost but not counted; a Loyal one stays and is not counted),
  but for every key slot that holds a real item, Loyal or not (sic; :5404, :5432).
- Even with `NONE`, outside a safe room you are moved to the room's hang-up room (when it
  has one) and `ST_HUNGUP` is set (:5434-5441), so the next entry shows the notice below with
  no penalty having been taken. **Who hears it:** every player in the game (not only the
  room) who has not blocked entry messages sees `<name> just disconnected!!!` in bright white
  (`tell_game`, :5443-5446), also when the hang-up was in a safe room; then `check_kill_user`
  runs, so the loss can kill (:5447). The record is saved before the penalty and again after
  (:5313, :5455-5456). At 0 HP or below the penalty is `DEATHHP` added, not the
  percentage (:5358-5361).
- **Shipped options.** The distributed `WCCMMUD.MSG` ships `PUNHUP NONE` and
  `MINHUPHP`/`MAXHUPHP`/`MAXHUPIT` all `0` (WCCMMUD.MSG:272, :413-424), not the module
  defaults 25/50/100: if a sysop switched `PUNHUP` to `HIGH` without setting them, a hang-up
  would cost 0 HP and drop 0 items. [C] (`dist/WCCMMUD_1.11p_DOS_MBBSEmu_v2.zip`)
- The next entry prints `Last time you were on, you disconnected while playing.` and `The gods
  have punished you appropriately.` before the banner (LJNUTIL.C:750-755).
- A clean exit (`x`, the meditation, then the menu) punishes nothing (`ljngame_lofrou`, :5257).
- **Inactivity.** The game checks every `INACTIVE` seconds (shipped 900, LJNGAME.C:542, 1050;
  module default 3600): a player who typed nothing since the previous check was marked, and one
  already marked is sent away with `You have been inactive for too long.   Bye!`
  (`byenow(INACHUP)`, LJNBKGND.C:955-968), so a silent player is cut off after between 900 and
  1800 s. Any typed line clears the mark (LJNGAME.C:2911, LJNPEND.C:147). The substate is set
  to the main menu first, so this is not a punished hang-up. Sysops are never hung up
  (LJNBKGND.C:955-957, :970-983). [C]

**The daily cleanup** (the BBS's nightly cleanup on the original, run with players on line):

- **The warning.** Under MBBSEmu, five minutes before the cleanup every online player gets
  `Sorry to interrupt here, but the server will be shutting down in 5 minutes for the nightly
  "auto-cleanup" process. Please finish up and log off... thank you!`, mixed into whatever the
  game is printing (seen in silvermere m4 live run 38997202711 at about 03:00 UTC, mid-exit) [C:
  live capture]. MBBSEmu's cleanup then drops every session and reloads the module [I: MBBSEmu
  survey, not seen in a capture]. The time is MBBSEmu's `Cleanup.Time` (stock `"03:00"`). A
  client should treat the line as a cue to finish up and expect a disconnect; it isn't game
  output to parse.
- `sesstime` counts **slow cycles** (`sesstime++` once per slow update, LJNBKGND.C:1722;
  `secslow` 30 s), so two to the minute. It is kept in the player record and builds up across
  logins until the cleanup folds `sesstime/2` into `lifemins` and zeroes it (LJNGAME.C:6172-6179;
  an odd half-minute is lost). PROFILE's `Life for this CHAR` is `sesstime/2 + lifemins`
  (LJNDISP.C:4825), and the race-change undo window is measured on the same sum (LJNSUPP.C:1589).
- Only a character with `sesstime > 0` (played since the last cleanup) is touched (:6172).
- **Evil forgiveness** per cleanup: `eps` = 2 if evil points were charged since
  (`PS_EVILADDED`), else `(minutes/EPCYCLE)*EPAMOUNT + 3`, at most `MAXEPDAY` (60, 1, 12 by
  default, :6182-6194). Below level 4 you are forgiven only while evil, and never past 0. From
  level 4 the points go on down: a result below 0 is pulled back up by 2; nothing goes below
  -200 (what is left above -200 is forgiven), and a character already at or below -200 is left
  as he is (:6195-6227).
- Items with "Recharge at cleanup" get the ability's value as their uses; "Delete from
  inventory at cleanup" items go (:6236-6320).
- **Floors:** coins on the floor of an ordinary room become hidden coins; floor items are
  **hidden**, not deleted (search for them), merged into an equal uncharged hidden stack
  (sic: only the extra copies are added) or put in the first free hidden slot, unless the room
  places them or they have "Remain Visible at cleanup". Unlimited free items (cost 0) and
  unlimited "Delete at cleanup" items are deleted, floor and hidden; limited ones are hidden like
  any other item. A room with more than 8 items also loses its cheap unlimited items (priced in
  gold at 14 or less, or in a lower currency), Remain Visible or not; the items the room places
  itself survive that rule too. `RF_CLEANUP` rooms lose everything except one copy of a limited item (sic: the copies
  are removed while extra copies remain) (:5843-6113).
- (sic) Each map is walked from room 1 and the walk of a map stops at its 50th missing room
  number, wherever the gaps are; rooms past that are never swept (:5670-5680, :6115-6129). On the stock data
  (post `WCCUPDAT`) the walk still reaches all 26,720 rooms, so the quirk only bites worlds with
  sparse room numbers [C, export check].
- **Gang houses:** the holder of a Gang House Deed (carried by a gang member) pays the deed's
  `ABL_GANGTAX` value in silver, times 100 in copper, from his bank book at bank 8; without the
  money (or the book) the house closes: its floor, hidden items and coins (not runic) are
  removed, its gang shop loses its first 10 slots and its owner, the deed goes back on sale in
  the deed shop, members holding its Gang House Items lose them (`PF_HOUSECLOSED` for the
  leader, `PF_HOUSEITEMS` for the gang; shown at the next entry, LJNUTIL.C:776-783), and anyone
  standing in it is moved to map 1, the evil reincarnation room with 40+ evil points, else the
  reincarnation room. A house nobody holds counts as closed every night (:5552-5662, :6326-6342).
- Gangs with no members, or marked for deletion, are deleted (:6363-6378).

## Hunger and thirst

- [C] `hunger` and `thirst` start at 1000 (`create_player`, LJNUTIL.C:1390-1391) and only ever decrement, by one each per 30 s slow tick (`slow_update_character`, LJNBKGND.C:1584-1585). Nothing in LJNBKGND.C reads either value: reaching 0 has no effect in the background cycles. The only other writes are the eat and drink increments (LJNCMDB.C:4149, :4261) and spell and room effects (LJNSPELL.C, LJNROOMC.C `ABL_HUNGER`/`ABL_THIRST`). Confirmed while porting silvermere T-037 (silvermere#35).
