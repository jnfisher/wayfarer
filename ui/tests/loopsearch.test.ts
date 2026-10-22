import { describe, expect, it } from "vitest";
import { matches, shownLoops } from "../src/lib/loopsearch";
import type { Loops } from "../src/lib/map";

const stop = (name: string) => ({ map: 1, room: 1, name, monsters: [], arena: false, lair: false, regen: 0, bosses: [] });
const loops = {
  type: "loops",
  session: "a",
  anchor: { map: 1, room: 1 },
  radius: 8,
  doors: true,
  generated: [
    { id: "g1", name: "Newhaven", stops: [stop("Arena"), stop("Dungeon Entrance")], path: [], steps: 4, doors: 1 },
    { id: "g2", name: "Hidden Keep", stops: [stop("Courtyard"), stop("Keep Gate")], path: [], steps: 9, doors: 2 },
  ],
  megamud: [
    {
      file: "HKEPLOOP.mp",
      description: "Hidden Keep loop",
      author: "Leo",
      path: [],
      steps: 30,
      exact: 30,
      stops: [stop("Courtyard")],
      runnable: true,
    },
  ],
  custom: [{ id: "custom:mine", name: "Keep with Leo the Quick", stops: [stop("Courtyard")], path: [], steps: 5, doors: 0 }],
} as unknown as Loops;

describe("finding a loop", () => {
  it("needs every word, in any case, anywhere in the loop's texts", () => {
    expect(matches("", ["anything"])).toBe(true);
    expect(matches("keep gate", ["Hidden Keep", "Keep Gate"])).toBe(true);
    expect(matches("KEEP leo", ["Keep with Leo the Quick"])).toBe(true);
    expect(matches("keep zzz", ["Hidden Keep"])).toBe(false);
    expect(matches("x", [undefined])).toBe(false);
  });

  it("looks at the name, a stop's room, the file, the description and the author", () => {
    const names = (q: string) => {
      const s = shownLoops(loops, q);
      return [...s.generated.map((l) => l.id), ...s.custom.map((l) => l.id), ...s.megamud.map((m) => m.file)];
    };
    expect(names("")).toEqual(["g1", "g2", "custom:mine", "HKEPLOOP.mp"]);
    expect(names("arena")).toEqual(["g1"]);
    expect(names("courtyard")).toEqual(["g2", "custom:mine", "HKEPLOOP.mp"]);
    expect(names("hkep")).toEqual(["HKEPLOOP.mp"]);
    expect(names("leo")).toEqual(["custom:mine", "HKEPLOOP.mp"]);
    expect(names("nothing like it")).toEqual([]);
  });

  it("counts what the list has and what the query leaves", () => {
    const s = shownLoops(loops, "courtyard");
    expect([s.count, s.total]).toEqual([3, 4]);
    expect(shownLoops(undefined, "x")).toMatchObject({ count: 0, total: 0 });
  });
});
