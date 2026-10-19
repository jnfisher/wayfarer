# Plan: group scripting (leader, followers, and talk between characters)

Status: **G0 done** (2026-10-13): patterns, `test/play/party.py` `Remotes`, `set maxpages 30` at bot
start-up, `tests/test_party.py`, live check `partycheck.py` 100/100. **G1-G2 in test** (2026-10-13):
`parties.json`/`parties.example.json`, `progress.py --party NAME --as CHAR`, `LeaderRole`/`FollowerRole`. This builds on `kb/grouping.md`, which covers the game rules for
parties, exp sharing, aggro and spawns. This file is about how *bots* run a party. Tags: **[C]** source or
data, **[F]** web or forum, **[I]** design or inference.

## 1. What MegaMud does, and what we copy

MegaMud ran one client per character. Each one was configured as **leader** or **follower**, and they
coordinated through ordinary game text: telepaths, plus room messages every client could see [F].

- **Roles.** The leader has a *friends* list it always invites, and does all the pathing. Followers accept
  the leader's invitation, follow, and never path on their own except to catch up [F].
- **Remote commands** are `@`-words "telepathed, spoken or paged to the character" ([kyau.net remote
  commands][kyau]):
  - Party: `@wait` (follower needs rest, so the leader stops moving) and `@ok` (done, so the leader may go
    on), `@comeback` (follower got separated; the leader backtracks up to five rooms), `@heal`, `@blind`,
    `@diseased`, `@held` (requests for help), `@party <cmd>` (leader: every follower runs `<cmd>`, then the
    leader re-checks the party and re-invites), `@panic!` (leader is about to hang up or teleport),
    `@kill <player>`, `@share`.
  - Queries: `@health`, `@exp`, `@level`, `@status`, `@where`, `@who`, `@wealth`, `@enc`, `@have <item>`,
    `@seen <name>`.
  - Control: `@invite`, `@join`, `@forget`, `@stop`, `@rego`, `@goto <room>`, `@loop <path>`,
    `@do <cmd>`, `@relog`, `@hangup`, auto-setting toggles (`@auto-combat on` and so on), `@divert`.
- **Party options** ([MegaMud Compendium][comp], and the mudengine re-implementation [PR 19][me19]):
  "Wait if Party Members are below" (HP %), "Wait No Longer Than (min)", "Ignore @wait If Leading",
  "Ignore @party If Following", "Request Party Health", "Par Every (s)", "Send par After Each Combat Round".
