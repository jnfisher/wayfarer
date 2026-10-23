// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { botButtons, botKeyAction, botWidth, errandItems, linkButton, narrationLine } from "../src/lib/bot";
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
import type { SpellPlan } from "../src/lib/spells";

const stopped: Bot = { state: "stopped", mode: "idle", narration: "" };
const running: Bot = { state: "running", mode: "fight", narration: "Fight: large giant rat" };
const paused: Bot = { state: "paused", mode: "fight", narration: "Paused: you took over", reason: "you took over" };

const looping: Bot = {
  ...running,
  task: { loop_id: "1:2140:8:doors:0", loop_name: "Newhaven", stop: 0, stops: [], skip: [] },
};
const going: Bot = { ...running, journey: { name: "Newhaven, Arena", to: { map: 1, room: 2150 } } };
const pausedGoing: Bot = { ...going, state: "paused", narration: "Paused: asked to pause" };

describe("the bot's controls", () => {
  it("are ► when off; ║ when on; and ■ beside either with a loop or a Go to", () => {
    const actions = (b?: Bot) => botButtons(b).map((x) => `${x.action} ${x.label}`);
    expect(actions(stopped)).toEqual(["start ►"]);
    expect(actions(running)).toEqual(["pause ║"]);
    expect(actions(looping)).toEqual(["pause ║", "stop ■"]);
    expect(actions(going)).toEqual(["pause ║", "stop ■"]);
    expect(actions(pausedGoing)).toEqual(["resume ►", "stop ■"]);
    expect(actions(paused)).toEqual(["resume ►"]);
    expect(actions(undefined)).toEqual([]);
    expect([botWidth(stopped), botWidth(running), botWidth(going), botWidth(undefined)]).toEqual([6, 6, 9, 0]);
  });

  it("say what ║ does: pause the route, or turn the bot off", () => {
    expect(botButtons(going)[0].title).toMatch(/^Pause the loop or Go to/);
    expect(botButtons(running)[0].title).toMatch(/^Turn the bot off/);
  });

  it("narrate while running or paused", () => {
    expect(narrationLine(running)).toBe("Fight: large giant rat");
    expect(narrationLine(paused)).toBe("Paused: you took over");
    expect(narrationLine(stopped)).toBeNull();
    expect(narrationLine(undefined)).toBeNull();
  });

  it("F9 is ║ and ►, F10 is ■", () => {
    expect(botKeyAction("F9", running)).toBe("pause");
    expect(botKeyAction("F9", paused)).toBe("resume");
    expect(botKeyAction("F9", stopped)).toBe("start");
    expect(botKeyAction("F10", stopped)).toBe("start");
    expect(botKeyAction("F10", running)).toBe("stop");
    expect(botKeyAction("F10", going)).toBe("stop");
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

  it("minimizes with its button and is hidden while minimized", async () => {
    const onminimize = vi.fn();
    const props = { info, number: 1, focused: false, zoom: 0, offline: null, client, sinks, onfocus() {}, onhelp() {}, onminimize };
    const r = render(Tile, { ...props, character: null });
    await fireEvent.click(screen.getByRole("button", { name: "Minimize Waydeva" }));
    expect(onminimize).toHaveBeenCalledOnce();
    expect(r.container.querySelector("section.tile.hidden")).toBeNull();
    await r.rerender({ ...props, character: null, hidden: true });
    expect(r.container.querySelector("section.tile.hidden")).not.toBeNull();
    expect(r.container.querySelector("section.tile")?.getAttribute("data-session")).toBe("a");
  });

  it("has an errand menu in the realm that starts an errand, and shows its progress", async () => {
    botControl.mockClear();
    tile({ bot: stopped });
    expect(screen.queryByRole("menu", { name: "Errands" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Errands" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Pick up all" }));
    expect(botControl).toHaveBeenCalledWith("a", "pick_up_all");
    expect(screen.queryByRole("menu", { name: "Errands" })).toBeNull();
    cleanup();
    const el = tile({ bot: stopped, errand: "Equip: wear padded vest" });
    expect(el.querySelector(".controls + .narration.errand")?.textContent).toBe("Equip: wear padded vest");
    await fireEvent.click(screen.getByRole("button", { name: "Errands" }));
    expect((screen.getByRole("menuitem", { name: "Equip all" }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Cancel errand" }));
    expect(botControl).toHaveBeenLastCalledWith("a", "cancel_errand");
  });

  it("offers the errands only while the bot is not running, and the spell plan always", () => {
    expect(errandItems(stopped, undefined).map((i) => [i.label, i.disabled])).toEqual([
      ["Pick up all", false],
      ["Equip all", false],
      ["Train", false],
      ["Recover death pile", true],
      ["Spell plan…", false],
    ]);
    expect(errandItems(running, undefined).map((i) => i.disabled)).toEqual([true, true, true, true, false]);
    expect(errandItems(paused, "Pick up: get torch").map((i) => i.label)).toEqual([
      "Pick up all",
      "Equip all",
      "Train",
      "Recover death pile",
      "Spell plan…",
      "Cancel errand",
    ]);
    // Recovering the pile needs a death on record.
    const died = [{ at: "2026-10-22T12:00:00Z", kind: "death", map: 1, room: 1217, name: "Slum Street" }];
    expect(errandItems(stopped, undefined, died).find((i) => i.action === "recover_pile")?.disabled).toBe(false);
  });

  it("shows the spell plan the brain sends after Spell plan… is picked", async () => {
    const requestSpellPlan = vi.fn();
    const planClient = { sendLine: vi.fn(), sendKeys: vi.fn(), botControl, requestSpellPlan } as never;
    const props = { info, number: 1, focused: false, zoom: 0, offline: null, client: planClient, sinks, onfocus() {}, onhelp() {} };
    const r = render(Tile, { ...props, character: { bot: stopped } });
    await fireEvent.click(screen.getByRole("button", { name: "Errands" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Spell plan…" }));
    expect(requestSpellPlan).toHaveBeenCalledWith("a");
    expect(screen.getByText("Asking the brain…")).toBeTruthy();
    const plan: SpellPlan = {
      type: "spell_plan",
      session: "a",
      cash: 500,
      items: [
        { spell: "illuminate", spell_level: 2, scroll: "scroll of illuminate", shop: "Newhaven Spell Shop", map: 1, room: 2144, price: 400, steps: 12, usable: true, affordable: true },
        { spell: "mend", spell_level: 3, scroll: "scroll of mend", shop: "Newhaven Spell Shop", map: 1, room: 2144, price: 800, steps: 12, usable: true, affordable: false },
        { spell: "firebolt", spell_level: 9, scroll: "scroll of firebolt", shop: "Mage Spell Shop", map: 1, room: 2200, price: 5000, usable: false, affordable: false },
      ],
    };
    await r.rerender({ ...props, character: { bot: stopped }, plan });
    const rows = [...document.querySelectorAll(".plan li")].map((li) => li.textContent);
    expect(rows).toEqual([
      "✓ illuminate: 4 gold, Newhaven Spell Shop, 12 steps",
      "  mend: 8 gold, Newhaven Spell Shop, 12 steps",
      "  firebolt (needs level 9): 50 gold, Mage Spell Shop, no way there",
    ]);
    expect(document.querySelector(".plan .totals")?.textContent).toBe(
      "With 5 gold on hand: 1 of 2 usable now (4 gold); all 2 cost 12 gold",
    );
    // Buying goes through the errand; Spell plan itself only shows.
    await fireEvent.click(screen.getByRole("button", { name: "buy spells" }));
    expect(botControl).toHaveBeenLastCalledWith("a", "buy_spells");
    await fireEvent.click(screen.getByRole("button", { name: "Close the spell plan" }));
    expect(screen.queryByRole("region", { name: "Spell plan" })).toBeNull();
  });

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
    const el = tile({ bot: going });
    expect(el.querySelector(".controls + .narration")?.textContent).toBe("Fight: large giant rat");
    expect([...el.querySelectorAll(".controls .ctl:not(.link):not([aria-label=Errands])")].map((b) => b.textContent)).toEqual(["[║]", "[■]"]);
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
      expect(el.querySelector(".controls .ctl:not(.link):not([aria-label=Errands])")).toBeNull();
      cleanup();
    }
  });
});
