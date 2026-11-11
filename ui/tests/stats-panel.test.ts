// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import StatsPanel from "../src/StatsPanel.svelte";
import {
  age,
  combatLines,
  combatSummary,
  count,
  duration,
  efficiency,
  perCent,
  sparkline,
  stashLines,
  startedAt,
  tenths,
  timeLines,
  type Stats,
} from "../src/lib/stats";
import type { CharacterState, TimeSpent } from "../src/proto";

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

  it("resets the run from its own button, shown only while there is one", async () => {
    const onreset = vi.fn();
    const { container, unmount } = render(StatsPanel, {
      session,
      character,
      stats,
      attached: true,
      onrefresh: () => {},
      onreset,
    });
    // No combat numbers yet: the run's is the only reset.
    expect(screen.getAllByRole("button", { name: "reset" })).toHaveLength(1);
    await fireEvent.click(container.querySelector('[data-reset="run"]')!);
    expect(onreset).toHaveBeenCalledWith("a", "run");
    unmount();
    const none = render(StatsPanel, {
      session,
      character: {},
      stats: undefined,
      attached: true,
      onrefresh: () => {},
      onreset,
    });
    expect(none.queryAllByRole("button", { name: "reset" })).toHaveLength(0);
  });

  it("leaves the monster rules to the settings tab", () => {
    const { container } = render(StatsPanel, { session, character, stats, attached: true, onrefresh: () => {} });
    expect(container.querySelector(".rules")).toBeNull();
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

describe("the combat numbers", () => {
  afterEach(cleanup);

  const combat: NonNullable<Stats["combat"]> = {
    since: "2026-10-24T10:00:00Z",
    swings: 20,
    accuracy_per_mille: 800,
    crit_rate_per_mille: 125,
    rows: [
      { kind: "hit", count: 12, per_mille: 600, min: 21, max: 84, avg_tenths: 525 },
      { kind: "crit", count: 1, per_mille: 50, min: 120, max: 120, avg_tenths: 1200 },
      { kind: "backstab", count: 2, per_mille: 100, min: 60, max: 90, avg_tenths: 750 },
    ],
    misses: 4,
    dodged: 1,
    glances: 1,
    backstab_tries: 3,
    backstab_crits: 1,
    cast_misses: 0,
    cast_accuracy_per_mille: 0,
    round: { kind: "round", count: 14, per_mille: 0, min: 0, max: 150, avg_tenths: 481 },
    kills: 5,
    flees: 1,
    lowest_hp: 41,
    weaves_resisted: 0,
    spell_kills: 0,
    intercepts: 0,
    no_weave: [],
  };

  it("formats rates, tenths and ages", () => {
    expect(perCent(714)).toBe("71.4%");
    expect(tenths(525)).toBe("52.5");
    const now = new Date("2026-10-24T13:12:00Z");
    expect(age("2026-10-24T10:00:00Z", now)).toBe("3h 12m");
    expect(age("2026-10-24T13:00:00Z", now)).toBe("12m");
    expect(age("2026-10-22T10:00:00Z", now)).toBe("2d 3h");
  });

  it("lists MegaMud's rows, leaving out those with nothing counted", () => {
    const l = combatLines(combat);
    expect(l.map((x) => x.label)).toEqual(["miss", "hit", "crit", "bs", "glance", "round"]);
    expect(l[0]).toMatchObject({ count: "4", share: "20.0%", note: "1 dodged" });
    expect(l[1]).toMatchObject({ count: "12", share: "60.0%", range: "21-84", avg: "52.5" });
    expect(l[3].note).toBe("2/3 landed, 1 crit");
    expect(l[5]).toMatchObject({ count: "14", share: "", range: "0-150", avg: "48.1" });
    // No cast row without casts.
    expect(l.some((x) => x.label === "cast")).toBe(false);
  });

  it("adds Quicksilver 1.20's openings, weaves, spell kills and intercepts", () => {
    const qs = {
      ...combat,
      rows: [
        ...combat.rows,
        { kind: "opening", count: 2, per_mille: 153, min: 23, max: 31, avg_tenths: 270 },
        { kind: "weave", count: 3, per_mille: 230, min: 0, max: 75, avg_tenths: 383 },
      ],
      weaves_resisted: 1,
      spell_kills: 2,
      intercepts: 4,
      no_weave: ["ghost"],
    };
    const l = combatLines(qs);
    expect(l.map((x) => x.label)).toEqual(["miss", "hit", "crit", "bs", "opening", "weave", "glance", "round"]);
    expect(l[4]).toMatchObject({ count: "2", share: "15.3%", range: "23-31", note: "per hit" });
    expect(l[5]).toMatchObject({ count: "3", share: "23.0%", note: "1 resisted" });
    expect(combatSummary(qs)).toBe(
      "killed 5 (2 by spell) · fled 1 · took 4 for the ward · lowest hp 41 · never woven into: ghost",
    );
    expect(combatSummary(combat)).toBe("killed 5 · fled 1 · lowest hp 41");
  });

  it("shows the table under the run, and says when there is nothing yet", () => {
    const { container, unmount } = render(StatsPanel, {
      session,
      character,
      stats: { ...stats, combat },
      attached: true,
      onrefresh: () => {},
    });
    const section = container.querySelector(".combat")!;
    expect(section.textContent).toContain("20 swings · 80.0% accuracy · 12.5% crits");
    expect(section.querySelectorAll(".row:not(.head)")).toHaveLength(6);
    expect(section.textContent).toContain("killed 5 · fled 1");
    expect(section.textContent).toContain("lowest hp 41");
    unmount();
    const none = render(StatsPanel, { session, character, stats, attached: true, onrefresh: () => {} });
    expect(none.container.textContent).toContain("No combat numbers since the last reset.");
  });

  it("resets the combat numbers apart from the run", async () => {
    const onreset = vi.fn();
    const { container } = render(StatsPanel, {
      session,
      character,
      stats: { ...stats, combat },
      attached: true,
      onrefresh: () => {},
      onreset,
    });
    expect(screen.getAllByRole("button", { name: "reset" })).toHaveLength(2);
    await fireEvent.click(container.querySelector('.combat [data-reset="combat"]')!);
    expect(onreset).toHaveBeenCalledExactlyOnceWith("a", "combat");
  });
});

describe("where the time went", () => {
  afterEach(cleanup);

  const time: TimeSpent = {
    fight_secs: 1200,
    flee_secs: 0,
    rest_secs: 600,
    walk_secs: 420,
    upkeep_secs: 60,
    wait_secs: 300,
    paused_secs: 120,
    away_secs: 0,
  };

  it("formats durations", () => {
    expect(duration(45)).toBe("45s");
    expect(duration(725)).toBe("12m 5s");
    expect(duration(4680)).toBe("1h 18m");
    expect(duration(1200)).toBe("20m");
  });

  it("lists the kinds biggest first, leaving out those with none", () => {
    const l = timeLines(time);
    expect(l.map((x) => x.kind)).toEqual(["fight", "rest", "walk", "wait", "paused", "upkeep"]);
    expect(l[0]).toMatchObject({ label: "fighting", pct: 44, secs: 1200 });
    expect(l.reduce((s, x) => s + x.share, 0)).toBeCloseTo(100);
    expect(timeLines({ ...time, fight_secs: 0, rest_secs: 0, walk_secs: 0, upkeep_secs: 0, wait_secs: 0, paused_secs: 0 })).toEqual([]);
  });

  it("works out kills an hour, exp a minute fighting and the time per kill", () => {
    // 2,580 s playing (paused left out), 12 kills, 3,000 exp.
    const e = efficiency(time, 12, 3000, 2700 + 600);
    expect(e.killsPerHour).toBe("17");
    expect(e.expPerFightMinute).toBe("150");
    expect(e.perKill).toEqual({ fight: "1m 40s", rest: "50s", between: "1m" });
    expect(e.unseen).toBe("10m");
    const none = efficiency(time, 0, 0, 2700);
    expect(none.perKill).toBeUndefined();
    expect(none.unseen).toBeUndefined();
  });

  it("shows the run's age, the breakdown and the efficiency under the run", () => {
    const { container } = render(StatsPanel, {
      session,
      character,
      stats: { ...stats, time },
      attached: true,
      onrefresh: () => {},
    });
    expect(container.querySelector(".current .since")!.textContent).toMatch(/run since .+ \(\d/);
    const section = container.querySelector(".time")!;
    expect(section.querySelectorAll(".bar .seg")).toHaveLength(6);
    expect(section.querySelector('[data-time="fight"]')!.textContent).toBe("fighting44%20m");
    expect(section.querySelector('[data-eff="rates"]')!.textContent!.trim()).toBe(
      "10 kills/hr playing · 16 exp a minute fighting",
    );
    expect(section.querySelector('[data-eff="per-kill"]')!.textContent).toContain("per kill:");
    expect(section.querySelector('[data-eff="rates"]')!.textContent).toContain("kills/hr playing");
  });

  it("has no time section without a run, and says when nothing is counted yet", () => {
    const { container, unmount } = render(StatsPanel, {
      session,
      character: {},
      stats: { ...stats, time },
      attached: true,
      onrefresh: () => {},
    });
    expect(container.querySelector(".time")).toBeNull();
    unmount();
    const zero = render(StatsPanel, {
      session,
      character,
      stats: { ...stats, time: { ...time, fight_secs: 0, rest_secs: 0, walk_secs: 0, upkeep_secs: 0, wait_secs: 0, paused_secs: 0 } },
      attached: true,
      onrefresh: () => {},
    });
    expect(zero.container.querySelector(".time")!.textContent).toContain("Nothing counted yet.");
  });
});

describe("the coins stashed", () => {
  afterEach(cleanup);

  const stashed = [
    { room: "15:12", gold: 1200, silver: 40, copper: 300 },
    { room: "1:2150", gold: 76, silver: 0, copper: 6 },
  ];

  it("sums the rooms and leaves out the coins with none", () => {
    expect(stashLines(undefined)).toBeUndefined();
    expect(stashLines([{ room: "15:12", gold: 0, silver: 0, copper: 0 }])).toBeUndefined();
    expect(stashLines(stashed)).toEqual({
      total: "1,276g 40s 306c",
      rooms: [
        { room: "15:12", coins: "1,200g 40s 300c" },
        { room: "1:2150", coins: "76g 6c" },
      ],
    });
  });

  it("shows the run's stashes above the combat numbers, and says when there are none", () => {
    const { container, unmount } = render(StatsPanel, {
      session,
      character,
      stats: { ...stats, stashed },
      attached: true,
      onrefresh: () => {},
    });
    const section = container.querySelector(".stash")!;
    expect(section.querySelector('[data-stash="total"]')!.textContent!.trim()).toBe(
      "stashed this run: 1,276g 40s 306c",
    );
    expect(section.querySelector('[data-stash-room="15:12"]')!.textContent).toBe("in 15:121,200g 40s 300c");
    const sections = [...container.querySelectorAll("section")].map((s) => s.className.split(" ")[0]);
    expect(sections.indexOf("stash")).toBeLessThan(sections.indexOf("combat"));
    unmount();
    const none = render(StatsPanel, { session, character, stats, attached: true, onrefresh: () => {} });
    expect(none.container.querySelector('[data-stash="none"]')!.textContent).toBe("nothing stashed this run");
    none.unmount();
    const norun = render(StatsPanel, {
      session,
      character: {},
      stats: { ...stats, stashed },
      attached: true,
      onrefresh: () => {},
    });
    expect(norun.container.querySelector(".stash")).toBeNull();
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
