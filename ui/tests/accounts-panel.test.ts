// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import AccountsPanel from "../src/AccountsPanel.svelte";
import type { Accounts, SessionInfo } from "../src/proto";

const accounts: Accounts = {
  servers: [{ id: "home", host: "127.0.0.1", port: 2323, preset: "silvermere", custom_logon: false, can_sign_up: true }],
  characters: [
    { id: "faye", name: "Faye", server: "home", autostart: true, user: "FAYE", password: "pw1", reason: "bad credentials" },
  ],
  presets: ["mbbsemu", "silvermere"],
};
const sessions: SessionInfo[] = [{ id: "faye", name: "Faye", server: "127.0.0.1:2323", state: "realm", last_seq: 1 }];

afterEach(cleanup);

function input(label: string, value: string, within?: HTMLElement): Promise<boolean> {
  const scope = within ?? document.body;
  const el = [...scope.querySelectorAll("label")].find((l) => l.textContent?.trim().startsWith(label))?.querySelector("input");
  if (!el) throw new Error(`no ${label}`);
  return fireEvent.input(el, { target: { value } });
}

describe("AccountsPanel", () => {
  it("shows the logins with the password hidden until asked, and each row's reason", async () => {
    render(AccountsPanel, { accounts, sessions, onsave: vi.fn() });
    expect(screen.getByText("bad credentials")).toBeTruthy();
    expect(screen.getByText("realm")).toBeTruthy();
    const pw = screen.getByDisplayValue("pw1") as HTMLInputElement;
    expect(pw.type).toBe("password");
    await fireEvent.click(screen.getByText("show"));
    expect((screen.getByDisplayValue("pw1") as HTMLInputElement).type).toBe("text");
  });

  it("adds a character and saves the whole set with its sign-up details", async () => {
    const onsave = vi.fn();
    render(AccountsPanel, { accounts, sessions, onsave });
    const save = screen.getByText("save") as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await fireEvent.click(screen.getByText("+ Character"));
    const row = screen.getByLabelText("Character 2");
    await input("name", "Thornwyn", row);
    await input("BBS user", "THORN", row);
    await input("password", "secret1", row);
    expect(screen.getByDisplayValue("thornwyn")).toBeTruthy();
    expect(screen.getByDisplayValue("h@h.com")).toBeTruthy();
    await fireEvent.click(screen.getByText("save"));
    expect(onsave).toHaveBeenCalledTimes(1);
    const sent: Accounts = onsave.mock.calls[0][0];
    expect(sent.characters[1]).toEqual({
      id: "thornwyn",
      name: "Thornwyn",
      server: "home",
      autostart: true,
      user: "THORN",
      password: "secret1",
      email: "h@h.com",
      gender: "F",
    });
    expect(sent.characters[0].reason).toBeUndefined();
  });

  it("asks again before a save that hangs a character up", async () => {
    const onsave = vi.fn();
    render(AccountsPanel, { accounts, sessions, onsave });
    await input("password", "changed");
    await fireEvent.click(screen.getByText("save"));
    expect(onsave).not.toHaveBeenCalled();
    expect(screen.getByText("Save: Faye will hang up and log on again.")).toBeTruthy();
    await fireEvent.click(screen.getByText("really save?"));
    expect(onsave).toHaveBeenCalledTimes(1);
  });

  it("says a remove waits for the save, and the save button stands out until then", async () => {
    const onsave = vi.fn();
    render(AccountsPanel, { accounts, sessions, onsave });
    expect(screen.queryByText(/Not saved yet/)).toBeNull();
    await fireEvent.click(screen.getByText("remove"));
    await fireEvent.click(screen.getByText("really remove?"));
    expect(screen.getByText("Not saved yet: nothing changes until you save.")).toBeTruthy();
    const save = screen.getByText("save") as HTMLButtonElement;
    expect(save.classList.contains("pending")).toBe(true);
    await fireEvent.click(screen.getByText("revert"));
    expect(screen.queryByText(/Not saved yet/)).toBeNull();
    expect(onsave).not.toHaveBeenCalled();
  });

  it("won't delete a server a character uses", () => {
    render(AccountsPanel, { accounts, sessions, onsave: vi.fn() });
    const del = screen.getByText("delete") as HTMLButtonElement;
    expect(del.disabled).toBe(true);
    expect(del.title).toBe("Used by Faye");
  });
});
