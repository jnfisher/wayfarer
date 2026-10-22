// Searching the loops list: a loop is found by part of its name, file, description, author or
// any of its stops' room names, every word of the query being somewhere in it.

import type { GeneratedLoop, LoopStop, MegaMudLoop } from "../proto";
import type { Loops } from "./map";

/** The radii the loops list can be asked for, in steps. */
export const LOOP_RADII = [8, 15, 30] as const;

/** Whether every word of `query` (any case) is in one of `texts`. An empty query matches. */
export function matches(query: string, texts: (string | undefined)[]): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const hay = texts
    .filter((t): t is string => !!t)
    .join("\n")
    .toLowerCase();
  return words.every((w) => hay.includes(w));
}

const stopNames = (stops: LoopStop[]) => stops.map((s) => s.name);

export function generatedMatches(l: GeneratedLoop, query: string): boolean {
  return matches(query, [l.name, ...stopNames(l.stops)]);
}

export function megamudMatches(m: MegaMudLoop, query: string): boolean {
  return matches(query, [m.file, m.description, m.author, ...stopNames(m.stops)]);
}

export interface ShownLoops {
  generated: GeneratedLoop[];
  custom: GeneratedLoop[];
  megamud: MegaMudLoop[];
  /** How many loops the list has, and how many the query leaves. */
  total: number;
  count: number;
}

/** The loops of a `loops` message that the query finds (all of them for an empty query). */
export function shownLoops(loops: Loops | undefined, query: string): ShownLoops {
  const generated = loops?.generated ?? [];
  const custom = loops?.custom ?? [];
  const megamud = loops?.megamud ?? [];
  const shown = {
    generated: generated.filter((l) => generatedMatches(l, query)),
    custom: custom.filter((l) => generatedMatches(l, query)),
    megamud: megamud.filter((m) => megamudMatches(m, query)),
  };
  return {
    ...shown,
    total: generated.length + custom.length + megamud.length,
    count: shown.generated.length + shown.custom.length + shown.megamud.length,
  };
}
