// Minimized tiles (docs/web-ui.md "Minimizing a tile"): the characters whose tile is hidden to
// the dock at the bottom, kept in the browser, and what a dock chip shows.

import type { CharacterState, SessionInfo } from "../proto";
import { bar, type Tone } from "./header";
import type { LayoutStorage } from "./layout";

export const MINIMIZED_KEY = "wayfarer.minimized";

/** The ids stored; anything missing or unreadable is "none". */
export function loadMinimized(storage: LayoutStorage | undefined): string[] {
  try {
    const parsed = JSON.parse(storage?.getItem(MINIMIZED_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}

/** Write the ids; with storage unavailable they are simply forgotten on reload. */
export function saveMinimized(storage: LayoutStorage | undefined, ids: Iterable<string>): void {
  try {
    storage?.setItem(MINIMIZED_KEY, JSON.stringify([...ids]));
  } catch {
    // quota, private mode, blocked site data
  }
}

/** The minimized sessions that still exist, in the sessions' order. */
export function dockSessions(sessions: SessionInfo[], minimized: ReadonlySet<string>): SessionInfo[] {
  return sessions.filter((s) => minimized.has(s.id));
}

/**
 * The sessions whose tile takes no cell: the minimized ones and, while one tile is zoomed to fill
 * the grid (Alt+Z), every other. Zooming on a minimized tile, or one gone, zooms nothing.
 */
export function offGrid(
  sessions: SessionInfo[],
  minimized: ReadonlySet<string>,
  zoomed: string | null,
): Set<string> {
  const zooming = zoomed !== null && !minimized.has(zoomed) && sessions.some((s) => s.id === zoomed);
  return new Set(sessions.filter((s) => minimized.has(s.id) || (zooming && s.id !== zoomed)).map((s) => s.id));
}

export interface Chip {
  /** `Waydeva 65/65`: the HP, `--` when not known, `offline` while not in the realm. */
  text: string;
  tone: Tone;
  /** The bot is playing it. */
  running: boolean;
}

/** What a dock chip shows of a character: enough to see it is alright, or not. */
export function chip(info: SessionInfo, c: CharacterState | null | undefined): Chip {
  const hp = bar(c?.hp, c?.hp_max);
  const here = info.state === "realm";
  return {
    text: `${info.name} ${here ? hp.text : info.state}`,
    tone: here ? hp.tone : "none",
    running: c?.bot?.state === "running",
  };
}
