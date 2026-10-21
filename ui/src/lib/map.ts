// The map panel's drawing (docs/web-ui.md, "The map"): a `map_view` from the brain plus the
// panel's view state in, draw calls on a 2D canvas out. No Svelte here, so the tests drive it
// with a fake context that records the calls, and a terminal renderer could reuse the rest.
//
// The brain lays the rooms out and plans the routes; this file only draws what it was sent.

import type { GeneratedLoop, LoopStop, MapExit, MapRoom, RoomKey, RoomKind, ToClient, Walking } from "../proto";

export type MapView = Extract<ToClient, { type: "map_view" }>;
export type Loops = Extract<ToClient, { type: "loops" }>;

/** The area a `loops_request` asks about: the rooms within this many steps of the character. */
export const LOOP_RADIUS = 8;

/** The radius a `map_request` asks for, and the most it ever asks for (C-022's review). */
export const MAP_RADIUS = 15;
export const MAX_MAP_RADIUS = 30;

/** A radius fit to send: whole steps from 1 to `MAX_MAP_RADIUS`, `MAP_RADIUS` if not a number. */
export function clampRadius(radius: number | undefined): number {
  if (radius === undefined || !Number.isFinite(radius)) return MAP_RADIUS;
  return Math.min(MAX_MAP_RADIUS, Math.max(1, Math.round(radius)));
}

/** The three cell sizes, as fractions of the VGA font's 8x16 cell; the last is the default. */
export const ZOOMS = [0.5, 0.75, 1] as const;
export const DEFAULT_ZOOM = ZOOMS.length - 1;

/** A room is a box of 3x2 cells; rooms sit 5 cells apart across and 3 down. */
const BOX_CELLS = [3, 2] as const;
const PITCH_CELLS = [5, 3] as const;

export interface Metrics {
  cellW: number;
  cellH: number;
  boxW: number;
  boxH: number;
  pitchX: number;
  pitchY: number;
  /** The font size in px. */
  font: number;
}

export function metrics(zoom: number): Metrics {
  const s = ZOOMS[Math.min(Math.max(zoom, 0), ZOOMS.length - 1)];
  const cellW = 8 * s;
  const cellH = 16 * s;
  return {
    cellW,
    cellH,
    boxW: BOX_CELLS[0] * cellW,
    boxH: BOX_CELLS[1] * cellH,
    pitchX: PITCH_CELLS[0] * cellW,
    pitchY: PITCH_CELLS[1] * cellH,
    font: 16 * s,
  };
}

/** What the panel adds to a `map_view`: its size, zoom, pan, the level shown and the hover. */
export interface ViewState {
  width: number;
  height: number;
  zoom: number;
  /** Pixels the believed room sits from the canvas' middle. */
  panX: number;
  panY: number;
  level: number;
  hover?: RoomKey | null;
  /** A loop to draw: every room it passes, and its stops (numbered in loop order). */
  loop?: { path: RoomKey[]; stops: RoomKey[] } | null;
}

/** The part of `CanvasRenderingContext2D` the drawing uses. */
export interface Ctx {
  fillStyle: string | CanvasGradient | CanvasPattern;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  lineWidth: number;
  font: string;
  textAlign: CanvasTextAlign;
  textBaseline: CanvasTextBaseline;
  setLineDash(segments: number[]): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  closePath(): void;
  stroke(): void;
  fill(): void;
  fillRect(x: number, y: number, w: number, h: number): void;
  strokeRect(x: number, y: number, w: number, h: number): void;
  fillText(text: string, x: number, y: number): void;
}

/** The 16 colours the map uses, at their VGA values (theme.ts). */
export const COLOURS = {
  black: "#000000",
  brown: "#aa5500",
  lightgray: "#aaaaaa",
  darkgray: "#555555",
  white: "#ffffff",
  yellow: "#ffff55",
  lightgreen: "#55ff55",
  lightcyan: "#55ffff",
  lightred: "#ff5555",
  /** A dim red fill for rooms that spawn monsters: VGA red at a third over black. */
  spawn: "#390000",
} as const;

