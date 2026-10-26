// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import GearPlanner from "../src/GearPlanner.svelte";
import {
  blowsText,
  changedWhatIf,
  defaultFilters,
  detailRequest,
  drText,
  emptyInputs,
  gearRefusal,
  GEAR_BUFFS_KEY,
  GEAR_DEBOUNCE_MS,
  isBestMove,
  liveValues,
  loadBuffTicks,
  mayKeepSmash,
  missingText,
  monstersOf,
  nextSort,
  passes,
  pct,
  planRequest,
  rangeText,
  roundsText,
  sortRows,
  sourceText,
  statusClass,
  statusText,
  strText,
  tenths,
  visibleRows,
  type GearDetailMessage,
  type GearPlanMessage,
  type SortKey,
} from "../src/lib/gear";
import type { CharacterState, GearMove, GearMoveScore, GearRow } from "../src/proto";

function score(move: GearMove, kill1: number, kill2: number, rounds: number, dmg1: number): GearMoveScore {
  return {
    move,
    energy: 500,
    blows: [1, 2, 1],
    hit: 750,
    crit: 20,
    damage_round1: dmg1,
    kill_round1: kill1,
    kill_round2: kill2,
    rounds,
  };
}

function row(item: number, name: string, over: Partial<GearRow> = {}): GearRow {
  return {
    item,
    name,
    min_damage: 5,
    max_damage: 15,
    speed: 1200,
    str_req: 40,
    two_handed: false,
    status: "now",
    reasons: [],
    sources: [],
    moves: [],
    ...over,
  };
}

// A maul that bashes, a staff that beats it, a sword for later and a bow for never.
const MAUL = row(10, "maul", {
  min_damage: 8,
  max_damage: 30,
  speed: 1800,
  str_req: 70,
  str_short: 10,
  two_handed: true,
  sources: [
    { kind: "shop", name: "Weaponsmith", price: 1500 },
    { kind: "shop", name: "Armoury", price: 1200 },
  ],
  moves: [score("attack", 100, 400, 350, 150), score("bash", 300, 700, 250, 220)],
  best_move: "bash",
});
const STAFF = row(20, "darkwood staff", {
  min_level: 5,
  two_handed: true,
  sources: [{ kind: "drop", name: "minotaur champion", percent: 25, boss: true, first_kill: true }],
  limited: 3,
  moves: [score("attack", 200, 800, 220, 180), score("bash", 400, 850, 200, 250)],
  best_move: "bash",
  best: true,
});
const SWORD = row(30, "Magebane sword", {
  status: "later",
  reasons: ["level 15"],
  min_level: 15,
  sources: [{ kind: "quest", name: "huge dwarven witchunter" }],
  moves: [score("attack", 500, 900, 150, 300), score("bash", 450, 880, 170, 290)],
  best_move: "attack",
});
const BOW = row(40, "elven bow", { status: "never", reasons: ["class"], max_damage: 40 });
const ROWS = [MAUL, STAFF, SWORD, BOW];

const NAMES = (rows: GearRow[]) => rows.map((r) => r.name);

