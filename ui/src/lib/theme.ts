// The DOS look: the 16 colours at their VGA values and the IBM VGA font (docs/plan/05-ui.md,
// "Look").

import type { ITheme } from "@xterm/xterm";

/** The font family declared in app.css, with a system monospace behind it. */
export const FONT_FAMILY = '"Px437 IBM VGA 8x16", ui-monospace, Menlo, Consolas, monospace';
export const FONT_NAME = "Px437 IBM VGA 8x16";

/** The 16 colours in xterm's ANSI order (black, red, green, yellow/brown, blue, ...). */
export const VGA_THEME: ITheme = {
  background: "#000000",
  foreground: "#aaaaaa",
  cursor: "#aaaaaa",
  cursorAccent: "#000000",
  selectionBackground: "#5555ff80",
  black: "#000000",
  red: "#aa0000",
  green: "#00aa00",
  yellow: "#aa5500",
  blue: "#0000aa",
  magenta: "#aa00aa",
  cyan: "#00aaaa",
  white: "#aaaaaa",
  brightBlack: "#555555",
  brightRed: "#ff5555",
  brightGreen: "#55ff55",
  brightYellow: "#ffff55",
  brightBlue: "#5555ff",
  brightMagenta: "#ff55ff",
  brightCyan: "#55ffff",
  brightWhite: "#ffffff",
};
