// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { KEY_ROWS } from "../src/lib/keys";

// xterm needs a real canvas; the help tests only care about the input line.
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
import Help from "../src/Help.svelte";

const squash = (s: string) => s.replace(/\s+/g, " ").trim();

describe("docs/web-ui.md", () => {
  it("has the Keys table of keys.ts, in order", () => {
    const doc = readFileSync(resolve(process.cwd(), "../docs/web-ui.md"), "utf8");
    const section = doc.split("\n## Keys\n")[1].split("\n## ")[0];
    const rows = section
      .split("\n")
      .filter((l: string) => l.startsWith("|") && !l.startsWith("| Key") && !l.startsWith("|---"))
      .map((l: string) => l.split(/(?<!\\)\|/).slice(1, -1).map((c: string) => squash(c.replaceAll("`", ""))));
    expect(rows).toEqual(KEY_ROWS.map((r) => [squash(r.keys), squash(r.does)]));
  });
});

describe("help overlay", () => {
  const sendLine = vi.fn();
  const sendKeys = vi.fn();
  const client = { sendLine, sendKeys } as never;
  const sinks = { attach: () => () => {} } as never;
  const info = { id: "a", name: "Aria", server: "silvermere", state: "realm" } as never;

  beforeEach(() => {
    sendLine.mockClear();
    sendKeys.mockClear();
    Object.defineProperty(document, "fonts", { value: { load: () => Promise.resolve([]) }, configurable: true });
    vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  });
  afterEach(cleanup);

  function mountTile(onhelp = vi.fn()) {
    render(Tile, { info, number: 1, focused: false, zoom: 0, offline: null, client, sinks, onfocus: () => {}, onhelp });
    return { onhelp, input: screen.getByLabelText("Input for Aria") as HTMLInputElement };
  }

  it("opens on ? at an empty line and sends nothing", async () => {
    const { onhelp, input } = mountTile();
    expect(await fireEvent.keyDown(input, { key: "?" })).toBe(false); // preventDefault'd
    expect(onhelp).toHaveBeenCalledOnce();
    expect(sendLine).not.toHaveBeenCalled();
  });

  it("treats ? after text as an ordinary character", async () => {
    const { onhelp, input } = mountTile();
    await fireEvent.input(input, { target: { value: "say hi" } });
    expect(await fireEvent.keyDown(input, { key: "?" })).toBe(true);
    expect(onhelp).not.toHaveBeenCalled();
  });

  it("has a ? button", async () => {
    const { onhelp } = mountTile();
    await fireEvent.click(screen.getByRole("button", { name: "Help" }));
    expect(onhelp).toHaveBeenCalledOnce();
  });

  it("lists the table's rows and closes on Esc, returning focus", async () => {
    const opener = document.createElement("input");
    document.body.append(opener);
    opener.focus();
    const onclose = vi.fn();
    const { unmount } = render(Help, { area: { left: 0, top: 0, width: 800, height: 600 }, onclose });
    expect(document.activeElement?.textContent).toBe("Esc");
    const dialog = screen.getByRole("dialog", { name: "wayfarer help" });
    const rows = [...dialog.querySelectorAll("h3 + table tr")].map((tr) => [
      squash(tr.querySelector("th")!.textContent!),
      squash(tr.querySelector("td")!.textContent!),
    ]);
    expect(rows).toEqual(KEY_ROWS.map((r) => [squash(r.keys), squash(r.does)]));
    await fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onclose).toHaveBeenCalledOnce();
    unmount();
    expect(document.activeElement).toBe(opener);
  });
});
