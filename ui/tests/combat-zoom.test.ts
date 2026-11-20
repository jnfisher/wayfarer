// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import CombatZoom from "../src/CombatZoom.svelte";
import {
  bin,
  binWidth,
  detailKey,
  foeLines,
  histChoices,
  median,
  partyLine,
  type CombatDetailMessage,
} from "../src/lib/combat-zoom";
import type { CombatStats, CombatWindow } from "../src/proto";

const base: CombatStats = {
  since: "2026-11-20T10:00:00Z",
  swings: 0,
  accuracy_per_mille: 0,
  crit_rate_per_mille: 0,
  rows: [],
  misses: 0,
  dodged: 0,
  glances: 0,
  backstab_tries: 0,
  backstab_crits: 0,
  cast_misses: 0,
  cast_accuracy_per_mille: 0,
  kills: 12,
  flees: 0,
  weaves_resisted: 0,
  spell_kills: 0,
  intercepts: 0,
  no_weave: [],
  attacks: [],
  monsters: [],
};

const blow = { crits: 0, dodged: 0, glances: 0, resisted: 0, mana: 0, per_mana_hundredths: 0 };

const sasha: CombatStats = {
  ...base,
  round: { kind: "round", count: 400, per_mille: 0, min: 0, max: 310, avg_tenths: 1106 },
  attacks: [
    { ...blow, kind: "swing", tries: 300, hits: 210, misses: 90, min: 4, max: 40, avg_tenths: 190 },
    { ...blow, kind: "backstab", tries: 50, hits: 40, crits: 10, misses: 10, min: 80, max: 260, avg_tenths: 1500 },
  ],
  taken: { blows: 200, hits: 50, misses: 100, dodged: 30, deflected: 20, min: 3, max: 40, avg_tenths: 120 },
  monsters: [
    { name: "salamander", kills: 10, exp: 15000, rounds: 40, dealt: 4400, taken: 300, blows: 80, hits: 20 },
    { name: "fire lizard", kills: 2, exp: 800, rounds: 6, dealt: 500, taken: 90, blows: 12, hits: 6 },
  ],
};

function message(session: string, window: CombatWindow, stats?: CombatStats): CombatDetailMessage {
  return {
    type: "combat_detail",
    session,
    window,
    stats,
    hists: stats
      ? [
          { kind: "taken", values: [[3, 10], [40, 2]] },
          { kind: "round", values: [[0, 20], [100, 50], [110, 30], [310, 1]] },
          { kind: "backstab", values: [[80, 10], [150, 30]] },
        ]
      : [],
  };
}

describe("the combat zoom helpers", () => {
  it("bins damage into bars of a round width, none left out between", () => {
    expect(binWidth(10, 24)).toBe(1);
    expect(binWidth(311, 24)).toBe(20);
    expect(binWidth(1500, 24)).toBe(100);
    const bars = bin([[3, 2], [5, 1], [61, 4]], 24);
    expect(bars[0]).toEqual({ lo: 0, hi: 4, n: 2 });
    expect(bars.at(-1)).toEqual({ lo: 60, hi: 64, n: 4 });
    expect(bars.reduce((s, b) => s + b.n, 0)).toBe(7);
    expect(bars.every((b, i) => i === 0 || b.lo === bars[i - 1].hi + 1)).toBe(true);
    expect(bin([])).toEqual([]);
  });

  it("finds the median of a histogram", () => {
    expect(median([[0, 20], [100, 50], [110, 30]])).toBe(100);
    expect(median([])).toBeUndefined();
  });

  it("orders the histograms: the round, every hit, the attacks, taken", () => {
    const c = histChoices([
      { kind: "taken", values: [[1, 1]] },
      { kind: "cast", name: "god's wrath", values: [[1, 1]] },
      { kind: "round", values: [[1, 1]] },
    ]);
    expect(c.map((x) => x.label)).toEqual(["damage a round", "god's wrath hits", "blows taken"]);
    expect(c[1].key).toBe("cast:god's wrath");
  });

  it("works out each monster's rounds a kill, damage each way and exp a kill", () => {
    const l = foeLines(sasha.monsters);
    expect(l.map((f) => f.name)).toEqual(["salamander", "fire lizard"]);
    expect(l[0]).toMatchObject({
      kills: "10",
      roundsPerKill: "4.0",
      dealtPerRound: "110.0",
      takenPerRound: "7.5",
      hitsYou: "25.0%",
      expPerKill: "1,500",
      few: false,
    });
    expect(l[1].few).toBe(true);
  });

  it("lines a party member up, blank without numbers", () => {
    expect(partyLine("s", "Sasha", sasha)).toMatchObject({
      round: "110.6",
      swingHit: "70.0%",
      bsHit: "80.0%",
      bsAvg: "150.0",
      takenHit: "25.0%",
      kills: "12",
      few: false,
    });
    expect(partyLine("r", "Rocco", undefined)).toMatchObject({ round: "", bsHit: "", kills: "", few: true });
  });
});

