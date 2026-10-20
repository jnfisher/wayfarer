// The map panel's go box (docs/web-ui.md, "Go to"): the brain's `found` in, the rows of the
// list out. No Svelte here, so the tests drive it directly.

import type { Found, FoundRoom, RoomKey, ToClient } from "../proto";
import { metrics, type MapView } from "./map";

export type FoundMessage = Extract<ToClient, { type: "found" }>;

/** The shortest query sent; the brain answers nothing to a shorter one. */
export const MIN_QUERY = 2;
/** How long typing must pause before a `find` goes. */
export const FIND_DEBOUNCE_MS = 200;

/** One row of the list: a result (walking to its nearest room), or one of its rooms. */
export interface GoRow {
  /** Unique within the list: the result's index, and the room's for a room row. */
  id: string;
  /** The result's index in `found.results`. */
  result: number;
  /** A room of an expanded result: indented under it. */
  sub: boolean;
  text: string;
  /** Where Enter walks: the result's nearest room, or this room. Absent: nowhere to go. */
  room?: FoundRoom;
  /** The result has more than one room, so it can be expanded. */
  expandable: boolean;
}

/** "3 steps", "far" or "no route". */
export function reach(r: FoundRoom): string {
  if (r.steps !== undefined) return r.steps === 1 ? "1 step" : `${r.steps} steps`;
  return r.no_route ? "no route" : "far";
}

const KIND_WORD: Record<Found["kind"], string> = {
  rooms: "",
  shop: "shop",
  bank: "bank",
  trainer: "trainer",
  boss: "boss",
  place: "place",
  monster: "",
};

/** A result as its row says it: "Wooden Passage, 63 rooms, nearest 43 steps", "minotaur
 * chieftain, boss, lairs at Large Wooden Room 1:1865, 46 steps". */
export function resultText(f: Found): string {
  const near = f.rooms[0];
  const parts = [f.name];
  if (KIND_WORD[f.kind]) parts.push(KIND_WORD[f.kind]);
  if (f.kind === "monster") {
    if (f.monsters?.includes(f.name)) parts.push("boss");
    if (!near) return parts.join(", ");
    const where = `${near.name} ${near.map}:${near.room}`;
    const more = f.count > 1 ? ` (and ${f.count - 1} more)` : "";
    parts.push(`${near.boss ? "lairs" : "spawns"} at ${where}${more}, ${reach(near)}`);
    return parts.join(", ");
  }
  if (!near) return parts.join(", ");
  if (f.count > 1) {
    parts.push(`${f.count} rooms`, `nearest ${reach(near)}`);
  } else {
    parts.push(reach(near));
  }
  if (f.kind !== "boss" && f.rooms.some((r) => r.boss)) parts.push("boss lair");
  return parts.join(", ");
}

/** A room under an expanded result: "Large Wooden Room 1:1865, 46 steps, boss lair". */
export function roomText(r: FoundRoom): string {
  return `${r.name} ${r.map}:${r.room}, ${reach(r)}${r.boss ? ", boss lair" : ""}`;
}

/** The rows of the list, with the results in `expanded` (by index) opened onto their rooms. */
export function goRows(found: Found[], expanded: ReadonlySet<number>): GoRow[] {
  const rows: GoRow[] = [];
  found.forEach((f, i) => {
    const room = f.rooms.find((r) => !r.no_route);
    rows.push({ id: `${i}`, result: i, sub: false, text: resultText(f), room, expandable: f.rooms.length > 1 });
    if (expanded.has(i) && f.rooms.length > 1) {
      f.rooms.forEach((r, j) =>
        rows.push({
          id: `${i}.${j}`,
          result: i,
          sub: true,
          text: roomText(r),
          room: r.no_route ? undefined : r,
          expandable: false,
        }),
      );
    }
  });
  return rows;
}

/** The pan and level that put `room` in the middle of the map, when the view holds it. */
export function centreOn(view: MapView | undefined, zoom: number, room: RoomKey | undefined) {
  const r = room && view?.rooms.find((x) => x.map === room.map && x.room === room.room && !x.collides);
  if (!r) return null;
  const m = metrics(zoom);
  return { panX: -r.x * m.pitchX, panY: -r.y * m.pitchY, level: r.level };
}

/** The row `step` rows on from `at` (-1: none chosen), staying in the list. */
export function moveChoice(rows: GoRow[], at: number, step: number): number {
  if (rows.length === 0) return -1;
  if (at < 0) return step > 0 ? 0 : rows.length - 1;
  return Math.min(rows.length - 1, Math.max(0, at + step));
}
