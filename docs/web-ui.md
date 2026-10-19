# The web UI

`wayfarer gateway` serves the web UI at its address (default `http://127.0.0.1:7373/`) once it
is built (`npm --prefix ui ci && npm --prefix ui run build`, which writes `ui/dist/`).

![Four tiles, replaying recorded sessions](images/web-ui-four-tiles.png)

## Tiles

One tile per character in `wayfarer.toml`, laid out by count: 1; 1x2; 2x2; 2x3 or 3x2 (two,
five or six tiles go side by side or stacked, whichever makes the terminals larger on your
screen). Each tile's header shows its number, the character's name, the server and the
session's state: `connecting`, `connected`, `logon`, `menu`, `realm`, `lost` or `closed`.

The terminal is always 80 columns wide, in the IBM VGA 8x16 font and the 16 DOS colours. The
font is sized so the 80 columns fill the tile ("fit to tile"), and the terminal has as many
rows as fit below that. It keeps 10,000 lines of scrollback (mouse wheel), and when the page
opens it already shows the last 2,000 pieces of output from the gateway's log.

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

## Connection

If the connection to the gateway drops (the gateway restarts, the network goes), every tile
says so and the page reconnects by itself, trying again after half a second and then less
often, up to every 10 seconds. Once back it fetches everything it missed, so nothing is lost or
shown twice. Reloading the page never logs a character off: the gateway holds the sessions.

If the gateway is reached over a non-loopback address it needs its token
(`docs/gateway.md`); the page asks for it once and remembers it in the browser.
