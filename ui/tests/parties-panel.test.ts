// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import PartiesPanel from "../src/PartiesPanel.svelte";
import type { PartyStatus, SessionInfo } from "../src/proto";

const sessions = [
  { id: "penny", name: "Penny" },
  { id: "lauren", name: "Lauren" },
  { id: "faye", name: "Faye" },
].map((s) => ({ ...s, server: "local", state: "realm", last_seq: 0 }) as SessionInfo);

afterEach(cleanup);

describe("PartiesPanel", () => {
  it("makes a new party: leader, ticked followers, save", async () => {
    const onsave = vi.fn();
    render(PartiesPanel, { sessions, parties: [], attached: true, onsave, ondelete: vi.fn() });
    await fireEvent.click(screen.getByText("+ New party"));
    const save = screen.getByText("save") as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await fireEvent.click(screen.getByLabelText("Lauren"));
    await fireEvent.click(screen.getByLabelText("Faye"));
    await fireEvent.click(screen.getByText("save"));
    expect(onsave).toHaveBeenCalledWith({ id: "", leader: "penny", followers: ["lauren", "faye"] });
  });

  it("shows each member's state and deletes on a second click", async () => {
    const ondelete = vi.fn();
    const parties: PartyStatus[] = [
      {
        party: { id: "party-1", leader: "penny", followers: ["lauren"] },
        members: [
          { session: "penny", state: "joined" },
          { session: "lauren", state: "not_joined", detail: "forming up" },
        ],
      },
    ];
    render(PartiesPanel, { sessions, parties, attached: true, onsave: vi.fn(), ondelete });
    expect(screen.getByText("not joined yet: forming up")).toBeTruthy();
    await fireEvent.click(screen.getByText("delete"));
    expect(ondelete).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByText("really delete?"));
    expect(ondelete).toHaveBeenCalledWith("party-1");
  });

  it("shows what each member is doing beside its state", () => {
    const parties: PartyStatus[] = [
      {
        party: { id: "party-1", leader: "penny", followers: ["lauren"] },
        members: [
          { session: "penny", state: "joined" },
          { session: "lauren", state: "joined" },
        ],
      },
    ];
    const characters = {
      penny: { hp: 20, hp_max: 40, bot: { state: "running", mode: "recover", narration: "", doing: "resting" } },
      lauren: {
        hp: 35,
        hp_max: 35,
        bot: { state: "running", mode: "recover", narration: "", doing: "healing Penny" },
      },
    } as const;
    render(PartiesPanel, { sessions, parties, attached: true, characters, onsave: vi.fn(), ondelete: vi.fn() });
    expect(screen.getByText("resting · 50%")).toBeTruthy();
    expect(screen.getByText("· healing Penny · 100%", { exact: false })).toBeTruthy();
  });

  it("says so with no brain", () => {
    render(PartiesPanel, { sessions, parties: undefined, attached: false, onsave: vi.fn(), ondelete: vi.fn() });
    expect(screen.getByText("No brain attached: parties are the brain's.")).toBeTruthy();
  });
});
