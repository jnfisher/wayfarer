// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import StatsPanel from "../src/StatsPanel.svelte";
import { count, sparkline, startedAt, type Stats } from "../src/lib/stats";
import type { CharacterState } from "../src/proto";

const session = { id: "a", name: "Aria", server: "silvermere", state: "realm", last_seq: 0 } as never;

const character: CharacterState = {
  run: {
    run_started_at: "2026-10-19T12:00:00Z",
    exp_per_hour: 1240,
    kills: 7,
    deaths: 0,
    flees: 1,
    rests: 2,
    exp_gained: 310,
  },
};

const stats: Stats = {
  type: "stats",
  session: "a",
  runs: [
    {
      started_at: "2026-10-19T12:00:00Z",
      minutes: 15,
      exp_per_hour: 1240,
      kills: 7,
      deaths: 0,
      flees: 1,
      rests: 2,
      exp_gained: 310,
    },
    {
      started_at: "2026-10-19T10:00:00Z",
      ended_at: "2026-10-19T10:30:00Z",
      loop_id: "1:2140:8:doors:0",
      minutes: 30,
      exp_per_hour: 900,
      kills: 12,
      deaths: 1,
      flees: 3,
      rests: 4,
      exp_gained: 450,
    },
  ],
  samples: [
    { at: "2026-10-19T12:01:00Z", exp: 1000, hp: 30 },
    { at: "2026-10-19T12:02:00Z", exp: 1050, hp: 28 },
    { at: "2026-10-19T12:03:00Z", exp: 1100 },
  ],
};

const rows = (c: HTMLElement) =>
  [...c.querySelectorAll(".list .row:not(.head)")].map((r) =>
    [...r.children].map((s) => s.textContent!.trim()).join(" "),
  );

describe("the stats tab", () => {
  afterEach(cleanup);

  it("shows the current run large and the last runs as rows", async () => {
    const onrefresh = vi.fn();
    const { container } = render(StatsPanel, { session, character, stats, attached: true, onrefresh });
    const big = (k: string) => container.querySelector(`[data-stat="${k}"]`)!.textContent;
    expect(big("exp_per_hour")).toBe("1,240");
    expect(big("kills")).toBe("7");
    expect(big("flees")).toBe("1");
    expect(big("exp_gained")).toBe("310");
    expect(container.querySelector(".spark")!.textContent).toBe("▁▅█");
    const r = rows(container);
    expect(r).toHaveLength(2);
    expect(r[0]).toMatch(/ here 15 1,240 7 0 1$/);
    expect(r[1]).toMatch(/ 1:2140:8:doors:0 30 900 12 1 3$/);
    expect(container.querySelectorAll(".row.live")).toHaveLength(1);
    await fireEvent.click(screen.getByRole("button", { name: "refresh" }));
    expect(onrefresh).toHaveBeenCalledWith("a");
  });

  it("says when there is no run, no rows or no brain", () => {
    const { container, unmount } = render(StatsPanel, {
      session,
      character: {},
      stats: undefined,
      attached: true,
      onrefresh: () => {},
    });
    expect(container.textContent).toContain("No run: Start the bot (F10)");
    expect(container.textContent).toContain("No runs yet.");
    unmount();
    const off = render(StatsPanel, { session, character, stats, attached: false, onrefresh: () => {} });
    expect(off.container.textContent).toContain("No brain attached");
  });
});

describe("the stats helpers", () => {
  it("draws exp over time scaled to the highest gain", () => {
    expect(sparkline([])).toBe("");
    expect(sparkline([{ at: "", exp: 5 }])).toBe("");
    expect(sparkline([{ at: "", exp: 5 }, { at: "" }, { at: "", exp: 5 }])).toBe("▁▁");
    expect(sparkline([0, 10, 20, 70].map((exp) => ({ at: "", exp })))).toBe("▁▂▃█");
    expect(sparkline([0, 1, 2, 3].map((exp) => ({ at: "", exp })), 2)).toBe("▁█");
  });

  it("formats times and counts", () => {
    const local = new Date(2026, 9, 19, 9, 5);
    expect(startedAt(local.toISOString(), new Date(2026, 9, 19, 23, 0))).toBe("09:05");
    expect(startedAt(local.toISOString(), new Date(2026, 9, 20, 1, 0))).toBe("10/19 09:05");
    expect(startedAt("not a time")).toBe("not a time");
    expect(count(1234567)).toBe("1,234,567");
  });
});
