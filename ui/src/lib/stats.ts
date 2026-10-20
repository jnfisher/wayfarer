// The stats tab (C-029): the brain's `stats` answer, and how its numbers are shown.

import type { ExpSample, ToClient } from "../proto";

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
