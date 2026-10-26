import { describe, expect, it } from "vitest";
import {
  drag,
  equalWeights,
  fitFontSize,
  fitsGrid,
  LAYOUT_KEY,
  loadLayout,
  saveLayout,
  shapeKey,
  tileGrid,
  toTemplate,
} from "../src/lib/layout";
import { InputHistory } from "../src/lib/input";
import { TerminalSinks } from "../src/lib/sinks";

describe("tileGrid", () => {
  const wide = [2560, 1400] as const;
  const tall = [1000, 1800] as const;

  it("lays tiles out by count", () => {
    expect(tileGrid(0, ...wide)).toEqual({ rows: 0, cols: 0 });
    expect(tileGrid(1, ...wide)).toEqual({ rows: 1, cols: 1 });
    expect(tileGrid(3, ...wide)).toEqual({ rows: 2, cols: 2 });
    expect(tileGrid(4, ...wide)).toEqual({ rows: 2, cols: 2 });
    expect(tileGrid(4, ...tall)).toEqual({ rows: 2, cols: 2 });
  });

  it("puts two tiles side by side, whatever the screen shape", () => {
    expect(tileGrid(2, ...wide)).toEqual({ rows: 1, cols: 2 });
    expect(tileGrid(2, ...tall)).toEqual({ rows: 1, cols: 2 });
    expect(tileGrid(2, 1600, 900)).toEqual({ rows: 1, cols: 2 });
  });

  it("chooses 2x3 or 3x2 by screen shape", () => {
    expect(tileGrid(6, ...wide)).toEqual({ rows: 2, cols: 3 });
    expect(tileGrid(5, ...wide)).toEqual({ rows: 2, cols: 3 });
    expect(tileGrid(6, ...tall)).toEqual({ rows: 3, cols: 2 });
    expect(tileGrid(6, 1920, 1080)).toEqual({ rows: 2, cols: 3 });
  });

  it("has room for every tile past six", () => {
    const grid = tileGrid(8, ...wide);
    expect(grid.rows * grid.cols).toBeGreaterThanOrEqual(8);
  });
});

describe("fitFontSize", () => {
  it("fits 80 columns of an 8x16 font in the width", () => {
    expect(fitFontSize(640)).toBe(16);
    expect(fitFontSize(1279)).toBe(31);
    expect(fitFontSize(500)).toBe(12);
    expect(fitFontSize(100)).toBe(6);
  });
});

describe("InputHistory", () => {
  it("walks back and forth through sent lines, keeping the draft", () => {
    const h = new InputHistory();
    expect(h.up("")).toBe("");
    h.push("look");
    h.push("");
    h.push("rest");
    h.push("rest");
    expect(h.up("typ")).toBe("rest");
    expect(h.up("rest")).toBe("look");
    expect(h.up("look")).toBe("look");
    expect(h.down()).toBe("rest");
    expect(h.down()).toBe("typ");
    expect(h.down()).toBe("typ");
  });
});

describe("TerminalSinks", () => {
  it("holds output until a terminal attaches", () => {
    const sinks = new TerminalSinks();
    const got: string[] = [];
    sinks.write("kasimir", "a");
    sinks.write("kasimir", "b");
    const detach = sinks.attach("kasimir", { write: (t) => got.push(t), reset: () => got.push("RESET") });
    sinks.write("kasimir", "c");
    sinks.reset("kasimir");
    detach();
    sinks.write("kasimir", "d");
    expect(got).toEqual(["ab", "c", "RESET"]);
  });
});

