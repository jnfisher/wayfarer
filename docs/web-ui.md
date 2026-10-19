# The web UI

`wayfarer gateway` serves the web UI at its address (default `http://127.0.0.1:7373/`) once it
is built (`npm --prefix ui ci && npm --prefix ui run build`, which writes `ui/dist/`).

![Four tiles, replaying recorded sessions, with the brain attached](images/web-ui-four-tiles.png)

## Tiles

One tile per character in `wayfarer.toml`, laid out by count: 1; 1x2; 2x2; 2x3 or 3x2 (two,
five or six tiles go side by side or stacked, whichever makes the terminals larger on your
screen). Each tile is framed in the font's double lines, with a header on the top line
(below).

## The header

```
╔═ 1 Waydeva  L3 Warrior ═ HP ███████░░░ 28/40  MA ░░░░░░░░░░ 0/0 ═ 1,230 (45%) ═ Town Square ═ 127.0.0.1:2323 ═ realm ═╗
```

From the left: the tile's number (for `Alt+n`), the character's name, then what the brain
knows of them: level and class, HP and mana as bars with their numbers, exp with how far
through the level it is, and the room's title. On the right, the server and the session's
state badge: `connecting`, `connected`, `logon`, `menu`, `realm`, `lost` or `closed`.

- **Bars** are ten blocks: green above two thirds, yellow above one third, red below. HP and
  mana maxima come from the status screen, which the brain asks for when it re-syncs (the
  game's prompt has none), so until then a bar is empty and its maximum shows `--`.
- **Unknown values** show as `--`. Values update live as the brain reads the game; on leaving
  the realm (`x` to the menu) the last values stay and the badge changes.
- **Narrow tiles** drop items from the right: the server first, then the room, exp, mana, HP
  and the level, so the name and the badge always show. Room titles longer than 30
  characters end in `…`.
- **No brain**: without a brain attached the header is just the number, name, server and
  badge, with a dim `no brain` after the badge. The brain is `wayfarer brain` (`docs/brain.md`).
- A page that opens or reconnects while a brain is attached asks it for every character's
  state, so the headers fill at once.

Colours: the frame dark grey (light cyan on the focused tile), the name yellow, level, class
and labels light grey, numbers and the room white, exp light cyan, the server cyan.

The frame is drawn with the VGA font's own glyphs (`╔ ═ ╗` along the top, a column of `║` down
each side, `╚ ═ ╝` along the bottom) at the header's fixed 16 px, so the corners meet the
sides at every tile size and browser zoom. In the font's 8x16 cell `║` is two 2 px strokes in
columns 2-3 and 5-6, and `═` two 1 px strokes in rows 5 and 7. `node ui/scripts/frame-check.mjs
URL OUT_DIR` checks them against a running gateway at 90%, 100% and 125%.

The terminal is always 80 columns wide, in the IBM VGA 8x16 font and the 16 DOS colours. The
font is sized so the 80 columns fill the tile ("fit to tile"), and the terminal has as many
rows as fit below that. It keeps 10,000 lines of scrollback (mouse wheel), and when the page
opens it already shows the last 2,000 pieces of output from the gateway's log.

## The bot

With a brain that has a world pack, every tile's header carries that character's bot
controls, just left of the server:

```
╔═ 1 Waydeva  L1 Warrior ═ HP ██████░░░░ 22/35 ═ 1,230 (45%) ═ Newhaven, Arena ═ [║][■] ═ 127.0.0.1:2323 ═ realm ═╗
 Fight: large giant rat
```

| State | Buttons | Line under the header |
|---|---|---|
| Stopped (every character to begin with) | `[►]` start | none |
| Running | `[║]` pause, `[■]` stop | the narration, light cyan: "Fight: large giant rat", "Recover: resting to 95% in Newhaven, Narrow Road", "Waiting here" |
| Paused | `[►]` resume, `[■]` stop | yellow: "Paused: you took over" |

