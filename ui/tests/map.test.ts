import { describe, expect, it } from "vitest";
import {
  carryPan,
  clampRadius,
  COLOURS,
  describeRoom,
  drawMap,
  MAP_RADIUS,
  MAX_MAP_RADIUS,
  metrics,
  roomAt,
  roomBox,
  roomCentre,
  walkText,
  zoomAt,
  ZOOMS,
  type Ctx,
  type MapView,
  type ViewState,
} from "../src/lib/map";
import type { MapRoom } from "../src/proto";

/** A 2D context that records what is drawn: rectangles, lines and text with their styles. */
class FakeCtx implements Ctx {
  fillStyle: Ctx["fillStyle"] = "";
  strokeStyle: Ctx["strokeStyle"] = "";
  lineWidth = 1;
  font = "";
  textAlign: CanvasTextAlign = "start";
  textBaseline: CanvasTextBaseline = "alphabetic";
  dash: number[] = [];
  at: [number, number] = [0, 0];
  calls: { op: string; args: number[]; style: string; dash: boolean; text?: string }[] = [];

  private rec(op: string, args: number[], style: string, text?: string) {
    this.calls.push({ op, args, style, dash: this.dash.length > 0, text });
  }
  setLineDash(d: number[]) {
    this.dash = d;
  }
  beginPath() {}
  moveTo(x: number, y: number) {
    this.at = [x, y];
  }
  lineTo(x: number, y: number) {
    this.rec("line", [...this.at, x, y], String(this.strokeStyle));
    this.at = [x, y];
  }
  closePath() {}
  stroke() {}
  fill() {
    this.rec("fill", [], String(this.fillStyle));
  }
  fillRect(x: number, y: number, w: number, h: number) {
    this.rec("fillRect", [x, y, w, h], String(this.fillStyle));
  }
  strokeRect(x: number, y: number, w: number, h: number) {
    this.rec("strokeRect", [x, y, w, h], String(this.strokeStyle));
  }
  fillText(text: string, x: number, y: number) {
    this.rec("text", [x, y], String(this.fillStyle), text);
  }
  texts() {
    return this.calls.filter((c) => c.op === "text").map((c) => c.text);
  }
}

const room = (n: number, x: number, y: number, extra: Partial<MapRoom> = {}): MapRoom => ({
  map: 1,
  room: n,
  name: `Room ${n}`,
  x,
  y,
  level: 0,
  spawns: false,
  visited: false,
  candidate: false,
  collides: false,
  ...extra,
});
const k = (n: number) => ({ map: 1, room: n });

// 10 (here) - e - 11 - door e - 12 ; 10 - n - 13 (shop, spawns) ; 11 - u - 14 (level 1) ;
// 12 -> one-way s -> 15 ; 15 collides with nothing but 16 collides at 13's cell.
const VIEW: MapView = {
  type: "map_view",
  session: "aria",
  center: k(10),
  level: 0,
  rooms: [
    room(10, 0, 0, { visited: true, candidate: true }),
    room(11, 1, 0, { visited: true }),
    room(12, 2, 0, { candidate: true }),
    room(13, 0, -1, { kind: "shop", spawns: true }),
    room(14, 1, 0, { level: 1 }),
    room(15, 2, 1),
    room(16, 0, -1, { collides: true }),
  ],
  exits: [
    { from: k(10), to: k(11), dir: "e", kind: "plain", one_way: false },
    { from: k(11), to: k(12), dir: "e", kind: "door", one_way: false },
    { from: k(10), to: k(13), dir: "n", kind: "hidden", one_way: false },
    { from: k(11), to: k(14), dir: "u", kind: "plain", one_way: false },
    { from: k(12), to: k(15), dir: "s", kind: "plain", one_way: true },
    { from: k(13), to: k(16), dir: "w", kind: "plain", one_way: false },
  ],
};

const vs = (over: Partial<ViewState> = {}): ViewState => ({
  width: 400,
  height: 300,
  zoom: 2,
  panX: 0,
  panY: 0,
  level: 0,
  ...over,
});

