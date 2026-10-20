# 04. Settings

MegaMud had everything: 16 settings tabs with about 277 settings, plus per-record options in
10 game-data tabs, with generic defaults (`kb/megamud-settings.md`). We keep the reach and fix
the start: the settings that matter most sit on one page with sensible defaults for the
character's class, and everything else is one level down.

**Lessons from MegaMud's settings** (`kb/megamud-settings.md` section 9):

- **Group by what the player wants, not by mechanism.** MegaMud split healing over four places
  (thresholds, which spell, party heals, a checklist) and fleeing over three. Here a group holds
  everything about one goal: Healing has the thresholds and the spells together.
- **Plain words.** "Flee below", not "Run if below"; "Log off below", not "Hang if below";
  "Area spell", not "Nuke".
- **Every switch has a sentence of help.** No bare checklists.
- **Real validation.** Ranges on every number, spell pickers that list only known spells, and
  the order of the HP thresholds enforced (flee < heal in combat < rest below < rest until).
- **Safe, unsurprising defaults.** MegaMud hung up the moment another player attacked, and
  logged off below 10% HP; both off here.

## 1. Declared once

Every setting is declared once in `wayfarer-settings`:

| Field | Example |
|---|---|
| key | `health.flee_below_hp` |
| label and help | "Flee below", "Leave the fight and run to a room with no spawns when HP falls below this." |
| type, unit, range | percent, 0–90 |
| default | 30 (melee classes), 50 (casters) |
| applies to | all classes, or a list; or a capability ("can backstab", "has heal spells") |
| tier | essential, standard or advanced |
| group | Health, Combat, Spells, Upkeep, Loot, Movement, Monsters and players, Safety, Alerts, Party |
| MegaMud equivalent | "HP Run If Below" (for the import and for players who know MegaMud) |

From that one declaration come the settings forms (web and terminal UI), validation, storage,
the `@`-style remote toggles, the docs page and the MegaMud import mapping. A test fails if a
setting lacks help text or a default.

## 2. Layers and presets

Values resolve through layers, most specific last (MegaMud layered its game data the same
way: installed, all characters, this BBS, this character):

```
built-in default  <  class preset  <  my defaults (all characters)  <  this server  <  this character
```

- **Class presets** come from the class guides (`kb/classes/`): a Witchunter bashes and never
  casts; a Cleric heals early and flees at 50%; a Thief sneaks and backstabs.
- **Style presets** shift the risk settings together: *Careful*, *Balanced* (default),
  *Bold*. Bold means tougher fights and a lower flee line, still relying on fleeing reliably,
  which is John's risk rule.
- The form shows where each value comes from ("Witchunter preset", "you") with a reset
  button, so a player can always get back to the defaults.
- Changes apply at once, mid-fight included, and are narrated on the character's card
  ("Flee below 30% → 35%").
- Stored as one readable TOML file per character (`characters/<server>/<name>.toml`) holding
  only the values that differ from the layers below it. Profiles export and import without
  credentials.

## 3. The essentials page

What a new player sees from the ⚙ button on a character's tile (`05-ui.md`). Everything has a
default, so the bot can start without anyone touching this page.

**Health**, drawn as one HP bar with draggable markers rather than separate boxes, so the
order is obvious:

```
HP  0 ──[Flee 30]────[Heal in combat 40]────[Rest below 80]────[Rest until 95]── 100
```

| Setting | Default | MegaMud | Notes |
|---|---|---|---|
| Rest below HP % | 80 | 90 | Between fights, rest when below this; never in a room that spawns. MegaMud's 90 is cautious: resting already dominates time in hard areas (`kb/recovery.md`) |
| Rest until HP % | 95 | 99 | A little under 100, so it doesn't wait a whole regen tick for a few HP |
| Flee below HP % | 30 melee, 50 casters | 30 | The percentage alone; the "twice the hardest hit this fight" addition was dropped (John, 2026-10-19) |
| Mana: rest below / until % | 30 / 95 | 20 / 100 | Casters only |

**Combat**

