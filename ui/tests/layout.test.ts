import { describe, expect, it } from "vitest";
import { fitFontSize, tileGrid } from "../src/lib/layout";
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

  it("chooses 1x2 or 2x1 by screen shape", () => {
    expect(tileGrid(2, ...wide)).toEqual({ rows: 1, cols: 2 });
    expect(tileGrid(2, ...tall)).toEqual({ rows: 2, cols: 1 });
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
