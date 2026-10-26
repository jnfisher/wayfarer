// The stats tab (C-029): the brain's `stats` answer, and how its numbers are shown; the combat
// numbers (C-050) as a table; where the run's time went, and how efficient it is.

import type { CombatStats, DamageRow, ExpSample, TimeSpent, ToClient } from "../proto";

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

/** Seconds as "1h 18m", "12m 5s" or "45s". */
export function duration(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return s % 60 ? `${m}m ${s % 60}s` : `${m}m`;
  return `${s}s`;
}

/** One kind of time on the stats tab: what the character was doing, how long, and its share. */
export interface TimeLine {
  kind: "fight" | "flee" | "rest" | "walk" | "upkeep" | "wait" | "paused" | "away";
  label: string;
  secs: number;
  /** Whole per cent of the counted time. */
  pct: number;
  /** The same, unrounded, for the bar's widths. */
  share: number;
}

const TIME_KINDS: { kind: TimeLine["kind"]; label: string; title: string }[] = [
  { kind: "fight", label: "fighting", title: "Fighting, or in combat" },
  { kind: "flee", label: "fleeing", title: "Running away" },
  { kind: "rest", label: "resting", title: "Resting, or waiting to rest, or healing" },
  { kind: "walk", label: "walking", title: "Walking: the loop, a Go to, to a rest and back" },
  { kind: "upkeep", label: "upkeep", title: "Picking up, casting blessings, sneaking" },
  { kind: "wait", label: "waiting", title: "Nothing to do: waiting for a monster to come, or for the party" },
  { kind: "paused", label: "paused", title: "The bot paused" },
  { kind: "away", label: "away", title: "Out of the realm, or the connection down" },
];

/** What a kind of time means, for its row's title. */
export function timeTitle(kind: TimeLine["kind"]): string {
  return TIME_KINDS.find((k) => k.kind === kind)?.title ?? "";
}

/** The counted time. */
export function counted(t: TimeSpent): number {
  return TIME_KINDS.reduce((sum, k) => sum + t[`${k.kind}_secs`], 0);
}

/** Where the run's time went, biggest first; kinds with none left out. */
export function timeLines(t: TimeSpent): TimeLine[] {
  const total = counted(t);
  if (total === 0) return [];
  return TIME_KINDS.map(({ kind, label }) => {
    const secs = t[`${kind}_secs`];
    return { kind, label, secs, pct: Math.round((secs * 100) / total), share: (secs * 100) / total };
  })
    .filter((l) => l.secs > 0)
    .sort((a, b) => b.secs - a.secs);
}

/** How efficient the run is, from its time and its kills and exp. */
export interface Efficiency {
  /** Kills an hour of the bot playing (paused and away left out); "" with no time yet. */
  killsPerHour: string;
  /** Exp a minute of fighting; "" with no fighting. */
  expPerFightMinute: string;
  /** Per kill: the time fighting, resting, and walking and waiting between kills. */
  perKill?: { fight: string; rest: string; between: string };
  /** The run's wall time the brain did not see (it was not running), when a minute or more. */
  unseen?: string;
}

export function efficiency(t: TimeSpent, kills: number, exp: number, runSecs?: number): Efficiency {
  const playing = counted(t) - t.paused_secs - t.away_secs;
  const e: Efficiency = {
    killsPerHour: playing > 0 ? count((kills * 3600) / playing) : "",
    expPerFightMinute: t.fight_secs > 0 ? count((exp * 60) / t.fight_secs) : "",
  };
  if (kills > 0) {
    e.perKill = {
      fight: duration(t.fight_secs / kills),
      rest: duration(t.rest_secs / kills),
      between: duration((t.walk_secs + t.wait_secs) / kills),
    };
  }
  if (runSecs !== undefined && runSecs - counted(t) >= 60) e.unseen = duration(runSecs - counted(t));
  return e;
}
