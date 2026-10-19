// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import DevPanel from "../src/DevPanel.svelte";
import { addEvent, addNote, emptyDevLog, type DevLog, type DevTab } from "../src/lib/state";

const session = { id: "a", name: "Aria", server: "silvermere", state: "realm", last_seq: 0 } as never;

function filled(): DevLog {
  const log = emptyDevLog();
  addEvent(log, "a", { seq: 4, line_no: 9, name: "room_title", cat: "room", fields: { title: "Town Square" }, line: "Town Square" });
  addEvent(log, "a", { seq: 4, line_no: 10, name: "room_exits", cat: "room", fields: {}, line: "Obvious exits: north" });
  addEvent(log, "a", { seq: 5, line_no: 11, name: "unknown", cat: "unclassified", fields: {}, line: "A wind blows." });
  addEvent(log, "a", { seq: 6, line_no: 12, name: "unknown", cat: "unclassified", fields: {}, line: "A wind blows." });
  addNote(log, "notice", "re-synced", "a", 0);
  return log;
}

function mount(tab: DevTab, attached: boolean | null, log = filled()) {
  const onresync = vi.fn();
  const r = render(DevPanel, {
    session,
    log,
    tick: 0,
    character: { parse: { lines: 400, unclassified: 2 } },
    attached,
    tab,
    onresync,
  });
  return { ...r, onresync, log };
}

const rows = (c: HTMLElement) =>
  [...c.querySelectorAll(".list .row")].map((r) => [...r.children].map((s) => s.textContent).join(" ").trim());

describe("the dev panel", () => {
  afterEach(cleanup);

  it("lists the focused character's events with their seq, and filters them", async () => {
    const { container, rerender, log } = mount("events", true);
    expect(rows(container)).toEqual([
      "4 room_title room title=Town Square",
      "4 room_exits room",
      "5 unknown unclassified",
      "6 unknown unclassified",
    ]);
    await fireEvent.input(screen.getByLabelText("Filter events"), { target: { value: "EXITS" } });
    expect(rows(container)).toEqual(["4 room_exits room"]);
    await fireEvent.click(container.querySelector(".list .row")!);
    expect(container.querySelector(".detail .line")!.textContent).toBe("Obvious exits: north");

    // Paused, new events wait.
    await fireEvent.input(screen.getByLabelText("Filter events"), { target: { value: "" } });
    await fireEvent.click(screen.getByRole("button", { name: "pause" }));
    addEvent(log, "a", { seq: 7, line_no: 13, name: "prompt", cat: "prompt", fields: {}, line: "[HP=1]:" });
    await rerender({ tick: 1 });
    expect(rows(container)).toHaveLength(4);
    await fireEvent.click(screen.getByRole("button", { name: "resume" }));
    expect(rows(container)).toHaveLength(5);
  });

  it("groups unclassified lines, shows the rate and copies them as Markdown", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const { container } = mount("unclassified", true);
    expect(rows(container)).toEqual(["2 A wind blows."]);
    expect(container.querySelector(".rate")!.textContent).toBe("0.50%");
    expect(container.querySelector(".rate")!.classList.contains("alarm")).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "copy as KB notes" }));
    expect(writeText).toHaveBeenCalledWith("- `A wind blows.` (2x)\n");
  });

  it("re-syncs the focused character, only with a brain", async () => {
    const { onresync, container } = mount("brain", true);
    expect(rows(container)[0]).toMatch(/a re-synced$/);
    await fireEvent.click(screen.getByRole("button", { name: "Re-sync Aria" }));
    expect(onresync).toHaveBeenCalledWith("a");
    cleanup();
    mount("brain", false);
    expect((screen.getByRole("button", { name: "Re-sync Aria" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
