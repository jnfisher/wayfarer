// The gear planner (docs/web-ui.md "Gear planner", C-060): the request built from the planner's
// inputs, the rows' filters and sorting, and how the brain's scaled numbers are shown (rates in
// thousandths, damage in tenths, rounds in hundredths; docs/protocol.md `gear_plan`).

import type {
  CharacterState,
  GearDetailRequest,
  GearMonster,
  GearMonsters,
  GearMove,
  GearMoveScore,
  GearPlanRequest,
  GearRow,
  GearSource,
  GearWhatIf,
  ToClient,
} from "../proto";
import { coinText } from "./spells";

export type GearPlanMessage = Extract<ToClient, { type: "gear_plan" }>;
export type GearDetailMessage = Extract<ToClient, { type: "gear_detail" }>;

/** How long the planner waits after a change before asking again. */
export const GEAR_DEBOUNCE_MS = 200;

/** The moves in the table's order. */
export const MOVES: GearMove[] = ["attack", "bash", "smash"];

/** The classes that may keep smash (`kb/combat.md`), for the "score smash" box. */
export const SMASH_CLASSES = ["Warrior", "Witchunter", "Paladin", "Cleric", "Warlock", "Ranger"];

export function mayKeepSmash(cls: string | undefined): boolean {
  return cls !== undefined && SMASH_CLASSES.some((c) => c.toLowerCase() === cls.toLowerCase());
}

// ---- The inputs and the request ----

/** The what-if values, in the header's order. */
export const WHAT_IF_KEYS = ["level", "str", "int", "wil", "agi", "hea", "cha", "load_percent"] as const;
export type WhatIfKey = (typeof WHAT_IF_KEYS)[number];
export type WhatIfValues = Partial<Record<WhatIfKey, number>>;

export const WHAT_IF_LABELS: Record<WhatIfKey, string> = {
  level: "level",
  str: "Str",
  int: "Int",
  wil: "Wil",
  agi: "Agi",
  hea: "Hea",
  cha: "Cha",
  load_percent: "load %",
};

/** Which monsters: the loop the bot runs (else the area around the character), an area, or names. */
export type MonsterChoice = { kind: "loop" } | { kind: "area"; area: string } | { kind: "names"; names: string };

export interface PlannerInputs {
  monsters: MonsterChoice;
  /** The what-if fields the player typed in; the others show the live values. */
  whatIf: WhatIfValues;
  /** The ticked buffs' names. */
  buffs: string[];
  smash: boolean;
}

export function emptyInputs(): PlannerInputs {
  return { monsters: { kind: "loop" }, whatIf: {}, buffs: [], smash: false };
}

/** The character's own values: from its snapshot, else from what a plan used that was not a
 * what-if. */
export function liveValues(c: CharacterState | undefined, plan?: GearPlanMessage): WhatIfValues {
  const s = c?.stats;
  const e = c?.encumbrance;
  const load = e?.current !== undefined && e.max ? Math.floor((e.current * 100) / e.max) : undefined;
  const own: WhatIfValues = {
    level: c?.level,
    str: s?.strength,
    int: s?.intellect,
    wil: s?.willpower,
    agi: s?.agility,
    hea: s?.health,
    cha: s?.charm,
    load_percent: load,
  };
  const out: WhatIfValues = {};
  for (const k of WHAT_IF_KEYS) {
    const used = plan && !plan.inputs.what_if.includes(k) ? plan.inputs[k] : undefined;
    const v = own[k] ?? used;
    if (v !== undefined) out[k] = v;
  }
  return out;
}

/** The what-if fields that differ from the live values: what the request sends. */
export function changedWhatIf(whatIf: WhatIfValues, live: WhatIfValues): GearWhatIf | undefined {
  const out: GearWhatIf = {};
  let any = false;
  for (const k of WHAT_IF_KEYS) {
    const v = whatIf[k];
    if (v === undefined || !Number.isFinite(v) || v === live[k]) continue;
    out[k] = Math.round(v);
    any = true;
  }
  return any ? out : undefined;
}

/** The names typed in the names field: split at commas and semicolons. */
export function splitNames(text: string): string[] {
  return text
    .split(/[,;]/)
    .map((n) => n.trim())
    .filter((n) => n.length > 0);
}

