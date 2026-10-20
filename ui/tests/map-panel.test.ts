// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import MapPanel from "../src/MapPanel.svelte";
import type { Loops, MapView } from "../src/lib/map";
import type { CharacterState } from "../src/proto";

const session = { id: "aria", name: "Aria", server: "silvermere", state: "realm", last_seq: 0 } as const;
const view: MapView = {
  type: "map_view",
  session: "aria",
  center: { map: 1, room: 10 },
  level: 0,
  rooms: [
    { map: 1, room: 10, name: "Gate", x: 0, y: 0, level: 0, spawns: false, visited: true, candidate: true, collides: false },
    { map: 1, room: 11, name: "Road", x: 0, y: -1, level: 0, spawns: false, visited: false, candidate: false, collides: false },
  ],
  exits: [{ from: { map: 1, room: 10 }, to: { map: 1, room: 11 }, dir: "n", kind: "plain", one_way: false }],
  route: [
    { map: 1, room: 10 },
    { map: 1, room: 11 },
  ],
};

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
});
afterEach(cleanup);

function mount(props: Partial<Parameters<typeof render<typeof MapPanel>>[1]> = {}) {
  const onwalk = vi.fn();
  const onstop = vi.fn();
  const onloops = vi.fn();
  const onwalkloop = vi.fn();
  const onrunloop = vi.fn();
  render(MapPanel, {
    session,
    view,
    character: {},
    attached: true,
    onwalk,
    onstop,
    onloops,
    onwalkloop,
    onrunloop,
    ...props,
  } as never);
  return { onwalk, onstop, onloops, onwalkloop, onrunloop };
}

const here = { position: { map: 1, room: 10, candidates: 1 } };
const stop = (room: number, name: string) => ({ map: 1, room, name, monsters: ["giant rat"], arena: room === 11, lair: false, regen: 0 });
const loops: Loops = {
  type: "loops",
  session: "aria",
  anchor: { map: 1, room: 10 },
  radius: 8,
  doors: true,
  generated: [
    {
      id: "1:10:8:doors:0",
      name: "Gate",
      stops: [stop(11, "Road"), stop(12, "Lair")],
      path: [
        { map: 1, room: 11 },
        { map: 1, room: 12 },
        { map: 1, room: 11 },
      ],
      steps: 2,
      doors: 1,
    },
  ],
  megamud: [
    {
      file: "ROADLOOP.mp",
      description: "Road loop",
      author: "someone",
      path: [
        { map: 1, room: 11 },
        { map: 1, room: 11 },
      ],
      steps: 6,
      exact: 6,
    },
  ],
};

