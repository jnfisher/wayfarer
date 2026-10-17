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
| `Alt+1` to `Alt+6` | Focus a tile (on a Mac, `Option`) |
| `Ctrl +`, `Ctrl -` | Make the focused tile's font larger or smaller |
| `Ctrl 0` | Back to "fit to tile" |

Clicking a tile focuses it. The **keys** button sends what you type in the terminal itself
straight to the game, key by key, instead of a line at a time: for the character editor and
`Hit any key` prompts. Click it again to go back to the input line.

## Connection

If the connection to the gateway drops (the gateway restarts, the network goes), every tile
says so and the page reconnects by itself, trying again after half a second and then less
often, up to every 10 seconds. Once back it fetches everything it missed, so nothing is lost or
shown twice. Reloading the page never logs a character off: the gateway holds the sessions.

If the gateway is reached over a non-loopback address it needs its token
(`docs/gateway.md`); the page asks for it once and remembers it in the browser.
