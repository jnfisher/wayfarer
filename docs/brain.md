# The brain

The brain is the part of wayfarer that understands the game. It attaches to a running gateway
as its one brain client, parses every session's output, keeps a model of each character and
sends the UIs events and snapshots of it. With a world pack it also knows where each character
is, serves the map panel, walks a character to a room when a UI asks ("Walking", below), and
plays a character where it stands when the player presses Start ("Fight here"). Otherwise it
plays nothing: the lines it sends to the game are a re-sync's, a walk's and the bot's. It can
be stopped and started at any time; the gateway keeps the characters logged in meanwhile.

## Running it

```sh
# once: the world pack, from the stock-world export in this repository
wayfarer import-world --export tests/fixtures/world --out "$WAYFARER_DATA_DIR/world.sqlite"

# with the gateway already running (scripts/dev/gateway.sh)
wayfarer brain
```

In a release, one process runs both: `wayfarer serve` takes the gateway's flags plus `--world`
and attaches the brain to the gateway over in-process channels, with no socket. The UIs see
the same `brain_state` and messages as with two processes, and stopping `serve` logs everyone
off as `gateway` does. With no world pack at `--world` (default: `world.sqlite` in the data
directory) it warns, prints the `import-world` command and runs the gateway alone. In
development the two stay apart (`scripts/dev/brain.sh`, [dev-rig.md](dev-rig.md)) so the
brain restarts without the characters.

`wayfarer brain` takes:

| Flag | Default | What |
|---|---|---|
| `--gateway URL` | `ws://127.0.0.1:7373/ws` | The gateway's WebSocket |
| `--world PATH` | `world.sqlite` in the data directory | The world pack. Missing, the brain stops at once and prints the `import-world` command that builds it |
| `--token TOKEN` | the `gateway-token` file in the data directory | The gateway's token, as `wayfarer gateway --new-token` made it. A gateway on a loopback address checks none |
| `--config PATH` | the user's data directory | The gateway's `wayfarer.toml`; its directory is the data directory |
| `--store PATH` | `store.sqlite` in the data directory | The brain's store: each bot's state and the statistics ("What it keeps", below). `wayfarer serve` takes it too |
| `--flee-from NAME` | none | Repeatable: every bot flees on sight from monsters whose name holds NAME (any case), as if the player had set *flee from* for it ("Monster rules", below). Use the full name |

The data directory is the gateway's: on macOS `~/Library/Application Support/wayfarer/`
(`WAYFARER_DATA_DIR` in the dev scripts). Logs go to stderr (`RUST_LOG` sets the level): one
line when it starts and attaches, one per re-sync. Nothing the player types is logged.

## What it does

- **Attach.** It says `hello` as the brain and sends `brain_info` (its build, start time and
  world pack). For each session it fetches the last 2,000 chunks of output and feeds them to a
  fresh parser *silently*: the character model is filled (so the last prompt's HP and mana are
  known) but no `event` goes out for that old text. Then it sends the whole `character` and
  follows the live output. A session that appears later gets the same.
- **Parse.** Every line of live output goes out as an `event` (with its output `seq` and
  `line_no`). Each line the gateway reports as `sent` is told to the parser, so the game's echo
  of it is read as an echo; the gateway sends a line's `sent` before the output that echoes it.
  The brain's own re-sync lines are told to the parser when they go; their `sent` is matched
  off and not told twice, and one whose `sent` has not come within 10 s (or after a `lagged`,
  or once the re-sync is done) is forgotten, so a line the player types later is never taken
  for the brain's. A line nothing recognises is counted (`parse.unclassified`), never an error.
  If the gateway's output log started again (its `last_seq` is below what the brain fed), the
  parser and the line count start afresh and events go out again from the new log's live
  output.
- **Model.** The character applies each typed event: the prompt's HP, mana and resting mark;
  the status screen (name, lives and CP, race, class, level, HP and mana with their maxima,
  stats, poison); the `exp` screen; the room's title and exits (a dark room has neither); combat
  on and off; exp gained; damage taken and a healer's healing (HP until the next prompt); the
  character's own collapse (HP at most 0, out of combat); monster deaths (`kills`); death and
  lives; poison (until a cure or death) and blindness (until death); inventory, wealth and
  encumbrance (`coins`, `carrying`, `encumbrance`). After each chunk that changed anything it
  sends a `character_patch`, the merge patch from the last snapshot sent.
- **Fell behind.** On `lagged`, or a gap in the live `seq`s, it fetches the missed chunks with
  `history_request` and holds live output until they are in.
- **Control.** `resync` starts a re-sync; `state_request` gets the whole `character` (one
  session's or all) and `brain_info` again; `walk_to` and `walk_stop` walk ("Walking");
  `bot_control` starts, stops, pauses and resumes the bot ("Fight here"); `run_loop` runs a
  loop ("Running a loop").
- **Closed sessions** keep their last snapshot; the UI shows the session's state beside it.

## The re-sync

A re-sync asks the game for what the model may lack: `st`, `exp`, `i`, then `look` (not an
empty line, which redisplays the room only in statline FULL modes, `kb/screen-parsing.md`),
and for a character that casts, `spells` ("Spells", below).
They are the "talk and look" kind of command (`docs/plan/03-engine.md` section 6).

- **When**: for a session in the realm that has not been re-synced since the brain attached;
  when a session enters the realm; when a UI sends `resync`.
- **Only at an idle prompt** for the first command: the last chunk ended in a prompt and
  nothing went to the game for 2 s. Each later command goes at the prompt after the previous
  one's answer (the status screen, the exp line, the inventory, the room), or after 5 s
  without one.
- **The player comes first**: if they type during a re-sync, the rest waits for the next idle
  prompt.
- **Never outside the realm**: nothing is sent while a session is `logon`, `menu`,
  `connected`, `connecting`, `lost` or `closed`. A re-sync asked for then waits for the realm;
  one cut short by leaving the realm starts again from `st`.
- **Done**: a whole `character` with `synced_at`, and a `notice` "re-synced".

## Position and the map

With a world pack the brain keeps, per character, a **position belief**: the rooms the
character may be in (`kb/navigation.md` "Position tracking"). Every room the game shows narrows
it (the title, when it was not lost behind a prompt, and the `Obvious exits`); every move that
goes to the game, the brain's or one the player typed (a direction, or the phrase of a text
exit such as `borrow skiff`), carries it through the exit first. So the map follows a character
played by hand as well as one the brain walks. The snapshot's `position` is the believed room
(the lowest key when several fit), how many fit, and the last title seen.

The map panel gets a `map_view`: the rooms within 15 steps (or what `map_request` asked for)
laid out on a grid with the believed room at (0, 0), each with its name, whether it spawns
monsters, its kind (shop, trainer, bank, arena, lair; from the room data and MegaMud's
waypoints), whether the character has certainly been there and whether it is a candidate; the
exits between them; and the walk's route. One goes out when the believed room changes, when a
route starts, changes or ends, and on `map_request` (`docs/protocol.md`).

## Walking

`walk_to {session, map, room}` from a UI walks the character there; it replaces a walk in
progress (after the move in flight).

- **The route**: the cheapest from the believed room for the character's level, class and race
  (as the model knows them): the shortest route, through rooms that spawn monsters as readily
  as any other (a setting later), avoiding the exits the game refused this session and the
  locked doors the character cannot open. Narrated: "Walking to Newhaven,
  Arena: 3 steps" (", one door" when there are doors). No route, or an unknown position: a
  warning, and nothing is sent.
