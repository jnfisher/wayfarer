# Client architecture: recommendations

## Core idea: data-driven, not trigger-driven

MegaMud worked mostly from what scrolled past: text triggers, recorded paths, and room
checksums learned by walking. A modern client can do much better because the game's own data
can be read directly. Room records give names, exits, exit types, light and spawn rules.
Monster records give HP, exp, hits, armour and alignment. Shop and item records give prices,
stock, stats and class restrictions. Message records give the typed phrases for special exits.

With that, a client can:

- plan any route without recording paths;
- know which rooms spawn what, how often they respawn, and how dangerous they are;
- price every upgrade and know where to buy it;
- know whether a monster is fightable before attacking it.

[C] All of this is proven in `test/play/mmud_data.py`.

If the client can't read the server's data files (a remote BBS), it needs an equivalent offline
data set for that world version, like MegaMud/MME's database. The parsers and planners stay the
same.

## Layers (as prototyped)

1. **Session/transport.** Telnet with option refusal, CP437 decode, raw capture, and a live
   mirror. See `connection.md`. Prototype: `bridge.py`, a daemon holding the socket so many
   short-lived controllers can drive one session.
2. **Screen parser.** Turns text into events: prompt and HP, room seen (name, visible exits,
   items, others here), combat lines, loot, errors, darkness, level and exp. See
   `screen-parsing.md`.
3. **World model.** Static data plus a live overlay: position belief, room cleared times, shop
   stock, known immune or unfightable names.
4. **Planner/policies.** Route, patrol, target choice, rest and flee, shop, train, choose a
   ground. Each policy is a small pure function of state plus data, so it can be tested
   offline.
5. **Executor.** Issues commands, waits for confirming events, and handles interrupts (being
   attacked while walking or resting).
6. **UI.** Live game view, narration of the bot's intent, and an override and pause. Watching
   the bot through a read-only tmux mirror with injected notes proved very useful.

## Lessons that shaped this

- **Position must be a belief, not a guess.** Many rooms share a name (Silvermere has three
  identical "River Street" rooms and long runs of "Slum Street"). Track the set of rooms
  matching the observed name and visible exits, and advance it through each move. [C]
- **Every action needs a confirming observation.** A purchase isn't done until "You just
  bought"; a move isn't done until a room title or exit list arrives. [C]
- **Choose on measured outcomes.** On paper the slums looked better than the arena. Measured,
  they gave 920 exp/hr against the arena's 2,830. Keep per-ground stats (`logs/grounds.json`)
  and let measurements override estimates. [C]
- **Make restarts cheap.** The controller crashed or was restarted around 15 times in one
  session. The transport daemon survived, logs were continuous, and the viewer kept going
  through a `live.ans` symlink. [C]

## Possible improvements over the prototype

- **Event stream instead of regex on text chunks.** Write a proper incremental parser keyed on
  the prompt boundary (`[HP=..]:`). Today's prototype reads "everything since the last command"
  and runs regexes on it.
- **Generate parsers from the source's format strings.** `prf("...%s...")` calls in `SRC/*.C`
  and the `.MSG` files list every message the module can produce. Extract them into a pattern
  table instead of discovering patterns by playing. See `screen-parsing.md`.
- **Use a simulator for policy testing.** The data and the source's formulas (combat,
  regeneration, spawning) are known, so an offline simulator could tune policies without real
  play time.
- **Replay tests.** Captured `raw.bin` sessions make deterministic parser tests.