export const KIND_LETTERS: Record<RoomKind, string> = {
  shop: "S",
  trainer: "T",
  bank: "B",
  arena: "A",
  lair: "L",
};

export const OPPOSITE: Record<string, string> = {
  n: "s",
  s: "n",
  e: "w",
  w: "e",
  ne: "sw",
  sw: "ne",
  nw: "se",
  se: "nw",
  u: "d",
  d: "u",
};

/** Where on a box's frame an exit in that direction leaves, as fractions of its size. */
const ANCHORS: Record<string, [number, number]> = {
  n: [0.5, 0],
  s: [0.5, 1],
  e: [1, 0.5],
  w: [0, 0.5],
  ne: [1, 0],
  nw: [0, 0],
  se: [1, 1],
  sw: [0, 1],
};

export const keyOf = (k: RoomKey): string => `${k.map}:${k.room}`;
const same = (a: RoomKey, b: RoomKey) => a.map === b.map && a.room === b.room;

/** The centre of a room's box on the canvas. */
export function roomCentre(r: { x: number; y: number }, v: ViewState): [number, number] {
  const m = metrics(v.zoom);
  return [Math.round(v.width / 2 + v.panX + r.x * m.pitchX), Math.round(v.height / 2 + v.panY + r.y * m.pitchY)];
}

/** A room's box: left, top, width, height. */
export function roomBox(r: { x: number; y: number }, v: ViewState): [number, number, number, number] {
  const m = metrics(v.zoom);
  const [cx, cy] = roomCentre(r, v);
  return [cx - m.boxW / 2, cy - m.boxH / 2, m.boxW, m.boxH];
}

/** The rooms whose box holds the point, on the level shown: a placed room before one that collided. */
export function roomsAt(view: MapView, v: ViewState, px: number, py: number): MapRoom[] {
  return view.rooms
    .filter((r) => {
      if (r.level !== v.level) return false;
      const [x, y, w, h] = roomBox(r, v);
      return px >= x && px < x + w && py >= y && py < y + h;
    })
    .sort((a, b) => Number(a.collides) - Number(b.collides));
}

/** The room under the point, if any. */
export function roomAt(view: MapView, v: ViewState, px: number, py: number): MapRoom | null {
  return roomsAt(view, v, px, py)[0] ?? null;
}

/** A room's exits within the view, as seen from it: `[dir, exit]`, the far side's reversed. */
export function exitsOf(view: MapView, room: RoomKey): [string, MapExit][] {
  const out: [string, MapExit][] = [];
  for (const e of view.exits) {
    if (same(e.from, room)) out.push([e.dir, e]);
    else if (same(e.to, room) && !e.one_way) out.push([OPPOSITE[e.dir] ?? e.dir, e]);
  }
  return out;
}

/** The tooltip for a room: name, map:room and kind, what spawns, exits, and the belief. */
export function describeRoom(view: MapView, room: MapRoom): string[] {
  const lines = [room.name || "(no name)", `${room.map}:${room.room}${room.kind ? ` ${room.kind}` : ""}`];
  lines.push(room.spawns ? "monsters spawn here" : "no spawns");
  const exits = exitsOf(view, room).map(([dir, e]) => (e.kind === "plain" ? dir : `${dir} (${e.kind})`));
  lines.push(`exits: ${exits.length ? exits.join(", ") : "none in view"}`);
  const flags = [
    same(view.center, room) ? "believed here" : room.candidate ? "maybe here" : "",
    room.visited ? "visited" : "",
    room.collides ? "drawn over another room" : "",
  ].filter(Boolean);
  if (flags.length) lines.push(flags.join(", "));
  return lines;
}

/** A room staged to go into a loop the player is making, and the stop it goes after. */
export interface StagedStop {
  map: number;
  room: number;
  name: string;
  /** After the loop's stop this many (1-based, as listed; 0 puts it first); the end when null. */
  after: number | null;
}