describe("drag", () => {
  const even = { cols: [1, 1, 1], rows: [1, 1] };

  it("moves one boundary, changing only its two neighbours", () => {
    // 900 px for three columns: 100 px is a third of a weight.
    const w = drag(even, "cols", 0, 150, 900, 100);
    expect(w.cols[0]).toBeCloseTo(1.5);
    expect(w.cols[1]).toBeCloseTo(0.5);
    expect(w.cols[2]).toBe(1);
    expect(w.rows).toBe(even.rows);
  });

  it("keeps the total, so the others keep their size", () => {
    const w = drag(even, "rows", 0, -40, 600, 100);
    expect(w.rows[0] + w.rows[1]).toBeCloseTo(2);
    expect(w.rows[0]).toBeLessThan(1);
  });

  it("stops at the minimum on either side", () => {
    const left = drag(even, "cols", 0, -5000, 900, 200);
    expect(left.cols[0]).toBeCloseTo(200 / 300);
    const right = drag(even, "cols", 0, 5000, 900, 200);
    expect(right.cols[1]).toBeCloseTo(200 / 300);
    expect(right.cols[0] + right.cols[1]).toBeCloseTo(2);
  });

  it("leaves weights alone when the pair cannot both have the minimum, or the index is bad", () => {
    expect(drag(even, "cols", 0, 10, 300, 200)).toBe(even);
    expect(drag(even, "cols", 2, 10, 900, 100)).toBe(even);
    expect(drag(even, "cols", 0, 10, 0, 100)).toBe(even);
  });
});

describe("toTemplate", () => {
  it("writes fr tracks that may shrink to nothing", () => {
    expect(toTemplate([1.4, 0.6])).toBe("minmax(0, 1.4fr) minmax(0, 0.6fr)");
    expect(toTemplate([1, 1], 6)).toBe("minmax(0, 1fr) 6px minmax(0, 1fr)");
    expect(toTemplate([])).toBe("");
  });
});

describe("shapes", () => {
  it("names a shape rows by columns and checks weights against it", () => {
    const grid = { rows: 2, cols: 3 };
    expect(shapeKey(grid)).toBe("2x3");
    expect(equalWeights(grid)).toEqual({ cols: [1, 1, 1], rows: [1, 1] });
    expect(fitsGrid(equalWeights(grid), grid)).toBe(true);
    expect(fitsGrid({ cols: [1, 1], rows: [1, 1] }, grid)).toBe(false);
    expect(fitsGrid({ cols: [1, 0, 1], rows: [1, 1] }, grid)).toBe(false);
    expect(fitsGrid({ cols: [1, NaN, 1], rows: [1, 1] }, grid)).toBe(false);
  });
});

describe("layout storage", () => {
  const fake = () => {
    const data = new Map<string, string>();
    return { data, getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
  };

  it("round-trips through storage under one key, with a version", () => {
    const storage = fake();
    const layout = { v: 1 as const, shapes: { "2x2": { cols: [1.4, 0.6], rows: [1, 1] } } };
    saveLayout(storage, layout);
    expect([...storage.data.keys()]).toEqual([LAYOUT_KEY]);
    expect(JSON.parse(storage.data.get(LAYOUT_KEY)!).v).toBe(1);
    expect(loadLayout(storage)).toEqual(layout);
  });

  it("starts empty on missing, corrupt or other-version data", () => {
    const storage = fake();
    const none = { v: 1, shapes: {} };
    expect(loadLayout(storage)).toEqual(none);
    storage.data.set(LAYOUT_KEY, "not json");
    expect(loadLayout(storage)).toEqual(none);
    storage.data.set(LAYOUT_KEY, JSON.stringify({ v: 2, shapes: { "1x2": {} } }));
    expect(loadLayout(storage)).toEqual(none);
    storage.data.set(LAYOUT_KEY, "null");
    expect(loadLayout(storage)).toEqual(none);
  });

  it("works when storage is missing or throws", () => {
    const throwing = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    expect(loadLayout(undefined)).toEqual({ v: 1, shapes: {} });
    expect(loadLayout(throwing)).toEqual({ v: 1, shapes: {} });
    expect(() => saveLayout(throwing, { v: 1, shapes: {} })).not.toThrow();
    expect(() => saveLayout(undefined, { v: 1, shapes: {} })).not.toThrow();
  });
});
