// The tile header's text: the bars, the numbers, and which items fit (docs/web-ui.md,
// "The header"). Tile.svelte draws the spans; everything here is measured in character
// cells, since the header is the VGA font at a fixed 16 px (8 px a cell).

import type { CharacterState } from "../proto";
import { botWidth } from "./bot";

/** Width of one header character cell in CSS px. */
export const HEADER_CELL_PX = 8;
/** Blocks in a bar. */
export const BAR_CELLS = 10;
/** The room title is cut (with an ellipsis) beyond this many cells. */
export const ROOM_MAX = 30;
export const UNKNOWN = "--";

/** good above two thirds, warn above one third, bad below; none without a maximum. */
export type Tone = "good" | "warn" | "bad" | "none";

export interface Bar {
  full: string;
  empty: string;
  tone: Tone;
  /** `28/40`, `28/--`, `--/--`. */
  text: string;
}

const num = (n: number | undefined) => (n === undefined ? UNKNOWN : String(n));

/** A ten-block bar for `value` of `max`; empty until the maximum is known. */
export function bar(value: number | undefined, max: number | undefined): Bar {
  const text = `${num(value)}/${num(max)}`;
  if (value === undefined || max === undefined || max <= 0) {
    return { full: "", empty: "░".repeat(BAR_CELLS), tone: "none", text };
  }
  const frac = Math.min(1, Math.max(0, value / max));
  let cells = Math.round(frac * BAR_CELLS);
  if (value > 0 && cells === 0) cells = 1;
  const tone: Tone = frac > 2 / 3 ? "good" : frac > 1 / 3 ? "warn" : "bad";
  return { full: "█".repeat(cells), empty: "░".repeat(BAR_CELLS - cells), tone, text };
}

/** `L3 Warrior`, with `--` for what is not known. */
export function levelClass(c: CharacterState): string {
  return `L${num(c.level)} ${c.class ?? UNKNOWN}`;
}

/** `1,230 (45%)`, `1,230` without the percent, `--` without the exp. */
export function expText(c: CharacterState): string {
  if (c.exp === undefined) return UNKNOWN;
  const exp = c.exp.toLocaleString("en-US");
  return c.exp_percent === undefined ? exp : `${exp} (${c.exp_percent}%)`;
}

export function roomText(c: CharacterState): string {
  return c.room_title ?? UNKNOWN;
}

/** The items that can be dropped, in the order they go (rightmost first). */
export type HeaderItem = "server" | "room" | "exp" | "mana" | "hp" | "level";
export const DROP_ORDER: HeaderItem[] = ["server", "room", "exp", "mana", "hp", "level"];

export interface HeaderText {
  number: number;
  name: string;
  server: string;
  state: string;
  /** Null when no brain is attached: the M0 header plus `no brain`. */
  character: CharacterState | null;
}

/** Cells each droppable item takes, separators included (see the spans in Tile.svelte). */
export function itemWidths(h: HeaderText): Partial<Record<HeaderItem, number>> {
  const widths: Partial<Record<HeaderItem, number>> = { server: h.server.length + 3 };
  const c = h.character;
  if (!c) return widths;
  widths.level = 2 + levelClass(c).length;
  widths.hp = 6 + BAR_CELLS + 1 + bar(c.hp, c.hp_max).text.length;
  widths.mana = 5 + BAR_CELLS + 1 + bar(c.mana, c.mana_max).text.length;
  widths.exp = 3 + expText(c).length;
  widths.room = 3 + Math.min(ROOM_MAX, roomText(c).length);
  return widths;
}

/**
 * Cells the items that always show take: corners, number, name, fill, the bot's buttons,
 * badge, `no brain`.
 */
export function fixedWidth(h: HeaderText): number {
  const start = 3 + String(h.number).length + 1 + h.name.length; // "╔═ 1 Name"
  const fill = 3; // " ═ " at its narrowest
  const bot = botWidth(h.character?.bot); // "[║][■] ═ "
  const badge = h.state.length + 2;
  const noBrain = h.character ? 0 : 9; // " no brain"
  const end = 3; // " ═╗"
  return start + fill + bot + badge + noBrain + end;
}

/**
 * The droppable items that fit in `cols` cells, dropping from the right (`DROP_ORDER`). A
 * header not measured yet (`cols` 0) shows everything.
 */
export function fitHeader(h: HeaderText, cols: number): Set<HeaderItem> {
  const widths = itemWidths(h);
  const shown = new Set(Object.keys(widths) as HeaderItem[]);
  if (cols <= 0) return shown;
  let total = fixedWidth(h) + Object.values(widths).reduce((a, b) => a + b, 0);
  for (const item of DROP_ORDER) {
    if (total <= cols) break;
    if (!shown.delete(item)) continue;
    total -= widths[item]!;
  }
  return shown;
}