describe("the request", () => {
  const live = { level: 15, str: 95, int: 30, wil: 45, agi: 56, hea: 91, cha: 41, load_percent: 51 };

  it("sends a what-if only for the values changed from the live ones", () => {
    expect(changedWhatIf({}, live)).toBeUndefined();
    expect(changedWhatIf({ level: 15, str: 95 }, live)).toBeUndefined();
    expect(changedWhatIf({ level: 20, str: 95, load_percent: 54 }, live)).toEqual({ level: 20, load_percent: 54 });
    // A value the character has not shown yet is a what-if whatever it is.
    expect(changedWhatIf({ str: 60 }, {})).toEqual({ str: 60 });
  });

  it("leaves out what the brain defaults: its loop, no buffs, no smash", () => {
    expect(planRequest("kasimir", emptyInputs(), live)).toEqual({ session: "kasimir" });
    expect(
      planRequest(
        "kasimir",
        { monsters: { kind: "names", names: "minotaur, minotaur champion;" }, whatIf: { level: 20 }, buffs: ["valour"], smash: true },
        live,
      ),
    ).toEqual({
      session: "kasimir",
      monsters: { names: ["minotaur", "minotaur champion"] },
      what_if: { level: 20 },
      buffs: ["valour"],
      smash: true,
    });
  });

  it("names an area, or falls back to the loop when the field is empty", () => {
    expect(monstersOf({ kind: "area", area: " Dwarven Mines " })).toEqual({ area: "Dwarven Mines" });
    expect(monstersOf({ kind: "area", area: "  " })).toBeUndefined();
    expect(monstersOf({ kind: "names", names: " , " })).toBeUndefined();
    expect(monstersOf({ kind: "loop" })).toBeUndefined();
  });

  it("sends only the ticked buffs this character is offered", () => {
    const inputs = { ...emptyInputs(), buffs: ["bless", "song of valour"] };
    expect(planRequest("faye", inputs, live, ["song of valour"]).buffs).toEqual(["song of valour"]);
    expect(planRequest("faye", inputs, live, []).buffs).toBeUndefined();
  });

  it("asks for a detail with the same inputs and the item", () => {
    const inputs = { ...emptyInputs(), whatIf: { level: 20 } };
    expect(detailRequest("kasimir", 646, inputs, live)).toEqual({ session: "kasimir", what_if: { level: 20 }, item: 646 });
  });

  it("reads the live values from the snapshot, else from the plan's own inputs", () => {
    const c: CharacterState = {
      level: 12,
      stats: { strength: 80, intellect: 30, willpower: 40, agility: 50, health: 70, charm: 35 },
      encumbrance: { current: 520, max: 3792 },
    };
    expect(liveValues(c)).toEqual({ level: 12, str: 80, int: 30, wil: 40, agi: 50, hea: 70, cha: 35, load_percent: 13 });
    const plan = { inputs: { level: 12, load_percent: 40, what_if: [], buffs: [], smash: false } } as unknown as GearPlanMessage;
    expect(liveValues({ level: 12 }, plan)).toEqual({ level: 12, load_percent: 40 });
    // A what-if value is not the character's own.
    const whatIf = { inputs: { level: 20, what_if: ["level"], buffs: [], smash: false } } as unknown as GearPlanMessage;
    expect(liveValues({}, whatIf)).toEqual({});
  });

  it("keeps the ticks per character in localStorage, and survives junk", () => {
    expect(loadBuffTicks({ getItem: () => JSON.stringify({ faye: ["bless", 3], kasimir: "x" }) })).toEqual({ faye: ["bless"] });
    expect(loadBuffTicks({ getItem: () => "{nope" })).toEqual({});
    expect(loadBuffTicks(undefined)).toEqual({});
  });

  it("offers smash only to the classes that may keep it", () => {
    expect(mayKeepSmash("Witchunter")).toBe(true);
    expect(mayKeepSmash("warrior")).toBe(true);
    expect(mayKeepSmash("Druid")).toBe(false);
    expect(mayKeepSmash(undefined)).toBe(false);
  });
});

describe("sorting", () => {
  // Each column's first click: the kill chances and damage highest first, the rest lowest.
  const cases: [SortKey, string[]][] = [
    ["name", ["darkwood staff", "elven bow", "Magebane sword", "maul"]],
    ["damage", ["elven bow", "maul", "darkwood staff", "Magebane sword"]],
    ["speed", ["darkwood staff", "elven bow", "Magebane sword", "maul"]],
    ["str", ["darkwood staff", "elven bow", "Magebane sword", "maul"]],
    ["level", ["elven bow", "maul", "darkwood staff", "Magebane sword"]],
    ["hands", ["elven bow", "Magebane sword", "darkwood staff", "maul"]],
    ["status", ["darkwood staff", "maul", "Magebane sword", "elven bow"]],
    ["source", ["maul", "darkwood staff", "elven bow", "Magebane sword"]],
    ["attack.kill1", ["Magebane sword", "darkwood staff", "maul", "elven bow"]],
    ["attack.kill2", ["Magebane sword", "darkwood staff", "maul", "elven bow"]],
    ["attack.rounds", ["Magebane sword", "darkwood staff", "maul", "elven bow"]],
    ["attack.dmg1", ["Magebane sword", "darkwood staff", "maul", "elven bow"]],
    ["bash.kill1", ["Magebane sword", "darkwood staff", "maul", "elven bow"]],
  ];

  it.each(cases)("by %s", (key, want) => {
    const fresh = nextSort({ key: key === "status" ? "name" : "status", dir: 1 }, key);
    expect(NAMES(sortRows(ROWS, fresh))).toEqual(want);
  });

  it("rounds lowest first, and an unscored row last whichever way", () => {
    expect(NAMES(sortRows(ROWS, { key: "bash.rounds", dir: 1 }))).toEqual(["Magebane sword", "darkwood staff", "maul", "elven bow"]);
    expect(NAMES(sortRows(ROWS, { key: "bash.rounds", dir: -1 }))).toEqual(["maul", "darkwood staff", "Magebane sword", "elven bow"]);
    expect(NAMES(sortRows(ROWS, { key: "smash.kill1", dir: -1 }))).toEqual(NAMES(sortRows(ROWS, { key: "name", dir: 1 })));
  });

  it("a second click turns it round", () => {
    const first = nextSort({ key: "status", dir: 1 }, "bash.kill1");
    expect(first).toEqual({ key: "bash.kill1", dir: -1 });
    expect(nextSort(first, "bash.kill1")).toEqual({ key: "bash.kill1", dir: 1 });
    expect(nextSort({ key: "status", dir: 1 }, "speed")).toEqual({ key: "speed", dir: 1 });
  });

  it("pins the best row on top whatever the order", () => {
    expect(NAMES(visibleRows(ROWS, defaultFilters(), { key: "name", dir: -1 }))).toEqual([
      "darkwood staff",
      "maul",
      "Magebane sword",
      "elven bow",
    ]);
  });
});

