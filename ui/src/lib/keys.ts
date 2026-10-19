// The one table of wayfarer's own keys and controls. Help.svelte renders it, the key handlers in
// App.svelte and Tile.svelte read their key names from here, and tests/keys.test.ts keeps the
// "Keys" table in docs/web-ui.md in step with it.

import type { SessionState } from "../proto";

export interface KeyRow {
  keys: string;
  does: string;
  group: string;
}

export const KEY_ROWS: KeyRow[] = [
  { group: "Input", keys: "Enter in a tile's input line", does: "Sends the line (an empty line sends just Enter)" },
  { group: "Input", keys: "Up, Down in the input line", does: "That tile's command history" },
  {
    group: "Input",
    keys: "keys button",
    does: "Sends what you type in the terminal straight to the game, key by key; click it again to go back to the input line",
  },
  { group: "Tiles", keys: "Alt+1 to Alt+6", does: "Focus a tile (on a Mac, Option)" },
  { group: "Display", keys: "Ctrl +, Ctrl -", does: "Make the focused tile's font larger or smaller" },
  { group: "Display", keys: "Ctrl 0", does: 'Back to "fit to tile"' },
  { group: "Layout", keys: "Drag the line between two tiles", does: "Gives one tile more room and the other less; remembered per browser" },
  { group: "Layout", keys: "Double-click a line between tiles", does: "Back to equal shares for that direction (columns or rows)" },
  { group: "Layout", keys: "Alt+0", does: "Back to equal shares for columns and rows (on a Mac, Option)" },
  { group: "Help", keys: "? on an empty input line, or the ? button", does: "Open or close this help" },
  { group: "Help", keys: "Esc, ? again, or a click outside", does: "Close this help" },
];

export const BADGE_MEANINGS: Record<SessionState, string> = {
  connecting: "opening the telnet connection to the game",
  connected: "connected, no logon script running (log on by hand)",
  logon: "the logon script is answering the game's prompts",
  menu: "at the game's main menu",
  realm: "in the realm, playing",
  lost: "the connection dropped; the gateway is reconnecting",
  closed: "logged off or given up (the tile says why)",
};

export const FILES_HELP = [
  "wayfarer.toml and credentials.toml: the data directory (on Linux ~/.local/share/wayfarer/), or the directory of the file given to `wayfarer gateway --config`",
  "logs: logs/<character id>/ in the same directory (output.log, raw.bin, sent.bin)",
];

export const INPUT_VS_KEYS =
  "The input line sends a whole line when you press Enter. In keys mode every key goes to the game as you press it, and the input line is off.";

// Key names the handlers share with the table above.
export const HELP_KEY = "?";
export const FOCUS_TILE_CODE = /^Digit[1-6]$/;
export const RESET_LAYOUT_CODE = "Digit0";
export const ZOOM_IN_KEYS = ["+", "="];
export const ZOOM_OUT_KEYS = ["-"];
export const ZOOM_RESET_KEYS = ["0"];