| Setting | Default | Notes |
|---|---|---|
| Attack with | the class's best measured: bash for two-handed melee, attack otherwise | attack, bash, smash; jumpkick for Mystics. Only what the class can do is offered |
| Sneak and backstab | on for classes that can backstab | Opens each fight with a backstab when sneaking worked |
| Fight neutral monsters too | off | Evil-only by default (MegaMud did the same); some neutrals give evil points |
| Most monsters to take on at once | 2 (Bold: 3) | Above this, skip the room. MegaMud: 2 |

**Spells** (only for characters with spells; the pickers list spells the character knows)

| Setting | Default | Notes |
|---|---|---|
| Attack spell | class preset (none for pure melee) | Cast while mana ≥ the next value |
| …while mana is above % | 40 | Keeps a reserve for heals (MegaMud: 20) |
| Small heal | the class's cheapest heal | Cast below the next value |
| …below HP % | 70 | Out of combat; in combat only below "Heal in combat" |
| Big heal | the class's strongest heal | For deep drops |
| …below HP % | 45 | |
| Heal in combat below HP % | 40 | Kept low: breaking off to heal costs rounds (MegaMud: 40) |

MegaMud had one solo heal spell (with "heal while resting below 60%" and "in combat below
40%"); the small and big pair only existed for party heals. Two heals fit how players actually
play casters.

**Bot** (the player decides; the bot carries out)

| Setting | Default | Notes |
|---|---|---|
| Hunting area | none until chosen | Set from "Where next?" or the map |
| When outlevelled | tell me | Or "move on to the top recommendation" |
| Train when a level is ready | on | Walks to the trainer and trains; never trains stats. MegaMud: on. Off to train by hand |
| Bank coins above | off | A wealth amount; returns to the same place in the loop. MegaMud: on, at 1,000 coins |
| Sell loot when heavy | off | Sells items marked for selling at their shop |
| Keep a light source | on in dark areas | Lights and puts out torches or a light spell as rooms need |

## 4. The full settings (standard and advanced)

One page per group, standard settings first, advanced folded away. The starting catalogue,
from MegaMud's dialogs (`kb/megamud.md`), the prototype and the KB:

| Group | Settings |
|---|---|
| Health | heal period (minimum seconds between heals); regen spells before resting; cure poison, blindness and disease spells (or wait it out); ignore poison, blindness, confusion; pre-rest and post-rest commands |
| Combat | pre-attack spell (once per monster); area spell when at least N monsters; casts per monster; stop casting once a round's damage exceeds X; run if the backstab fails; polite attacks (not when other players are here); most total monster exp to take on; break combat before running; run distance |
| Monsters and players | per name: friend, avoid, enemy, flee from, never backstab, attack priority, spell overrides; matching by substring with longest names first |
| Upkeep | trainer choice; bank and keep-on-hand amount; items to sell, stash or never pick up; minimum stock of consumables (torches) |
| Loot | per currency: collect, ignore or drop (copper and silver ignored by default); items to collect; don't become heavy or medium; collect only after combat |
| Movement | sneak (and don't move unless sneaking); hide when idle; pick locks or bash doors, attempts; disarm traps, attempts; search for hidden exits; avoid rooms; run backwards along the loop when fleeing |
| Safety | when a player attacks: flee (default), log off, fight back, or nothing (MegaMud defaulted to hanging up at once); log off below HP % (off; MegaMud 10%); log off if exp/hr falls below X; reconnect after cleanup; schedule (log on, log off, run a loop) |
| Assists while stopped | MegaMud's manual-mode helpers, each opt-in: fight back when attacked, rest and heal, keep a light, pick up coins, sneak when moving |
| Alerts | sounds and desktop notifications per event (`05-ui.md`) |
| Party | (M7) heal party members below %, ask for heals, wait for members below %, assist the leader's target, follow and re-invite |

## 5. What we leave out

- **Gear buying and auto-equipping upgrades.** The data could price every upgrade; the player
  decides what to buy. (A "best gear I can use" report is a possible later query, like
  "Where next?".)
- **Training stats.** Never, by any setting.
- **Trigger scripting.** MegaMud's message table with responses was its scripting language.
  Our events and modes cover the cases it was used for; a small, safe rules table can come
  later if the fixed settings fall short.
- **Modem and dial-up settings.**