describe("filters", () => {
  const only = (f: Partial<ReturnType<typeof defaultFilters>>) =>
    NAMES(ROWS.filter((r) => passes(r, { ...defaultFilters(), ...f })));

  it("usable now, now or later, or everything", () => {
    expect(only({ use: "now" })).toEqual(["maul", "darkwood staff"]);
    expect(only({ use: "usable" })).toEqual(["maul", "darkwood staff", "Magebane sword"]);
    expect(only({ use: "all" })).toEqual(["maul", "darkwood staff", "Magebane sword", "elven bow"]);
  });

  it("has a source hides what nothing sells, drops or gives", () => {
    expect(only({ hasSource: true })).toEqual(["maul", "darkwood staff", "Magebane sword"]);
  });

  it("a maximum price hides what the cheapest shop asks more for, and keeps what no shop sells", () => {
    expect(only({ maxPrice: 1000 })).toEqual(["darkwood staff", "Magebane sword", "elven bow"]);
    expect(only({ maxPrice: 1200 })).toEqual(["maul", "darkwood staff", "Magebane sword", "elven bow"]);
  });
});

describe("what a row says", () => {
  it("is green now, amber later with the reason, grey never with the reason", () => {
    expect([MAUL, SWORD, BOW].map(statusClass)).toEqual(["now", "later", "never"]);
    expect([MAUL, SWORD, BOW].map(statusText)).toEqual(["now", "level 15", "class"]);
    expect(statusText(row(1, "x", { status: "later", reasons: ["level 15", "alignment evil"] }))).toBe("level 15, alignment evil");
  });

  it("marks the row's best move", () => {
    expect(isBestMove(MAUL, "bash")).toBe(true);
    expect(isBestMove(MAUL, "attack")).toBe(false);
    expect(isBestMove(BOW, "attack")).toBe(false);
  });

  it("names the cheapest shop, else the best drop, else quest or placed", () => {
    expect(sourceText(MAUL)).toBe("Armoury 12 gold");
    expect(sourceText(STAFF)).toBe("25% minotaur champion, boss, first kill");
    expect(sourceText(SWORD)).toBe("quest");
    expect(sourceText(row(1, "x", { sources: [{ kind: "placed", name: "" }] }))).toBe("placed");
    expect(sourceText(BOW)).toBe("");
  });

  it("shows the Str shortfall", () => {
    expect(strText(MAUL)).toBe("70 −10");
    expect(strText(STAFF)).toBe("40");
  });
});

describe("numbers", () => {
  it("percentages from thousandths, never rounded to 0% or 100% unless they are", () => {
    expect([0, 3, 10, 52, 458, 714, 985, 995, 999, 1000].map(pct)).toEqual([
      "0%",
      "<1%",
      "1%",
      "5%",
      "46%",
      "71%",
      "99%",
      ">99%",
      ">99%",
      "100%",
    ]);
  });

  it("the blows of each round, damage, rounds, ranges and DR", () => {
    expect(blowsText([1, 2, 1, 2, 1, 2, 2, 1])).toBe("1 2 1 2 1 2 2 1");
    expect(tenths(364)).toBe("36.4");
    expect(roundsText(376)).toBe("3.76");
    expect(roundsText(2000, true)).toBe("20+");
    expect(rangeText([17, 33])).toBe("17-33");
    expect(rangeText([5, 5])).toBe("5");
    expect(rangeText(undefined)).toBe("");
    expect(drText(50)).toBe("5");
    expect(drText(25)).toBe("2.5");
  });

  it("says why nothing is scored, and reads the brain's refusal", () => {
    expect(missingText(["stats"])).toBe("No stats yet: type `st` in the tile.");
    expect(missingText(["monsters"])).toMatch(/pick a set/);
    expect(missingText([])).toBeNull();
    expect(gearRefusal("gear plan: the brain has no world pack")).toBe("The brain has no world pack");
    expect(gearRefusal("spell plan: no")).toBeNull();
  });
});

