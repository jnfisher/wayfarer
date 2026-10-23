// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DROP_ORDER, ROOM_MIN, bar, controlsWidth, expText, fitHeader, fixedWidth, itemWidths, type HeaderText } from "../src/lib/header";
import type { CharacterState } from "../src/proto";

// xterm needs a real canvas; the header tests only look at the header.
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    options: Record<string, unknown> = {};
    rows = 24;
    cols = 80;
    loadAddon() {}
    open() {}
    onData() {}
    focus() {}
    resize() {}
    write() {}
    reset() {}
    dispose() {}
  },
}));
vi.mock("@xterm/addon-fit", () => ({ FitAddon: class { proposeDimensions() { return { cols: 80, rows: 24 }; } } }));
vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: class {
    constructor() {
      throw new Error("no webgl in tests");
    }
  },
}));
vi.mock("@xterm/xterm/css/xterm.css", () => ({}));

import Tile from "../src/Tile.svelte";

const FULL: CharacterState = {
  name: "Waydeva",
  class: "Warrior",
  level: 3,
  hp: 28,
  hp_max: 40,
  mana: 0,
  mana_max: 0,
  exp: 1230,
  exp_percent: 45,
  room_title: "Town Square",
};

describe("bar", () => {
  it("is ten blocks, coloured by thirds", () => {
    expect(bar(28, 40)).toEqual({ full: "███████", empty: "░░░", tone: "good", text: "28/40" });
    expect(bar(20, 40)).toMatchObject({ full: "█████", tone: "warn" });
    expect(bar(10, 40)).toMatchObject({ full: "███", tone: "bad" });
    expect(bar(40, 40)).toMatchObject({ full: "██████████", empty: "" });
  });

  it("shows a sliver for any HP left, none below zero", () => {
    expect(bar(1, 100).full).toBe("█");
    expect(bar(-3, 40)).toMatchObject({ full: "", empty: "░░░░░░░░░░", tone: "bad", text: "-3/40" });
  });

  it("is empty with -- until the maximum is known", () => {
    expect(bar(28, undefined)).toEqual({ full: "", empty: "░░░░░░░░░░", tone: "none", text: "28/--" });
    expect(bar(undefined, undefined).text).toBe("--/--");
    expect(bar(0, 0)).toMatchObject({ tone: "none", text: "0/0" });
  });
});

describe("expText", () => {
  it("groups thousands and adds the percent", () => {
    expect(expText({ exp: 1230, exp_percent: 45 })).toBe("1,230 (45%)");
    expect(expText({ exp: 1230 })).toBe("1,230");
    expect(expText({})).toBe("--");
  });
});

describe("fitHeader", () => {
  const h: HeaderText = { number: 1, name: "Waydeva", server: "127.0.0.1:2323", state: "realm", character: FULL };
  const w = itemWidths(h);
  const first = () => fixedWidth(h) + DROP_ORDER.reduce((a, i) => a + (w[i] ?? 0), 0);

  it("keeps everything when it fits, or before the header is measured", () => {
    expect([...fitHeader(h, 200)].sort()).toEqual(["exp", "hp", "level", "mana", "server"]);
    expect(fitHeader(h, 0).size).toBe(5);
  });

  it("drops the first row from the right: exp, mana, hp, level", () => {
    const cols = Math.max(first(), controlsWidth(h) + w.server!);
    expect([...fitHeader(h, cols)].sort()).toEqual(["exp", "hp", "level", "mana", "server"]);
    const less = cols - (cols - first()) - 1;
    expect(fitHeader(h, less).has("exp")).toBe(false);
    expect([...fitHeader(h, fixedWidth(h) + w.level!)]).toEqual(["level"]);
    expect(fitHeader(h, 10).size).toBe(0);
  });

  it("drops the server from the second row before the room gets under its least", () => {
    const cols = controlsWidth(h) + w.server!;
    expect(fitHeader(h, cols).has("server")).toBe(true);
    expect(fitHeader(h, cols - 1).has("server")).toBe(false);
    // The party chip takes room too.
    expect(fitHeader(h, cols, 20).has("server")).toBe(false);
    expect(controlsWidth(h)).toBeGreaterThan(ROOM_MIN);
  });

  it("has only the server to drop without a brain", () => {
    const m0 = { ...h, character: null };
    expect([...fitHeader(m0, 200)]).toEqual(["server"]);
    expect(fitHeader(m0, 20).size).toBe(0);
  });

  it("counts the cells of the first row's design line", () => {
    // From ╔ to ╗ with the fill at its narrowest.
    const line = "╔═ 1 Waydeva  L3 Warrior ═ HP ███████░░░ 28/40  MA ░░░░░░░░░░ 0/0 ═ 1,230 (45%) ═ ═╗";
    expect(first()).toBe([...line].length);
  });
});