/** The set the request names; undefined for the brain's default (the loop, else the area). */
export function monstersOf(choice: MonsterChoice): GearMonsters | undefined {
  if (choice.kind === "area") {
    const area = choice.area.trim();
    return area ? { area } : undefined;
  }
  if (choice.kind === "names") {
    const names = splitNames(choice.names);
    return names.length > 0 ? { names } : undefined;
  }
  return undefined;
}

/** A `gear_plan_request` for the inputs: only what differs from the brain's defaults. The
 * buffs are kept to those `offered` when a plan has said which (ticks kept for another
 * character's buffs are not sent). */
export function planRequest(
  session: string,
  inputs: PlannerInputs,
  live: WhatIfValues,
  offered?: string[],
): GearPlanRequest {
  const req: GearPlanRequest = { session };
  const monsters = monstersOf(inputs.monsters);
  if (monsters) req.monsters = monsters;
  const whatIf = changedWhatIf(inputs.whatIf, live);
  if (whatIf) req.what_if = whatIf;
  const buffs = offered ? inputs.buffs.filter((b) => offered.includes(b)) : inputs.buffs;
  if (buffs.length > 0) req.buffs = [...buffs].sort();
  if (inputs.smash) req.smash = true;
  return req;
}

/** A `gear_detail_request` for one weapon, with the plan's inputs. */
export function detailRequest(
  session: string,
  item: number,
  inputs: PlannerInputs,
  live: WhatIfValues,
  offered?: string[],
): GearDetailRequest {
  return { ...planRequest(session, inputs, live, offered), item };
}

// ---- Buff ticks, kept per character ----

export const GEAR_BUFFS_KEY = "wayfarer.gear.buffs";

/** The ticked buffs of every character, by session. */
export function loadBuffTicks(storage: Pick<Storage, "getItem"> | undefined): Record<string, string[]> {
  try {
    const raw: unknown = JSON.parse(storage?.getItem(GEAR_BUFFS_KEY) ?? "null");
    if (raw && typeof raw === "object" && !Array.isArray(raw)) {
      const out: Record<string, string[]> = {};
      for (const [k, v] of Object.entries(raw)) {
        if (Array.isArray(v)) out[k] = v.filter((x): x is string => typeof x === "string");
      }
      return out;
    }
  } catch {
    // Unreadable or blocked: none ticked.
  }
  return {};
}

export function saveBuffTicks(storage: Pick<Storage, "setItem"> | undefined, ticks: Record<string, string[]>) {
  try {
    storage?.setItem(GEAR_BUFFS_KEY, JSON.stringify(ticks));
  } catch {
    // Blocked or full: the ticks last until the page closes.
  }
}

// ---- Filters ----

/** `now`: usable now; `usable`: now or later; `all`: the never-usable too. */
export type UseFilter = "now" | "usable" | "all";

export interface RowFilters {
  use: UseFilter;
  /** Hide the weapons nothing sells, drops or gives. */
  hasSource: boolean;
  /** The most a shop may ask, in copper; weapons no shop sells are kept. */
  maxPrice?: number;
}

export function defaultFilters(): RowFilters {
  return { use: "all", hasSource: false };
}

/** The cheapest shop's price, in copper; undefined when no shop sells it. */
export function cheapestPrice(row: GearRow): number | undefined {
  const prices = row.sources.filter((s) => s.kind === "shop" && s.price !== undefined).map((s) => s.price as number);
  return prices.length > 0 ? Math.min(...prices) : undefined;
}

export function passes(row: GearRow, f: RowFilters): boolean {
  if (f.use === "now" && row.status !== "now") return false;
  if (f.use === "usable" && row.status === "never") return false;
  if (f.hasSource && row.sources.length === 0) return false;
  if (f.maxPrice !== undefined) {
    const p = cheapestPrice(row);
    if (p !== undefined && p > f.maxPrice) return false;
  }
  return true;
}

// ---- Sorting ----

export type Metric = "kill1" | "kill2" | "rounds" | "dmg1";
export const METRICS: Metric[] = ["kill1", "kill2", "rounds", "dmg1"];
export type SortKey =
  | "name"
  | "damage"
  | "speed"
  | "str"
  | "level"
  | "hands"
  | "status"
  | "source"
  | `${GearMove}.${Metric}`;

export interface Sort {
  key: SortKey;
  /** 1 ascending, -1 descending. */
  dir: 1 | -1;
}