- **Assist without messages.** The Compendium's group recipe has followers trigger on room text, not
  telepaths: "`{Leader} moves to attack {target}`" answered by `attack {target}`, and "`{Leader} stops to
  rest`" answered by `rest` (healers: `mahe`, then `rest`) [F]. Both lines exist in the source [C]:
  `LJNCOMBT.C:2603`, `LJNCMDS.C:973`.

**Lessons from mudengine** (an open-source MegaMud-style client, PRs [6][me6], [19][me19], [39][me39]) [F]:
1. After a **text exit** (`go portal` and the like) the follow link silently breaks, and the leader gets no
   message. Body in the room ≠ still following. Fix: before the crossing, relay `@party <exit cmd>`; after
   arriving, re-invite everyone who was standing with the leader, then confirm with `par`.
2. Followers have to cross **at the same moment** as the leader. A pacing gap between relay and move let a
   follower arrive first, and the follow mechanic then dragged it back.
3. Send `@ok` only after **5 s standing**. Otherwise a mid-rest heal produces `@ok` and then `@wait` a second
   later.
4. **Don't send `@wait` while the leader is already resting.** It caused a rest loop.
5. Parse the follow-confirmation line exactly. One missed line ("You are now following X" without a
   period) left a follower unaware it was in a party, so none of the party logic ran.
6. Bound every wait ("Wait No Longer Than"). A member who never answers mustn't freeze the party.

## 2. Game facts the design rests on [C]

| Fact | Consequence |
|---|---|
| A telepath (`/name msg`) reaches anyone online. It is **refused silently** (`--- Telepath Not Sent ---` to the sender) when the target's per-round page allowance is used up. The allowance resets every **medium cycle (3 s)**. The default is 3, and the check is `<=`, so 4 get through. `set maxpages N` raises it, up to 255. (`LJNCMDA.C:4554`, `LJNBKGND.C:1920`, `LJNUTIL.C:1485`, `LJNGLOBL.C` `secmed=3`) | Telepaths are fine for coordination. Every bot runs `set maxpages 30` at start-up, sends ≤ 3 per 3 s to any one target, and retries on `Not Sent`. (This corrects `grouping.md` §1, which said not to use them.) |
| Receiver sees `<Name> telepaths: <msg>`. Sender sees `--- Telepath Sent to <Name> ---`. | Registry events `telepath` / `telepath_status` already exist (`patterns.py:643`). Every message gets a delivery receipt. |
| `say` (`.msg`) reaches the room. `>name msg` (say-to) also counts against the allowance but isn't checked, and is refused only when the target is in another room or map, or hidden, never by blocking telepaths or the forget list (`LJNCMDA.C:4668-4669`, `:4713`). | Use telepaths, not say. Say leaks to strangers and is easy to spoof. |
| Party = `invite` + `follow`, max 6. Followers move in the leader's fast cycle but run their own exit checks, and silently drop out on failure (`You are no longer following X.`). | The leader must poll `par` after risky moves. Route planning uses the most restricted member. |
| Exp only goes to members **in autocombat on the dying monster** (equal split). Casting ends autocombat. | Followers must assist the leader's target and re-engage after every cast. |
| Monsters pick targets at random. There's no taunt. | Everyone takes about 1/N of the hits, so each member's own flee line still matters. |
| More players in a Normal or Arena room means more spawns. | Grounds get busier with a party. The leader plans for it. |

## 3. Architecture

**One bot process per character, MegaMud style** (one `progress.py` per bridge session, as today). They
coordinate only through the game: telepaths for requests and answers, room text for assist and rest, and
`par` for status. This keeps each character independent, works across machines, and lets one of the
"characters" be a human or a real MegaMud.

```
progress.py --role leader  --party plans/party-brynja.json   (Brynja: paths, picks fights, rests)
progress.py --role follower --leader Brynja                   (Isolde: follows, assists, heals, asks)
        │                                       ▲
        └── telepath: "@wait", "@req 7 train", "@ack 7" ───┘   room text: "Brynja moves to attack rat."
