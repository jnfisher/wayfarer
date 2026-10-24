// The stats tab (C-029): the brain's `stats` answer, and how its numbers are shown; the combat
// numbers (C-050) as a table.

import type { CombatStats, DamageRow, ExpSample, ToClient } from "../proto";

export type Stats = Extract<ToClient, { type: "stats" }>;

/** How often the open stats tab asks the brain again (it samples once a minute). */
export const STATS_REFRESH_MS = 60_000;

const BARS = "▁▂▃▄▅▆▇█";

/**
 * Exp over time as block characters, one per sample: the exp gained since the first sample,
 * scaled to the highest. Samples without exp are left out; fewer than two give "".
 */
export function sparkline(samples: ExpSample[], width = 40): string {
  const exp = samples.map((s) => s.exp).filter((e): e is number => e !== undefined);
  if (exp.length < 2) return "";
  const shown = exp.slice(-width);
  const base = shown[0];
  const top = Math.max(...shown.map((e) => e - base));
  return shown
    .map((e) => (top <= 0 ? BARS[0] : BARS[Math.round(((e - base) / top) * (BARS.length - 1))]))
    .join("");
}

/** `2026-10-19T12:00:05Z` as the browser's local `12:00`, with the date when not today. */
export function startedAt(rfc3339: string, now = new Date()): string {
  const d = new Date(rfc3339);
  if (Number.isNaN(d.getTime())) return rfc3339;
  const hm = [d.getHours(), d.getMinutes()].map((n) => String(n).padStart(2, "0")).join(":");
  const sameDay = d.toDateString() === now.toDateString();
  return sameDay ? hm : `${d.getMonth() + 1}/${d.getDate()} ${hm}`;
}

/** A number with thousands separated: 12,345. */
export function count(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}

/** Thousandths as a per cent with one decimal: 714 → "71.4%". */
export function perCent(perMille: number): string {
  return `${(perMille / 10).toFixed(1)}%`;
}

/** Tenths with one decimal: 525 → "52.5". */
export function tenths(n: number): string {
  return (n / 10).toFixed(1);
}

/** A line of the combat table (C-050). */
export interface CombatLine {
  label: string;
  count: string;
  /** Of all swings, or a rate of its own (accuracy); "" when it has none. */
  share: string;
  range: string;
  avg: string;
  /** A few words more: the dodges among the misses, a backstab's tries and crits. */
  note: string;
}

const LABELS: Record<string, string> = { hit: "hit", crit: "crit", backstab: "bs", cast: "cast", round: "round" };

function damageLine(r: DamageRow, share: string, note = ""): CombatLine {
  return {
    label: LABELS[r.kind] ?? r.kind,
    count: count(r.count),
    share,
    range: `${r.min}-${r.max}`,
    avg: tenths(r.avg_tenths),
    note,
  };
}

/** The combat table, MegaMud's Combat tab's rows: kinds nothing was counted for are left out. */
export function combatLines(c: CombatStats): CombatLine[] {
  const lines: CombatLine[] = [];
  const of = (n: number) => (c.swings === 0 ? "" : perCent(Math.floor((n * 1000) / c.swings)));
  if (c.misses > 0) {
    lines.push({
      label: "miss",
      count: count(c.misses),
      share: of(c.misses),
      range: "",
      avg: "",
      note: c.dodged > 0 ? `${count(c.dodged)} dodged` : "",
    });
  }
  for (const r of c.rows) {
    if (r.kind === "backstab") {
      const notes = [];
      if (c.backstab_tries > 0) notes.push(`${count(r.count)}/${count(c.backstab_tries)} landed`);
      if (c.backstab_crits > 0) notes.push(`${count(c.backstab_crits)} crit`);
      lines.push(damageLine(r, perCent(r.per_mille), notes.join(", ")));
    } else if (r.kind === "cast") {
      const note = c.cast_misses > 0 ? `${count(c.cast_misses)} failed` : "";
      lines.push(damageLine(r, perCent(c.cast_accuracy_per_mille), note));
    } else {
      lines.push(damageLine(r, perCent(r.per_mille)));
    }
  }
  if (c.glances > 0) {
    lines.push({ label: "glance", count: count(c.glances), share: of(c.glances), range: "", avg: "", note: "" });
  }
  if (c.round) lines.push(damageLine(c.round, ""));
  return lines;
}

/** How long ago `rfc3339` was: "3h 12m", "45m", "2d 4h". */
export function age(rfc3339: string, now = new Date()): string {
  const mins = Math.max(0, Math.floor((now.getTime() - new Date(rfc3339).getTime()) / 60_000));
  if (Number.isNaN(mins)) return "";
  const d = Math.floor(mins / 1440);
  const h = Math.floor((mins % 1440) / 60);
  const m = mins % 60;
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}