- **Locked doors and the character**: a door or gate counts as the game last showed it this
  session, else as the pack has it. A locked one is in a route only if the character can open
  it: a bash opens it on `Str + exitpar2` out of 100, a pick on `picklocks + exitpar2` (with
  picklocks above 0), `kb/navigation.md` "Door / Gate"; with both nil it is left out (the Hidden
  Keep's courtyard door, `exitpar2` -250, needs a strength over 250). Strength and picklocks come
  from the status screen (`st`); unknown, a locked door is not checked. A shut door that is not
  locked opens with `open` and is always passable. A locked door costs one extra step per
  expected try, `ceil(100 / odds)`: two at 90 %, five at 20 %, so a sure way round of a few
  more steps wins. Another character with more strength may get through the same door.
- **One step at a time**: each move goes at a prompt, in the realm, and only once the room the
  last one led to has been seen and is the one the route expected. One line is in flight at a
  time: a re-sync waits for the walk's action and the walk for a re-sync command. A room that is
  not the one expected replans from the belief once; a second time stops the walk.
- **Doors**: `The door is Closed!` (shut) sends `open <dir>`, then the move again. `The door is
  closed!` (locked), or `The door is locked.` after `open`, sends `bash <dir>`, at most three
  times; then the exit is marked refused and the route planned again. A locked door whose odds
  are nil for the character is given up at once, with no `bash`: "The door east is beyond my
  strength (needs more than 999)", the exit marked refused, the route planned again.
- **Keys and items**: a Key exit (exit type 2, `exitpar1` the key's item number) is routed through
  only when the character holds that key (the `You have the following keys:` line of `i`), and an
  Item exit (`exitpar1` the item, a rope and grapple, a room ticket) only when the item is carried
  (the `i` list). At a Key exit's shut door the walker sends `use <key name>` (twice at most), then
  moves again; an Item exit just works while the item is carried. Without them the exit is not
  taken, and the reason names it: "the exit east of Stone Passageway, Guardroom needs the large iron
  key" (Thrag's cavern, 1:1810, is in by that key and nothing else).
- **Items a shop sells**: when the item an Item or Key exit asks for is sold in a shop (a room
  ticket for the Homely Hearth's stairs in Silvermere, a rope and grapple, a wooden skiff, a mine
  pass), the exit is not a wall: the walk also plans the way through the nearest shop selling it,
  from here to the shop and on from there holding it, and takes it when it is the only way or
  shorter than the plain one ("Walking to Bard's Room: 6 steps, one door, buying a room ticket at
  The Homely Hearth, Common Room"). In the shop it sends `buy <item>`: "You just bought ..." and
  it goes on; a refusal stops the walk ("Stopped walking: could not buy a room ticket: cannot
  afford it"). An item with one charge (the room ticket) is used up by its exit (`kb/navigation.md`
  "Item"), so the walker stops counting it as carried and the next walk up buys another. One
  item is bought per plan; a second is found when the walk plans again. Moves don't go ahead of
  the buy. The pack's items, shops and charges are read once (`wayfarer_world::exit_items`).
- **Hidden exits as a last resort**: when no route takes only the exits known, a walk the player
  asked for (`walk_to`, the go box, an errand) may use the pack's Hidden exits (`route_or_search`),
  each costing a search, and `find` counts a room only hidden exits lead to as reachable
  (`distances_any`) instead of "no route". A route through known exits is never given up for one
  through an unknown hidden exit, and a loop's legs and the loop list keep to the exits its path
  knows.
- **Passable hidden exits**: most Hidden exits (993 of 1,383, nearly all in the wilderness
  of maps 17 and 7) are marked passable: the game never lists them but lets anyone walk
  through without a search. A route takes them as it takes any exit, at no extra cost, and the
  walker moves through without searching. What follows is about the others.
- **Hidden exits**: a route takes a Hidden exit the character knows of: one it found this
  session (a `search` of its own or the player's), one an exits line showed (`secret passage
  south`: found lately by anyone), one a loop's path names, or, when the character is shut
  in a pocket (60 rooms or fewer reachable, say a room entered by a hidden way that was not
  remembered), the hidden exits out of the pocket: without that a restart in such a room
  left every loop "unreachable". Arriving in the room, if the
  exit is not in the room's `Obvious exits` and was not found in the last five minutes, it
  sends `search <dir>` ("Searching for the hidden exit north") and waits for the answer: "You
  found an exit to the north!" and it moves; "You notice nothing different to the north." and
  it sends `look` (an exit found lately by someone else needs no search, so a search for it
  finds nothing), moves if the exit shows, else searches again. A search is a dice roll
  (`max(3, perception - 15)` percent, perception being `(Int*5 + Will*2 + Cha)/8` plus race,
  class and item bonuses), so it keeps searching until the exit is found: a `look` after the
  first empty search and every fifth after, and "Still searching for the hidden exit north (10
  tries)" every tenth. Only after `MAX_SEARCHES` (300) empty searches does it mark the exit
  refused for the session and plan again. A fight, a flee or a stop ends the walk and so the
  search; the next walk starts counting again. A search needs light: in a room too dark to see the game answers
  "The room is very dark - you can't see anything" and the walk stops ("too dark to search
  for the hidden exit north"; the bot does not light a torch yet). A found exit hides again after about five minutes (one step of 300
  fast ticks, `kb/navigation.md` "Hidden"), so the brain forgets it showing then and searches
  again next time. A search costs two extra steps in the route. Moves are not pipelined through
  a hidden exit.
- **Blocked hidden exits**: some Hidden exits are not searched for but blocked, and no search
  ever finds them; a phrase typed in the room takes the blocks off (a Remote Action exit of
  the room, `kb/navigation.md` "Hidden"). The pack says which: the Slimy Tunnel's stone door
  west in the sewers opens at `push switch`, the Dark Sanctum's at `press d`, `press e`,
  `press a`, `press t`, `press h`, typed in that order. Arriving at one that is not showing,
  the walker types its phrases ("Opening the hidden exit west: push switch"), then `look`s: it
  moves if the exit shows (`secret passage west`), else marks the exit refused for the session
  and plans again ("the hidden exit west did not open"). A blocked exit whose phrases are typed
  in another room, or that asks for an item, is still searched for as before.
- **Refusals** (level, alignment, class or race, not permitted, a spell or item needed, a toll
  not covered, a timed exit): the exit is marked refused for the session and the route planned
  again, at most five times a walk.
- **"There is no exit in that direction!"** means the belief is wrong (`kb/pitfalls.md` 34):
  the brain sends `look`, relocates from the room shown, and plans again. A second one in the
  same walk stops it.
- **A room the map lacks** (C-053). A room shown whole (title and `Obvious exits`) whose title
  no room of the pack has, a player's house ("Shop White House Room"), leaves the position
  unknown: the walk stops ("…; in Shop White House Room, a room not on the map") and the next
  room the pack knows relocates it, from the rooms around where it was. A title seen alone (a
  line misread as one) changes nothing. The doors of players' houses (map 15, 34 of them) are in
  the graph, so `Obvious exits` still fit, but no route, rest or flee takes them.
- **Stops**: `walk_stop`, `*Combat Engaged*`, a line the player typed, or no answer to a move
  within 5 s stop the walk after the move in flight (the walker never fights); leaving the realm
  or the brain re-attaching stop it at once. Narrated: "Stopped walking: combat started".
- **Arrival**: "Arrived: Newhaven, Arena"; `walking` leaves the snapshot.
- **A boss lair** is walked to one room short unless `walk_to` says `stop_short: false`
  ("Go to", below).

## Go to

`find {session, query}` answers with `found`: the rooms, well-known places and monsters whose
name holds the query (two characters or more, any case, anywhere in the name), at most 30,
for the map panel's go box (`docs/web-ui.md`, "The map").

- **What it finds**: room names, one result per exact name with how many rooms have it and
  the nearest 20 ("Wooden Passage, 63 rooms, nearest 43 steps"); shops, banks and trainers
  by their room name, and MegaMud's named rooms (waypoints) when the world pack has them;
  monsters by name, with the rooms they spawn in. A query that names no monster but ends in
  one ("thin giant rat") finds it as the parser does, leading words stripped.
- **Best first**: results the character has no way to come last; then the better match (the
  whole name, its start, the start of a word, anywhere); then the nearer; then places before
  monsters before room names. Route lengths come from one breadth-first pass from the
  believed room by the exits the character may take (some milliseconds over the whole
  world); a room beyond 200 steps is "far", one not reached at all "no route". With the
  position unknown, nothing has steps.
- **Boss lairs**: a room whose spawn pool (its `genmonster`, its group's monsters, or its
  `specmon`) holds a boss, or that a MegaMud waypoint flags boss. A boss is a monster that
  spawns one at a time and comes back hours after its kill (`maxalive` 1, `regenhrs` set,
  `kb/progression.md`), is not an NPC and is worth experience: the Minotaur Chieftain's Large
  Wooden Room (1/1865) and the champion's cavern (1/2124) are lairs; a Wooden Passage room of
  plain minotaurs, the Newhaven Arena and the Dungeon Entrance (a Lair room, not a boss's)
  are not, and neither is the healer's room. Only boss lairs, not every Lair room (John,
  2026-10-20).
- **Stopping short**: `walk_to` a boss lair stops in the room before it, as MegaMud's "Stop
  Before Entering Room" does (`kb/megamud-manual.md`), so the player can look first;
  `stop_short: false` walks in, and it changes nothing for any other room. It is a route cut,
  not a mode: the walk goes to the second-last room of the route, plans again around the lair
  if it has to, and says so: "Walking to Large Wooden Room: 5 steps, stopping one room short",
  then "Stopped one room short of Large Wooden Room (boss: minotaur chieftain)". `walking`
  carries `short_of` (the lair and its bosses) meanwhile. Already next to the lair, it stops
  at once with the same narration. Looking into a dead boss's room brings it back
  (`kb/pitfalls.md` 51): stopping short does not look in.

- **The bot walks it.** `walk_to` gives the bot a *journey* there and turns it on, whatever it
  was doing (it replaces a loop or another journey; a bot that was off re-syncs first: "Going
  to Newhaven, Arena: re-syncing"). The journey is in Task's place in the mode stack, so the
  bot fights on the way and walks on from wherever the fight left it; a journey's moves go one
  at a time. At the end the journey is over and the bot plays there, that room its home. A
  walk the walker cannot start (no route) or gives up by itself (lost, a door that will not
  open) ends the journey: "Not going to Burning Valley: the walk stopped; playing here". The
  snapshot's `bot.journey` says where it goes (`{name, to}`). See "The bot's states".
- **Passing monsters by.** `walk_to` with `fight: false` (the map panel's *fight on the way*
  unticked) makes the journey *passive*: on the way the bot fights nothing, not even a monster
  that attacks the character; it walks on, and the walk does not stop for one. Flee still runs
  under the flee line (and from a *flee from* monster), and Recover does not rest while the
  character is attacked. Narrated "Going to Newhaven, Arena, fighting nothing on the way";
  `bot.journey.passive` is set. `journey_fight` changes it on the way: off in the middle of a
  fight, the bot walks out of it; on, it fights from the next room (or here, once it looks).
  A loop walked once always fights. Once it arrives the journey is over and the bot plays
  there, fighting, as after any Go to.

**The guard** lives in the executor every action goes through, so nothing the bot ever does can
break it: never `train stats` (`docs/plan/01-goals.md` "Rules").

## Loops

The first of the three loop sources (`docs/plan/03-engine.md` section 4): loops generated from
the world data, beside MegaMud's verified loops. A UI lists them, walks one once so the player
can watch the route, and runs one ("Running a loop", below).

`loops_request {session, anchor?, radius, doors?, query?, limit?}` answers with `loops`
(`docs/protocol.md`). The answer is the **nearest** loops, nearest first, at most `limit`
(12; 50 at most): each loop's `distance` is the steps from the believed room (or `anchor`) to the
nearest room of its walk, by a breadth-first walk over the whole world by exits the character
may take. The generated loops come from the area (below); MegaMud's verified loops and the
player's own come from anywhere the character can walk to, so a search finds a loop however far
it is, and `matches` says how many matched before the cut. A `query` keeps the loops whose
name, file, description, author or any stop's room name holds every word of it (any case),
among the loops already generated over the area: a search never generates over a wider one.
A loop the character cannot
walk to at all is left out.

- **The area** of the generated loops: the rooms within `radius` steps (at most 30) of
  `anchor`, the believed room by default, by exits the character may take: a gated exit outside its level, class or race,
  or a locked door it cannot open ("Walking"), leaves what is behind it out, and with
  `doors: false` so does every door.
- **Hunt rooms**: the area's rooms whose spawn pool (`genmonster`, or the room's `mongroup`
  within its `minlevel`..`maxlevel`, plus its `specmon`; only Normal, Arena and Lair rooms
  spawn) holds a monster worth hunting (evil, not an NPC, worth exp). A shop whose pool is its
  shopkeeper is none. Each stop says its huntable monsters, whether it is an Arena (monsters
  back at once after a kill) or a Lair, and its `regendelay`.
- **Generated loops**: a closed circuit through the hunt rooms, nearest neighbour from the stop
  nearest the anchor, then 2-opt; each leg the character's cheapest route between two stops
  inside the area. One loop over every hunt room; when the area splits (stops that can't
  reach each other inside it: the river off Newhaven is one-way above level 3) one more per
  part of two or more stops, and the loop over all of them may leave the area between parts,
  or is left out when it can't close. Named after the anchor's place ("Newhaven"; a part adds
  its first stop: "Newhaven: The Silver River, Silvermere Docks"). Within 8 steps of Newhaven,
  Village Entrance: the Arena, the Dungeon Entrance behind its locked door and the Small Cavern
  north of that, 4 steps; without doors, the Arena alone.
- **MegaMud's loops** through the area: those whose verification walk passes one of its rooms,
  only ever ones that walked to the end and closed in their start room. None unless the world
  pack was built with MegaMud's paths. Each lists its **stops**: the rooms of its walk, in path
  order and each once, that are hunt rooms as above (`MmLoop::stops`; the format has no step
  that waits or attacks, so no other room is a stop; a Wooden Passages loop passes many corridor
  rooms twice, its stops once each). `runnable` says whether `run_loop` runs it for this
  character, with a `reason` when not: "nothing to hunt on it" (7 of the 183 verified loops:
  the Oasis, both Lost City loops, both Storm Giant Fortress loops, the Small Pyramid, the
  White Forest), or "no way from Crypt, Stone Hallway to Crypt, Sealed Tomb: the exit north of
  Crypt, Stone Hallway is for level 10 only" for a leg between two stops (in loop order, the
  last back to the first) that no route takes. For a strong, high-level character 12 of the
  183 have such a leg: Item exits (BHB1LOOP, BHB2LOOP, BHMPLOOP, BHRQLOO2, RP18LOOP, SHEHLOOP,
  SHNELOOP), a Cast exit (LCSSLOOP), a level 10-only exit (CRSTLOOP), and doors or gates no
  bash opens, which the path opens with a lever or a ring (DUELLOO2, SLSTLOO3, Vdtsloop).
- **What the paths know** (`PathKnowledge`, built once from the pack): for every step of a path
  whose walk reached its end, the room it is typed in and the exit it takes: a direction, with
  the actions written before it (`n[search n]`), or a phrase that takes a Text exit (`climb
  tree`). The Hidden exits of every verified path become ones the character knows are there
  when a MegaMud loop is walked or run (`Traveller::searchable`, `all_hidden_exits`): routes may
  take them, and the walker searches before each ("Walking"). A way into an area is usually
  another path's: the Bandit Keep's (`Ftrehkep.mp`: search north at the Hidden Keep, `climb
  tree` on the Keep Wall) is not `HKEPLOOP.mp`'s own, and `HKEPLOOP.mp` run from the Hidden
  Keep searches north first. A Text exit whose message has no text takes the phrase a path
  types for it.
- **Ways through that are not exits** (C-052). A room's command text that teleports is a way
  through when the line does nothing else that matters: the first line with its pattern, a
  plain pattern (no wildcard), and only `message`, `adddelay`, `minlevel`, `maxlevel`,
  `nomonsters` and one `teleport <room> <map>` (`minlevel` and `maxlevel` gate it as a Level
  exit). `ring chime` on Slum Street (1:1182) puts the character in the Strange Mansion's
  Entrance (1:65), the only way in for most: the front door is -800, beyond any bash. A line
  with a `cast`, a `random`, a skill test, an item taken or checked, or an ability or alignment
  check is left out (120 of the 552 teleport lines of the stock data are ways through). The
  game reads a room's command text only once its command parser has found no command in the
  line, so the brain gives each one the first phrase its command table does not claim (`ring
  chime`, not `use chime`); one with none is not taken. And a step of a verified MegaMud path
  that types a phrase where the pack has no Text exit or room command to the room the next step
  starts in is a way through too, with MegaMud's phrase (`go arch`, `pull bone`; `@party `
  dropped). Both cost a step, never show in `Obvious exits` or on the map, and Flee never takes
  them; Go to finds what is behind them. The walker types the phrase and waits for its prompt,
  then sends `look` (C-055): a teleport prints no room ("You find yourself...elsewhere."), and a
  phrase where the room has no such script is only speech (`You say "ring chime"`). The room
  the look shows is checked as a move's: the one it was going to, or a drift and a new plan.
  A stop that comes while the phrase is in flight (a party leader's hold for a follower the
  teleport left behind, a fight, the player) still waits for that look: without it the
  character's room is unknown, and the followers have nowhere to walk back to. A teleport
  ends the party (`kb/grouping.md` §2), so the followers walk back by the same phrase, and
  the leader invites them again.
- Made once per anchor, radius, level, strength, picklocks and doors for the session, so asking
  again is cheap.
- **Boss status.** A stop that is a boss's lair (a unique monster, `maxalive` 1, back `regenhrs`
  hours after its kill: the Minotaur Chieftain 24, Leo the Quick 2) lists the boss with whether
  it should be up: `bosses: [{name, regen_hours, up_in_minutes?}]`, filled each time the list
  is asked for (the loops themselves stay cached). The brain keeps each boss the character
  kills, bot running or not (the `boss_kills` table, schema version 3), and the status is that
  kill plus the hours; a boss it never saw die "should be up". It is a guess, since other
  players kill bosses too, and information only: a loop does not wait for a boss or plan
  around one, it walks through the room and fights what is there.
- **Custom loops.** A loop the player makes from another: a name and the stops in order, kept in
  the store (`custom_loops`, schema version 4) and listed in `loops` as `custom` for the
  areas they have a stop in. `save_loop {name, base?, stops}` makes or changes one (the id is
  `custom:` and a slug of the name, so the same name again replaces it; every room must exist,
  1 to 300 stops); `delete_loop {id}` forgets one. `run_loop` and `walk_loop` take its id like
  any other, and run it like a generated loop: the nearest ticked stop first, then round the
  stops in the order saved, legs routed by the router. The hidden exits of every verified
  MegaMud path are ones the loop's traveller searches for, so a stop behind one is reached (a
  stop no route reaches is passed by each round, as in "Running a loop"). A stop need not
  spawn anything; a boss's lair shows `bosses` ("Boss status" above). The path in the list is the
  route the loop would take, stop to stop and back. The web UI makes one from the picked loop
  (a name, and rooms found in the go box put in after a stop), and the live check does the
  same (`--save-as NAME --add-stop MAP:ROOM@N`, `tests/live/README.md`). Example: Bandit Keep's
  stops with Leo the Quick's Elaborate Bedroom (9:870, 15 steps and seven doors beyond the
  Living Quarters, behind the hidden exit west of 9:819 that `Hkepleoq.mp` searches) put in
  after the last Living Quarters stop: "Hidden Keep with Leo the Quick".

`walk_loop {session, id}` walks a loop once (a generated loop's id, or a MegaMud loop's file):
a journey ("Go to", above) to its stop nearest the character, then to each stop in turn, and
back to that first one. Narrated: "Walking loop Newhaven: 3 stops, 4 steps, from Newhaven,
Arena", the walker's own lines for each leg, "Loop stop 2/3: Dungeon, Entrance" at each stop,
and "Loop Newhaven done: back at Newhaven, Arena"; then the bot plays there. A fight on the way
stops the leg and the walk goes on after it; `■` ends it, and a `walk_to` replaces it. A generated loop's `id` says the area and the loop
(`map:room:radius:doors|nodoors:index`), so a restarted brain walks the same one; a MegaMud
loop's is its file (`Mwodloop.mp`, any case).

## The bot's states

The bot is **on or off**, and it may have a **route**: a loop it runs ("Running a loop") or a
journey (a Go to or a loop walked once, "Go to" and "Loops"). Each control changes one of the
two (John, 2026-10-23: "both go to and loop are initialized by me, so different behavior
doesn't make sense"):

| State | `bot.state` | `pause` (║) / `resume`, `start` (►) | `stop` (■, the map's `stop`) |
|---|---|---|---|
| On, with a route: walking it, fighting what comes | `running` | Paused, the route kept | The route dropped: it plays where it is ("Stopped loop Newhaven: playing here") |
| On, no route: it plays where it stands ("Fight here") | `running` | Off ("Stopped: asked to pause") | Off ("Stopped: asked to stop") |
| Off, with a route (paused) | `paused` | `resume`: re-sync, then on with the route from wherever the character is | Off, the route dropped |
| Off, no route | `stopped` | `start`: on where it stands | (nothing to do) |

- **Entering the realm starts it** (C-054, `bot.start_on_logon`, on by default): a session
  that goes into the realm (logged on, or back after a dropped connection) gets `start`, "Entered
  the realm: starting the bot", so it fights back, heals, rests and flees at once. A bot a
  brain before this one left is resumed instead, as it was: one stopped by hand stays stopped
  across a brain restart.
- **A Go to or a loop** (`walk_to`, `walk_loop`, `run_loop`) gives the bot that route and turns
  it on, from any state.
- **A Go to arriving** ends the journey: the bot stays on and plays there, that room its home.
- **A move the player makes** (a typed move, keys) while the bot is on drops the route, and the
  bot plays on wherever the player takes the character; any other typed line pauses nothing
  ("Typing beside the bot").
- **A death, a hang-up, the realm left, logging off** turn it off and drop the route, as before.
- A journey is not stored: a brain restarted on one puts the bot back on (or paused) with no
  route.

## Fight here

The first slice of the bot (`docs/plan/03-engine.md` sections 4 and 5). With the character
standing in a room the player chose, **Start** (`F10` or the tile's `[►]`, `bot_control
start`) makes it fight what comes there, rest when low, and flee when HP is dangerously low
("Flee", below). Every character starts **stopped**; the snapshot's `bot` says `{state, mode,
narration, reason?}`, and every change of what the bot is doing is a `notice` (its narration).
It needs a world pack (who is worth fighting, which rooms spawn) and the realm.

- **Start** re-syncs first (`st` for the HP maximum, `look` for who is here), remembers the room
  as *home*, then decides at every prompt with nothing in flight. If the room is not known at
  Start (a brain just attached, its history showing none), home is the first room the bot
  believes it is in after that, narrated "Playing from <room>".
- **The modes**, highest first; the first that wants control has it:

  | Mode | Wants control when | Does |
  |---|---|---|
  | Flee | HP below 25% (`health.flee_below_hp`) while running, whether or not engaged, with something to run from (a fight, an attacker, or a room that spawns); a monster to *flee from* in the room, whatever the HP; a stalemate; until it is done | Leaves by a confirmed exit toward a room that spawns nothing, keeps running if followed, then hands over to Recover ("Flee", below) |
  | Recover | HP below 60% (`health.rest_below_hp`) (or poisoned) and not fighting; or a rest it began is not finished. A rest starts only when the fight is over: not while a monster Fight would attack is in the room (one the party still fights, say), nor before the `look` after a kill; a rest under way is woken only by an attack, and under the flee line Flee goes first | Rests where it stands (a lair or an arena too: spawns are slow enough, John, 2026-10-24). With `health.rest_quiet_room` on: in this room if nothing spawns here; else walks (through the walker, each step confirmed) to the nearest room within 5 steps where nothing spawns, never a player's house (a room the pack has no name for), and rests there; else rests here anyway. `rest` again when the next prompt lacks `(Resting)` (three times, then a 30 s wait; `You are too sick to rest!` waits 30 s too). Rested at 95%, it walks back home. Each walk is tried twice (a fight can stop one): after two walks out that were stopped it rests where it is, against the quiet-room rule, and says so with a warning ("Recover: resting in Newhaven, Arena though it spawns: 2 walks to a quiet room were stopped"); after two walks home that were stopped it waits where it rested: "Rested; could not walk home, waiting here" |
  | Fight | Engaged; or a monster worth fighting is here | Picks one that is attacking the character first, else the first listed, by the monster rules ("Monster rules", below: never one to *never fight* unless it attacks, never one to *flee from*); sends `a <its last word>` once and lets auto-combat run. After `*Combat Off*` or a kill it looks (`look`) and picks the next. A target the game refuses (`You do not see rat here!`, `Your weapon has no effect`) is left alone for the rest of the session. Four rounds of the character's swings with no damage dealt, one of them glancing off the target's armour at least, are a stalemate: it gives the target up and the bot leaves ("Monster rules"). Never leaves the room itself |
  | Idle | Otherwise | Sends nothing: "Waiting here" |

- **Worth fighting**: every record the name finds (adjectives stripped: `fat giant rat` is a
  giant rat) is evil, not an NPC and worth experience (`kb/pitfalls.md` 1). **A room spawns**
  when the spawn pass makes monsters there or it holds one worth fighting; a room the position
  is not sure of counts as spawning. A shopkeeper's room is quiet.
- **Engaged** comes from `*Combat Engaged*` and `*Combat Off*`. Auto-combat can also end without
  a word (pitfall 42): 12 s with none of the character's own blows and it counts as over, and
  the snapshot's `in_combat` turns `false`. Knocked down (`You are smashed to the ground!`)
  the character has no blows until it rises (`Slightly dazed you get back onto your feet.`):
  that is not quiet time, and the 12 s start again when it rises. A monster's blows are not
  the character's: one attacking a character that is not fighting back is fought at once.
  Before the brain has seen either, the character counts as engaged until 12 s pass without
  its blows.
- **Players** are never fought, and a player in the room is not an attacker, even when a line
  of theirs reads like a monster's blow, unless a PvP line says so (`Bob moves to attack
  you!`, `Bob just attacked <the character>!`). So a player standing in the room does not
  keep Recover from resting.
- **A monster that attacks while resting** wakes the bot: Fight takes the character (in the
  quiet room, if it is there), then Recover carries on and walks home when rested. One that
  arrives and doesn't attack leaves the rest alone.
- **The bot stops itself** when the character dies (`You have been killed!`) and when the
  session leaves the realm, with a warning (and when a log-off under attack finds no way out,
  "Flee", below). Auto-combat goes on in
  the game: the fight is the player's.
- **Off and the player**: with no loop or journey, `pause` (`F9`, `[║]`) and `stop` (`F10`)
  both turn the bot off ("The bot's states"): it sends nothing more, and a walk of the bot's
  stops after the move in flight. A line the player types pauses nothing; a move typed (or
  keys) makes the room the character goes to its home ("Typing beside the bot", below). A
  `walk_to` from the map gives it a journey there, and it plays where it arrives.
- **Constants** in this slice, settings in M4 (`docs/plan/04-settings.md` section 3): rest
  below 60%, rest until 85%, flee below 25%, a 1.5 s watch for a follower, a quiet room within
  5 steps (`crates/wayfarer-brain/src/policy.rs`).

Not in this slice: coins and loot, healers (spells are in "Spells", below), bash and the other attacks,
"guard while paused" (paused means nothing is sent, Flee included), and a fight the bot thinks
it cannot win.

## Flee

Below the flee line (25% of the maximum HP; a plain percentage, John dropped "twice the
hardest hit this fight" on 2026-10-19) the bot leaves the fight. Every HP value is checked,
whatever is in flight: a walk of the bot's stops after the move in flight, an action of its not
sent yet is dropped, and Flee acts at the next prompt.

- **The exit**, from the room the position is sure of, best first: plain exits to rooms that
  spawn nothing, then plain exits to rooms that spawn, then doors and gates (quiet first).
  Within each, a room that is not protected comes first (a protected room, the shops and the
  Guild in Newhaven, refuses a character in combat: `You may not enter that room while in
  combat.`), and not back to the room it just left if another is as good. Never an exit the
  game refused this session, a gated exit (level, class, race, alignment, item, toll, spell,
  key), a Text, Hidden, Action or Remote Action exit, or a door that is locked: a door counts
  as the game last showed it this session (`open door north` in the exits, `The door is
  closed!`, `The door is locked.`, `The door is now open.`), else as the pack has it, and a cold
  start locks every door. A door is opened (`open <dir>`) then walked through, never bashed.
- **A hop**: the direction through the executor, confirmed by the room it shows (as a walker's
  step is). A refusal or a locked door tries the next exit at once; a shut door that is not
  locked (`The door is Closed!`) is opened first. Then it waits up to 1.5 s at the prompt for a
  follower: the monster's follow text (`The giant rat creeps in after you!`), a hostile monster
  arriving, or an attack. Followed, it hops again; not followed, and nothing spawns here, it is
  **safe**; not followed but in a room that spawns, it hops on toward a quiet one. At most
  `health.flee_rooms` hops (6) while HP stays under the line.
- **After**: Recover takes over, whatever the HP: it rests where Flee got to (walking to a quiet
  room within 5 steps if it must) to 85%, healing as it rests if it has a heal and the mana
  (`spells.heal_resting_below_hp`, "Spells"), then walks back home, and Fight carries on there.
  A monster that comes in and attacks while HP is still under the line sets off another flee,
  with all its rooms again.
- **Cornered**: with no exit to take (the position not sure of the room, every exit refused,
  locked or gated), or still followed after the last hop, the character turns and fights what
  attacks it, healing in the fight as the settings allow (`spells.heal_in_combat_below_hp`), and
  the bot goes on, with a warning: "Flee: followed by large giant rat through 6 rooms; fighting
  back". Flee does not take control again in that room until HP is over the line. Only a
  log-off under attack that finds no way out still ends in `break` and a stopped bot, so that
  the log-off can go on.
- **Whatever the HP**: a monster to *flee from* seen in the room (listed, or arriving; whether
  or not it attacks), and a stalemate, set a flee off at full HP too ("Monster rules", below):
  "Fleeing: gigantic black ooze is here (flee from): south to ...", "Fleeing: stalemate with
  gigantic black ooze: ...". Such a flee is over at the first room nothing follows into,
  quiet or not. Cornered, it does not stop the bot: it breaks off a fight, stays, and fights
  only what attacks it ("Flee: cornered: not sure which room this is; fighting only what
  attacks"); it does not try again in that room.
- Narrated as it goes: "Fleeing: HP 22%: up to Newhaven, Narrow Road", "Followed by large giant
  rat: east to Newhaven, Narrow Path", "Flee: in Newhaven, Narrow Path, watching for
  followers", "Safe in Newhaven, Narrow Path". The snapshot's `bot.mode` is `flee` meanwhile.
- **Hang up** (`health.hangup`, on by default, under `health.hangup_below_hp`, 10%): the last
  resort. A clean log-off (`x`) takes six quiet seconds and any blow interrupts it, so under attack
  it cannot finish; a character that lies at 0 HP is killed. With the bot running (not stopped),
  HP under the line and something hitting the character, the brain stops the bot ("Stopped: hung up
  at 8% HP (under 10%): something was hitting it"), warns, and sends the gateway
  `session_control` `hang_up`: the connection is dropped at once, the session shows `closed`
  and stays so until **connect** on its tile. The game ends the fight but may punish a hang-up in
  the realm: with `PUNHUP HIGH` it costs 25–50% of the maximum HP and half the inventory, which
  can kill (`kb/recovery.md`, "Hanging up"), so on such a server turn it off. The place shows in
  the recent places as "stopped". It is the one exception to "log off cleanly".
- **Mortally wounded** (HP 0 or below): the game refuses every move, attack, cast, rest and `x`
  ("You may not do that while you are mortally wounded!") while monsters go on hitting, and
  nothing a script can send helps (only another player's help, or rebirth at -200). The brain
  sends nothing for the bot or a walk then (a walk is dropped, not counted as a failed leg
  of a loop), warns once, "Mortally wounded (HP -36): nothing can be done until it is helped
  or reborn; waiting", and acts again when HP is above 0. What the player types still goes.
- **Paused or stopped**, nothing is sent: Flee does not act either (until "guard while paused",
  M4).
- **The live check's flee line**: `wayfarer brain --flee-below 100` (hidden, test only) raises
  the line so the first blow sets a flee off (`tests/live/README.md` "flee"). The settings give
  the player the line in M4.

## Monster rules

Per monster name, from `docs/plan/04-settings.md` "Monsters and players" (brought forward
from M4 by C-035, after the M3 soak's bot fought the gigantic black ooze for as long as it
stayed):

| Rule | The bot |
|---|---|
| *never fight* | does not attack it; fights back if it attacks (the flee line still applies) |
| *flee from* | leaves the room as soon as it is seen, whatever the HP (Flee, above); fights it only when it attacks and Flee found no way out |
| *avoid* | does not walk through the room it was last seen in when there is another way |

- **Matching** as MegaMud's monster lists: a pattern matches a name that holds it, in any case,
  and the longest matching pattern wins. So `ooze` would match the glittering ooze too (monster
  172, fightable): use the full name.
- **Three layers**, later over earlier for the same pattern: wayfarer's **defaults** (in code,
  `DEFAULT_RULES` in `crates/wayfarer-world/src/monsters.rs`, one entry with why: the
  *gigantic black ooze* is *flee from*: 2,500 HP, 500 regenerated a round, damage reduction
  40); what the bot learned this **session** (a stalemate); and the **player**'s
  (`set_monster_rule` from the stats tab, or `wayfarer brain --flee-from NAME`, repeatable).
  The session's and the player's are kept with the bot's record (the store's `monster_rules`)
  and survive a brain restart. The snapshot's `bot.monster_rules` lists all three.
- **A stalemate**: the character's own swings at its target are counted by round (a round's
  swings come within 2 s; rounds are 5 s apart). Four rounds in a row with no damage dealt,
  one of them a glance at least (`Your swing at <name> hits, but glances off its armour.`):
  "Stalemate: my swings do nothing to gigantic black ooze; leaving". Fight gives the target
  up, the monster becomes *never fight* for the session ("Monster rule: never fight
  gigantic black ooze (a stalemate)"), and the bot leaves as Flee does. A monster already kept
  off by a rule is not added again.
- **Sightings**: where a monster to avoid or flee from (or one a stalemate gave up) was last
  seen this session, for 30 minutes. The router treats those rooms as costly (1,000 a step,
  not impassable: another way wins when there is one, the only way is still taken). Recover
  does not walk back home into such a room ("Waiting here: large giant rat was last seen in
  Newhaven, Arena") until the sighting is forgotten; a loop stop in one is passed by this time
  round ("Skipping stop 2/3 ...: gigantic black ooze was last seen there").
- **Mid-walk**: a monster to flee from in the room the bot's walk ends in stops the walk, as
  the flee line does; one in a room the walk only passes through does not (the next step
  leaves it anyway, and a flee would turn the walk back).
- **In Task mode** a flee from a monster the bot does not fight ends that stop for this time
  round, not cleared ("Stop 1/3 Newhaven, Arena: leaving it for now"), and the loop goes on
  from the next stop.

## Running a loop

Task mode (`docs/plan/03-engine.md` section 4): the bot hunts round a loop the player chose,
generated or MegaMud's. `run_loop {session, id, skip?}` (the map panel's **run**; `id` a
generated loop's id or a MegaMud loop's file) starts it, replacing a running
"fight here" or a journey; `pause` keeps it and `resume` goes on with it, `stop` drops it and
the bot plays where it is ("The bot's states"). The snapshot's
`bot.mode` is `task` while it walks between stops or waits, and `bot.task` says the loop, the
stop it is at or walking to, and the stops it passes by.

- **Start**: the bot re-syncs, then walks (each step confirmed, through the walker) to the
  loop's ticked stop nearest the character by route, the first of equals in loop order:
  "Loop Newhaven: to stop 1/3 Newhaven, Arena (3 steps)". With none reachable it does not
  start, and says why for the nearest by any way at all: "Not running loop Bandit Keep: no way
  to any of its ticked stops (the nearest, Courtyard, Entrance Door: the door east of
  Darkwood Forest, Hidden Keep cannot be opened)", so the player can walk there by hand or
  with go to.
- **A MegaMud loop** runs like a generated one: its name is its description (its file when
  that is blank), its stops are its hunt rooms in path order, and the laps, waits, rests and
  statistics below are the same. Legs between stops are routed by the router, not replayed
  from the file's commands: the path's rooms are a known way, so a leg along them costs no more
  than the router's own, and one that leaves the path (a shortcut through a door) is taken
  when the router finds it. The path's hidden exits are known for its legs ("Loops", above). A
  loop listed not runnable is refused with its reason: "Not running loop Oasis Loop: nothing
  to hunt on it".
- **The mode stack** is Flee, Recover, Fight, then **Task**, then Idle: Task has control only
  when nothing above wants it. The stop it is at or walking to is *home*, so Recover rests from
  it and walks back to it, and Fight fights there, as in "fight here".
- **At a stop**: the room shown on arrival (or Fight's `look` after a kill) says who is there.
  When nothing is left that Fight would attack, the stop is **cleared**: "Stop 1/3 cleared: 1
  large giant rat" (what died there since it arrived), or "nothing there". A stop where hostile
  monsters Fight will not attack are left (refused targets) is cleared once the character's own
  combat has been quiet there for 20 s.
- **On** to the next stop in loop order that is ticked and ready, passing by the others with a
  word each: "Skipping Small Cavern (unticked)", "Stop 3/3 not ready (1 m)". **Ready**: every
  stop, except one the bot passed by in the last five minutes (below). There is no respawn
  clock: the bot walks the loop in order and fights whatever it finds, in a stop or on the way.
  A stop that spawns at once or on entering (an instant-spawn lair, an arena) gives its monster
  when the character steps out and in again; a boss on a long timer costs a walk through its
  room. Two 30-minute Bandit Keep soaks (2026-10-20, Normal rooms under an area spawner) gave
  31.4k exp/hr with a five-minute clock on every stop and 31.6k without it, with no rests and
  no idle waits in the second: what limits exp there is the supply of monsters, not the walk.
  (`task::passed_left`, `PASSED_FOR`.)
- **None ready** (every stop unticked, unreachable or passed by lately): with HP under 85% it
  rests first ("Waiting for respawns: resting first": Recover rests, in a quiet room if this one
  spawns, and walks back); then it waits where it is, "Waiting for respawns: next in 1 m", and
  goes when the next stop is ready. An Arena the bot stands in waits there and Fight takes what
  comes (its monsters come back at once). Any other stop that is the only one ready, the
  character in it, is stepped out of and back into ("Loop X: stepping out of Y and back in"):
  an instant-spawn lair (the cave worm in the sewers, the slime beast in the goblin caves)
  spawns when a player enters, so a one-room loop is a walk out to a plain exit beside it (no
  door, not a boss lair, a room that is no Lair first) and back. With no such exit it waits.
- **On the way**: a monster Fight would attack in a room on the way, or one attacking, stops the
  leg after the moves in flight; Fight takes the character, and then Task walks on to the same
  stop from wherever it is. `*Combat Engaged*` stops it the same way. Flee and Recover work as in
  "fight here"; after a flee Recover rests and walks back to the stop. After a leg that ends
  elsewhere the next waits 5 s (`LEG_PAUSE`, "Loop Newhaven: did not get to stop 2/3; trying
  again in a moment"), so a walk that keeps failing at once never floods the game (C-053). A leg
  that ends elsewhere three times with no fight between is given up: the stop is passed by this time round, with a
  warning-like notice ("Stop 2/3 Dungeon, Entrance: could not get there; passing it by this time
  round"). A stop passed by (a leg that would not get there, a monster to keep away from, a
  stalemate) is left alone for five minutes (`PASSED_FOR`) before the bot tries it again. Three stops passed by in a row, with no stop reached and nothing
  killed between, and the bot stops itself with a warning ("Stopped: loop Newhaven: could not
  get to a stop 3 times in a row").
- **Stops it cannot reach**: a ticked stop no route reaches from where the character is (behind
  a door it cannot open or that would not bash open, past a hidden exit it does not know) is
  passed by this round with no leg, and does not count towards the three above: those are
  for walks that keep failing. It is tried again next round (a door seen open, an exit found).
  The first round that skips them says how many and why, once while that stays the same:
  "Skipping 2 stops it cannot reach this round: the door north of Newhaven, Arena would not
  open" (or "... cannot be opened", "the hidden exit north of ... is not known"). With no ticked
  stop reachable at all the bot stops itself at once: "Stopped: loop Newhaven: none of its
  stops can be reached from here".
- **Pipelining**: on a leg, with the belief sure of the room and the character known to be out of
  combat, the next plain move (a direction, no door on it or on the move in flight) goes ahead
  of the move in flight, as MegaMud ran paths: two moves in flight at most, each confirmed by
  its room in order. Nothing but a move ever goes ahead, so two other actions never overlap, and
  the guards are the executor's as before. When the first move goes wrong (a room that is not
  the one expected, a refusal, a door) the second has gone already: its confirmation is void,
  and once its answer is in the walker plans again from wherever the belief puts the character
  ("Replanned (a move went ahead of a wrong step): one step"). `*Combat Engaged*` drops a move
  ahead that has not gone yet, and stops the leg after those in flight. A room on the way
  with a monster Fight would attack stops the leg too; when the move ahead has gone already,
  the character ends a room on, and the loop's next leg goes back to the monster's room first
  ("Loop Bandit Keep: back to Inner Keep Entrance (a monster there)"), once. Only Task's legs
  pipeline; a `walk_to`, `walk_loop` or Recover's walks go one move at a time. Recover's walk
  home after a rest stops for a monster on the way as a leg does, and once rested, a monster
  in the room comes before the walk home.
- **Ticks**: `run_loop` again for the loop the bot runs changes only the stops it passes by
  ("Loop Newhaven: passing by Small Cavern"); a leg to a stop just unticked stops after the
  move in flight. An unticked stop is not avoided as a room on the way to another (a loop's
  stops are rarely on each other's routes; a dead end such as the Minotaur Chieftain's room
  never is).
- **Pause and resume**: `pause` pauses it after the action in flight (a leg stops after its
  moves in flight); resume re-syncs and goes on to the same stop. `stop` drops the loop and
  the bot plays where it is; a move the player types or a `walk_to` drops it too, and typed
  lines that are no move go between its actions ("Typing beside the bot", "The bot's states").
- **Restarts**: the loop's id, its stops, the stop it is at or walking to and the stops it passes
  by are saved on every change ("Restarts", below), so a restarted brain goes on to the same
  stop: "Resumed: running loop Newhaven at stop 2/3 (brain restarted)". When the stops were
  cleared is not kept: after a restart every stop counts as ready. A MegaMud loop is resumed by
  its file under its own name, its path's hidden exits known again.
- **Statistics**: the run's kills, flees, deaths and rests are recorded with the room they
  happened in ("Statistics", below), so a run on a loop has them per stop. A kill is named by the
  monster the room showed (`large giant rat`), not the first of every monster that shares its
  death text.

## Typing beside the bot

While a character's bot is running or paused, the brain **arbitrates** its input
(`docs/plan/03-engine.md` section 6): it tells the gateway so (`arbitrate`), and every line a
UI types for that character comes to the brain (`user_input`) instead of going to the game.
The brain reads the line's first word with the game's own command table
(`crates/wayfarer-brain/src/commands.rs`: every abbreviation the game's parser accepts, checked
against silvermere's oracle, `tests/fixtures/command-words/`) and sends it as its kind says:

| Kind | Commands | What happens |
|---|---|---|
| Talk and look | `gossip`, `broadcast` (`br`, `'`, `-`), `auction`, `whisper` (`/`), `>`, `broadgang`; `look` (not at a direction), `st`, `exp`, `i`, `health`, `who`/`scan`, `help`/`?`, `wealth`, `spells`, `exits`, `keys`, `topten`, `version`; `set`; an empty line | Goes at the next prompt with nothing in flight, between the bot's actions: the bot's next action waits for the prompt after it. The bot keeps going, unpaused. Its output is parsed like any (a `st` refreshes HP), and confirms none of the bot's actions, which only go once it is answered |
| Moves | a direction (`n`, `u`, `nw`), an exit's word or a Text exit's phrase the position may take | Goes the same way. A running bot drops its loop or journey ("You moved: stopped loop Newhaven; playing where you go"), a walk of the bot's stops after its move in flight, and the bot plays on where the character goes: the room it is in becomes its home once the move's room shows. A paused bot keeps its route |
| Acts | `cast`, `rest`, `get`, `drop`, `buy`, `sell`, `wear`, `wield`, `open`, `search`, `look <dir>` (it shows the next room), `bash <dir>`, `x`, ... | Goes the same way, between the bot's actions; the bot keeps going, unpaused (John, 2026-10-23: only a move changes what the bot does) |
| Attacks | `a`/`attack`/`au`, `bash` (`aa`, `allout`, `force`), `backstab` (`bs`), `killblow` (`kil`), `punch` (`pu`, `purg`) | As an act: the line goes, and the bot keeps going |
| No command | `borrow skiff`, `hello there`, `say hi`, `heya` (MajorMUD has no SAY: talking is a line the parser does not take) | A text exit's phrase moves the character, so it is a move if it may be one: if the position is in doubt, or the line is a Text exit's phrase of a room the character may be in. Otherwise (the position is sure and no exit of the room takes the line) it is talk |

- **Keys** (`input_keys`) always go straight to the game; the gateway tells the brain
  (`keys_sent`), and since they may be a move they count as one: a walk stops, and a running
  bot drops its route and plays where the character ends up.
- **One line at a time**: a typed line goes only at a prompt with nothing in flight and no
  re-sync command waiting for its answer, so it never interleaves with the bot's exchange;
  several typed lines go one per prompt, in order. One that has waited 5 s for a prompt goes
  anyway (the game showed text and no prompt after it).
- **Stopped**, the brain arbitrates nothing (`arbitrate` off): the gateway sends lines straight
  to the game, and a typed line pauses nothing (it still stops a walk from the map). A line
  that reaches the brain after its bot stopped, or out of the realm, goes at once. While the
  gateway logs the character off, a typed line is not sent (a warning says so).
- **When the brain goes**, the gateway sends every session's input straight through at once; a
  brain that attaches again says which sessions it arbitrates.
- The plan's "after this fight, `sell sword`" is the same queue with one entry; the UI has no
  control for it yet.

## Stopping, restarting, the gateway going away

- **The gateway stops** (it logs every character off): before each log-off it sends
  `logging_off`. If the character is under attack (a monster or a player attacked it in this
  room, or its own auto-combat is going on), the brain flees first, whatever the bot was doing
  and whatever the HP, since every attack cancels the exit (`kb/recovery.md`): "Logging off
  under attack: fleeing first", the same hops as Flee, to a room where nothing follows (or
  cornered: `break`). Then, and at once when it was not attacked, it stops that character's bot
  ("Stopped: the gateway is logging off"), stops a walk after the move in flight, sends nothing
  more to it (no re-sync either) and answers `ready_to_leave`. The gateway waits 30 s for that
  answer at most. The brain stays attached while the gateway logs off (the retried `x`s come as
  `sent`), then the gateway closes the connection.
- **Ctrl-C or SIGTERM** closes the WebSocket and exits. Nothing is sent to the game; the
  characters stay where they are. The UIs see `brain_state false`.
- **Starting it again** attaches afresh: catch-up, a whole snapshot, a re-sync for every
  session in the realm. The UIs see `brain_state true`. A bot the store has running or paused
  is put back as it was, with its home and loop, once the catch-up is done ("Restarts",
  below).
- **A second brain** is refused by the gateway (`a brain is already attached`) and exits
  non-zero with that reason, as it does for a wrong token or another protocol version.
- **The gateway goes away**: the brain retries with the web UI's backoff (half a second,
  doubling up to ten) and, when it is back, catches up from the last `seq` it saw (those
  chunks' events do go out: they are new to the UIs) and sends a whole snapshot. It re-syncs
  only a session whose realm state was lost meanwhile: one not in the realm any more, one
  that left and came back, or one whose output went by unseen.

## Settings

The values the bot used to have as constants are settings (`wayfarer-settings`,
`docs/plan/04-settings.md`, "Status"). They come from files in the data directory (beside
`wayfarer.toml`), each holding only what differs from the layer below it. **Starter files**: when
a session's settings are first read the brain writes `settings.toml` and
`characters/<id>.toml` if they are not there: every setting with its help and default, all
commented out, so the file changes nothing until a line is uncommented. `wayfarer settings
--write [--character ID] [--server HOST:PORT] [--force]` writes one by hand (an existing file is
kept unless `--force`).

**Saved changes apply by themselves.** Every 3 s the brain looks at the modification time and
size of each character's files; when one changed it reads them again and puts them in force
without a restart or a pause of the bot: the flee line and the search limit at once, the rest at
the bot's next decision (an action in flight finishes as it was sent). The tile says what
changed (`Settings reloaded: health.flee_below_hp 25 → 30, spells.heal "auto" → "off"`). A file
that is not valid (a bad word, a number out of range) changes nothing: the earlier settings stay
and a warning says why (`Settings changed but not used, the earlier settings stay: ...`); mend
the file and it is read again. The first look after a session starts only notes the files, and
a settings change by the character's class (below) still applies when the class is known.

| Layer, least specific first | File |
|---|---|
| The default | built in |
| The class and style presets | the HP lines of the character's class, moved by `bot.style` (below) |
| My defaults, all characters | `settings.toml` |
| This server | `servers/<host>_<port>.toml` (`servers/127.0.0.1_2323.toml` for `127.0.0.1:2323`) |
| This character | `characters/<character id>.toml` (the id in `wayfarer.toml`) |

A missing file is an empty layer. `settings.toml` is checked when the brain starts, and a bad one
stops it, saying what is wrong (an unknown key, a number out of its range, a word that is not
one of the options, the HP lines out of order). A server's or a character's file is read when
that session is first seen; if it is not valid, that session runs on the defaults and its tile
shows "Settings not used, the defaults apply: ..." with the file and the reason. A change to a
file needs a restart of the brain (the bot resumes). `wayfarer settings [--character ID]
[--server HOST:PORT]` prints the files read and every setting with its value, the layer it
came from and its label, and checks the files.

```toml
# characters/thorgrim.toml
[bot]
style = "bold"
[health]
rest_until_hp = 92
```

| Key | Default | Range | What |
|---|---|---|---|
| `bot.style` | `"balanced"` | careful, balanced, bold | How far to move the three HP lines (flee below / rest below / rest until) from the class's: Careful adds 5 / 10 / 5, Bold takes them off, Balanced leaves them. A line set in a file beats the preset |
| `health.flee_below_hp` | 25 | 0–90 | Flee takes control below this per cent of the maximum HP |
| `health.flee_rooms` | 6 | 1–30 | Rooms Flee runs through while a monster keeps following, before the character turns and fights |
| `health.rest_below_hp` | 60 | 10–99 | Recover takes control below this per cent |
| `health.rest_until_hp` | 85 | 20–100 | Recover rests until this per cent |
| `health.hangup` | on | on, off | Hang up when nearly dead (below): drop the connection when the bot is running, HP is under the line and something is hitting the character. Turn it off on a server that punishes a hang-up in the realm |
| `health.hangup_below_hp` | 10 | 1–30 | The per cent of the maximum HP under which it hangs up. Keep it under the flee line, so fleeing is tried first |
| `movement.search_limit` | 300 | 1–1000 | Searches for a hidden exit before the walker gives it up |
| `bot.passed_by_minutes` | 5 | 1–120 | How long a loop stop the bot passed by is left alone |
| `bot.leg_tries` | 3 | 1–10 | Walks to one loop stop that end elsewhere before it is passed by |
| `bot.give_ups` | 3 | 1–20 | Stops passed by in a row before the bot stops itself |
| `bot.stop_quiet_seconds` | 20 | 5–120 | Quiet time that clears a stop where only monsters it will not attack are left |

**Class presets.** The HP lines (flee below / rest below / rest until) start from the
character's class, which the status screen names, so a class that needs a bigger cushion leaves
earlier. Until the first `st` the class is not known and a Fighter's lines apply; when it is,
the settings are resolved again, the flee line and the search limit take effect at once and the
tile says so ("Settings for a Mage: flee below 45%, rest below 70% to 90%"). Fixed settings
(a test's) are not moved by the class.

| Kind | Classes | Balanced lines | Careful | Bold |
|---|---|---|---|---|
| Fighter | Warrior, Witchunter, Paladin, Ranger, Cleric | 25 / 60 / 85 | 30 / 70 / 90 | 20 / 50 / 80 |
| Skirmisher | Ninja, Thief, Mystic, Warlock, Missionary, Gypsy, Bard | 35 / 65 / 85 | 40 / 75 / 90 | 30 / 55 / 80 |
| Caster | Mage, Priest, Druid | 45 / 70 / 90 | 50 / 80 / 95 | 40 / 60 / 85 |

The gaps between the kinds are an inference from the plan ("flee below 30 melee, 50 casters"),
not measured; the defaults (a Fighter's) are John's. `wayfarer settings --class Mage` shows a
class's lines before the character exists.

The HP lines must run flee below < rest below <= rest until. `wayfarer brain --flee-below` (a
test knob) overrides the flee line whatever the files say.

## What it keeps

The character itself is rebuilt from the gateway's history and a re-sync. What the game cannot
tell it is kept in the **store**, `store.sqlite` beside `wayfarer.toml` (`--store` to put it
elsewhere): SQLite, one file, written as things change. It holds nothing the logs do not
already show. A missing store is made; one that cannot be read (not SQLite, damaged, tables
missing) is moved to `store.sqlite.corrupt` and made afresh, with a warning in the log; one
written by a newer wayfarer is left alone and the brain runs without a store (and so resumes
nothing after a restart).

| Table | What |
|---|---|
| `bot_state` | Per session: the bot's state (`stopped`, `running`, `paused`), its home room, the loop it runs (`loop_id`, the stops as they were when it started, the stop it is at, the stops to skip; Task mode fills these), why it is paused, when it was saved |
| `monster_rules` | Per session: the monster rules the session learned and the player set, each with its pattern, rule, source and when it was made (schema version 2) |
| `boss_kills` | Per session and boss: when the character last killed it (schema version 3), for "should be up" |
| `custom_loops` | The loops the player made: id, name, the stops in order, the loop it was made from (schema version 4) |
| `runs` | Per session: each run, from Start to Stop or a death, and its loop |
| `events` | Per run: kills (the monster and the exp it gave), exp gained with no kill, deaths, flees, rests and level-ups, each with the time and the room |
| `samples` | Per run: the character's exp and HP, once a minute |

### Restarts

The bot's state is saved on every change: start, stop, pause, resume, its home found, a loop
stop reached. A brain that starts (the watcher, a crash, `scripts/dev/brain.sh`) reads it for
each session as it first sees it and, once that session's catch-up is done:

- **running** or **paused**: puts the bot back in that state with its home and loop, narrating
  "Resumed: fighting in Newhaven, Arena (brain restarted)", "Resumed paused (asked to pause):
  ..." or "Resumed: running loop Newhaven at stop 2/3 (brain restarted)"; the attach's re-sync
  goes first, then a running bot decides as usual. Its run goes on (the same row: the run's
  wall time includes the restart).
- **stopped**: nothing; the bot stays stopped.
- A session **not in the realm**: not resumed ("Not resuming the bot: the character is not in
  the realm"); the bot is saved stopped and its run ends.

A generated loop's id resolves against the character's level, so after a level-up it may make
another loop or none. The stored stops win: the narration then names the loop by its id, with a
warning that the id no longer makes the stored stops.

### Statistics

A run starts at Start and ends at Stop, a death, the session leaving the realm or the gateway
logging it off; pauses and brain restarts are inside it. While a run is open the brain records
what the game shows (not the catch-up's history, which a brain before it saw): a monster's
death text is a kill, the `You gain N experience.` after it its exp; `You have been killed!` a
death; `rest` starting a rest; the level going up a level-up; each flee a flee (the HP per
cent it started at, the first attacker, the room it left), a flee before a log-off included,
counted before the log-off ends the run.
Every minute it samples the character's exp and HP. Times come from the brain's clock, so a
replay can run on a throwaway store and its recorded clock.

The snapshot's `run` has the current run's numbers, recomputed on each event and sample:
`exp_per_hour` is the exp gained over the wall time since Start (a minute at least, so a first
kill is not thousands an hour), not since the last kill. `stats_request` answers with the last
ten runs and the newest one's samples ([protocol.md](protocol.md)); the web UI's stats tab
shows them ([web-ui.md](web-ui.md) "The stats"). `stats_reset` zeroes the combat numbers
(below), and ends the current run and starts another at once with its numbers at zero (the old
run stays in the list, its loop carried over): the way to measure a stretch of play without
stopping the bot. With no run it zeroes the combat numbers alone, and the notice says so.
`only: "run"` resets the run and keeps the combat numbers ("Reset: no run to reset" with
none), `only: "combat"` zeroes the combat numbers and lets the run go on: the stats tab's two
**reset** buttons.

### Combat statistics

C-050, in the shape of MegaMud's Combat tab. Every one of the character's own attack lines
counts, whether the bot or the player is driving (a party member's blows count on its own
numbers): `You smack rat for 9 damage!` a hit, with `critically` a crit, with `surprise` a
backstab (one that also crits counts as a backstab, and among the backstab crits); `You swing
at rat!` a miss, `… who dodges your attack!` a miss the target dodged; `… glances off its
armour` a glance (landed, no damage); a cast with damage a cast, and `cast_fail` or
`cast_resisted` in a fight (the character's auto-combat not quiet, or engaged) a cast that
missed. A `bs` or `backstab` line sent, the bot's or typed, is a backstab try. Accuracy is
landed (hit, crit, backstab, glance) of swings; the crit rate is crits (backstabs that crit
among them) of landed. A **round** is the character's own attack lines within 2.5 s of the
first (half the game's 5 s round; one round's lines arrive together): its damage, blows and
spells, is one round's, 0 for a round of misses. Kills, flees and the lowest HP and mana seen
are counted too.

The numbers are kept per session in the store (`combat_stats`, as JSON), saved within 5 s of
a change and when the session goes, so a brain restart loses a few seconds at most (and a
round still going). Stop, Start, a death, a level-up and restarts leave them; only
`stats_reset` zeroes them (not with `only: "run"`). `stats` carries them as `combat`.

## Errands

An errand is a one-shot job the player starts from the `[▼]` menu on a tile
([web-ui.md](web-ui.md)): `bot_control` `pick_up_all`, `equip_all` and `cancel_errand`. It runs only
with the bot stopped or paused (the player is playing; starting the bot, or anything attacking
the character, ends it with a warning, "Pick up stopped: something is attacking"), one at a time,
in the realm. Like the bot it sends a command only when nothing is in flight and the game is at a
prompt, and the snapshot's `errand` carries its narration (`Pick up: get torch`). Flee and Fight
do not apply: it is short, and the player stands by.

- **Pick up all** sends `look`, then `get N <coin>` for each coin on the floor, runic down to
  copper (copper too, unlike the bot's loot, but under the same `cash.max_encumbrance` load
  limit), then `get <item>` once for each copy of each thing lying there (ten at most). "You
  cannot carry that much!" ends the pickups: the rest are left and the summary says so ("Pick
  up done: 5 gold, torch; left: anvil (the load is too much)").
- **Equip all** sends `i`, then `wear <item>` for each carried item that is not already worn or
  held, leaving torches, lanterns, scrolls, potions, elixirs, flasks, keys and bread alone. A
  `wear` the game refuses ("may not be worn", "no more room") is tried as `wield <item>` when no
  weapon is readied, and only for the first; a taken slot is never emptied. "Equip done: wearing
  padded vest, holding short sword; left: iron helm".

### Training and recovering the death pile

Two errands go somewhere first (`trip.rs`), walking with the player's walker; one that cannot get
there says so and stops ("Train stopped: could not get to Training Room").

- **Train** (`bot_control` `train`): the trainer is a shop of type Training whose `trainclass` is
  0 (any) or the character's class and whose `minlevel..maxlevel` holds the character's *next*
  level (`kb/progression.md` "Training"), the cheapest by its markup, then the nearest by route:
  the universal Training Room (no markup) for levels 1 to 10 over a class trainer's 4x, the
  "Super <class>" trainer at 11, then the High Level and city trainers. The sysop's is left out. It
  sends `train` there until the trainer will not ("not enough experience for the next level", "not
  enough money", "this trainer does not teach the next level", ...), at most 20 levels, and says
  what it paid ("Train done: 2 levels, paid 6 gold; stopped: not enough experience for the next
  level. At Training Room: train stats here when ready"). **The character stays in the trainer's
  room**: the bot never `train stats`, the guard refuses it, so the player spends the character
  points there.
- **Recover death pile** (`bot_control` `recover_pile`): walks to the room of the character's last
  death (the recent places, kept whether or not a run was going), then runs Pick up all and Equip
  all there ("Pick up done: ...; Equip done: ..."). With no death on record it says so.

An errand in progress is asked again every half second even when the game says nothing, so a walk
or a re-sync that ends quietly does not leave it waiting.

## Buying spells

`spell_plan_request` is answered with a `spell_plan`: the spells the character's class could learn
that it does not know, each at its best shop, and what they would cost. It only reads: nothing
is bought or walked (the errand that does comes after the plan).

- **Where the spells are**: a scroll is an item of type 9 with ability 42 ("Link to Spell"),
  whose value is the spell's number (`kb/spells.md` "Learning"). A shop sells it when the slot's
  `maxqty` is above 0 (a 0 slot only resells what players sold). The pack's scrolls are read
  once, with each shop's room (`wayfarer_world::spellshop`).
- **What a class can learn**: the spell's magery is the class's, its magery level is at most the
  class's, and its level is at most the character's. Those above the character's level are listed
  after, marked "needs level N". Alignment is not checked.
- **What it lacks**: not in the `spells` screen's rows (the re-sync reads them): the plan waits
  for the class and the spellbook ("spell plan: the spellbook is not known yet").
- **The best shop** for a spell: the cheapest it can be reached at, then the nearest by route
  from the believed room; a shop with no way there is listed with no steps. A shop behind an exit
  whose item a shop sells (the Bard Song Store, up the Homely Hearth's ticket stairs) counts as
  reachable, in steps as if the item were held: the walk buys it on the way ("Walking").
- **Prices** are in copper: the item's cost in its currency, plus the shop's markup, then the
  Charm discount (`kb/economy.md` "Prices"; a Charm not known counts as 50). `cash` is the coins
  carried, in copper (the bank is not counted: its balance is read at the bank).
- **Affordable**: the usable spells in price order, each marked while the cash still covers it
  with the cheaper ones before it ("buy what the money covers").

**The shopping errand** (`bot_control` `buy_spells`, the plan's **buy spells** button) does it, with
the bot stopped or paused like the other errands ("Errands"), and `cancel_errand` ends it:

1. Scrolls at a shop with no way there are left at once ("song of lore (no way to Bard Song
   Store)"), with no walk, and don't count towards the bank.
2. When the cash on hand is less than all the reachable usable scrolls cost, it walks to the **Bank of
   Godfrey in Silvermere** (the room of that name; never another bank, John 2026-10-22), sends
   `bankbook`, reads the `On deposit:` line, and `withdraw`s the most the balance and the need
   allow. A bank it cannot reach, or an empty balance, is noted and the shopping goes on with the
   cash on hand.
3. It picks the scrolls the cash now covers, the cheapest first ("if there is not enough cash, buy
   what we can"; the others are listed as left with their price), groups them by shop and walks to
   each shop, the nearest first, with the player's walker.
4. In a shop it sends `buy <scroll>` and, on "You just bought ...", `read <scroll>`: "You read ...
   and learn the spell ..." counts it learned. A refusal (cannot afford, not sold now, unknown),
   "you already know this scroll" and an unreadable scroll are noted, not retried. A shop it cannot
   reach is left and the next goes on.
5. It says what it did: "Spells done: learned illuminate (spent 4 gold 1 silver 6 copper); left:
   mend (8 gold)". Something attacking, or the bot starting, ends it ("Spells stopped: something is
   attacking"), keeping what was bought.

## Sneaking

With *Sneak* on (`movement.sneak`, Movement, off by default; C-048) the bot keeps the
character sneaking. An `Attempting to sneak...` that did not say it failed is only a hope: a
failure the character's perception does not catch prints nothing. A move tells more, about the
move before it: a move made while sneaking, the character's own or its leader's, prints
`Sneaking...` before the game rolls for it, and a failed roll ends the sneak silently. So
`Sneaking...` means the character was still sneaking when the move began (this move may still
have been seen), and a move without it means the sneak had already ended: the move before it
failed, or something else ended it. Only Perfect Stealth makes a move sure. The bot believes the
character sneaking after an attempt with no failure and after each `Sneaking...`, and stops
believing it on a move without one, the failure the character noticed (`You don't think
you're sneaking.` on the same line), `You are no longer sneaking.`, `You make a sound as you
enter the room!`, a fight, a cast, a rest or a death. It never tries to sneak with a monster
in the room (`kb/classes/ninja.md`: the chance falls by one for every other player and monster
there; a fight comes first anyway).

- **Before a move the bot makes itself** (a leader's or a solo character's loop, Recover's
  walks to a quiet room and back; never Flee), it sends `sneak` until an attempt does not say it
  failed, narrated "Sneak: sneaking (attempt 2)". `You may not sneak right now!` (just
  attacked) or `You must wait before you may do that!` waits a round. A walk in progress stops
  when a move shows no `Sneaking...` or the game says the sneak ended, and the bot sneaks again
  before going on; a sneaker's moves are not pipelined, so no move has gone ahead by then. A
  failure found by a move counts as a failed attempt. After five failures in a row it moves on
  unsneaked, with a warning, until the next fight.
- **A follower**, which the game moves with its leader, sneaks whenever it is with the party
  and not sneaking, not fighting, not resting and with no monster in the room: after each
  fight, after a failure it noticed, and after a party move that showed no `Sneaking...`.
- **Backstab.** With *Open with a backstab* on (`combat.backstab`, Combat, on by default),
  Fight opens each fight of a sneaker with `bs <monster>` instead of its attack command, and,
  while it believes the character sneaking, instead of its attack spell too. Not sneaking after
  all, `bs` is a plain attack, which is fine. Any sneaker may try it: a one-handed weapon made for backstabbing,
  or bare hands, backstabs; any other weapon prints `You cannot backstab with this weapon.` and
  strikes a normal blow, and the fight goes on as any other.
- On the local server the attempt's line (with or without the failure) ends with CR LF and the
  prompt follows once the command delay is over, about half a second to a second later; the
  executor takes the attempt's line as the answer and is done at that prompt.

## Coins and the attack command

**Loot** is a mode between Flee and Recover: when nothing is attacking and the bot is not
engaged, and the last room view (`You notice 12 gold crowns, torch here.`) showed coins of a
kind the settings want, it sends `get <n> <coin>` (`kb/economy.md`: `get` takes one to 99,998)
and the character's coins and load are updated from `You picked up ...` (the next `i` makes
them exact). After a kill Fight looks, and that look is what shows the coins, so they are
picked up straight after the fight, before the bot rests or goes on. Coins seen while walking
between stops are not stopped for: Loot acts only when the bot stands still.

- **Which coins**: `cash.silver`, `cash.gold`, `cash.platinum` and `cash.runic`; platinum and
  runic are on by default, silver and gold off. Copper is never taken.
- **How many**: `cash.max_encumbrance` (`light`, `medium` default, `heavy`) is the heaviest load
  coins may bring the character to, in the game's words for it (Light up to 33%, Medium up to
  66%, Heavy over that). Three coins of a type weigh one unit, each type on its own, so the
  bot takes as many as fit under the limit (`get 59 gold` of 100) and none when none fit, not
  all or nothing. The capacity is the game's own from `i` (`Encumbrance: 650/1200`), and it
  already holds a dwarf's +20% (and any other Alter Encumbrance), so no race rule is needed:
  the same load is a smaller share of a bigger capacity. Without a known load (before the
  first `i`) it takes everything and the game's own limit stands (`You cannot carry that much!`
  ends the tries).
- A `get` that is not answered by `You picked up` is tried twice at most for a room view.
- **The attack command**: `combat.attack_command` is the verb Fight sends before the monster's
  name: `a` (default), `attack`, `bash` (`allout`, `force`), `bs` (`backstab`; the character
  must be sneaking), `kil` (`kill`: killblow) or `pu` (`punch`). The list is the settings' choices,
  and a test checks every one is an attack in the game's command table, so the executor's
  rule (no attack while engaged) covers whichever is chosen. MegaMud's free text allows
  anything its table knows; this list is the attacks silvermere's table has (there is no
  `smash`).

## Spells

A character whose class has magery (the pack's class record: Paladin, Cleric, Priest, Missionary,
Bard, Gypsy, Warlock, Mage, Druid, Ranger, Mystic) gets one more re-sync command, `spells`, after
`look`. Its rows (level, mana, short name, name) are matched to the pack's spells for the class
(same magery, magery level at or below the class's): single-target attacks (type 0, target 0, 4
or 8, ability 17) and self-castable heals (type 3, target 1, 2 or 6, ability 18) are the
spellbook; buffs, light and area spells are left out, and a Warrior's re-sync is unchanged. The
mana cost is the game's own, from the screen. The list is read again at every re-sync, so a
scroll read since is found the next time.

- **Attacks.** Fight sends `<short> <word>` instead of `a <word>` when `spells.attack` is `auto`,
  the mana is above `spells.attack_mana_above` (15%) and an attack is affordable: the one with the
  highest average damage at the character's level (`minval + mininc*L/minlvls` to `maxval +
  maxinc*L/maxlvls`, `L` held to the level cap), the cheaper of two equal. Out of combat the
  spell engages and auto-combat casts it every round (`kb/spells.md`); when the mana runs short
  the game falls back to melee itself. A cast the game refuses (`You do not have enough mana`,
  `You have already cast a spell this round!`) is not the target's fault: the fight uses the
  weapon for 12 s.
- **Casts per monster.** `spells.max_casts_per_monster` (0, the default, is no limit) caps the
  casts of the attack spell at one monster to spend a little mana on each fight: auto-combat
  casts the spell every round on its own, so after that many casts (counted from the game's
  own cast lines, a failed cast too) Fight sends the attack command at the same monster
  (`a rat`), which carries the fight on with the weapon, for the rest of that monster's
  fight, whatever the mana. A heal in the middle does not start the count again; a new
  monster does. A cast line counts as the character's own combat, so a
  fight fought with a spell does not lapse after 12 s of "no blows" and start over.
- **Heals while recovering.** Recover casts the cheapest heal that covers the HP that are
  missing (else the strongest it can pay for) while HP is below `spells.heal_resting_below_hp`
  (60%; 0 turns it off), one per round (5 s), before it walks to a quiet room, and again while
  it rests: a cast ends the rest and it rests again at once, so the heals come between the
  rest's regeneration, as long as the mana lasts (`You do not have enough mana` stops them for
  30 s). A Cleric at 57% heals itself instead of waiting several minutes. Resting is still how
  it reaches `health.rest_until_hp`. `spells.heal` `off` makes it only rest.
- **Heals in a fight.** Below `spells.heal_in_combat_below_hp` (50%) Fight casts a heal instead
  of waiting, once per 5 s (a round). Heals cost no energy (`energy` 0 in the pack), so the
  point is to heal between rounds and be engaged again before the next one: the cast ends
  auto-combat (`*Combat Off*`) and Fight attacks the same monster at once, with no `look`.
  The heal line is how far HP may fall before it heals each round. The flee line wins when it
  is higher (Flee is the higher mode), so the setting only matters above it. Only instant heals
  count: a heal that comes with a buff (righteousness, blood ritual), one over time
  (regeneration, way of the troll) and a resurrection are left out.
- **Mana rests.** Off by default (C-051): `health.rest_mana_below` is 0, and then mana plays
  no part in resting, solo or in a party: a rest is for HP and ends at the HP line whatever
  the mana. With a line set, a character that casts what it has rests when mana is below it,
  to `health.rest_mana_until` (90%), on the same rules as an HP rest (a quiet room if one is
  near, a rest started is finished), and every rest waits for the mana too. With both spell
  settings `off` mana is ignored.
- **Blessings** (C-047). The lasting spells the player ticks for a character in the
  Blessings panel (`blessings_save`, kept in the store) are kept on. They are the general
  spells (type 3) with a duration that the character's `spells` screen lists and that are cast
  on the caster (target 1, 2 or 6) or its party (10 or 13): bless, barkskin, holy aura. A Bless
  mode, ahead of Recover's rests in the stack, casts one (`bles`, no target word) when the
  spell is not believed on and the cast leaves the mana at or above `spells.bless_mana_above`
  (20%); narrated "Blessing: casting bless". Between fights it waits for no party heal to be
  due and for the character to be over its own rest line (it rests first). In a fight
  (`spells.bless_in_combat`, on by default, C-056) it comes after a heal, its own or a
  member's, and not in a round a spell of ours already went: the cast ends auto-combat, and
  that `*Combat Off*` is the bot's own, as after a heal, so the same monster is attacked again
  at once with no `look`. A cast ends a rest, the party's too, and Recover rests
  again after it: a blessing that wore off is not left off for a whole party rest. A cast is on from its prompt for as long as the spell
  lasts at the character's level (`duration + durinc*(L/durlvls)` 5-s rounds, the level held to
  the cap, at least `durlvl*L`), or until its wear-off line (the spell's Descriptive Message,
  `kb/spells.md`: "The effects of bless wear off!"); the line its target sees ("You feel
  lucky!") turns it on too, whoever cast it; a death ends them all. A failed cast ("You
  attempt to cast bless, but fail.", or no mana) is tried again after a round; three in a row
  hold the spell for five minutes, since one already on may not renew. A cast is done at the
  first prompt after it, and another player's line can bring one before the cast's own reply
  (Manuel, 2026-10-25): a failure line naming the spell that comes within the round after a
  cast taken as on still counts as its failure. Between fights, while a ticked spell is
  believed on, the bot looks at the status screen every `spells.bless_check_seconds` (60; 0
  never; any `st` screen, a re-sync's or the player's, counts), narrated "Blessing: looking at
  the status screen". The screen shows, after its last row, the line its target sees for every
  spell on the character, whoever cast it (`kb/screen-patterns.md` "Status screens"); one
  believed on whose line is not there is off, and is cast again ("Blessing: bless is not on
  the status screen; casting it"). A spell with no such line, or a sysop's debug list in place
  of the lines, is left as believed. `st` ends no rest (`kb/recovery.md`). The panel shows which
  are on (`blessings`).
- **Not done.** Buffs on other members, light, area spells, cures, spells
  that need a target the bot does not pick (a Thief's backstab is not a spell), and the
  Witchunter's no-cast rule are later slices. The average damage is the KB's formula; whether
  one spell beats another against resistances is not modelled.

## Parties

Party play (`docs/plan/10-party-play.md`, cards C-038 to C-043). A party is a
leader and one to five followers, by session id, made, changed and deleted with `party_save`,
`party_delete`, `party_leave` and `party_join` ([protocol.md](protocol.md)) and kept in the
store, so it survives a restart. A character is in one party at most: saving it into another
moves it.

**The game's party.** Each session keeps its party as the game last showed it: whom it
follows, the last whole `par` screen's rows (name, class, mana and HP per cent, the rest mark,
the rank, or an invitation), when that screen came, the last invitation, and when the game
last moved it after its leader. The invite, follow, drop and removal lines between two `par`
screens only mark it out of date, since the game drops a follower an exit refuses without a
word (`kb/grouping.md` §2a); a `par` screen cut off before the character's own row (always
last) is not taken.

**The coordinator** runs every second while there are parties, after the sessions' own steps.
It sends its lines through each session's executor between the bot's actions, as the bot's
own, one at a time:

- **Who counts.** A follower in the realm with its bot running; the leader whenever it is in
  the realm, whatever moves it (its bot, Go to, or the player typing). The others are `away`
  and not formed up.
- **Forming up.** A follower not in the leader's party in the game, in the leader's room,
  sends `follow <leader>`; a follower the game dropped at an exit is still on the leader's
  list, and `follow` alone takes it back. On `You must be invited first!` the leader sends
  `invite <follower>`, and on the invitation the follower follows. Names are the in-game first
  names, never the config's. The same `follow` or `invite` waits 10 s before it goes again.
- **Checking.** The leader sends `par` when it has none yet (a fresh brain: it forms nothing
  before), after any join, drop or removal line, when a joined follower has been out of its
  room for 3 s, and every 30 s while the party has members; at most one every 5 s. A follower
  the leader's `par` does not list is not joined, whatever its own lines said. "Out of its
  room" is by both beliefs: on a street of many rooms of one name each may be unsure, and two
  that hold a room in common are taken as together (C-053; before, the lowest of each one's
  rooms was compared, and a party in the slums sent `par` every 5 s).
- **Followers don't walk.** A follower's own `run_loop`, `walk_loop` and `walk_to` are refused
  with a warning ("following Penny: it goes where the party goes"). Its errands run: pick up all
  and equip all where it stands, in the party; one that walks (train, recover the pile, buy
  spells) takes it out of the party first, as `party_leave` does ("Left the party for the
  errand"). Made a follower,
  a bot running a loop stops the loop and fights where it is ("In Penny's party: the loop
  stops"). Its modes never walk it, Flee aside: Recover rests where it stands and does not
  walk home. Play and pause work as for any bot.
- **Re-sync.** `party_resync` (the panel's **re-sync**, one party or all) is for a party that
  is stuck: the game broke it without a word and the next routine `par` is half a minute off,
  or a follower gave up on a walk back. Every member in the realm sends `par` at once (a
  follower's tells it whom it follows), what each knows of its party counts as out of date
  until those screens come, and the `invite`s and `follow`s they call for go without their
  10-s wait. A follower's walk back that found no way is tried again, and the leader's wait for
  a separated member starts afresh (a loop it stopped stays stopped). A notice says "Re-sync:
  the parties are forming up again".
- **Leaving.** `party_leave` takes a follower out (a party with no followers left is deleted;
  the leader's, the whole party): it sends `leave` while the game has it following, and plays
  on its own. A follower dropped from a party by `party_save` or `party_delete` does the same.
- **Separation.** A follower that was in the party in the game and no longer is (the
  leader's `par` leaves it out: an exit refused it, a flee, a stun) is `separated`. Its bot
  walks it to the leader's believed room, the one walk a follower makes, fighting what attacks
  it on the way (`walking`, with the rooms left), and re-targets when the leader moves; there
  it forms up again. A follower added with `party_join` walks in the same way; one saved with
  the party but never in it does not. Meanwhile the leader **holds**: its loop's next leg (and
  a leg already walking) waits, narrated "Waiting for Lauren to catch up" (all who are out, by
  name); it still fights and flees, and moves the player types or a Go to are not held. With
  no way to the leader's room known, the follower is `unreachable` ("no way to Penny's room is
  known", a warning on its tile) and tries again after 30 s or when the leader moves. After
  `party.wait_separated_minutes` (Party, 5 by default) the leader's loop stops with a warning
  ("Loop stopped: Lauren did not catch up in 5 minutes"); it never walks on without the
  member, and the follower keeps trying. While the leader is out of the realm its followers
  stand where they are.
- **Party rests.** The leader and the followers with it (joined, in its room, bots running)
  rest together: when any of them falls below its own rest line (`health.rest_below_hp`, or
  `health.rest_mana_below` for a character that casts and has one set), the party rests until
  every one of them is back to its own rest-until lines (the mana's only with a mana line). The leader's loop holds; its Recover rests too,
  walking the party to a quiet room first when the room spawns and `health.rest_quiet_room` is
  on (the game's follow brings the
  others), and waits there, narrated "Recover: Resting for Lauren (62%)", while it is rested
  itself. Every follower with the leader that is below its own rest-until lines rests where
  it stands too ("Recover: resting with the party"), not only those below their rest lines:
  standing, a character gets its HP back several times more slowly than resting (C-045). A
  monster that comes is fought,
  then the rest goes on. After `party.wait_rest_minutes` (Party, 10 by default) the leader's
  loop stops with a warning naming who has not recovered. Afterwards the leader walks back to
  its loop as after any rest.
- **Party flees.** A follower with the leader never flees on its own (that would break the
  party): when it goes under its own flee line, the leader runs, once for each time it goes
  under ("Fleeing with the party: Lauren at 20%"), by its own Flee and `health.flee_rooms`,
  and the game's follow takes the others out behind it. The leader under its own line flees as
  ever, and the party with it. Afterwards, anyone the game left behind is separated (above),
  and the party rests. A follower away from the leader flees on its own.
- **Assist.** A follower with the leader, with nothing attacking it, goes for the monster the
  leader fights (if its own monster rules would fight it): the game shares exp only among
  those fighting the monster that dies (`kb/grouping.md` §9). Attacked by another, it fights
  that one.
- **Helping bash a door.** While the leader's walk bashes a locked door, a follower in its
  room, out of a fight, bashes the same door (`bash <dir>`, "Helping Penny bash the door
  north") until it gives way, by *Help the leader bash doors* (`party.help_bash`, Party, on by
  default; MegaMud's *Help Leader Bash Doors*). Each bash is the basher's own roll on Strength
  (`kb/navigation.md` "Door / Gate"), so each member at it opens the door sooner. Its own
  `You bashed the door open.`, or `The door is already open.` (another got it first), ends its
  help at that door; a failure tries again. The leader's next `bash` then finds the door open
  and walks through, and the game's follow takes the party with it.
- **Healing members.** A character with a heal that can be cast on another player (target
  0, 2 or 6: minor healing, mend) or on the party (target 10 or 13: healing rains, divine
  mercy), with `spells.heal` on, heals the other members in its room the way it heals itself,
  by a second pair of lines in the Party group: *Heal a party member in a fight below*
  (`party.heal_member_in_combat_below_hp`, 50%) and *Heal a party member while resting
  below* (`party.heal_member_resting_below_hp`, 60%; between rests, so a party rest is
  shorter). HP comes from each member's own prompt. Its own heal stays its self-heal's. The
  member furthest under its line goes first, one under its flee line before anyone, with the
  cheapest single heal that covers what it is missing (`mihe lauren`). With *Use party
  heals* (`party.heals`, auto) and a party heal known, that goes instead when two or more
  (the caster counted) are under their lines, or when it heals more over all of them than the
  single heal would on the worst; a member under its flee line still gets the single heal.
  A party heal is found by its record, never by its word: `rain` is also the Druid's acid
  rain. In a fight the healer goes back to its target after the cast, as after its own heal.
  A Witchunter is healed like anyone, though its anti-magic may resist the cast. There is no
  mana kept back for heals.
- **Heals in a party rest** (C-045). While the party rests, a healer heals each member up to
  that member's own rest-until line (`health.rest_until_hp`), itself included, instead of
  only below the heal-a-member line, and says so ("Recover: casting minor healing on lauren
  (party rest)"). A cast goes only when it leaves the rest no longer than it is: the rest
  lasts until the slowest member has its HP back at the resting rate and its mana back at
  its rest-until mana line, and the cast moves HP off someone's wait onto the healer's mana.
  The rates are the game's base ones from the level and the stats `st` shows (`kb/recovery.md`:
  `max((level+20)*Hea/750, 1)` HP a unit, one unit per 30 s and three more every 21 s
  resting; mana `stat*(level+20)*(magery level+2)/1650` per 30 s whether resting or not, the
  stat by magery); gear that alters them is not counted. Until a member's stats are known the
  cast goes only if it leaves the healer above its rest mana line. A heal-a-member line of 0
  turns these heals off too.
- **The UIs** get `parties` when one changes or a member's state does: `joined`,
  `not_joined` ("forming up", "in another room"), `away` (with why), `separated`, `walking`,
  `unreachable` or `resting` (with its HP).

## Not yet

No Maintain mode; no "Where next?" and no moving on to the next area (M4: the loop is the
player's choice); no "after this fight" control for the typed-line queue; the refused exits and
the stops' cleared times are forgotten when the brain restarts; player-recorded loops are not
walked or run; a MegaMud step's actions other than a search (`n[pull lever]`, `show signet
ring`) are known but not typed, so the doors and gates they open are not routed through.
