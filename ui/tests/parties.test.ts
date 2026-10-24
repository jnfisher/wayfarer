import { describe, expect, it } from "vitest";
import {
  chipFor,
  chipMenu,
  draftProblem,
  elsewhere,
  followerErrands,
  followingName,
  memberLine,
  activityLine,
  newDraft,
  setLeader,
  toggleFollower,
} from "../src/lib/parties";
import type { PartyStatus, SessionInfo } from "../src/proto";

const names = { penny: "Penny", lauren: "Lauren", faye: "Faye", manuel: "Manuel" };
const sessions = Object.entries(names).map(
  ([id, name]) => ({ id, name, server: "local", state: "realm", last_seq: 0 }) as SessionInfo,
);

function party(state: PartyStatus["members"][number]["state"] = "joined", detail?: string): PartyStatus[] {
  return [
    {
      party: { id: "party-1", leader: "penny", followers: ["lauren", "faye", "manuel"] },
      members: [
        { session: "penny", state: "joined" },
        { session: "lauren", state, ...(detail ? { detail } : {}) },
        { session: "faye", state: "joined" },
        { session: "manuel", state: "not_joined", detail: "forming up" },
      ],
    },
  ];
}

describe("the party chip", () => {
  it("says who leads and who follows, and nothing outside a party", () => {
    expect(chipFor(party(), "penny", names)).toMatchObject({ text: "★ Leading: Lauren, Faye, Manuel", tone: "leader" });
    expect(chipFor(party(), "lauren", names)).toMatchObject({ text: "Following Penny", tone: "follower", title: "joined" });
    expect(chipFor([], "lauren", names)).toBeNull();
  });

  it("warns when a follower is out of the party", () => {
    for (const state of ["separated", "walking", "unreachable"] as const) {
      expect(chipFor(party(state, "4 rooms"), "lauren", names)).toMatchObject({ text: "Separated", tone: "warn" });
    }
  });

  it("offers Leave to a follower and Join to a character in no party", () => {
    expect(chipMenu(party(), "lauren", names)).toEqual([{ kind: "leave", label: "Leave party" }]);
    expect(chipMenu(party(), "penny", names)).toEqual([]);
    const two: PartyStatus[] = [{ party: { id: "party-1", leader: "penny", followers: ["lauren"] }, members: [] }];
    expect(chipMenu(two, "faye", names)).toEqual([{ kind: "join", label: "Join Penny's party", party: "party-1" }]);
  });

  it("locks a follower's own walks, not the leader's", () => {
    expect(followingName(party(), "lauren", names)).toBe("Penny");
    expect(followingName(party(), "penny", names)).toBeNull();
    expect(followingName(party(), undefined, names)).toBeNull();
  });
});

describe("the panel's lines", () => {
  it("says each state in words", () => {
    expect(memberLine({ session: "x", state: "joined" }, "Penny")).toBe("joined");
    expect(memberLine({ session: "x", state: "not_joined", detail: "in another room" }, "Penny")).toBe(
      "not joined yet: in another room",
    );
    expect(memberLine({ session: "x", state: "away", detail: "its bot is not running" }, "Penny")).toBe(
      "away: its bot is not running",
    );
    expect(memberLine({ session: "x", state: "walking", detail: "4 rooms" }, "Penny")).toBe("walking to Penny (4 rooms)");
    expect(memberLine({ session: "x", state: "resting", detail: "62%" }, "Penny")).toBe("resting 62%");
    expect(memberLine({ session: "x", state: "unreachable", detail: "a locked gate" }, "Penny")).toBe(
      "can't reach Penny: a locked gate",
    );
  });

  it("names the other party a character is in", () => {
    expect(elsewhere(party(), "lauren", "", names)).toBe("in Penny's party");
    expect(elsewhere(party(), "lauren", "party-1", names)).toBeNull();
  });
});

describe("drafts", () => {
  it("starts a new party with a free leader and takes up to five followers", () => {
    const d = newDraft(sessions, party());
    expect(d).toEqual({ id: "", leader: "penny", followers: [] });
    expect(draftProblem(d)).toBe("tick at least one follower");
    const one = toggleFollower(d, "lauren");
    expect(one.followers).toEqual(["lauren"]);
    expect(toggleFollower(one, "penny")).toBe(one);
    expect(draftProblem(one)).toBeNull();
    let many = d;
    for (const f of ["a", "b", "c", "d", "e", "f"]) many = toggleFollower(many, f);
    expect(many.followers).toHaveLength(5);
  });

  it("a new leader stops following", () => {
    const d = { id: "", leader: "penny", followers: ["lauren", "faye"] };
    expect(setLeader(d, "lauren")).toEqual({ id: "", leader: "lauren", followers: ["faye"] });
  });
});

describe("activityLine", () => {
  it("says what a running bot is doing, with the HP", () => {
    expect(
      activityLine({
        hp: 25,
        hp_max: 35,
        bot: { state: "running", mode: "fight", narration: "Fight: rat", doing: "attacking large giant rat" },
      }),
    ).toBe("attacking large giant rat · 71%");
  });
  it("says paused or stopped instead, and the HP alone without a bot", () => {
    expect(activityLine({ bot: { state: "paused", mode: "idle", narration: "", doing: "resting" } })).toBe("paused");
    expect(activityLine({ hp: 35, hp_max: 35 })).toBe("100%");
    expect(activityLine(undefined)).toBeNull();
    expect(activityLine({})).toBeNull();
  });
});

describe("followerErrands", () => {
  const items = ["pick_up_all", "equip_all", "train", "recover_pile", "spell_plan", "cancel_errand"].map(
    (action) => ({ action, label: action, title: "Why", disabled: false }),
  );
  it("keeps every errand open to a follower; those that walk leave the party first", () => {
    const menu = followerErrands(items, "Penny");
    expect(menu.every((i) => !i.disabled)).toBe(true);
    const leaving = menu.filter((i) => i.title.startsWith("Leaves Penny's party first")).map((i) => i.action);
    expect(leaving).toEqual(["train", "recover_pile"]);
    expect(menu.find((i) => i.action === "train")?.title).toBe("Leaves Penny's party first. Why");
  });
  it("leaves a disabled errand's reason alone", () => {
    const off = items.map((i) => ({ ...i, disabled: true, title: "Stop or pause the bot first" }));
    expect(followerErrands(off, "Penny")).toEqual(off);
  });
  it("changes nothing for a character that follows no one", () => {
    expect(followerErrands(items, null)).toEqual(items);
  });
});