describe("the combat zoom", () => {
  afterEach(cleanup);
  const party = [
    { id: "r", name: "Rocco" },
    { id: "s", name: "Sasha" },
  ];

  it("asks for every party member's numbers, and again for another window", async () => {
    const onrequest = vi.fn();
    render(CombatZoom, { party, initial: "s", details: {}, onrequest, onclose: () => {} });
    expect(onrequest.mock.calls).toEqual([
      ["r", "reset"],
      ["s", "reset"],
    ]);
    expect(screen.getByText("Asking the brain...")).toBeTruthy();
    onrequest.mockClear();
    await fireEvent.click(screen.getByRole("button", { name: "last hour" }));
    expect(onrequest.mock.calls).toEqual([
      ["r", "hour"],
      ["s", "hour"],
    ]);
  });

  it("shows the histogram, the attacks, the monsters and the party", async () => {
    const details = {
      [detailKey("s", "reset")]: message("s", "reset", sasha),
      [detailKey("r", "reset")]: message("r", "reset", { ...base, kills: 3 }),
    };
    const { container } = render(CombatZoom, {
      party,
      initial: "s",
      details,
      onrequest: () => {},
      onclose: () => {},
    });
    expect(container.querySelector(".tab")!.textContent).toBe("combat · Sasha");
    const hist = container.querySelector(".hist")!;
    expect(hist.getAttribute("data-hist")).toBe("round");
    expect(container.textContent).toContain("101 in all, median 100");
    expect(container.querySelectorAll("[data-attack]")).toHaveLength(2);
    expect(container.querySelector('[data-foe="salamander"]')!.textContent).toContain("1,500");
    expect(container.textContent).toContain("took 200 blows");
    expect(container.querySelector('[data-party="Sasha"]')!.classList.contains("on")).toBe(true);
    // Another histogram.
    const select = screen.getByRole("combobox", { name: "Histogram" }) as HTMLSelectElement;
    await fireEvent.change(select, { target: { value: "backstab" } });
    expect(container.querySelector(".hist")!.getAttribute("data-hist")).toBe("backstab");
    // Another party member.
    await fireEvent.click(screen.getByRole("button", { name: "Rocco" }));
    expect(container.querySelector(".tab")!.textContent).toBe("combat · Rocco");
  });

  it("says when nothing was counted in the window, and closes", async () => {
    const onclose = vi.fn();
    render(CombatZoom, {
      party: [{ id: "s", name: "Sasha" }],
      initial: "s",
      details: { [detailKey("s", "reset")]: message("s", "reset") },
      onrequest: () => {},
      onclose,
    });
    expect(screen.getByText("Nothing counted in this window.")).toBeTruthy();
    expect(screen.queryByText(/the party/)).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "close" }));
    expect(onclose).toHaveBeenCalledOnce();
  });
});