describe("radius", () => {
  it("is 15 by default and never more than 30", () => {
    expect(clampRadius(undefined)).toBe(MAP_RADIUS);
    expect(clampRadius(Number.NaN)).toBe(MAP_RADIUS);
    expect(clampRadius(99)).toBe(MAX_MAP_RADIUS);
    expect(clampRadius(0)).toBe(1);
    expect(clampRadius(7.4)).toBe(7);
  });
});

describe("drawing a map_view", () => {
  const ctx = new FakeCtx();
  drawMap(ctx, VIEW, vs(), "VGA");
  const m = metrics(2);

  it("clears to black and uses the VGA font at 16 px", () => {
    expect(ctx.calls[0]).toMatchObject({ op: "fillRect", args: [0, 0, 400, 300], style: COLOURS.black });
    expect(ctx.font).toBe("16px VGA");
  });

  it("draws each room of the level as a 3x2-cell box at its (x, y)", () => {
    expect(m).toMatchObject({ boxW: 24, boxH: 32, pitchX: 40, pitchY: 48 });
    const fills = ctx.calls.filter((c) => c.op === "fillRect").slice(1);
    // 10, 11, 12, 13, 15: not 14 (level 1) nor 16 (collided, no fill).
    expect(fills.map((c) => c.args)).toEqual([
      [188, 134, 24, 32],
      [228, 134, 24, 32],
      [268, 134, 24, 32],
      [188, 86, 24, 32],
      [268, 182, 24, 32],
    ]);
    expect(fills[3].style).toBe(COLOURS.spawn);
    expect(fills[0].style).toBe(COLOURS.black);
  });

  it("frames the believed room white and doubled, candidates yellow, visited brighter", () => {
    const frames = ctx.calls.filter((c) => c.op === "strokeRect");
    const at = (x: number, y: number) => frames.filter((c) => c.args[0] < x + 8 && c.args[0] >= x && c.args[1] >= y && c.args[1] < y + 8);
    expect(at(188, 134).map((c) => c.style)).toEqual([COLOURS.white, COLOURS.white]);
    expect(at(268, 134).map((c) => c.style)).toEqual([COLOURS.yellow]);
    expect(at(228, 134).map((c) => c.style)).toEqual([COLOURS.lightgray]);
    expect(at(268, 182).map((c) => c.style)).toEqual([COLOURS.darkgray]);
  });

  it("draws a collided room as a dashed frame and none of its exits", () => {
    const dashed = ctx.calls.filter((c) => c.op === "strokeRect" && c.dash);
    expect(dashed).toHaveLength(1);
    expect(dashed[0].args[0]).toBeGreaterThan(188);
    // 13-16 (w) is not drawn: the lines are 10-11, 11-12 (two halves), 10-13, 12-15 and the
    // arrow head's two sides.
    const lines = ctx.calls.filter((c) => c.op === "line");
    expect(lines.filter((c) => c.args[0] < 188 || c.args[2] < 188)).toEqual([]);
  });

  it("draws exits by kind: solid, a door's gap and +, hidden dotted, one-way with a head", () => {
    const lines = ctx.calls.filter((c) => c.op === "line");
    // 10 e 11: from 10's east edge to 11's west edge, solid.
    expect(lines).toContainEqual(expect.objectContaining({ args: [212, 150, 228, 150], dash: false }));
    // 11 e 12, a door: two halves around x = 260 and a +.
    expect(lines.filter((c) => c.args[1] === 150 && c.args[0] >= 252 && c.args[2] <= 268).map((c) => c.args)).toEqual([
      [252, 150, 256, 150],
      [264, 150, 268, 150],
    ]);
    expect(ctx.calls).toContainEqual(expect.objectContaining({ op: "text", text: "+", args: [260, 150] }));
    // 10 n 13: hidden, dotted, from 10's top to 13's bottom.
    expect(lines).toContainEqual(expect.objectContaining({ args: [200, 134, 200, 118], dash: true }));
    // 12 s 15: one way, an arrow head filled at 15's top.
    expect(lines).toContainEqual(expect.objectContaining({ args: [280, 166, 280, 182] }));
    expect(ctx.calls.filter((c) => c.op === "fill")).toHaveLength(1);
  });

  it("marks up and down exits, kinds by letter", () => {
    expect(ctx.texts()).toEqual(expect.arrayContaining(["▲", "S", "+"]));
    const up = ctx.calls.find((c) => c.text === "▲")!;
    expect(up.args).toEqual([232, 142]); // 11's first cell, top row
    expect(ctx.texts()).not.toContain("▼");
  });

  it("shows the other level with a ▼ where the stairs come up", () => {
    const c = new FakeCtx();
    drawMap(c, VIEW, vs({ level: 1 }), "VGA");
    expect(c.texts()).toEqual(["▼"]);
  });

  it("draws the route as a path with step numbers", () => {
    const c = new FakeCtx();
    drawMap(c, { ...VIEW, route: [k(10), k(11), k(12)] }, vs(), "VGA");
    const route = c.calls.filter((x) => x.op === "line" && x.style === COLOURS.lightgreen);
    expect(route.map((x) => x.args)).toEqual([
      [200, 150, 240, 150],
      [240, 150, 280, 150],
    ]);
    expect(c.calls.filter((x) => x.op === "text" && x.style === COLOURS.lightgreen).map((x) => x.text)).toEqual(["1", "2"]);
  });
});

