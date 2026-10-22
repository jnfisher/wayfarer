// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import SettingsPanel from "../src/SettingsPanel.svelte";
import { groupRows, numberChange, resetChange, show, sourceLabel, type SettingsMessage } from "../src/lib/settings";
import type { SettingRow } from "../src/proto";

const session = { id: "a", name: "Aria", server: "silvermere", state: "realm", last_seq: 0 } as never;

const flee: SettingRow = {
  key: "health.flee_below_hp",
  label: "Flee below",
  help: "Leave the fight and run when HP falls below this.",
  group: "health",
  tier: "essential",
  kind: { kind: "int", min: 0, max: 90 },
  unit: "%",
  default: 25,
  value: 35,
  source: "character",
  own: 35,
  megamud: "HP Run If Below",
};
const heal: SettingRow = {
  key: "spells.heal",
  label: "Healing spells",
  help: "Auto or off.",
  group: "spells",
  tier: "essential",
  kind: { kind: "choice", options: ["auto", "off"] },
  unit: "",
  default: "auto",
  value: "auto",
  source: "default",
};
const gold: SettingRow = {
  key: "cash.gold",
  label: "Pick up gold",
  help: "Gold crowns.",
  group: "cash",
  tier: "essential",
  kind: { kind: "bool" },
  unit: "",
  default: false,
  value: false,
  source: "default",
};
const leg: SettingRow = {
  key: "bot.leg_tries",
  label: "Tries to reach a stop",
  help: "Walks.",
  group: "bot",
  tier: "advanced",
  kind: { kind: "int", min: 1, max: 10 },
  unit: "tries",
  default: 3,
  value: 3,
  source: "default",
};

const settings: SettingsMessage = {
  type: "settings",
  session: "a",
  layer: "character",
  files: [{ layer: "character", path: "/data/characters/a.toml", exists: true }],
  rows: [flee, heal, gold, leg],
};

describe("the settings rows", () => {
  it("are grouped in the forms' order, advanced ones only on request", () => {
    expect(groupRows(settings.rows, false).map((g) => g.id)).toEqual(["health", "spells", "cash"]);
    expect(groupRows(settings.rows, true).map((g) => g.id)).toEqual(["health", "spells", "cash", "bot"]);
  });

  it("check a number against the range before it is sent", () => {
    expect(numberChange(flee, "40")).toEqual({ key: "health.flee_below_hp", value: 40 });
    expect(numberChange(flee, " 40 ")).toEqual({ key: "health.flee_below_hp", value: 40 });
    for (const bad of ["", "abc", "4.5", "91", "-1"]) expect(numberChange(flee, bad)).toBeUndefined();
    expect(numberChange(heal, "1")).toBeUndefined();
  });

  it("say what they hold and where it comes from", () => {
    expect([show(35), show(true), show(false), show("auto")]).toEqual(["35", "on", "off", "auto"]);
    expect(sourceLabel("preset")).toBe("class & style");
    expect(resetChange(flee)).toEqual({ key: "health.flee_below_hp" });
  });
});

describe("the settings tab", () => {
  afterEach(cleanup);

  const render_ = (extra: Record<string, unknown> = {}) =>
    render(SettingsPanel, {
      session,
      settings,
      attached: true,
      onrequest: () => {},
      onsave: () => {},
      ...extra,
    });

  it("shows each setting with its value, its source and the file", () => {
    const { container } = render_();
    const row = (k: string) => container.querySelector(`[data-key="${k}"]`)!;
    expect((row("health.flee_below_hp").querySelector("input") as HTMLInputElement).value).toBe("35");
    expect(row("health.flee_below_hp").textContent).toContain("set here");
    expect(row("spells.heal").textContent).toContain("default");
    expect(container.textContent).toContain("/data/characters/a.toml");
    // The advanced setting is hidden until "all".
    expect(container.querySelector('[data-key="bot.leg_tries"]')).toBeNull();
  });

  it("shows the advanced settings with 'all'", async () => {
    const { container } = render_();
    await fireEvent.click(screen.getByLabelText("all"));
    expect(container.querySelector('[data-key="bot.leg_tries"]')).not.toBeNull();
  });

  it("saves a number, a switch and a word to the layer being edited", async () => {
    const onsave = vi.fn();
    const { container } = render_({ onsave });
    const input = container.querySelector('[data-key="health.flee_below_hp"] input') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "40" } });
    await fireEvent.change(input);
    expect(onsave).toHaveBeenLastCalledWith("a", "character", [{ key: "health.flee_below_hp", value: 40 }]);
    await fireEvent.click(container.querySelector('[data-key="cash.gold"] input') as HTMLInputElement);
    expect(onsave).toHaveBeenLastCalledWith("a", "character", [{ key: "cash.gold", value: true }]);
    const select = container.querySelector('[data-key="spells.heal"] select') as HTMLSelectElement;
    await fireEvent.change(select, { target: { value: "off" } });
    expect(onsave).toHaveBeenLastCalledWith("a", "character", [{ key: "spells.heal", value: "off" }]);
  });

  it("sends nothing for a number out of range and marks the box", async () => {
    const onsave = vi.fn();
    const { container } = render_({ onsave });
    const input = container.querySelector('[data-key="health.flee_below_hp"] input') as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "200" } });
    await fireEvent.change(input);
    expect(onsave).not.toHaveBeenCalled();
    expect(input.classList.contains("bad")).toBe(true);
  });

  it("resets a value the file sets, and only those", async () => {
    const onsave = vi.fn();
    const { container } = render_({ onsave });
    expect(container.querySelector('[data-key="spells.heal"] .reset')).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "reset Flee below" }));
    expect(onsave).toHaveBeenCalledWith("a", "character", [{ key: "health.flee_below_hp" }]);
  });

  it("asks for another layer's file", async () => {
    const onrequest = vi.fn();
    render_({ onrequest });
    await fireEvent.click(screen.getByRole("button", { name: "my defaults" }));
    expect(onrequest).toHaveBeenCalledWith("a", "mine");
  });

  it("says when there is no brain or no answer yet", () => {
    const off = render_({ attached: false });
    expect(off.container.textContent).toContain("No brain attached");
    off.unmount();
    const wait = render_({ settings: undefined });
    expect(wait.container.textContent).toContain("Asking the brain");
  });
});