describe("the map panel", () => {
  it("says so without a brain", () => {
    mount({ attached: false });
    expect(screen.getByText(/No brain attached/)).toBeTruthy();
  });

  it("says so without a position", () => {
    mount({ character: { position: { candidates: 0 } } });
    expect(screen.getByText("position unknown: re-sync or look")).toBeTruthy();
  });

  it("shows the walk with its commands, and stop sends walk_stop", async () => {
    const { onstop } = mount({
      character: {
        position: { map: 1, room: 10, candidates: 1 },
        walking: { to: { map: 1, room: 11 }, name: "Road", steps_left: 1, step: 1, commands: ["n"] },
      },
    });
    expect(screen.getByText("→ Road")).toBeTruthy();
    expect(screen.getByText("step 1, 1 left")).toBeTruthy();
    expect(screen.getByText("n")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "stop" }));
    expect(onstop).toHaveBeenCalledWith("aria");
  });

  it("opens the loops list, asking the brain for them", async () => {
    const { onloops } = mount({ character: here });
    expect(screen.queryByText("walk once")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "▸ loops" }));
    expect(onloops).toHaveBeenCalledWith("aria", true);
    expect(screen.getByText("Asking the brain…")).toBeTruthy();
    // Without doors, and again here.
    await fireEvent.click(screen.getByRole("checkbox", { name: "doors" }));
    expect(onloops).toHaveBeenLastCalledWith("aria", false);
    await fireEvent.click(screen.getByRole("button", { name: "here" }));
    expect(onloops).toHaveBeenCalledTimes(3);
  });

  it("lists the generated and MegaMud loops; walk once walks a generated one", async () => {
    const { onloops, onwalkloop } = mount({ character: here, loops });
    await fireEvent.click(screen.getByRole("button", { name: "▸ loops" }));
    // The loops it has are for this session: not asked again.
    expect(onloops).not.toHaveBeenCalled();
    expect(screen.getByText("8 steps around Gate")).toBeTruthy();
    expect(screen.getByText("2 stops, 2 steps, 1 door")).toBeTruthy();
    const gate = screen.getByRole("button", { name: "Gate" });
    expect(gate.getAttribute("title")).toBe("1. Road (arena)\n2. Lair");
    expect(screen.getByText("ROADLOOP.mp")).toBeTruthy();
    expect(screen.getByText("Road loop, 6 steps")).toBeTruthy();
    // One walk once: MegaMud's loops are listed, not walked.
    const walks = screen.getAllByRole("button", { name: "walk once" });
    expect(walks).toHaveLength(1);
    await fireEvent.click(walks[0]);
    expect(onwalkloop).toHaveBeenCalledWith("aria", "1:10:8:doors:0");
    // Picking a loop keeps it drawn; picking it again lets it go.
    await fireEvent.click(gate);
    expect(gate.getAttribute("aria-pressed")).toBe("true");
    await fireEvent.click(gate);
    expect(gate.getAttribute("aria-pressed")).toBe("false");
  });

  it("runs a loop passing by the stops unticked; hovering a stop shows what spawns there", async () => {
    const { onrunloop } = mount({ character: here, loops });
    await fireEvent.click(screen.getByRole("button", { name: "▸ loops" }));
    // The stops show once the loop is picked.
    expect(screen.queryByRole("list", { name: "Stops of Gate" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Gate" }));
    const lair = screen.getByRole("checkbox", { name: "2. Lair" }) as HTMLInputElement;
    expect(lair.checked).toBe(true);
    expect(lair.closest("label")?.getAttribute("title")).toBe("giant rat; back 5 m after a kill");
    expect(screen.getByRole("checkbox", { name: "1. Road" }).closest("label")?.getAttribute("title")).toBe(
      "giant rat; arena: back at once",
    );
    await fireEvent.click(lair);
    expect(lair.checked).toBe(false);
    // Not running yet: nothing is sent for a tick.
    expect(onrunloop).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "run" }));
    expect(onrunloop).toHaveBeenCalledWith("aria", "1:10:8:doors:0", [{ map: 1, room: 12 }]);
  });

  it("shows the loop the bot runs, its stop, and sends a tick at once", async () => {
    const running: CharacterState = {
      ...here,
      bot: {
        state: "running",
        mode: "task",
        narration: "Loop Gate: to stop 2/2 Lair (one step)",
        task: {
          loop_id: "1:10:8:doors:0",
          loop_name: "Gate",
          stop: 1,
          stops: [
            { map: 1, room: 11 },
            { map: 1, room: 12 },
          ],
          skip: [{ map: 1, room: 11 }],
        },
      },
    };
    const { onrunloop } = mount({ character: running, loops });
    await fireEvent.click(screen.getByRole("button", { name: "▸ loops" }));
    expect(screen.getByText("running: stop 2/2")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "run" })).toBeNull();
    // The brain's ticks, without picking the loop.
    const road = screen.getByRole("checkbox", { name: "1. Road" }) as HTMLInputElement;
    expect(road.checked).toBe(false);
    expect(screen.getByRole("checkbox", { name: "2. Lair" }).closest("li")?.classList.contains("at")).toBe(true);
    await fireEvent.click(road);
    expect(onrunloop).toHaveBeenCalledWith("aria", "1:10:8:doors:0", []);
  });

  it("says when there is nothing to hunt in reach", async () => {
    mount({ character: here, loops: { ...loops, generated: [], megamud: [] } });
    await fireEvent.click(screen.getByRole("button", { name: "▸ loops" }));
    expect(screen.getByText("No hunt rooms in reach.")).toBeTruthy();
  });

  it("shows the candidates while the belief has several", () => {
    mount({ character: { position: { map: 1, room: 10, candidates: 3 } } });
    expect(screen.getByText("3 candidates")).toBeTruthy();
    expect(screen.getByText("Click a room to walk there.")).toBeTruthy();
  });
});