// ---- The component ----

function plan(over: Partial<GearPlanMessage> = {}): GearPlanMessage {
  return {
    type: "gear_plan",
    session: "kasimir",
    inputs: { class: "Witchunter", race: "Kang", level: 15, str: 95, load_percent: 51, what_if: [], buffs: [], smash: false },
    monsters: [{ number: 60, name: "minotaur champion", hp: 200, ac: 40, dr: 50, dodge: 0, rooms: 1, boss: true }],
    buffs_offered: [{ name: "song of valour", classes: ["Bard"], effect: "song of valour: +5 AV" }],
    rows: ROWS,
    areas: ["Dwarven Mines", "Newhaven"],
    ...over,
  };
}

const SESSIONS = [
  { id: "kasimir", name: "Kasimir", server: "s", state: "realm", last_seq: 0 },
  { id: "faye", name: "Faye", server: "s", state: "realm", last_seq: 0 },
] as never[];
const CHARACTERS: Record<string, CharacterState> = {
  kasimir: { class: "Witchunter", level: 15, stats: { strength: 95 }, encumbrance: { current: 51, max: 100 } },
  faye: { class: "Druid", level: 10 },
};

function mount(props: Record<string, unknown> = {}) {
  const onplan = vi.fn();
  const ondetail = vi.fn();
  const onclose = vi.fn();
  const store: Record<string, string> = {};
  const storage = { getItem: (k: string) => store[k] ?? null, setItem: (k: string, v: string) => (store[k] = v) };
  const r = render(GearPlanner, {
    sessions: SESSIONS,
    initial: "kasimir",
    characters: CHARACTERS,
    attached: true,
    plans: { kasimir: plan() },
    details: {},
    refusals: {},
    storage,
    onplan,
    ondetail,
    onclose,
    ...props,
  });
  return { ...r, onplan, ondetail, onclose, store };
}