**Start** plays the character where it stands: it fights what comes, keeps auto-combat on,
rests when HP is low (in the nearest quiet room if this one spawns, then walks back), and stops
itself below 30% HP with a warning, since fleeing is not built yet ([brain.md](brain.md) "Fight
here"). **Any line you type** in the tile pauses it, after the command in flight ("You took
over Waydeva" in the dev panel); so does clicking a room on the map. `F9` or `[►]` resumes: the
brain re-syncs first. `F10` starts or stops the focused character's bot. Hovering the line
shows why the bot stopped or paused. The glyphs are the VGA font's own: code page 437 has `►`
and `■` but no pause sign, so pause is `[║]`.

## Keys

| Key | Does |
|---|---|
| `Enter` in a tile's input line | Sends the line (an empty line sends just Enter) |
| `Up`, `Down` in the input line | That tile's command history |
| `keys` button | Sends what you type in the terminal straight to the game, key by key; click it again to go back to the input line |
| `Alt+1` to `Alt+6` | Focus a tile (on a Mac, `Option`) |
| `Ctrl +`, `Ctrl -` | Make the focused tile's font larger or smaller |
| `Ctrl 0` | Back to "fit to tile" |
| Drag the line between two tiles | Gives one tile more room and the other less; remembered per browser |
| Double-click a line between tiles | Back to equal shares for that direction (columns or rows) |
| `Alt+0` | Back to equal shares for columns and rows (on a Mac, `Option`) |
| `?` on an empty input line, or the `?` button | Open or close this help |
| `Esc`, `?` again, or a click outside | Close this help |
| `Alt+M`, or the `map` button | Open the drawer on the map of the focused character's surroundings, or close it (on a Mac, `Option`) |
| Click a room on the map | Walk the character there; `stop` under the map ends the walk |
| Mouse wheel, drag on the map | Zoom (three sizes), pan; `centre` goes back to the character |
| `F10`, or `[►]` and `[■]` in the tile's header | Start the focused character's bot where it stands (fight what comes, rest when low), or stop it |
| `F9`, or `[║]` and `[►]` in the tile's header | Pause or resume the focused character's bot; a line you type pauses it too |
| `Alt+D`, or the `dev` button | Open the drawer on the dev panel: the focused character's events, unclassified lines and the brain; or close it (on a Mac, `Option`) |

The `?` overlay shows these keys, what the state badges mean and where the data files live. It
is wayfarer's help only, never the game's; a `?` typed after other text on the line is sent to
the game as text. The table above and the overlay come from one table in `ui/src/lib/keys.ts`,
and a test fails when they differ.

Clicking a tile focuses it. The **keys** button sends what you type in the terminal itself
straight to the game, key by key, instead of a line at a time: for the character editor and
`Hit any key` prompts. Click it again to go back to the input line.

## Resizing tiles

Between two columns or two rows of tiles is a line about 6 px wide. Drag it to give one tile
more room; only the two tiles beside it change, and a drag stops when a tile would be narrower
than an 80-column terminal at the smallest font (6 px) or shorter than a few rows. Touch works
too. Double-click a line to put that direction back to equal shares; `Alt+0` does both.
Tiles refit their font as they change, still 80 columns wide.

The sizes are proportions, so resizing the window keeps them. They are kept per grid shape
(`1x2`, `2x2`, `2x3`, and so on, rows by columns) in the browser's `localStorage` under
`wayfarer.layout`, as `{"v": 1, "shapes": {"2x2": {"cols": [1.4, 0.6], "rows": [1, 1]}}}`;
adding or removing a character changes the shape, so that shape's own sizes apply. The layout
is per browser until settings arrive (`docs/plan/04-settings.md`); it is not written to
`wayfarer.toml`. With storage unavailable the layout still works and is forgotten on reload.

## The drawer

The map and the dev panel share one drawer on the right, a third of the window at most; the
tiles refit beside it. Its title has the two tabs, `map` and `dev`, the focused character's
name and a close button. `Alt+M` (on a Mac, `Option+M`) or the `map` button opens it on the
map, and again closes it; `Alt+D` or the `dev` button does the same for the dev panel, and
either key switches tabs when the drawer is open on the other. It follows the focused tile.
It is closed by default; whether it is open, its tab and the dev panel's own tab are kept in
the browser's `localStorage` under `wayfarer.dev`.

## The map

The rooms around the focused character, drawn the way the old mapper programs drew them:
each room a box, exits as lines between boxes, one level at a time. The brain lays the rooms
out and sends them (`map_view`, [protocol.md](protocol.md)); the panel draws what it is sent
and works nothing out itself.

![The map: the believed room lit, a walk under way with its route and steps, beside one tile](images/web-ui-map.png)

- **Rooms**: a box of 3x2 character cells per room. The room the brain believes the
  character is in is bright white with a double frame. While the brain is not sure (several
  rooms fit what was seen) the other candidates are framed in yellow, and the tools line says
  how many there are; they collapse to one as the character moves. Rooms the character has
  certainly been in since the brain started are framed light grey, the rest dark grey. Rooms
  where monsters spawn are filled dim red. A letter marks the kind of room: `S` shop, `T`
  trainer, `B` bank, `A` arena, `L` lair. `▲` and `▼` in a box mean an exit up or down.
- **Exits**: a solid line for a plain exit, a gap with a brown `+` for a door (gates and key
  doors too), a dotted line for a hidden or text exit, and an arrow head at the far end of a
  one-way exit. A room whose place on the grid was already taken by another room is drawn
  there as a dashed frame inside the other one, without exits; hovering the cell names the
  room underneath first.
- **Levels**: one level at a time, the believed room's first. The `level` buttons show the
  floors above and below that are in the view.
- **Moving around**: the map centres on the believed room and follows it as the character
  moves. Drag to pan; once panned (or on another level) it stays where you put it, and a
  `centre` button brings it back. The mouse wheel zooms between three sizes (the VGA font at
  8, 12 and 16 px; 16 to start), about the pointer.
- **Hover** a room for its name, `map:room` and kind, whether monsters spawn there, its exits
  in view (with their kind) and whether the character was there.
- **Walking**: click a room and the character walks there, confirming each step
  ([brain.md](brain.md), "Walking"). The route shows on the map as a green path with each
  room's step number, and in the bar under the map: the destination, the step in progress and
  the steps left, and the commands the walker will still type (`n, n, e, borrow skiff`), from
  the brain. A door shows on the map; the walker types `open` only when it finds the door
  closed, so that is not listed ahead. **stop** stops the walk
  after the move in flight. Clicking another room while walking replaces the walk. Two
  characters can walk at once; the panel shows the focused one.
- **Without a brain** the panel says so and draws nothing; with a brain but no known position
  (no world pack, or the room did not match) it says `position unknown: re-sync or look`.

The panel asks the brain for the view when it opens on a character, and again after a
reconnect or when the brain re-attaches; after that the brain sends a new one whenever the
believed room or the route changes. The view holds the rooms within 15 steps.

## The dev panel

For development: what the brain sees. It is the drawer's `dev` tab (above).

![The dev panel on the Events tab, a room_title picked, beside two tiles replaying recorded sessions](images/web-ui-dev-panel.png)

- **Events**: the brain's parsed events for the character, newest at the bottom, each with
  the output `seq` it came from, its name, category and fields (`hp=28 maxhp=40`). The list
  follows new events until you scroll up. The filter box matches the name or the line's text;
  **pause** holds the list while events keep arriving (**resume** shows them). Click an event
  to see its whole line and line number.
- **Unclassified**: the lines the parser did not recognise (`unknown` events), grouped by
  text with their counts, most frequent first, and the unclassified rate: the snapshot's
  unclassified lines over lines parsed since the brain attached, as a percentage to two
  decimals, red above 0.2% (the soak metric, `docs/plan/03-engine.md`). **copy as KB notes**
  puts the list on the clipboard as a Markdown list (``- `line` (3x)``) for a PR's "KB notes".
- **Brain**: the brain's build, start time and world pack, whether it is attached, and a log:
  the brain attaching and leaving, each new start, `brain_lagged` (after which the page asks
  for every snapshot again), and the brain's notices and warnings with their character and
  time. **Re-sync** asks the brain to re-sync the focused character (`st`, `exp`, `i`, look);
  it is off with no brain attached.

The panel keeps the last 2,000 events and 500 notices per character, and up to 1,000
distinct unclassified lines, from when the page opened; a reload starts it empty.

## Connection

If the connection to the gateway drops (the gateway restarts, the network goes), every tile
says so and the page reconnects by itself, trying again after half a second and then less
often, up to every 10 seconds. Once back it fetches everything it missed, so nothing is lost or
shown twice. Reloading the page never logs a character off: the gateway holds the sessions.

If the gateway is reached over a non-loopback address it needs its token
(`docs/gateway.md`); the page asks for it once and remembers it in the browser.
