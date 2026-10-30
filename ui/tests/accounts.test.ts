import { describe, expect, it } from "vitest";
import {
  changed,
  confirmText,
  draftOf,
  idFromName,
  newCharacter,
  problems,
  saveEffect,
  toAccounts,
  usersOf,
  withName,
} from "../src/lib/accounts";
import type { Accounts, SessionInfo } from "../src/proto";

const saved: Accounts = {
  servers: [
    { id: "home", host: "bbs.example.org", port: 23, preset: "mbbsemu", custom_logon: false, can_sign_up: true },
  ],
  characters: [
    { id: "faye", name: "Faye", server: "home", autostart: true, user: "FAYE", password: "pw1" },
    { id: "manuel", name: "Manuel", server: "home", autostart: true, user: "MANUEL", password: "pw2" },
    { id: "penny", name: "Penny", server: "home", autostart: false, user: "PENNY", password: "pw3", reason: "bad credentials" },
  ],
  presets: ["mbbsemu", "silvermere"],
};

const sessions: SessionInfo[] = [
  { id: "faye", name: "Faye", server: "bbs.example.org:23", state: "realm", last_seq: 1 },
  { id: "manuel", name: "Manuel", server: "bbs.example.org:23", state: "realm", last_seq: 1 },
  { id: "penny", name: "Penny", server: "bbs.example.org:23", state: "closed", last_seq: 1 },
];

describe("the accounts draft", () => {
  it("makes an id from the name until one is typed", () => {
    expect(idFromName("Sir Kasimir II!")).toBe("sir_kasimir_ii");
    const d = draftOf(saved);
    const c = withName(newCharacter(d), "Thornwyn");
    expect(c.id).toBe("thornwyn");
    expect(withName({ ...c, idFromName: false, id: "t1" }, "Other").id).toBe("t1");
  });

  it("checks what the gateway checks", () => {
    const d = draftOf(saved);
    expect(problems(d)).toEqual({});
    d.characters.push({ ...newCharacter(d), name: "Faye 2", id: "faye" });
    d.servers.push({ ...d.servers[0], id: "bad id!", fresh: true });
    expect(problems(d)).toEqual({
      "server:1": "id: 1-64 letters, digits, - or _",
      "character:3": "faye is listed twice",
    });
    const blank = draftOf(saved);
    blank.characters.push(withName(newCharacter(blank), "New"));
    expect(problems(blank)["character:3"]).toBe("no BBS user");
    for (let i = 0; i < 4; i++) blank.characters.push({ ...withName(newCharacter(blank), `X${i}`), user: "u" });
    blank.characters[3].user = "u";
    expect(problems(blank).all).toBe("at most 6 characters");
  });

  it("knows which characters use a server", () => {
    expect(usersOf(draftOf(saved), "home")).toEqual(["Faye", "Manuel", "Penny"]);
    expect(usersOf(draftOf(saved), "other")).toEqual([]);
  });

  it("sends the sign-up details only for a new character or a changed user", () => {
    const d = draftOf(saved);
    expect(changed(d, saved)).toBe(false);
    d.characters[0].password = "new";
    d.characters[1].user = "MANNY";
    d.characters.push({ ...withName(newCharacter(d), "Thornwyn"), user: "THORN", password: "pw" });
    expect(changed(d, saved)).toBe(true);
    const sent = toAccounts(d, saved);
    expect(sent.characters[0]).toEqual({ id: "faye", name: "Faye", server: "home", autostart: true, user: "FAYE", password: "new" });
    expect(sent.characters[1]).toMatchObject({ user: "MANNY", email: "h@h.com", gender: "F" });
    expect(sent.characters[3]).toMatchObject({ id: "thornwyn", email: "h@h.com", gender: "F" });
    expect(sent.characters[2].reason).toBeUndefined();
  });
});

describe("what a save does", () => {
  it("names who hangs up, and says nothing when nobody does", () => {
    const d = draftOf(saved);
    d.characters[0].name = "Fay";
    expect(confirmText(saveEffect(d, saved, sessions))).toBeNull();

    d.characters[0].password = "new";
    d.characters[1].user = "MANNY";
    // Penny is closed: her change hangs nobody up.
    d.characters[2].password = "x";
    d.characters.push({ ...withName(newCharacter(d), "Thornwyn"), user: "THORN" });
    const effect = saveEffect(d, saved, sessions);
    expect(effect).toEqual({ reconnect: ["Faye", "Manuel"], removed: [], added: ["Thornwyn"] });
    expect(confirmText(effect)).toBe("Save: Faye and Manuel will hang up and log on again.");

    const moved = draftOf(saved);
    moved.servers[0].port = 2323;
    moved.characters.splice(0, 1);
    expect(confirmText(saveEffect(moved, saved, sessions))).toBe(
      "Save: Manuel will hang up and log on again; Faye will hang up and go.",
    );
  });
});