describe("hover and zoom", () => {
  it("finds the room under the pointer, a placed one before one that collided", () => {
    expect(roomAt(VIEW, vs(), 200, 150)?.room).toBe(10);
    expect(roomAt(VIEW, vs(), 200, 100)?.room).toBe(13);
    expect(roomAt(VIEW, vs(), 220, 150)).toBeNull(); // between boxes
    expect(roomAt(VIEW, vs({ level: 1 }), 240, 150)?.room).toBe(14);
    expect(roomAt(VIEW, vs({ panX: 40 }), 240, 150)?.room).toBe(10);
  });

  it("describes a room: name, map:room, spawns, exits, the belief", () => {
    expect(describeRoom(VIEW, VIEW.rooms[1])).toEqual([
      "Room 11",
      "1:11",
      "no spawns",
      "exits: w, e (door), u",
      "visited",
    ]);
    expect(describeRoom(VIEW, VIEW.rooms[3])).toEqual([
      "Room 13",
      "1:13 shop",
      "monsters spawn here",
      "exits: s (hidden), w",
    ]);
    expect(describeRoom(VIEW, VIEW.rooms[0]).at(-1)).toBe("believed here, visited");
  });

  it("has three cell sizes and zooms about the pointer", () => {
    expect(ZOOMS).toHaveLength(3);
    expect([0, 1, 2].map((z) => metrics(z).font)).toEqual([8, 12, 16]);
    const v = vs();
    // Out, at room 11's centre: 11 stays under the pointer.
    const out = zoomAt(v, -1, 240, 150);
    expect(out.zoom).toBe(1);
    expect(roomCentre(VIEW.rooms[1], { ...v, ...out })).toEqual([240, 150]);
    // Already at the largest: nothing changes.
    expect(zoomAt(v, 1, 0, 0)).toEqual({ zoom: 2, panX: 0, panY: 0 });
    expect(roomBox(VIEW.rooms[0], { ...v, zoom: 0 })).toEqual([194, 142, 12, 16]);
  });

  it("keeps a panned view still when the centre moves", () => {
    // The character moved east to 11: the new view is laid out around it.
    const next: MapView = { ...VIEW, center: k(11), rooms: VIEW.rooms.map((r) => ({ ...r, x: r.x - 1 })) };
    const v = vs({ panX: 15, panY: -5 });
    const pan = carryPan(VIEW, next, v);
    expect(roomCentre(next.rooms[2], { ...v, ...pan })).toEqual(roomCentre(VIEW.rooms[2], v));
    expect(carryPan(VIEW, { ...next, rooms: [] }, v)).toEqual({ panX: 15, panY: -5 });
  });
});

describe("the route bar", () => {
  it("lists the walker's commands still to type", () => {
    const walking = { to: k(15), name: "Small Pier", steps_left: 3, step: 2, commands: ["n", "e", "borrow skiff"] };
    expect(walkText(walking)).toBe("n, e, borrow skiff");
    expect(walkText(undefined)).toBe("");
    // From a brain that did not send them.
    expect(walkText({ ...walking, commands: undefined } as never)).toBe("");
  });
});
