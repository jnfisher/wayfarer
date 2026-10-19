// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import MapPanel from "../src/MapPanel.svelte";
import type { MapView } from "../src/lib/map";

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
  render(MapPanel, { session, view, character: {}, attached: true, onwalk, onstop, ...props } as never);
  return { onwalk, onstop };
}

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

  it("shows the candidates while the belief has several", () => {
    mount({ character: { position: { map: 1, room: 10, candidates: 3 } } });
    expect(screen.getByText("3 candidates")).toBeTruthy();
    expect(screen.getByText("Click a room to walk there.")).toBeTruthy();
  });
});
