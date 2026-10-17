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