```

New module `test/play/party.py`, used by `progress.py`:

| Piece | Job |
|---|---|
| `Remotes` | Parse incoming telepaths into commands. **Accept only from the configured party** (anyone can telepath; never obey strangers). Send with rate-limit, delivery receipt and retry. |
| `Roster` | The leader's view: members, invited/joined, H%/M%, resting, last seen, outstanding `@wait`s and requests. Fed by `par`, `@health` answers and room text. |
| `LeaderRole` | Formation and re-invites, movement holds, relays for special exits, target calling, party rest, answering requests. |
| `FollowerRole` | Accept invites, assist, heal, ask (`@wait`, `@req`), catch up and rejoin, never path or pick fights. |
| Transport | Telepath by default. A local socket bus behind the same interface, for an offline simulator and tests only. |

Rejected alternative: **one process drives every character.** Coordination would be trivial (shared memory),
but it isn't how MegaMud works, it can't include outside players, and it hides the timing problems that a
real client has to solve. The local-bus transport keeps that option open for simulation.

## 4. Configuration

`plans/party-<leader>.json` (the party's own progression plan, MegaMud style):

```json
{
  "leader": "Brynja",
  "friends": ["Isolde"],
  "ranks": {"Brynja": "front", "Isolde": "front"},
  "wait_below_hp": 60,
  "wait_no_longer_than_s": 300,
  "catch_up_s": 30,
  "ignore_wait_if_leading": false,
  "ignore_party_if_following": false,
  "par_every_s": 20,
  "stages": [ {"levels": [1, 3], "area": "Newhaven Arena", "anchor": "1:2150", "radius": 2} ]
}
```

- A stage's `levels` are matched against the **lowest member level**. The arena's maxlevel 3 means the whole
  party leaves when anyone outlevels it, so the check uses the highest level as well.
- Followers need only `--leader NAME`. Their individual `plans/<class>.json` is ignored while in a party.

## 5. Protocol

Plain MegaMud `@`-words, so a real MegaMud could take part, plus a small request/acknowledge layer for the
"follower suggests, leader decides" flow.

### 5.1 MegaMud-compatible words (both directions)
| Word | From → to | Meaning / bot behaviour |
|---|---|---|
| `@invite` / `@join` | either | Ask the other side to invite you, or to follow you. Leader answers `@invite` from a friend by inviting. |
| `@wait` / `@ok` | follower → leader | Hold movement / may go on. `@ok` only after 5 s standing. No `@wait` while the leader rests. The leader drops a `@wait` after `wait_no_longer_than_s`. |
| `@comeback` | follower → leader | "I'm separated." The leader holds, answers with `@where`-style text, and walks back up to 5 rooms toward the follower's last known room. |
| `@heal` / `@held` / `@blind` / `@diseased` | follower → party | Help request. A healer queues the matching spell on the sender. |
| `@party <cmd>` | leader → followers | Run `<cmd>` now (used for exit relays, `rest`, `frontrank`). Followers check `<cmd>` against an allow-list: movement, `rest`, rank, `go`/`use` exit phrases. Never `@do`-anything. |
| `@panic!` | leader → followers | The leader is about to flee hard or hang up. Followers flee to the leader's announced refuge, or log off if told to. |
| `@health`, `@where`, `@status`, `@exp`, `@level` | any → any | Status queries. The answer is a telepath back, in MegaMud's wording where it is known. |
| `@stop` / `@rego` | follower → leader (or human → bot) | Pause or resume the leader's movement. |

### 5.2 Requests with acknowledgement (our extension)
A follower can't make the leader do anything. It can **ask**, and the leader decides:

```
Isolde → Brynja:  @req 7 train            (I have the exp to train; please route via my trainer)
Brynja → Isolde:  @ack 7                  (accepted, queued)    or   @nak 7 busy  /  @nak 7 unsafe
Brynja → Isolde:  @done 7                 (done: e.g. we're at the trainer; go ahead and train)
```

- `id` = a per-sender counter. Resend the same id if there's no `@ack`/`@nak` in 10 s, at most 3 tries. The
  leader treats duplicate ids as one request.
- Initial verbs: `train`, `shop`, `heal` (healer shop), `rest`, `light` (buy one), `town`, `bank`, `pile`
  (one recovery attempt, per the rules), `leave` (follower wants out), `area <next>` (follower thinks it
  has outlevelled the area).
- The leader's own decisions that followers must know about are announced the same way (`@party`), so a
  human watching the viewer can read the whole conversation as plain text.

### 5.3 Delivery
- Telepath `/Name @word …`. One line. Max 3 per 3 s per target. Wait for `--- Telepath Sent to Name ---`.
  On `--- Telepath Not Sent ---`, back off one medium cycle (3 s) and retry. After 3 failures, mark the
  member unreachable (offline, or `forget`-ing us).
- Incoming: accept only `telepath` events whose `who` is the leader or a friend. Log everything else.
- Every message also goes into the bot log and as a note in the viewer, so John can follow the talk.

## 6. Behaviour

### 6.1 Formation
- **Leader:** in town (or wherever members are), `invite` each friend in the room. On `X started to follow
  you`, mark joined, then `par` to confirm. A friend who is online but elsewhere gets `@join`. The leader
  waits up to `catch_up_s` for them to walk over. Set ranks (`frontrank`/`backrank` via `@party`).
- **Follower:** on `Leader has invited you to follow` → `follow Leader`, then confirm on `You are now
  following Leader`. When the leader is elsewhere, walk to them (ask `@where`, locate the room with
  `mmud_data`), then send `@invite`.

### 6.2 Movement (the leader paths, followers ride along)
- Before each step, the leader holds while: any `@wait` is outstanding, any member's H% < `wait_below_hp`,
  any member is resting, meditating or missing, or a request asked for a stop. Every hold is bounded.
- **Special exits** (text, key, door, class/level, hidden): relay `@party <cmd>` with no gap, make its own
  move, re-invite everyone who stood with it, then `par`. Anyone missing gets one trip back (a route to the
  room they were left in), then the party goes on without them and says so (lessons 1 and 2).
- Route planning uses the **intersection** of the members' constraints: level and class exits, keys held,
  light (does every member have one?), encumbrance.
- `par` every `par_every_s` and after each special exit. A follower that sees `You are no longer following`
  sends `@comeback` right away.

### 6.3 Combat
- **The leader picks the target**, using its own rules (evil only, winnable for the *party*: summed DPR,
  each member's HP).
- **Followers assist** on the room line `Leader moves to attack <mon>` by attacking the same monster (same
  name). No telepath needed. If a monster attacks a follower first, it fights back as today.
- **Healer follower (Isolde):** heal the most-hurt member under the threshold (self first if needed), then
  **re-engage** with `a <target>` to stay in the exp split. Party rain once she has it (L10).
- **Flee:** the leader flees for the party. Party flee line = any member at their own flee line, or the
  summed estimate going bad. A follower below its *critical* line flees alone, then `@comeback`s. This
  breaks the party, which is better than dying.

### 6.4 Rest and recovery
- The leader rests when any member is below `wait_below_hp` or the healer's mana is low. Followers rest on
  `Leader stops to rest` (healers heal first). The leader resumes when `par` shows everyone above their
  rest-to line, and all `@wait`s are cleared.
- Never rest in spawn rooms (existing rule). The leader picks the rest room.

### 6.5 Town, training, loot
- **Loot:** the leader picks up coins and items. `share` coins when a follower's `@req train` needs money,
  or every N kills.
- **Training:** followers `@req train`. The leader routes via each needed trainer (class trainers differ)
  and holds there. No one runs `train stats` (John spends CP).
- **Death:** a member's death ends the party for that member. The leader takes the party to safety and
  waits `wait_no_longer_than_s` for them to come back (`@invite`). One pile-recovery attempt, then abandon
  (existing rule).

## 7. Screen patterns needed
In the registry already: `telepath`, `telepath_status`, `invited_follow`, `you_invited`, `invite_needed`,
`party_row`/`party_header` (`par`). To add, with golden tests (`grouping.md` §9 has the exact strings):
`now_following`, `started_follow`, `no_longer_following`, `removed_follower`, `following_party_leader`,
`player_moves_to_attack` (`%s moves to attack %s.`), `player_stops_rest`, `player_breaks_combat`,
`cant_move` variants, `party_rank`, `maxpages_set`.

## 7a. G0 findings [C]
- `You are now following %s` has **no period** (LJNCMDA.C:2565); `par` says `You are following %s.` with one.
  The registry's `following` pattern takes both.
- The leader is **never told** when a follower drops: `stop_following` writes only to the follower
  (LJNUTIL.C:7723). The leader must poll `par` or rely on the follower's `@comeback`.
- `par` rows: `  %-30.30s %-12.12s [M:%3s%] [H:%3s%]PR - Frontrank` (first + last name, class in
  parentheses, M or K bar only when the member has mana/kai, P poisoned, R resting / M meditating).
- Telepaths that arrive between a bot's commands were invisible to it (pitfall 35). Fixed in the bridge.
- `set maxpages N` answers `Maxpages set to N`; it is saved with the character.

## 7b. G1 findings [C live]
- The leader's own `par` list **includes the leader** (`Brynja Ashvale (Warrior) [H: 94%] R - Frontrank`),
  so drop your own name when building the roster.
- A member joins at **Midrank**; ranks must be set after joining (`frontrank`, `backrank`).
- The follower's screen shows the leader's attacks (`Brynja moves to attack thin giant rat.`) and the
  split (`You gain 4 experience.` for a 9-exp rat): the hooks for assisting (G3) are there.

## 7c. G1/G2 live findings and decisions (2026-10-13) [C live / John]
- **Unconscious is not dead.** This server's `DEATHHP` is **-200** (`WCCMMUD.MSG`; source default -10), so a
  downed character can lie at -100 for minutes while monsters keep hitting. Mortally wounded players may
  still telepath (LJNCMDS.C:156): the bot now answers `@where`/`@health` with **`@down M:R`**, and a
  follower that hears it waits somewhere quiet. (`suicide` would shortcut the wait; John: never script it.)
- **A follower's role must see every screen.** Invitations and drops that arrived while the follower sat in
  `rest()` (or a fight, or a walk) were consumed there and lost. Every read now goes to the role's
  `on_events`; the role acts at the next `tick()`, which also runs inside the rest loop. The leader also
  telepaths **`@join`** after a successful `invite` and waits 3 s for `started to follow you`.
- **The leader puts reuniting first** (John). A member missing from `par` gets `@stay` + `@where`; the leader
  walks to the reported room and invites there, repeating until they follow or `wait_no_longer_than_s`
  runs out. A follower told `@stay` stops chasing (chasing a stale `@here` led Isolde into a lone fight).
  A badly hurt follower doesn't walk off to rejoin either.
- **Followers never flee on their own** (John). Where the solo bot would flee, a follower telepaths
  **`@flee`** (at most every 5 s) and keeps fighting; the leader treats a fresh `@flee` as top priority and
  takes the party out. Our word, not MegaMud's (MegaMud's `@panic!` goes the other way, leader to followers).
- **Clerics bash** (John): member setting `"melee": true` / `--melee` drops attack spells from the choice.
  Casting ends autocombat (out of the exp split) and spends heal mana.
- Party rest worked live: `Party rest: Isolde @wait, Isolde at 52%`.
- `@flee` worked live: Isolde asked at 36/40, Brynja broke off (`Party: fleeing for Isolde`).
- **A follower healer must heal itself:** Isolde sat at 4/40 HP with 10 mana resting in the Arena; the
  follower loop only sent `rest`. It now casts its heal first (`self_heal`), then rests.
- Each party rest gets its own `wait_no_longer_than_s`; a stale start time ended every later rest at once.
- **A party hunts at its lowest member's level** (John). Followers report `@level N` (on joining, on
  training, and when asked); the leader picks the plan stage by the lowest level (`plans/party-duo.json`)
  and ends the patrol when that level changes.
- **Newhaven refuses level 4+** [C data]: the only way back from Silvermere (Docks 1:33 north) is a Level
  exit, open to levels 1-3, so all level 1-3 hunting is out of reach of a level-4 leader. When the
  lowest member's stage is somewhere the leader can't go, the leader sends **`@solo N`** (N = the first
  level of its own stage) and plans without that member; the follower stops following (FOLLOW alone),
  plays its own class plan solo, still answers `@where`/`@level`, and at level N reports in; the leader
  then fetches it (John: "Isolde catches up solo"). Class exp rates matter: a Warrior (+0%) outruns a
  Cleric (+120%), so a mixed party will split and regroup.

## 8. Build order (each stage ships with golden tests and one live run on the server)

| Stage | Delivers | Done when |
|---|---|---|
| G0 | Patterns (§7). `party.py` `Remotes` (parse, allow-list, rate limit, receipts, retry). `set maxpages 30` at start-up. | Golden tests from real transcripts. Two bots exchange 100 telepaths with no loss. |
| G1 | Formation and follow: invite, follow, `par` roster, `@join`/`@invite`, rejoin after a drop. | Brynja leads Isolde around Newhaven for 30 min, follow intact or restored. |
| G2 | Leader movement holds: `@wait`/`@ok`, `wait_below_hp`, `par` polling, bounded waits. | No step taken while a member is hurt or resting. No deadlocks in a 2-hour run. |
| G3 | Combat: assist on `moves to attack`, healer heals and re-engages, party flee, follower critical flee and `@comeback`. | Both characters credited on ≥ 90% of kills. Zero deaths at Arena/Graveyard level. |
| G4 | Special-exit relay and re-invite, intersection routing, catch-up trip. | Party crosses `borrow skiff`, a door and a key exit intact. |
| G5 | `@req`/`@ack`/`@nak`/`@done`: train, shop, rest, town, area; `share`. Party plans (`plans/party-*.json`). | Party levels 1→10 together with no human help, except CP and quests. |
| G6 | Third member, ranks, `@party` allow-list, `@panic!`, a human or MegaMud as a member. | 3-party session. Bot follows a human leader who telepaths `@party` commands. |

## 9. Risks and open questions
- **Channels:** `BBS.Channels` is 4 in `test/config/appsettings.json`. Two bots plus John's telnet use 3.
  Raise it before G6.
- **Spoofing:** anyone can telepath `@party quit`. Hence the friends-only acceptance and the command
  allow-list. `@do` is never supported.
- **Timing:** the party moves in the leader's fast cycle (1 s). A follower's bot must not send its own
  commands mid-move (they cancel or confuse). Followers stay quiet between `@party` and the arrival line.
- **Split exp vs speed:** a duo kills slower per head early on (Arena: 1 monster per player). Measure
  exp/hour solo vs party before committing to party-only progression.
- **Decided (John, 2026-10-13):**
  - A follower obeys telepaths from a **human** too, as long as that character is the one it is configured
    to follow (`--leader NAME`). The same friends-only and allow-list rules apply.
  - When the leader is offline, the follower **waits** (in a safe, no-spawn room) instead of playing solo.
  - The 4-channel BBS limit is understood; raise `BBS.Channels` before a third character.

[kyau]: https://kyau.net/wiki/MajorMUD:Remote_Commands
[comp]: https://www.megamud.net/comp.html
[me6]: https://github.com/jasonluck/mudengine/pull/6
[me19]: https://github.com/fatavatar/mudengine/pull/19
[me39]: https://github.com/fatavatar/mudengine/pull/39
