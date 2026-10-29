// The tile header's text: the bars, the numbers, and which items fit (docs/web-ui.md,
// "The header"). Tile.svelte draws the spans; everything here is measured in character
// cells, since the header is the VGA font at a fixed 16 px (8 px a cell).

import type { CharacterState } from "../proto";
import { botWidth, linkWidth, menuWidth } from "./bot";

/** Width of one header character cell in CSS px. */
export const HEADER_CELL_PX = 8;
/** Blocks in a bar. */
export const BAR_CELLS = 10;
/** Blocks in a bar when the header is short of room: the bars shrink before anything goes. */
export const SHORT_BAR_CELLS = 5;
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

/** A bar of `size` blocks (ten) for `value` of `max`; empty until the maximum is known. */
export function bar(value: number | undefined, max: number | undefined, size = BAR_CELLS): Bar {
  const text = `${num(value)}/${num(max)}`;
  if (value === undefined || max === undefined || max <= 0) {
    return { full: "", empty: "░".repeat(size), tone: "none", text };
  }
  const frac = Math.min(1, Math.max(0, value / max));
  let cells = Math.round(frac * size);
  if (value > 0 && cells === 0) cells = 1;
  const tone: Tone = frac > 2 / 3 ? "good" : frac > 1 / 3 ? "warn" : "bad";
  return { full: "█".repeat(cells), empty: "░".repeat(size - cells), tone, text };
}

/** A character with no mana (a Warrior, a Witchunter): its prompt is `[HP=..]:` alone and no
 * mana is known. Its header has no MA bar, which would only ever say `--/--`. */
export function noMana(c: CharacterState): boolean {
  return c.prompt_form === "hp" && !c.mana_max;
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

/** The items that can be dropped: the first row's in the order they go (rightmost first), and
 * the second row's server. The room always shows on the second row, cut to fit. */
export type HeaderItem = "server" | "exp" | "mana" | "hp" | "level";
export const DROP_ORDER: HeaderItem[] = ["exp", "mana", "hp", "level"];

/** Cells of the minimize button `[▬]`, with the space before it. */
export const MINIMIZE_CELLS = 4;
/** The second row keeps this many cells for the room before it drops the server. */
export const ROOM_MIN = 8;

export interface HeaderText {
  number: number;
  name: string;
  server: string;
  state: string;
  /** Null when no brain is attached: the M0 header plus `no brain`. */
  character: CharacterState | null;
}

/** Cells each droppable item takes, separators included (see the spans in Tile.svelte), with
 * bars of `size` blocks. */
export function itemWidths(h: HeaderText, size = BAR_CELLS): Partial<Record<HeaderItem, number>> {
  const widths: Partial<Record<HeaderItem, number>> = { server: h.server.length + 3 };
  const c = h.character;
  if (!c) return widths;
  widths.level = 2 + levelClass(c).length;
  widths.hp = 6 + size + 1 + bar(c.hp, c.hp_max).text.length;
  if (!noMana(c)) widths.mana = 5 + size + 1 + bar(c.mana, c.mana_max).text.length;
  widths.exp = 3 + expText(c).length;
  return widths;
}

/** The first row's cells with everything shown, bars of `size` blocks. */
function firstRow(h: HeaderText, size: number): number {
  const widths = itemWidths(h, size);
  return fixedWidth(h) + DROP_ORDER.reduce((a, item) => a + (widths[item] ?? 0), 0);
}

/** The blocks in the header's bars: ten when the whole first row fits in `cols` cells (or the
 * header is not measured yet), else five, so the exp keeps its place a while longer. */
export function barSize(h: HeaderText, cols: number): number {
  return cols <= 0 || firstRow(h, BAR_CELLS) <= cols ? BAR_CELLS : SHORT_BAR_CELLS;
}

/** Cells the first row always takes: the corners, number, name and fill. */
export function fixedWidth(h: HeaderText): number {
  const start = 3 + String(h.number).length + 1 + h.name.length; // "╔═ 1 Name"
  const fill = 3; // " ═ " at its narrowest
  const end = 2; // "═╗"
  return start + fill + end;
}

/**
 * Cells the second row always takes, the room's least included: the side frames, the party
 * chip (`chip` cells with its separator), the bot's buttons, the badge and its buttons, `no
 * brain`.
 */
export function controlsWidth(h: HeaderText, chip = 0): number {
  const sides = 2 + 1; // ║ on each side, and the space before the right one
  const bot = botWidth(h.character?.bot); // "[║][■] │ "
  // "realm [log off] [▬] [▼]"
  const badge =
    h.state.length + linkWidth(h.state) + menuWidth(h.state, h.character !== null) + MINIMIZE_CELLS;
  const noBrain = h.character ? 0 : 9; // " no brain"
  return sides + ROOM_MIN + chip + bot + badge + noBrain;
}

/**
 * The droppable items that fit in `cols` cells: the first row's bars shrink to
 * [`SHORT_BAR_CELLS`] ([`barSize`]), then it drops from the right (`DROP_ORDER`); the second
 * drops the server when the room would have less than [`ROOM_MIN`] cells. A header not
 * measured yet (`cols` 0) shows everything.
 */
export function fitHeader(h: HeaderText, cols: number, chip = 0): Set<HeaderItem> {
  const size = barSize(h, cols);
  const widths = itemWidths(h, size);
  const shown = new Set(Object.keys(widths) as HeaderItem[]);
  if (cols <= 0) return shown;
  let total = firstRow(h, size);
  for (const item of DROP_ORDER) {
    if (total <= cols) break;
    if (!shown.delete(item)) continue;
    total -= widths[item]!;
  }
  if (controlsWidth(h, chip) + widths.server! > cols) shown.delete("server");
  return shown;
}