describe("the tile header", () => {
  const client = { sendLine: vi.fn(), sendKeys: vi.fn() } as never;
  const sinks = { attach: () => () => {} } as never;
  const info = { id: "a", name: "Waydeva", server: "127.0.0.1:2323", state: "realm", last_seq: 0 } as const;

  beforeEach(() => {
    Object.defineProperty(document, "fonts", { value: { load: () => Promise.resolve([]) }, configurable: true });
    vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  });
  afterEach(cleanup);

  function header(character: CharacterState | null | undefined) {
    const props = { info, number: 1, focused: false, zoom: 0, offline: null, client, sinks, onfocus() {}, onhelp() {} };
    const { container } = render(Tile, character === undefined ? props : { ...props, character });
    // Both rows: the frame's top and the controls under it.
    const el = container.querySelector("section")!;
    const text = (cls: string) => [...el.querySelectorAll(`.${cls}`)].map((s) => s.textContent);
    return { el, text };
  }

  it("shows the whole snapshot", () => {
    const { el, text } = header(FULL);
    expect(text("name")).toEqual([" Waydeva"]);
    expect(text("level")).toEqual(["  L3 Warrior"]);
    expect(text("label")).toEqual(["HP ", "  MA "]);
    expect(text("full")).toEqual(["███████", ""]);
    expect(text("empty")).toEqual(["░░░", "░░░░░░░░░░"]);
    expect(text("nums")).toEqual([" 28/40", " 0/0"]);
    expect([...el.querySelectorAll(".bar")].map((b) => b.className.split(" ")[1])).toEqual(["good", "none"]);
    expect(text("exp")).toEqual(["1,230 (45%)"]);
    expect(text("room")).toEqual(["Town Square"]);
    expect(text("server")).toEqual(["127.0.0.1:2323"]);
    expect(text("badge")).toEqual(["realm"]);
    expect(text("nobrain")).toEqual([]);
  });

  it("shows -- for an empty snapshot", () => {
    const { text } = header({});
    expect(text("level")).toEqual(["  L-- --"]);
    expect(text("nums")).toEqual([" --/--", " --/--"]);
    expect(text("full")).toEqual(["", ""]);
    expect(text("exp")).toEqual(["--"]);
    expect(text("room")).toEqual(["--"]);
    expect(text("nobrain")).toEqual([]);
  });

  it("is the M0 header plus `no brain` without a brain", () => {
    for (const character of [null, undefined]) {
      const { el, text } = header(character);
      expect(el.querySelector(".level, .bar, .exp")).toBeNull();
      expect(text("room")).toEqual([""]);
      expect(text("server")).toEqual(["127.0.0.1:2323"]);
      expect(text("nobrain")).toEqual([" no brain"]);
      cleanup();
    }
  });

  it("starts with ╔ and ends with ╗", () => {
    const top = header(FULL).el.querySelector("header")!;
    expect(top.firstElementChild!.textContent).toBe("╔═ ");
    expect(top.lastElementChild!.textContent).toBe("═╗");
  });
});