/** The order a column sorts in first: the kill chances and damage highest first, the rest
 * lowest first. */
export function firstDir(key: SortKey): 1 | -1 {
  return key === "damage" || key.endsWith(".kill1") || key.endsWith(".kill2") || key.endsWith(".dmg1") ? -1 : 1;
}

/** Click on a column: sort by it, or the other way round when it already is. */
export function nextSort(current: Sort, key: SortKey): Sort {
  return current.key === key ? { key, dir: current.dir === 1 ? -1 : 1 } : { key, dir: firstDir(key) };
}

/** The brain's own order: now, later, never, by name within each. */
export const DEFAULT_SORT: Sort = { key: "status", dir: 1 };

export function moveScore(row: GearRow, move: GearMove): GearMoveScore | undefined {
  return row.moves.find((m) => m.move === move);
}

const STATUS_ORDER = { now: 0, later: 1, never: 2 } as const;

/** A row's value for a column; undefined sorts last whichever way. */
function sortValue(row: GearRow, key: SortKey): number | string | undefined {
  switch (key) {
    case "name":
      return row.name.toLowerCase();
    case "damage":
      return row.min_damage + row.max_damage;
    case "speed":
      return row.speed;
    case "str":
      return row.str_req;
    case "level":
      return row.min_level ?? 0;
    case "hands":
      return row.two_handed ? 2 : 1;
    case "status":
      return STATUS_ORDER[row.status];
    case "source":
      return cheapestPrice(row);
  }
  const [move, metric] = key.split(".") as [GearMove, Metric];
  const s = moveScore(row, move);
  if (!s) return undefined;
  return metric === "kill1" ? s.kill_round1 : metric === "kill2" ? s.kill_round2 : metric === "rounds" ? s.rounds : s.damage_round1;
}

/** The rows sorted by a column, ties by name; rows without a value (an unscored move, no shop)
 * last. */
export function sortRows(rows: GearRow[], sort: Sort): GearRow[] {
  return [...rows].sort((a, b) => {
    const va = sortValue(a, sort.key);
    const vb = sortValue(b, sort.key);
    if (va === undefined || vb === undefined) {
      if (va !== vb) return va === undefined ? 1 : -1;
    } else if (va !== vb) {
      return (va < vb ? -1 : 1) * sort.dir;
    }
    return a.name.toLowerCase() < b.name.toLowerCase() ? -1 : a.name.toLowerCase() > b.name.toLowerCase() ? 1 : a.item - b.item;
  });
}

/** What the table shows: the rows that pass the filters, sorted, the best one pinned on top. */
export function visibleRows(rows: GearRow[], filters: RowFilters, sort: Sort): GearRow[] {
  const sorted = sortRows(
    rows.filter((r) => passes(r, filters)),
    sort,
  );
  const best = sorted.findIndex((r) => r.best);
  if (best > 0) sorted.unshift(...sorted.splice(best, 1));
  return sorted;
}

// ---- What a row says ----

/** The row's colour: green now, amber later, grey never. */
export function statusClass(row: GearRow): "now" | "later" | "never" {
  return row.status;
}

/** The status column: `now`, or why not ("level 15", "class"). */
export function statusText(row: GearRow): string {
  if (row.status === "now") return "now";
  return row.reasons.length > 0 ? row.reasons.join(", ") : row.status;
}

/** The move cells to mark: the row's best move. */
export function isBestMove(row: GearRow, move: GearMove): boolean {
  return row.best_move === move;
}

/** The best drop: the highest chance. */
function bestDrop(sources: GearSource[]): GearSource | undefined {
  return sources
    .filter((s) => s.kind === "drop")
    .reduce<GearSource | undefined>((b, s) => (b === undefined || (s.percent ?? 0) > (b.percent ?? 0) ? s : b), undefined);
}

/** Where it comes from: the cheapest shop and its price, else the best drop ("25% minotaur
 * champion, boss, first kill"), else `quest` or `placed`; empty for nothing. */
