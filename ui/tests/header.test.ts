// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bar, expText, fitHeader, fixedWidth, itemWidths, type HeaderText } from "../src/lib/header";
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
  const all = () => fixedWidth(h) + Object.values(itemWidths(h)).reduce((a, b) => a + b, 0);

  it("keeps everything when it fits, or before the header is measured", () => {
    expect([...fitHeader(h, all())].sort()).toEqual(["exp", "hp", "level", "mana", "room", "server"]);
    expect(fitHeader(h, 0).size).toBe(6);
  });

  it("drops from the right: server, room, exp, mana, hp, level", () => {
    const w = itemWidths(h);
    expect([...fitHeader(h, all() - 1)].sort()).toEqual(["exp", "hp", "level", "mana", "room"]);
    expect([...fitHeader(h, all() - w.server! - w.room!)].sort()).toEqual(["exp", "hp", "level", "mana"]);
    expect([...fitHeader(h, fixedWidth(h) + w.level!)]).toEqual(["level"]);
    expect(fitHeader(h, 10).size).toBe(0);
  });

  it("has only the server to drop without a brain", () => {
    const m0 = { ...h, character: null };
    expect([...fitHeader(m0, 200)]).toEqual(["server"]);
    expect(fitHeader(m0, 20).size).toBe(0);
  });

  it("counts the cells of the header's design line", () => {
    // The card's example, from ╔ to ╗ with one cell of fill.
    const line =
      "╔═ 1 Waydeva  L3 Warrior ═ HP ███████░░░ 28/40  MA ░░░░░░░░░░ 0/0 ═ 1,230 (45%) ═ Town Square ═ 127.0.0.1:2323 ═  realm  [log off] [▼] [▬] ═╗";
    expect(all()).toBe([...line].length);
  });

  it("keeps room for the bot's buttons", () => {
    const running = { ...h, character: { ...FULL, bot: { state: "running", mode: "fight", narration: "" } } } as const;
    const line =
      "╔═ 1 Waydeva  L3 Warrior ═ HP ███████░░░ 28/40  MA ░░░░░░░░░░ 0/0 ═ 1,230 (45%) ═ Town Square ═ [║][■] ═ 127.0.0.1:2323 ═  realm  [log off] [▼] [▬] ═╗";
    const width = fixedWidth(running) + Object.values(itemWidths(running)).reduce((a, b) => a + b, 0);
    expect(width).toBe([...line].length);
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
    const el = container.querySelector("header")!;
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
      expect(el.querySelector(".level, .bar, .exp, .room")).toBeNull();
      expect(text("server")).toEqual(["127.0.0.1:2323"]);
      expect(text("nobrain")).toEqual([" no brain"]);
      cleanup();
    }
  });

  it("starts with ╔ and ends with ╗", () => {
    const { el } = header(FULL);
    expect(el.firstElementChild!.textContent).toBe("╔═ ");
    expect(el.lastElementChild!.textContent).toBe(" ═╗");
  });
});
