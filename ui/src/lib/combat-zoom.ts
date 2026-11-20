// The combat zoom (C-125, docs/web-ui.md "Combat zoom"): a character's combat numbers over a
// window, their histograms, each monster fought, and the party side by side.

import type { CombatStats, CombatWindow, FoeRow, HistRow, ToClient } from "../proto";
import { count, MIN_SAMPLE, pct, tenths } from "./stats";

export type CombatDetailMessage = Extract<ToClient, { type: "combat_detail" }>;

export const WINDOWS: { window: CombatWindow; label: string; title: string }[] = [
  { window: "reset", label: "since reset", title: "Since the combat numbers were last reset" },
  { window: "hour", label: "last hour", title: "The last hour (kept every ten minutes)" },
  { window: "day", label: "last 24 h", title: "The last 24 hours (kept every ten minutes)" },
];

/** The key of a session's detail for a window. */
export function detailKey(session: string, window: CombatWindow): string {
  return `${session}:${window}`;
}

/** A histogram to pick: its key, and what it shows. */
export interface HistChoice {
  key: string;
  label: string;
}

/** The key of a histogram row. */
export function histKey(h: HistRow): string {
  return h.kind === "cast" ? `cast:${h.name ?? ""}` : h.kind;
}

const HIST_LABELS: Record<string, string> = {
  round: "damage a round",
  all: "every hit",
  swing: "swing hits",
  backstab: "backstab hits",
  taken: "blows taken",
};

/** The histograms there are, in a fixed order: the round, every hit, the attacks, taken. */
export function histChoices(hists: HistRow[]): HistChoice[] {
  const order = (h: HistRow) => ["round", "all", "backstab", "swing", "cast", "taken"].indexOf(h.kind);
  return [...hists]
    .sort((a, b) => order(a) - order(b))
    .map((h) => ({ key: histKey(h), label: h.kind === "cast" ? `${h.name} hits` : (HIST_LABELS[h.kind] ?? h.kind) }));
}

/** One bar of a histogram: damage from `lo` to `hi`, `n` times. */
export interface Bin {
  lo: number;
  hi: number;
  n: number;
}

/** A width for about `bins` bars over `span` values: 1, 2, 5, 10, 20, 50... */
export function binWidth(span: number, bins: number): number {
  const raw = Math.max(1, span / bins);
  for (let p = 1; ; p *= 10) {
    for (const m of [1, 2, 5]) if (m * p >= raw) return m * p;
  }
}

/** `values` (`[damage, count]`) as bars of equal width, about `bins` of them, none left out
 * between the smallest and the largest. */
export function bin(values: [number, number][], bins = 24): Bin[] {
  if (values.length === 0) return [];
  const min = values[0][0];
  const max = values[values.length - 1][0];
  const w = binWidth(max - min + 1, bins);
  const start = Math.floor(min / w) * w;
  const out: Bin[] = [];
  for (let lo = start; lo <= max; lo += w) out.push({ lo, hi: lo + w - 1, n: 0 });
  for (const [d, n] of values) out[Math.floor((d - start) / w)].n += n;
  return out;
}

/** The middle value of a histogram's `[damage, count]` pairs; undefined with none. */
export function median(values: [number, number][]): number | undefined {
  const total = values.reduce((s, [, n]) => s + n, 0);
  if (total === 0) return undefined;
  let seen = 0;
  for (const [d, n] of values) {
    seen += n;
    if (seen * 2 >= total) return d;
  }
  return undefined;
}

/** A line of the monsters table. */
export interface FoeLine {
  name: string;
  kills: string;
  /** Rounds a kill, the character's damage and the monster's a round. */
  roundsPerKill: string;
  dealtPerRound: string;
  takenPerRound: string;
  /** The monster's blows that hit the character. */
  hitsYou: string;
  expPerKill: string;
  /** Fewer than {@link MIN_SAMPLE} rounds. */
  few: boolean;
}

/** Each monster fought, the most rounds first. */
export function foeLines(monsters: FoeRow[]): FoeLine[] {
  const per = (n: number, of: number) => (of > 0 ? (n / of).toFixed(1) : "");
  return [...monsters]
    .sort((a, b) => b.rounds - a.rounds || b.kills - a.kills)
    .map((f) => ({
      name: f.name,
      kills: count(f.kills),
      roundsPerKill: per(f.rounds, f.kills),
      dealtPerRound: per(f.dealt, f.rounds),
      takenPerRound: per(f.taken, f.rounds),
      hitsYou: pct(f.hits, f.blows),
      expPerKill: f.kills > 0 ? count(f.exp / f.kills) : "",
      few: f.rounds < MIN_SAMPLE,
    }));
}

/** A character's line in the party table. */
export interface PartyLine {
  session: string;
  name: string;
  round: string;
  swingHit: string;
  bsHit: string;
  bsAvg: string;
  takenHit: string;
  kills: string;
  /** Fewer than {@link MIN_SAMPLE} rounds. */
  few: boolean;
}

/** A party member's numbers side by side with the others'; empty cells without numbers. */
export function partyLine(session: string, name: string, c: CombatStats | undefined): PartyLine {
  const attack = (kind: string) => c?.attacks.find((a) => a.kind === kind);
  const swing = attack("swing");
  const bs = attack("backstab");
  return {
    session,
    name,
    round: c?.round ? tenths(c.round.avg_tenths) : "",
    swingHit: swing ? pct(swing.hits, swing.tries) : "",
    bsHit: bs ? pct(bs.hits, bs.tries) : "",
    bsAvg: bs && bs.hits > 0 ? tenths(bs.avg_tenths) : "",
    takenHit: c?.taken ? pct(c.taken.hits, c.taken.blows) : "",
    kills: c ? count(c.kills) : "",
    few: (c?.round?.count ?? 0) < MIN_SAMPLE,
  };
}