export function sourceText(row: GearRow): string {
  const shops = row.sources.filter((s) => s.kind === "shop");
  const price = cheapestPrice(row);
  const shop = shops.find((s) => s.price === price) ?? shops[0];
  if (shop) return price === undefined ? shop.name : `${shop.name} ${coinText(price)}`;
  const drop = bestDrop(row.sources);
  if (drop) {
    const parts = [`${drop.percent !== undefined ? `${drop.percent}% ` : ""}${drop.name}`];
    if (drop.boss) parts.push("boss");
    if (drop.first_kill) parts.push("first kill");
    return parts.join(", ");
  }
  if (row.sources.some((s) => s.kind === "quest")) return "quest";
  if (row.sources.some((s) => s.kind === "placed")) return "placed";
  return "";
}

/** Every source, a line each, for the source cell's tooltip. */
export function sourcesTitle(row: GearRow): string {
  const lines = row.sources.map((s) => {
    switch (s.kind) {
      case "shop":
        return `shop: ${s.name}${s.price !== undefined ? `, ${coinText(s.price)}` : ""}${s.steps !== undefined ? `, ${s.steps} steps` : ""}`;
      case "drop":
        return `drop: ${s.percent ?? "?"}% ${s.name}${s.boss ? ", boss" : ""}${s.first_kill ? ", first kill" : ""}`;
      case "quest":
        return `quest: ${s.name}`;
      case "placed":
        return "placed in a room";
    }
  });
  if (row.limited !== undefined) lines.push(`limited: at most ${row.limited} in play`);
  if (row.deleted_at_cleanup) lines.push("deleted at cleanup");
  return lines.join("\n");
}

/** The Str cell: the requirement, and the shortfall when short ("60 −5"). */
export function strText(row: GearRow): string {
  return row.str_short ? `${row.str_req} −${row.str_short}` : String(row.str_req);
}

// ---- Numbers ----

/** A rate in thousandths as a whole per cent, never rounded to 0% or 100% unless it is:
 * `<1%`, `>99%`. */
export function pct(thousandths: number): string {
  if (thousandths <= 0) return "0%";
  if (thousandths >= 1000) return "100%";
  if (thousandths < 10) return "<1%";
  if (thousandths > 990) return ">99%";
  return `${Math.round(thousandths / 10)}%`;
}

/** Tenths with one decimal: `364` is `36.4`. */
export function tenths(v: number): string {
  return (v / 10).toFixed(1);
}

/** Expected rounds, from hundredths: `3.76`; `20+` when the count was capped. */
export function roundsText(hundredths: number, capped?: boolean): string {
  return capped ? "20+" : (hundredths / 100).toFixed(2);
}

/** The blows struck in each round: `1 2 1 2 1 2 2 1`. */
export function blowsText(blows: number[]): string {
  return blows.join(" ");
}

/** A landed blow's damage range: `17-33` (whole points). */
export function rangeText(r: number[] | undefined): string {
  if (!r || r.length < 2) return "";
  return r[0] === r[1] ? String(r[0]) : `${r[0]}-${r[1]}`;
}

/** A monster's DR, from tenths: `5`, `2.5`. */
export function drText(dr: number): string {
  return dr % 10 === 0 ? String(dr / 10) : tenths(dr);
}

/** A monster of the set as the header lists it. */
export function monsterText(m: GearMonster): string {
  const rooms = m.rooms > 0 ? `, ${m.rooms} ${m.rooms === 1 ? "room" : "rooms"}` : "";
  return `${m.name}${m.boss ? " (boss)" : ""}: HP ${m.hp}, AC ${m.ac}, DR ${drText(m.dr)}, dodge ${m.dodge}${rooms}`;
}

// ---- What the view says when there is nothing to score ----

/** Why nothing is scored, in words, from the plan's `missing`; null when it scores. */
export function missingText(missing: string[] | undefined): string | null {
  if (!missing || missing.length === 0) return null;
  if (missing.includes("stats") || missing.includes("class") || missing.includes("race") || missing.includes("level")) {
    return "No stats yet: type `st` in the tile.";
  }
  if (missing.includes("load")) return "No inventory yet: type `i` in the tile, or set a load % under What if.";
  if (missing.includes("monsters")) {
    return "No loop running and no position for the area around the character: pick a set (an area or names).";
  }
  return `Missing: ${missing.join(", ")}.`;
}

/** The brain's reason for not giving a plan or a detail, from its `warning`; null for any other. */
export function gearRefusal(text: string): string | null {
  const why = text.match(/^gear (?:plan|detail): (.+)$/s)?.[1];
  return why ? why[0].toUpperCase() + why.slice(1) : null;
}
