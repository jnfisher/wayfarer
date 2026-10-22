import { describe, expect, it } from "vitest";
import { chip, dockSessions, loadMinimized, MINIMIZED_KEY, saveMinimized } from "../src/lib/minimized";
import type { SessionInfo } from "../src/proto";

const info = (id: string, state: SessionInfo["state"] = "realm"): SessionInfo => ({
  id,
  name: id.toUpperCase(),
  server: "127.0.0.1:2323",
  state,
  last_seq: 0,
});

function memory(initial?: string) {
  const data = new Map<string, string>(initial === undefined ? [] : [[MINIMIZED_KEY, initial]]);
  return { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
}

describe("minimized tiles", () => {
  it("keeps the ids in the browser, and forgets what it cannot read", () => {
    const s = memory();
    saveMinimized(s, new Set(["b", "d"]));
    expect(loadMinimized(s)).toEqual(["b", "d"]);
    expect(loadMinimized(memory("not json"))).toEqual([]);
    expect(loadMinimized(memory('{"a":1}'))).toEqual([]);
    expect(loadMinimized(memory('["a", 3, "c"]'))).toEqual(["a", "c"]);
    expect(loadMinimized(undefined)).toEqual([]);
    // Storage that throws is not a problem.
    const broken = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    expect(loadMinimized(broken)).toEqual([]);
    expect(() => saveMinimized(broken, ["a"])).not.toThrow();
  });

  it("docks the minimized sessions in the sessions' own order", () => {
    const all = [info("a"), info("b"), info("c")];
    expect(dockSessions(all, new Set(["c", "a"])).map((s) => s.id)).toEqual(["a", "c"]);
    expect(dockSessions(all, new Set())).toEqual([]);
  });

  it("shows a chip's HP with its tone, the state when not in the realm, and whether the bot runs", () => {
    expect(chip(info("a"), { hp: 65, hp_max: 65 })).toEqual({ text: "A 65/65", tone: "good", running: false });
    expect(chip(info("a"), { hp: 10, hp_max: 65, bot: { state: "running", mode: "fight", narration: "" } })).toEqual({
      text: "A 10/65",
      tone: "bad",
      running: true,
    });
    expect(chip(info("a", "closed"), { hp: 65, hp_max: 65 })).toEqual({ text: "A closed", tone: "none", running: false });
    expect(chip(info("a"), null).text).toBe("A --/--");
  });
});