/**
 * The loop's stops with the staged rooms put in, as `save_loop` takes them. Each goes after the
 * stop it names, counted in the loop as listed, so adding one does not shift another; several
 * after the same stop keep their order; a number past the end, or none, is the end.
 */
export function insertStops(base: RoomKey[], adds: StagedStop[]): RoomKey[] {
  const key = (s: StagedStop): RoomKey => ({ map: s.map, room: s.room });
  const after = (n: number) => adds.filter((a) => a.after === n).map(key);
  const out: RoomKey[] = after(0);
  base.forEach((stop, i) => {
    out.push({ map: stop.map, room: stop.room }, ...after(i + 1));
  });
  out.push(...adds.filter((a) => a.after === null || a.after > base.length).map(key));
  return out;
}

/** A generated loop's size: `3 stops, 4 steps, 1 door`. */
export function loopText(l: GeneratedLoop): string {
  const n = (k: number, one: string) => `${k} ${one}${k === 1 ? "" : "s"}`;
  const parts = [n(l.stops.length, "stop"), n(l.steps, "step")];
  if (l.doors > 0) parts.push(n(l.doors, "door"));
  return parts.join(", ");
}

/** Minutes as `1 h 20 m`, `2 h`, `45 m`. */
export function minutesText(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} m`;
  return m === 0 ? `${h} h` : `${h} h ${m} m`;
}

/**
 * A loop stop's tooltip: what spawns there, how soon it is back after a kill, and for a boss
 * whether it should be up (the character's own kills; other players kill bosses too).
 */
export function stopTip(s: LoopStop): string {
  const what = s.monsters.length > 0 ? s.monsters.join(", ") : "nothing listed";
  const back = s.arena ? "arena: back at once" : `back ${s.regen > 0 ? s.regen : 5} m after a kill`;
  const bosses = (s.bosses ?? []).map((b) =>
    b.up_in_minutes === undefined
      ? `${b.name} should be up`
      : `${b.name} should be up in ${minutesText(b.up_in_minutes)}`,
  );
  return `${what}; ${s.lair ? "lair, " : ""}${back}${bosses.length > 0 ? `; ${bosses.join("; ")}` : ""}`;
}

/** The route bar's line: the walker's commands still to type, `n, n, e, borrow skiff`. */
export function walkText(walking: Walking | undefined): string {
  return walking?.commands?.join(", ") ?? "";
}

/** One zoom step at the point (px, py), keeping what is under it in place. */
export function zoomAt(v: ViewState, step: number, px: number, py: number): Pick<ViewState, "zoom" | "panX" | "panY"> {
  const zoom = Math.min(Math.max(v.zoom + step, 0), ZOOMS.length - 1);
  if (zoom === v.zoom) return { zoom, panX: v.panX, panY: v.panY };
  const [from, to] = [metrics(v.zoom), metrics(zoom)];
  const ux = (px - v.width / 2 - v.panX) / from.pitchX;
  const uy = (py - v.height / 2 - v.panY) / from.pitchY;
  return { zoom, panX: px - v.width / 2 - ux * to.pitchX, panY: py - v.height / 2 - uy * to.pitchY };
}

/**
 * The pan that keeps a panned view still when a new `map_view` arrives: each view is laid out
 * around its own centre, so shift by where the old centre sits in the new one. Unchanged when
 * the old centre is not in it.
 */
export function carryPan(prev: MapView, next: MapView, v: ViewState): Pick<ViewState, "panX" | "panY"> {
  const old = next.rooms.find((r) => same(r, prev.center));
  if (!old) return { panX: v.panX, panY: v.panY };
  const m = metrics(v.zoom);
  return { panX: v.panX - old.x * m.pitchX, panY: v.panY - old.y * m.pitchY };
}

/** Draw the view. `fontFamily` is the VGA font's CSS family. */
export function drawMap(ctx: Ctx, view: MapView, v: ViewState, fontFamily: string): void {
  const m = metrics(v.zoom);
  const s = ZOOMS[v.zoom] ?? 1;
  ctx.setLineDash([]);
  ctx.fillStyle = COLOURS.black;
  ctx.fillRect(0, 0, v.width, v.height);
  ctx.font = `${m.font}px ${fontFamily}`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";

  const shown = view.rooms.filter((r) => r.level === v.level);
  const byKey = new Map(shown.map((r) => [keyOf(r), r]));
  const up = new Set<string>();
  const down = new Set<string>();
  for (const e of view.exits) {
    const [f, t] = [keyOf(e.from), keyOf(e.to)];
    if (e.dir === "u" || e.dir === "d") {
      (e.dir === "u" ? up : down).add(f);
      if (!e.one_way) (e.dir === "u" ? down : up).add(t);
    }
  }

  // Exits: between two placed rooms of this level, in a compass direction.
  for (const e of view.exits) {
    const [a, b] = [byKey.get(keyOf(e.from)), byKey.get(keyOf(e.to))];
    if (!a || !b || a.collides || b.collides || !ANCHORS[e.dir]) continue;
    drawExit(ctx, e, a, b, v, s);
  }

  // A loop hovered or picked in the loops list, in yellow under the route.
  const loop = v.loop?.path ?? [];
  const stop = new Map<string, number>();
  v.loop?.stops.forEach((k, i) => stop.set(keyOf(k), i + 1));
  ctx.setLineDash([]);
  ctx.strokeStyle = COLOURS.yellow;
  ctx.lineWidth = Math.max(2, 3 * s);
  for (let i = 0; i + 1 < loop.length; i++) {
    const [a, b] = [byKey.get(keyOf(loop[i])), byKey.get(keyOf(loop[i + 1]))];
    if (!a || !b) continue;
    ctx.beginPath();
    ctx.moveTo(...roomCentre(a, v));
    ctx.lineTo(...roomCentre(b, v));
    ctx.stroke();
  }

  // The route, over the exits and under the boxes.
  const route = view.route ?? [];
  const step = new Map<string, number>();
  route.forEach((k, i) => i > 0 && step.set(keyOf(k), i));
  ctx.setLineDash([]);
  ctx.strokeStyle = COLOURS.lightgreen;
  ctx.lineWidth = Math.max(2, 3 * s);
  for (let i = 0; i + 1 < route.length; i++) {
    const [a, b] = [byKey.get(keyOf(route[i])), byKey.get(keyOf(route[i + 1]))];
    if (!a || !b) continue;
    ctx.beginPath();
    ctx.moveTo(...roomCentre(a, v));
    ctx.lineTo(...roomCentre(b, v));
    ctx.stroke();
  }

  // Boxes: placed rooms, then the ones that collided on top as dashed frames.
  const order = [...shown.filter((r) => !r.collides), ...shown.filter((r) => r.collides)];
  for (const r of order) {
    const k = keyOf(r);
    drawRoom(ctx, view, r, v, m, {
      up: up.has(k),
      down: down.has(k),
      step: step.get(k),
      stop: stop.get(k),
      hover: !!v.hover && same(v.hover, r),
    });
  }
}

function drawExit(ctx: Ctx, e: MapExit, a: MapRoom, b: MapRoom, v: ViewState, s: number): void {
  const [ax, ay, aw, ah] = roomBox(a, v);
  const [bx, by, bw, bh] = roomBox(b, v);
  const [fa, fb] = [ANCHORS[e.dir], ANCHORS[OPPOSITE[e.dir]]];
  const x1 = ax + fa[0] * aw;
  const y1 = ay + fa[1] * ah;
  const x2 = bx + fb[0] * bw;
  const y2 = by + fb[1] * bh;
  ctx.strokeStyle = a.visited || b.visited ? COLOURS.lightgray : COLOURS.darkgray;
  ctx.lineWidth = Math.max(1, Math.round(2 * s));
  ctx.setLineDash(e.kind === "hidden" || e.kind === "text" ? [2 * s, 3 * s] : []);
  if (e.kind === "door") {
    // A gap in the middle with a `+` in it.
    const gap = 0.5 * metrics(v.zoom).cellW;
    const [mx, my] = [(x1 + x2) / 2, (y1 + y2) / 2];
    const len = Math.hypot(x2 - x1, y2 - y1) || 1;
    const [ux, uy] = [(x2 - x1) / len, (y2 - y1) / len];
    line(ctx, x1, y1, mx - ux * gap, my - uy * gap);
    line(ctx, mx + ux * gap, my + uy * gap, x2, y2);
    ctx.fillStyle = COLOURS.brown;
    ctx.fillText("+", mx, my);
  } else {
    line(ctx, x1, y1, x2, y2);
  }
  if (e.one_way) {
    // An arrow head at the far end.
    ctx.setLineDash([]);
    const len = Math.hypot(x2 - x1, y2 - y1) || 1;
    const [ux, uy] = [(x2 - x1) / len, (y2 - y1) / len];
    const size = 5 * s + 2;
    ctx.fillStyle = ctx.strokeStyle as string;
    ctx.beginPath();
    ctx.moveTo(x2, y2);
    ctx.lineTo(x2 - ux * size - uy * size * 0.6, y2 - uy * size + ux * size * 0.6);
    ctx.lineTo(x2 - ux * size + uy * size * 0.6, y2 - uy * size - ux * size * 0.6);
    ctx.closePath();
    ctx.fill();
  }
}

function line(ctx: Ctx, x1: number, y1: number, x2: number, y2: number): void {
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.stroke();
}

interface Marks {
  up: boolean;
  down: boolean;
  /** Its step on the route, from 1. */
  step?: number;
  /** Its stop on the loop drawn, from 1. */
  stop?: number;
  hover: boolean;
}

function drawRoom(ctx: Ctx, view: MapView, r: MapRoom, v: ViewState, m: Metrics, marks: Marks): void {
  const [x, y, w, h] = roomBox(r, v);
  const s = ZOOMS[v.zoom] ?? 1;
  const here = same(view.center, r);
  const lw = Math.max(1, Math.round(2 * s));
  ctx.setLineDash([]);
  if (!r.collides) {
    ctx.fillStyle = r.spawns ? COLOURS.spawn : COLOURS.black;
    ctx.fillRect(x, y, w, h);
  }
  ctx.lineWidth = lw;
  ctx.strokeStyle = marks.hover
    ? COLOURS.lightcyan
    : here
      ? COLOURS.white
      : r.candidate
        ? COLOURS.yellow
        : r.visited
          ? COLOURS.lightgray
          : COLOURS.darkgray;
  if (r.collides) {
    // Drawn where it would be, over the room that took the cell, inset and dashed.
    ctx.setLineDash([3 * s, 3 * s]);
    const i = 3 * lw;
    ctx.strokeRect(x + i, y + i, w - 2 * i, h - 2 * i);
    ctx.setLineDash([]);
    return;
  }
  ctx.strokeRect(x + lw / 2, y + lw / 2, w - lw, h - lw);
  if (here) {
    const i = lw * 2.5;
    ctx.strokeRect(x + i, y + i, w - 2 * i, h - 2 * i);
  }

  // Glyphs: top row ▲ kind ▼, bottom row the route step.
  const col = (c: number) => x + (c + 0.5) * m.cellW;
  const top = y + 0.5 * m.cellH;
  const bottom = y + 1.5 * m.cellH;
  if (marks.up || marks.down) {
    ctx.fillStyle = COLOURS.lightcyan;
    if (marks.up) ctx.fillText("▲", col(0), top);
    if (marks.down) ctx.fillText("▼", col(2), top);
  }
  if (r.kind) {
    ctx.fillStyle = COLOURS.yellow;
    ctx.fillText(KIND_LETTERS[r.kind], col(1), top);
  }
  if (marks.step !== undefined) {
    ctx.fillStyle = COLOURS.lightgreen;
    ctx.fillText(String(marks.step), col(1), bottom);
  } else if (marks.stop !== undefined) {
    ctx.fillStyle = COLOURS.yellow;
    ctx.fillText(String(marks.stop), col(1), bottom);
  }
}
