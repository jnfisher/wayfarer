import { describe, expect, it } from "vitest";
import { centreOn, goRows, moveChoice, reach, resultText, roomText } from "../src/lib/goto";
import { metrics, type MapView } from "../src/lib/map";
import type { Found, FoundRoom } from "../src/proto";

const room = (r: number, name: string, extra: Partial<FoundRoom> = {}): FoundRoom => ({
  map: 1,
  room: r,
  name,
  boss: false,
  ...extra,
});

const passage: Found = {
  kind: "rooms",
  name: "Wooden Passage",
  rooms: [room(1839, "Wooden Passage", { steps: 43 }), room(1840, "Wooden Passage", { steps: 44 })],
  count: 63,
};
const chief: Found = {
  kind: "monster",
  name: "minotaur chieftain",
  rooms: [room(1865, "Large Wooden Room", { steps: 46, boss: true })],
  count: 1,
  monsters: ["minotaur chieftain"],
};
const skali: Found = {
  kind: "shop",
  name: "Skali's Fine Armour, Front Room",
  rooms: [room(305, "Skali's Fine Armour, Front Room", { steps: 7 })],
  count: 1,
};
const island: Found = {
  kind: "rooms",
  name: "Tournament Arena",
  rooms: [room(1, "Tournament Arena", { no_route: true })],
  count: 25,
};

describe("the go box's rows", () => {
  it("say how far: steps, far or no route", () => {
    expect(reach(room(1, "A", { steps: 1 }))).toBe("1 step");
    expect(reach(room(1, "A", { steps: 46 }))).toBe("46 steps");
    expect(reach(room(1, "A"))).toBe("far");
    expect(reach(room(1, "A", { no_route: true }))).toBe("no route");
  });

  it("say a result as the card does", () => {
    expect(resultText(passage)).toBe("Wooden Passage, 63 rooms, nearest 43 steps");
    expect(resultText(chief)).toBe("minotaur chieftain, boss, lairs at Large Wooden Room 1:1865, 46 steps");
    expect(resultText(skali)).toBe("Skali's Fine Armour, Front Room, shop, 7 steps");
    expect(resultText(island)).toBe("Tournament Arena, 25 rooms, nearest no route");
    expect(roomText(chief.rooms[0])).toBe("Large Wooden Room 1:1865, 46 steps, boss lair");
  });

  it("list a result's rooms under it when it is opened; a room with no route goes nowhere", () => {
    const closed = goRows([passage, chief, island], new Set());
    expect(closed.map((r) => r.id)).toEqual(["0", "1", "2"]);
    expect(closed[0].expandable).toBe(true);
    expect(closed[0].room?.room).toBe(1839);
    expect(closed[2].room).toBeUndefined();
    const open = goRows([passage, chief], new Set([0]));
    expect(open.map((r) => [r.id, r.sub])).toEqual([
      ["0", false],
      ["0.0", true],
      ["0.1", true],
      ["1", false],
    ]);
    expect(open[2].room?.room).toBe(1840);
  });

  it("move the choice within the list", () => {
    const rows = goRows([passage, chief], new Set());
    expect(moveChoice(rows, -1, 1)).toBe(0);
    expect(moveChoice(rows, -1, -1)).toBe(1);
    expect(moveChoice(rows, 0, 1)).toBe(1);
    expect(moveChoice(rows, 1, 1)).toBe(1);
    expect(moveChoice(rows, 0, -1)).toBe(0);
    expect(moveChoice([], 0, 1)).toBe(-1);
  });

  it("centre the map on a chosen room that is in view", () => {
    const view = {
      type: "map_view",
      session: "aria",
      center: { map: 1, room: 10 },
      level: 0,
      rooms: [
        { map: 1, room: 10, name: "Gate", x: 0, y: 0, level: 0, spawns: false, visited: true, candidate: true, collides: false },
        { map: 1, room: 12, name: "Cellar", x: 2, y: -1, level: -1, spawns: false, visited: false, candidate: false, collides: false },
      ],
      exits: [],
    } satisfies MapView;
    const m = metrics(2);
    expect(centreOn(view, 2, { map: 1, room: 12 })).toEqual({ panX: -2 * m.pitchX, panY: m.pitchY, level: -1 });
    expect(centreOn(view, 2, { map: 1, room: 99 })).toBeNull();
    expect(centreOn(undefined, 2, { map: 1, room: 12 })).toBeNull();
  });
});
