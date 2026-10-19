// Tile layout and font fitting (docs/plan/05-ui.md, "Views" and "Fonts and scaling").

export interface Grid {
  rows: number;
  cols: number;
}

/** The terminal's shape in character cells: 80 columns, and the 24 rows the game draws for. */
export const TERM_COLS = 80;
export const TERM_ROWS = 24;
/** Width over height of one cell of the VGA 8x16 font. */
export const CELL_ASPECT = 8 / 16;

/**
 * The grid for `count` tiles in a `width` x `height` area: 1; 1x2; 2x2; 2x3 or 3x2. Where two
 * shapes are possible (2, 5 or 6 tiles) the one that lets an 80x24 terminal be drawn largest
 * wins, so a wide screen puts tiles side by side and a tall one stacks them.
 */
export function tileGrid(count: number, width: number, height: number): Grid {
  if (count <= 0) return { rows: 0, cols: 0 };
  const shapes: Grid[] =
    count === 1
      ? [{ rows: 1, cols: 1 }]
      : count === 2
        ? [
            { rows: 1, cols: 2 },
            { rows: 2, cols: 1 },
          ]
        : count <= 4
          ? [{ rows: 2, cols: 2 }]
          : count <= 6
            ? [
                { rows: 2, cols: 3 },
                { rows: 3, cols: 2 },
              ]
            : [{ rows: Math.ceil(count / 3), cols: 3 }];
  let best = shapes[0];
  let bestScale = -1;
  for (const shape of shapes) {
    const scale = terminalScale(width / shape.cols, height / shape.rows);
    if (scale > bestScale) {
      best = shape;
      bestScale = scale;
    }
  }
  return best;
}

/** The font size (px) an 80x24 terminal could have in a `width` x `height` box. */
function terminalScale(width: number, height: number): number {
  return Math.min(width / (TERM_COLS * CELL_ASPECT), height / TERM_ROWS);
}

/**
 * The largest whole font size whose `cols` columns fit in `width` pixels, for a font whose
 * cell is `cellAspect` times its size wide. At least `min`.
 */
export function fitFontSize(width: number, cellAspect = CELL_ASPECT, cols = TERM_COLS, min = 6) {
  return Math.max(min, Math.floor(width / (cols * cellAspect)));
}

/** Relative sizes of a grid's columns and rows (proportions, not pixels). */
export interface Weights {
  cols: number[];
  rows: number[];
}

export type Axis = "cols" | "rows";

/** The key a grid shape's weights are kept under: `2x3` is two rows of three. */
export function shapeKey(grid: Grid): string {
  return `${grid.rows}x${grid.cols}`;
}

export function equalWeights(grid: Grid): Weights {
  return { cols: Array(grid.cols).fill(1), rows: Array(grid.rows).fill(1) };
}

/** Whether `w` is usable for `grid`: the right number of finite, positive weights per axis. */
export function fitsGrid(w: Weights, grid: Grid): boolean {
  const ok = (a: unknown, n: number) =>
    Array.isArray(a) && a.length === n && a.every((x) => typeof x === "number" && Number.isFinite(x) && x > 0);
  return ok(w?.cols, grid.cols) && ok(w?.rows, grid.rows);
}

/**
 * Move the boundary between track `index` and `index + 1` of `axis` by `deltaPx`. `totalPx` is
 * the room the axis' tracks share (without gutters) and `minPx` the least either neighbour may
 * have; the drag stops at the limit. Only those two neighbours change. When the pair cannot
 * both have `minPx`, the weights come back unchanged.
 */
export function drag(w: Weights, axis: Axis, index: number, deltaPx: number, totalPx: number, minPx: number): Weights {
  const tracks = w[axis];
  if (index < 0 || index + 1 >= tracks.length || totalPx <= 0) return w;
  const sum = tracks.reduce((a, b) => a + b, 0);
  const perPx = sum / totalPx;
  const min = minPx * perPx;
  const pair = tracks[index] + tracks[index + 1];
  if (pair < 2 * min) return w;
  const first = Math.min(pair - min, Math.max(min, tracks[index] + deltaPx * perPx));
  const next = tracks.slice();
  next[index] = first;
  next[index + 1] = pair - first;
  return { ...w, [axis]: next };
}

/** The `grid-template-*` value for `tracks`, with a `gutterPx` track between each pair. */
export function toTemplate(tracks: number[], gutterPx = 0): string {
  const fr = tracks.map((t) => `minmax(0, ${Math.round(t * 1000) / 1000}fr)`);
  return fr.join(gutterPx > 0 ? ` ${gutterPx}px ` : " ");
}

/** What is kept between visits: weights per grid shape, versioned for a later move to settings. */
export interface StoredLayout {
  v: 1;
  shapes: Record<string, Weights>;
}

export const LAYOUT_KEY = "wayfarer.layout";

export interface LayoutStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** Read the stored layout; anything missing, unreadable or of another version is "none yet". */
export function loadLayout(storage: LayoutStorage | undefined): StoredLayout {
  const none: StoredLayout = { v: 1, shapes: {} };
  try {
    const raw = storage?.getItem(LAYOUT_KEY);
    if (!raw) return none;
    const parsed = JSON.parse(raw);
    if (parsed?.v !== 1 || typeof parsed.shapes !== "object" || parsed.shapes === null) return none;
    return { v: 1, shapes: parsed.shapes };
  } catch {
    return none;
  }
}

/** Write the layout; with storage unavailable it is simply forgotten on reload. */
export function saveLayout(storage: LayoutStorage | undefined, layout: StoredLayout): void {
  try {
    storage?.setItem(LAYOUT_KEY, JSON.stringify(layout));
  } catch {
    // quota, private mode, blocked site data
  }
}
