# 10. Party play

From a conversation with John on 2026-10-22. It turns milestone M7 ("Party play",
`09-milestones.md`) and the party seam in `03-engine.md` §8 into a design. The game rules it
rests on are in `kb/grouping.md` (§2a quirks, §9 messages); MegaMud's party behaviour and the
lessons from mudengine are in `kb/plan-grouping.md` §1. Where this file and `plan-grouping.md`
disagree, this file wins: `plan-grouping.md` was written for the Python prototype's one
process per character.

Marks: **(John)** decided by John in the conversation; **(call)** a call made while writing
this, open to change; everything unmarked is left to the builder.

## 1. Why

John runs several characters on one server (today Penny, Lauren, Faye and Manuel) and wants
them to grind together as MegaMud parties did: one leader plays, the rest follow, fight beside
it and keep it alive. The point of a party is that **John steers one character** and the
others come along. He walks the leader by hand, sends it somewhere with Go to, or starts it on
a loop; the followers need no attention unless something goes wrong. **(John)**

**When.** Now, ahead of M5 and M6, with no separate soak step: John tests continuously as it
lands. **(John, 2026-10-22)** `09-milestones.md` is updated to match.

Success: John forms Penny's party, starts Penny's loop, and the four grind for an hour with
rests, heals, a flee or two and a separation, without his touching a follower.

## 2. Goals, non-goals, anti-goals

**Goals (John)**
- Parties defined by a leader and followers, several at once (Penny with Lauren, Manuel with
  Faye), in the web UI.
- The leader invites its followers and they join, in the game, by themselves.
- Followers fight in auto mode but never walk, except to get back to the leader.
- The party stops for anyone's rest and anyone's flee; healers heal other members.
- The party's real state is tracked with `par`, because the game breaks parties silently.

**Non-goals (for this design)**
- Parties with other people's characters or a real MegaMud (the `@` telepath protocol of
  `plan-grouping.md` §5). All members are wayfarer's own characters in one brain. **(call)**
- A mana reserve for healers. **(John: "not for now")**
- Choosing where the party hunts: the leader's loop or Go to is John's choice, as for a solo
  character (`01-goals.md` goal 2).
- Ranks (`frontrank`/`backrank`), aiding or dragging a downed member, loot assignment and
  `share`, sneak scouts. **(call: later slices; the game defaults to midrank)**
- A **Gather** button that walks every follower to the leader. **(call: later)**

**Anti-goals: it fails, even working, if**
- A follower wanders off on its own loop or path while in a party.
- A follower sits out of the party believing it is in, because a drop went unnoticed (the
  mudengine lesson, `plan-grouping.md` §1 lesson 5).
- The party waits forever on one member with no word to John.
- A healer casts the druid's *acid rain* (a room damage spell) taking it for *healing rain*.

## 3. Parties

- A party is a **leader** and one to five **followers** (the game's limit is six members,
  `kb/grouping.md`). **(John: leader and followers; size: game fact)**
- A character is in at most one party. Adding it to a second moves it. **(call)**
- Parties are saved and survive a restart of the brain, the gateway and the browser. They
  belong to the brain, which runs them; where they are stored (the brain's store or a settings
  file) is the builder's choice. **(call: saved)**
- Several parties run at once, independently.
- Party definitions use the characters' ids from `wayfarer.toml`.
- **Who counts.** The leader waits for (rest, separation, forming) only the members that are
  in the realm with their bot running. A member logged off or paused is skipped until it is
  back; a paused follower still marches with the leader by the game's follow, it just isn't
  waited for or walked back. **(call)**
- **A leader walked by hand.** The party works whatever moves the leader: its bot, Go to, or
  John typing. The leader's holds (§5) stop only the bot's own moves; if John walks the leader
  on while a member is separated, that is his choice, and the separated follower keeps walking
  to wherever the leader now is. **(call)**

## 4. Forming up and staying formed

**Forming.** John brings the characters into one room himself (walking or Go to). When a
leader and one of its followers who is not in the leader's party in the game stand in the same
room, the leader `invite`s it and the follower `follow`s the leader; the follower confirms
with the join line. No "form up" button. **(John: automatic; call: no button)**

**Joining stops what the follower was doing**: its loop, Go to or errand ends. **(John)**

**Tracking.** The game's party state is the truth, not the definition. `par` is read
regularly and after anything that can split the party: every leader move that is not followed
by the follower's ` -- Following your Party leader <dir> --` line, a text exit (`go portal`),
a flee, a death, a log-off or a re-sync. A missing follow line counts as a drop
(`kb/grouping.md` §2a: an exit refusal drops a follower with no message). Exact strings are in
`kb/grouping.md` §9. **(John: `par` is important; the triggers are the builder's, these are
the known ones)**

**Text exits.** Followers do cross text exits, but the link can break (mudengine lesson 1).
After a text exit the leader checks with `par` and re-invites anyone who was with it.

## 5. What each member does while in a party

**Leader.** Plays as a solo character does: its loop, Go to, errands, fights, all as now,
with three holds added: it does not move while the party is resting, while a member is
separated, or while a member is not yet joined after forming. **(John)**

**Followers.** Stay in auto mode and fight. They never start a walk, loop, Go to or errand of
their own while in a party; their loop and Go to controls are disabled. Their bot play/pause
still works. **(John)**

**Assist.** Followers attack the leader's target, since the game shares experience only among
members fighting the monster that dies (`kb/grouping.md` §9 "Target calling"). A follower
attacked by something else fights it. **(call)**

## 6. Separation

When `par` shows a follower has dropped out: **(John)**

1. The leader stops where it is and waits.
2. The separated follower walks to the leader's room by itself (the one walk a follower
   makes). The brain knows both positions, so no message passes between them. It fights what
   attacks it on the way, in auto mode.
