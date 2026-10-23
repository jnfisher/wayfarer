// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import BlessingsPanel from "../src/BlessingsPanel.svelte";
import type { Blessings, SessionInfo } from "../src/proto";

const sessions = [{ id: "lauren", name: "Lauren" }].map(
  (s) => ({ ...s, server: "local", state: "realm", last_seq: 0 }) as SessionInfo,
);
const blessings: Blessings[] = [
  {
    session: "lauren",
    spells: [
      { short: "bles", name: "bless", mana: 4, chosen: true, on: true },
      { short: "aura", name: "holy aura", mana: 15, chosen: false, on: false },
    ],
  },
];

afterEach(cleanup);

describe("BlessingsPanel", () => {
  it("ticks a spell and saves the character's ticks", async () => {
    const onsave = vi.fn();
    render(BlessingsPanel, { sessions, blessings, attached: true, onsave });
    expect(screen.getByTitle("On now")).toBeTruthy();
    expect(screen.queryByText("save")).toBeNull();
    await fireEvent.click(screen.getByLabelText("holy aura (aura, 15 mana)"));
    await fireEvent.click(screen.getByText("save"));
    expect(onsave).toHaveBeenCalledWith("lauren", ["bles", "aura"]);
  });

  it("says so with no brain", () => {
    render(BlessingsPanel, { sessions, blessings: undefined, attached: false, onsave: vi.fn() });
    expect(screen.getByText("No brain attached: blessings are the brain's.")).toBeTruthy();
  });
});
