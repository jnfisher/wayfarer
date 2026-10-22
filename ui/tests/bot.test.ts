// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { botButtons, botKeyAction, botWidth, linkButton, narrationLine } from "../src/lib/bot";
import type { Bot, CharacterState } from "../src/proto";

// xterm needs a real canvas; these tests only look at the header and the narration line.
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

const stopped: Bot = { state: "stopped", mode: "idle", narration: "" };
const running: Bot = { state: "running", mode: "fight", narration: "Fight: large giant rat" };
const paused: Bot = { state: "paused", mode: "fight", narration: "Paused: you took over", reason: "you took over" };

describe("the bot's controls", () => {
  it("are start; pause and stop; resume and stop; none without a bot", () => {
    const actions = (b?: Bot) => botButtons(b).map((x) => `${x.action} ${x.label}`);
    expect(actions(stopped)).toEqual(["start ►"]);
    expect(actions(running)).toEqual(["pause ║", "stop ■"]);
    expect(actions(paused)).toEqual(["resume ►", "stop ■"]);
    expect(actions(undefined)).toEqual([]);
    expect([botWidth(stopped), botWidth(running), botWidth(undefined)]).toEqual([6, 9, 0]);
  });

  it("narrate while running or paused", () => {
    expect(narrationLine(running)).toBe("Fight: large giant rat");
    expect(narrationLine(paused)).toBe("Paused: you took over");
    expect(narrationLine(stopped)).toBeNull();
    expect(narrationLine(undefined)).toBeNull();
  });

  it("F9 pauses or resumes, F10 starts or stops", () => {
    expect(botKeyAction("F9", running)).toBe("pause");
    expect(botKeyAction("F9", paused)).toBe("resume");
    expect(botKeyAction("F9", stopped)).toBeNull();
    expect(botKeyAction("F10", stopped)).toBe("start");
    expect(botKeyAction("F10", running)).toBe("stop");
    expect(botKeyAction("F10", paused)).toBe("stop");
    expect(botKeyAction("F10", undefined)).toBeNull();
    expect(botKeyAction("F8", running)).toBeNull();
  });
});

describe("the tile", () => {
  const botControl = vi.fn();
  const sessionControl = vi.fn();
  const client = { sendLine: vi.fn(), sendKeys: vi.fn(), botControl, sessionControl } as never;
  const sinks = { attach: () => () => {} } as never;
  const info = { id: "a", name: "Waydeva", server: "127.0.0.1:2323", state: "realm", last_seq: 0 } as const;

  beforeEach(() => {
    botControl.mockClear();
    Object.defineProperty(document, "fonts", { value: { load: () => Promise.resolve([]) }, configurable: true });
    vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  });
  afterEach(cleanup);

  function tile(character: CharacterState | null) {
    const props = { info, number: 1, focused: false, zoom: 0, offline: null, client, sinks, onfocus() {}, onhelp() {} };
    return render(Tile, { ...props, character }).container;
  }

  it("has a log off button in the realm, and a connect button once closed or lost", async () => {
    sessionControl.mockClear();
    tile({ bot: stopped });
    await fireEvent.click(screen.getByRole("button", { name: "[log off]" }));
    expect(sessionControl).toHaveBeenCalledWith("a", "log_off");
    cleanup();
    for (const state of ["closed", "lost"] as const) {
      render(Tile, { info: { ...info, state }, number: 1, focused: false, zoom: 0, offline: null, client, sinks, onfocus() {}, onhelp() {}, character: null });
      await fireEvent.click(screen.getByRole("button", { name: "[connect]" }));
      cleanup();
    }
    expect(sessionControl.mock.calls).toEqual([["a", "log_off"], ["a", "connect"], ["a", "connect"]]);
    expect(linkButton("connecting")).toBeNull();
  });

  it("has a start button and no narration while stopped", async () => {
    const el = tile({ bot: stopped });
    expect(el.querySelector(".narration")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "start the bot" }));
    expect(botControl).toHaveBeenCalledWith("a", "start");
  });

  it("shows the narration under the header while running, with pause and stop", async () => {
    const el = tile({ bot: running });
    expect(el.querySelector("header + .narration")?.textContent).toBe("Fight: large giant rat");
    expect([...el.querySelectorAll("header .ctl:not(.link)")].map((b) => b.textContent)).toEqual(["[║]", "[■]"]);
    await fireEvent.click(screen.getByRole("button", { name: "pause the bot" }));
    await fireEvent.click(screen.getByRole("button", { name: "stop the bot" }));
    expect(botControl.mock.calls).toEqual([
      ["a", "pause"],
      ["a", "stop"],
    ]);
  });

  it("resumes when paused", async () => {
    const el = tile({ bot: paused });
    expect(el.querySelector(".narration.paused")?.textContent).toBe("Paused: you took over");
    await fireEvent.click(screen.getByRole("button", { name: "resume the bot" }));
    expect(botControl).toHaveBeenCalledWith("a", "resume");
  });

  it("has no controls without a bot (no world pack, or no brain)", () => {
    for (const character of [{}, null]) {
      const el = tile(character);
      expect(el.querySelector("header .ctl:not(.link)")).toBeNull();
      cleanup();
    }
  });
});