3. In the same room, the leader re-invites, the follower re-joins, `par` confirms, and the
   leader carries on.

- A follower that can't reach the leader (a key it lacks, a level or class exit, a failed
  climb) stops and says why on its tile and in a notice. **(call)**
- The leader's wait has a limit (a setting; MegaMud's "Wait No Longer Than"). At the limit the
  leader stops its loop and tells John; it never leaves a follower behind. **(call)**
- Several separated followers are waited for together.

## 7. Resting

- When any member falls below its own *Rest below* line (HP, or mana for a caster:
  `health.rest_below_hp`, `health.rest_mana_below`), the whole party stops and rests in place.
  The leader resting by its own lines does the same. **(John)**
- The party moves on when every member is at or above its own *Rest until* lines.
- During the rest, healers heal other members (§8); that is what keeps party rests short.
  **(John)**
- A monster arriving mid-rest is fought by everyone; then the rest goes on.
- A party rest has a time limit like the separation wait; at the limit the leader stops and
  tells John. **(call)**
- The usual quiet-room rule (`never rest in a room that spawns monsters`, a setting) applies to
  where the leader stops, as it does solo.

## 8. Healing other members

**The mechanic is self-healing with a second line.** A character with a heal spell already
heals itself in a fight below `spells.heal_in_combat_below_hp` and while resting below
`spells.heal_resting_below_hp`. It gets the same pair for other party members, e.g.
*Heal a party member below* (in a fight) and *Heal a party member while resting below*,
watching every member including itself. A "healer" is only a character with a heal spell;
healers fight exactly as they do now. **(John)**

- Who first: the member furthest below their line. **(call)**
- Party members' HP comes from their own prompts (they are our characters), not from `par`'s
  rounded percentages. **(call)**
- After a heal in a fight the healer re-engages its target, as self-heals do now.
- Witchunters resist friendly heals (`kb/grouping.md`, "Witchunters in groups"); healing one
  wastes casts. The builder decides whether to skip them or just accept the waste.

**Party heals.** One on/off setting, *Use party heals*, on by default for a character that
knows one. **(John: on/off only)**

- When several members are hurt, the healer casts its party heal; when one is, the single
  heal. A decent rule (**call**): the party heal when two or more members are below their
  heal line, or when its average heal summed over every hurt member beats the single heal on
  the worst one. A member below their flee line still gets the single heal first.
- Party heals are found in the pack's spell data by what they do (a heal whose target is the
  party), **never by the command word**: `rain` is the Priest's healing rain and also the
  Druid's acid rain. **(call, from the KB)**
- Who has them (`kb/classes/`): Priests (healing rains from L10, divine mercy L30), Paladins
  (healing rain L10), Druids (a heal over time, L32); Clerics and Missionaries are listed as
  party healers but not checked. Picking by spell data covers all of them.

## 9. Fleeing

- When any member reaches its flee line, the **whole party flees**, led by the leader: the
  leader picks the exit and the others follow it out by the game's own follow. **(John)**
- The fleeing is today's Flee (`docs/brain.md` "Flee"): a confirmed exit toward a room with no
  spawns, running on while followed, up to the leader's *Flee rooms*. **(call)**
- A follower whose flee triggered does not run on its own; that would break the party. **(call)**
- The leader reaching its own flee line also flees the party. **(call)**
- After the flee, `par`; anyone left behind is separated (§6). Then the usual party rest
  before the leader goes back to its loop. **(call)**

## 10. The UI

As described to John, who approved it. **(John)**

**A Parties panel**, beside Map, Settings and Stats:
- One box per party: the leader from a dropdown, the followers ticked from the characters.
  **+ New party** adds one; a party can be deleted.
- Each member shows a live status: *joined*, *not joined yet*, *separated, walking to Penny
  (4 rooms)*, *resting 62%*, *can't reach Penny* with the reason.

**A party chip on each tile's header**, next to the bot button:
- Leader: **★ Leading: Lauren, Faye, Manuel**. Follower: **Following Penny**, or
  **Separated** in a warning colour.
- Click: a follower gets **Leave party** (sends `leave`, takes it out of the party so the
  leader stops waiting for it and won't re-invite it, and gives it back its own controls). A
  character outside a party gets **Join Penny's party** (it walks to the leader and joins).
- Leave party is how a follower goes off on its own. There is no separate "follow behaviour
  off" switch: a party member always tries to rejoin, a non-member never does.

**Follower tiles**: loop and Go to greyed with "Following Penny"; play/pause works.

**Settings**: the existing Party group (`04-settings.md`): the two heal-a-member lines, *Use
party heals*, and the two time limits (waiting for a separated member, a party rest).

## 11. Constraints

- The rules in `CLAUDE.md`: the bot never trains stats; the player picks where to hunt; log
  off cleanly (a party's members log off one by one, never dropped).
- All coordination happens inside the one brain (`03-engine.md` §8): the coordinator sees
  every member's real state and sends constraints and requests to their mode stacks. No
  telepaths between our own characters.
- The protocol (`docs/protocol.md`) gains whatever the panel and chips need; the UI checks
  (`npm run check`, `npm test`) and the Rust checks stay green; tests are in-process
  (scripted transcripts and the replay fixtures), with the live server only through
  `scripts/live-check.sh`.
- Any party fact newly confirmed against silvermere goes to the KB with its tag, per the
  conventions.

## 12. Left to the builder

- How the work splits into cards and in what order.
- The coordinator's shape and the new modes or holds in each mode stack.
- The new typed events for the party lines and `par`, and `par`'s timing.
- Exact setting keys, defaults and ranges (the time limits, the party heal lines; 60% is a
  reasonable start).
- Where party definitions are stored, and the protocol messages.
