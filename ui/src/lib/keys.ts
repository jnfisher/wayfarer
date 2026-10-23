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
  {
    group: "Map",
    keys: "Alt+M, or the map button",
    does: "Open the drawer on the map of the focused character's surroundings, or close it (on a Mac, Option)",
  },
  { group: "Map", keys: "Click a room on the map", does: "Walk the character there; stop under the map ends the walk" },
  {
    group: "Map",
    keys: "Alt+G, or the go button",
    does:
      "Open the go box above the map: type part of the name of a room, a shop, bank or trainer, or a monster (two letters or more) and the list fills as you type (on a Mac, Option)",
  },
  {
    group: "Map",
    keys: "Up, Down, Enter in the go box",
    does:
      "Choose a row (the map centres on its room when in view) and walk there; a boss lair's row has a ticked stop short box: the walk stops one room before it",
  },
  { group: "Map", keys: "Esc in the go box", does: "Clear it; Esc again closes it" },
  { group: "Map", keys: "Mouse wheel, drag on the map", does: "Zoom (three sizes), pan; centre goes back to the character" },
  {
    group: "Map",
    keys: "loops, beside go",
    does:
      "The 12 loops nearest the character, nearest first, with a box to find one by name, file, author or a stop, however far: hover or pick one to draw it in yellow, walk once walks it, run hunts round it (untick a stop to pass it by)",
  },
  {
    group: "Parties",
    keys: "Alt+P, or the parties button",
    does: "Open the drawer on the parties: pick a leader, tick its followers, save; each member's state under it (on a Mac, Option)",
  },
  {
    group: "Parties",
    keys: "The party chip in the tile's header",
    does: "★ Leading, Following or Separated; click it to leave the party, or to join one",
  },
  {
    group: "Blessings",
    keys: "Alt+B, or the blessings button",
    does: "Open the drawer on the blessings: tick the lasting spells each character keeps on itself (bless, barkskin, holy aura); ● marks one that is on (on a Mac, Option)",
  },
  {
    group: "Bot",
    keys: "F10, or [►] and [■] in the tile's header",
    does: "Start the focused character's bot where it stands (fight what comes, rest when low), or stop it",
  },
  {
    group: "Bot",
    keys: "F9, or [║] and [►] in the tile's header",
    does: "Pause or resume the focused character's bot; a line you type pauses it too",
  },
  {
    group: "Bot",
    keys: "Alt+S, or the stats button",
    does: "Open the drawer on the stats of the focused character's runs: exp per hour, kills, deaths and flees, now and for the last runs; or close it (on a Mac, Option)",
  },
  {
    group: "Bot",
    keys: "Alt+T, or the settings button",
    does: "Open the drawer on the focused character's settings (the form, \"The settings\" below), or close it (on a Mac, Option)",
  },
  {
    group: "Development",
    keys: "Alt+D, or the dev button",
    does: "Open the drawer on the dev panel: the focused character's events, unclassified lines and the brain; or close it (on a Mac, Option)",
  },
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
  "wayfarer.toml and credentials.toml: the data directory (on macOS ~/Library/Application Support/wayfarer/, on Linux ~/.local/share/wayfarer/), or the directory of the file given to `wayfarer gateway --config`",
  "logs: logs/<character id>/ in the same directory (output.log, raw.bin, sent.bin)",
];

export const INPUT_VS_KEYS =
  "The input line sends a whole line when you press Enter. In keys mode every key goes to the game as you press it, and the input line is off.";

// Key names the handlers share with the table above.
export const HELP_KEY = "?";
export const FOCUS_TILE_CODE = /^Digit[1-6]$/;
export const RESET_LAYOUT_CODE = "Digit0";
export const DEV_PANEL_CODE = "KeyD";
export const MAP_PANEL_CODE = "KeyM";
export const PARTIES_PANEL_CODE = "KeyP";
export const BLESSINGS_PANEL_CODE = "KeyB";
export const GO_CODE = "KeyG";
export const STATS_PANEL_CODE = "KeyS";
export const SETTINGS_PANEL_CODE = "KeyT";
export const ZOOM_IN_KEYS = ["+", "="];
export const ZOOM_OUT_KEYS = ["-"];
export const ZOOM_RESET_KEYS = ["0"];