describe("the planner", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("asks for the focused character's plan once the inputs settle", async () => {
    const { onplan } = mount();
    expect(onplan).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    expect(onplan).toHaveBeenCalledTimes(1);
    expect(onplan).toHaveBeenLastCalledWith({ session: "kasimir" });
  });

  it("does not ask again when an answer comes with the inputs unchanged", async () => {
    const { onplan, rerender } = mount({ plans: {} });
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    await rerender({ plans: { kasimir: plan() } });
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS * 2);
    expect(onplan).toHaveBeenCalledTimes(1);
  });

  it("re-asks with a what-if for a changed stat only, and live clears it", async () => {
    const { onplan } = mount();
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    const level = screen.getByLabelText("level") as HTMLInputElement;
    expect(level.value).toBe("15");
    await fireEvent.input(level, { target: { value: "2" } });
    await fireEvent.input(level, { target: { value: "20" } });
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    expect(onplan).toHaveBeenCalledTimes(2);
    expect(onplan).toHaveBeenLastCalledWith({ session: "kasimir", what_if: { level: 20 } });
    expect(level.closest("label")?.classList.contains("changed")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "live" }));
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    expect(onplan).toHaveBeenLastCalledWith({ session: "kasimir" });
  });

  it("ticks a buff, keeps it per character, and sends it", async () => {
    const { onplan, store } = mount();
    await fireEvent.click(screen.getByLabelText(/song of valour: \+5 AV/));
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    expect(onplan).toHaveBeenLastCalledWith({ session: "kasimir", buffs: ["song of valour"] });
    expect(JSON.parse(store[GEAR_BUFFS_KEY])).toEqual({ kasimir: ["song of valour"] });
  });

  it("colours the rows, pins the best and marks its best move", () => {
    const { container } = mount();
    const rows = [...container.querySelectorAll("tr.row")];
    expect(rows.map((r) => r.querySelector(".name")?.textContent)).toEqual([
      "★ darkwood staff",
      "maul",
      "Magebane sword",
      "elven bow",
    ]);
    expect(rows.map((r) => [...r.classList].filter((c) => ["now", "later", "never", "best"].includes(c)))).toEqual([
      ["now", "best"],
      ["now"],
      ["later"],
      ["never"],
    ]);
    // Attack and Bash, four cells each; the staff's bash cells are its best.
    expect(rows[0].querySelectorAll("td.bestmove").length).toBe(4);
    expect(rows[3].querySelectorAll("td.bestmove").length).toBe(0);
    expect(rows[1].querySelector("td.short")?.textContent).toBe("70 −10");
  });

  it("shows smash only for a class that may keep it, and scores it when ticked", async () => {
    const { onplan, container } = mount();
    expect(container.querySelectorAll("th.move").length).toBe(2);
    await fireEvent.click(screen.getByLabelText(/score smash/));
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    expect(onplan).toHaveBeenLastCalledWith({ session: "kasimir", smash: true });
    expect(container.querySelectorAll("th.move").length).toBe(3);
    cleanup();
    mount({ initial: "faye", plans: { faye: plan({ session: "faye", inputs: { class: "Druid", what_if: [], buffs: [], smash: false } }) } });
    expect(screen.queryByLabelText(/score smash/)).toBeNull();
  });

  it("sorts by a column on a click", async () => {
    const { container } = mount();
    await fireEvent.click(screen.getByRole("button", { name: "weapon" }));
    const names = () => [...container.querySelectorAll("tr.row .name")].map((n) => n.textContent);
    expect(names()).toEqual(["★ darkwood staff", "elven bow", "Magebane sword", "maul"]);
    await fireEvent.click(screen.getByRole("button", { name: /^weapon/ }));
    expect(names()).toEqual(["★ darkwood staff", "maul", "Magebane sword", "elven bow"]);
  });

  it("opens a row's detail under it", async () => {
    const detail: GearDetailMessage = {
      type: "gear_detail",
      session: "kasimir",
      item: 10,
      name: "maul",
      inputs: plan().inputs,
      monsters: plan().monsters,
      moves: [
        {
          move: "bash",
          energy: 620,
          blows_per_round: [1, 2, 1, 2, 1, 2, 2, 1],
          monsters: [
            {
              monster: 60,
              hit: 745,
              crit: 8,
              dodge: 0,
              damage: [17, 33],
              damage_per_blow: 209,
              damage_round1: 364,
              damage_round2: 627,
              kill_by_round: [52, 458, 900, 990, 1000, 1000, 1000, 1000],
              rounds: 376,
            },
          ],
        },
      ],
    };
    const { ondetail, container, rerender } = mount();
    await fireEvent.click(screen.getByText("maul"));
    expect(container.textContent).toMatch(/Asking for maul's numbers/);
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    expect(ondetail).toHaveBeenLastCalledWith({ session: "kasimir", item: 10 });
    await rerender({ details: { kasimir: detail } });
    const d = container.querySelector("tr.detail")!;
    expect(d.previousElementSibling?.querySelector(".name")?.textContent).toBe("maul");
    expect(d.querySelector(".blows")?.textContent).toBe("1 2 1 2 1 2 2 1");
    expect(d.textContent).toMatch(/minotaur champion/);
    expect(d.textContent).toMatch(/75%/);
    expect(d.textContent).toMatch(/17-33/);
  });

  it("says plainly when there is nothing to score", () => {
    mount({ plans: { kasimir: plan({ rows: [], missing: ["stats"] }) } });
    expect(screen.getByText("No stats yet: type `st` in the tile.")).toBeTruthy();
    cleanup();
    mount({ plans: { kasimir: plan({ rows: [], missing: ["monsters"] }) } });
    expect(screen.getByText(/pick a set/)).toBeTruthy();
    cleanup();
    mount({ attached: false });
    expect(screen.getByText("No brain attached: no gear plan.")).toBeTruthy();
    cleanup();
    mount({ refusals: { kasimir: "The brain has no world pack" } });
    expect(screen.getByText("The brain has no world pack")).toBeTruthy();
  });

  it("switches character from its picker, and closes", async () => {
    const { onplan, onclose } = mount();
    await fireEvent.change(screen.getByLabelText("Character"), { target: { value: "faye" } });
    await vi.advanceTimersByTimeAsync(GEAR_DEBOUNCE_MS);
    expect(onplan).toHaveBeenLastCalledWith({ session: "faye" });
    await fireEvent.click(screen.getByRole("button", { name: /Esc/ }));
    expect(onclose).toHaveBeenCalled();
  });
});
