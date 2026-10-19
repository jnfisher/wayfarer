import { describe, expect, it } from "vitest";
import { GatewayClient, type SocketLike } from "../src/lib/gateway";
import {
  addEvent,
  addNote,
  allNotes,
  applyBrainState,
  applyCharacter,
  applyCharacterPatch,
  clock,
  compactFields,
  emptyBrainView,
  emptyDevLog,
  eventMatches,
  formatRate,
  loadDevPrefs,
  MAX_EVENTS,
  MAX_NOTICES,
  MAX_UNKNOWN,
  mergePatch,
  pruneCharacters,
  saveDevPrefs,
  trim,
  unknownLines,
  unknownMarkdown,
} from "../src/lib/state";
import { PROTOCOL, type ToClient, type ToGateway } from "../src/proto";

describe("mergePatch (RFC 7386)", () => {
  it("recurses into objects", () => {
    expect(mergePatch({ a: { b: 1, c: 2 }, d: 3 }, { a: { b: 9 } })).toEqual({ a: { b: 9, c: 2 }, d: 3 });
  });

  it("deletes on null, at any depth", () => {
    expect(mergePatch({ a: 1, b: { c: 2, d: 3 } }, { a: null, b: { c: null } })).toEqual({ b: { d: 3 } });
  });

  it("replaces arrays and scalars, and an object with a non-object", () => {
    expect(mergePatch({ a: [1, 2], b: "x", c: { d: 1 } }, { a: [3], b: 4, c: "y" })).toEqual({ a: [3], b: 4, c: "y" });
  });

  it("builds an object where the target is not one", () => {
    expect(mergePatch("x", { a: { b: 1 } })).toEqual({ a: { b: 1 } });
    expect(mergePatch({ a: 1 }, "x")).toBe("x");
  });

  it("changes the target in place", () => {
    const target = { hp: 30, room_title: "Town Square" };
    expect(mergePatch(target, { hp: 23, room_title: null })).toBe(target);
    expect(target).toEqual({ hp: 23 });
  });

  // The RFC's appendix A, a few of its rows.
  it.each([
    [{ a: "b" }, { a: "c" }, { a: "c" }],
    [{ a: "b" }, { b: "c" }, { a: "b", b: "c" }],
    [{ a: ["b"] }, { a: "c" }, { a: "c" }],
    [{ a: { b: "c" } }, { a: { b: "d", c: null } }, { a: { b: "d" } }],
    [{ e: null }, { a: 1 }, { e: null, a: 1 }],
    [{}, { a: { bb: { ccc: null } } }, { a: { bb: {} } }],
  ])("%j + %j = %j", (target, patch, result) => {
    expect(mergePatch(target, patch)).toEqual(result);
  });
});

describe("the brain view", () => {
  it("character replaces, character_patch merges", () => {
    const view = emptyBrainView();
    applyCharacter(view, "a", { name: "Aria", hp: 30, hp_max: 40, statuses: ["poisoned"] });
    applyCharacterPatch(view, "a", { hp: 23, statuses: [] });
    expect(view.characters.a).toEqual({ name: "Aria", hp: 23, hp_max: 40, statuses: [] });
    applyCharacter(view, "a", { name: "Aria", level: 4 });
    expect(view.characters.a).toEqual({ name: "Aria", level: 4 });
  });

  it("a patch before any snapshot starts from empty", () => {
    const view = emptyBrainView();
    applyCharacterPatch(view, "b", { hp: 5, room_title: null });
    expect(view.characters.b).toEqual({ hp: 5 });
  });

  it("merges the new optional fields", () => {
    const view = emptyBrainView();
    applyCharacter(view, "a", { name: "Aria", coins: { copper: 30, carried: [] } });
    applyCharacterPatch(view, "a", {
      coins: { carried: [{ coin: "silver nobles", count: 3 }] },
      carrying: ["padded vest"],
      encumbrance: { current: 515, max: 2400, level: "Light" },
      kills: 1,
    });
    expect(view.characters.a).toEqual({
      name: "Aria",
      coins: { copper: 30, carried: [{ coin: "silver nobles", count: 3 }] },
      carrying: ["padded vest"],
      encumbrance: { current: 515, max: 2400, level: "Light" },
      kills: 1,
    });
  });

  it("a session list forgets the characters of sessions not in it", () => {
    const view = emptyBrainView();
    applyCharacter(view, "a", { name: "Aria" });
    applyCharacter(view, "b", { name: "Brienne" });
    pruneCharacters(view, ["a", "c"]);
    expect(view.characters).toEqual({ a: { name: "Aria" } });
  });

  it("keeps brain_state", () => {
    const view = emptyBrainView();
    expect(view.attached).toBeNull();
    applyBrainState(view, true);
    expect(view.attached).toBe(true);
  });
});

class FakeSocket implements SocketLike {
  readyState = 0;
  onopen: SocketLike["onopen"] = null;
  onmessage: SocketLike["onmessage"] = null;
  onclose: SocketLike["onclose"] = null;
  onerror: SocketLike["onerror"] = null;
  readonly sent: ToGateway[] = [];
  send(data: string) {
    this.sent.push(JSON.parse(data) as ToGateway);
  }
  close() {
    this.readyState = 3;
  }
  open() {
    this.readyState = 1;
    this.onopen?.({});
  }
  receive(message: ToClient) {
    this.onmessage?.({ data: JSON.stringify(message) });
  }
  drop() {
    this.readyState = 3;
    this.onclose?.({});
  }
  stateRequests() {
    return this.sent.filter((m) => m.type === "state_request");
  }
}

describe("state_request", () => {
  const welcome: ToClient = { type: "welcome", protocol: PROTOCOL, server_build: "test", sessions: [] };

  function connect() {
    const sockets: FakeSocket[] = [];
    const seen: string[] = [];
    const client = new GatewayClient({
      url: "ws://test/ws",
      socket: () => {
        const s = new FakeSocket();
        sockets.push(s);
        return s;
      },
      minRetryMs: 0,
      events: {
        character: (id, state) => seen.push(`character ${id} ${JSON.stringify(state)}`),
        characterPatch: (id, patch) => seen.push(`patch ${id} ${JSON.stringify(patch)}`),
        brainState: (attached) => seen.push(`brain ${attached}`),
      },
    });
    client.connect();
    return { client, sockets, seen };
  }

  it("is sent once a brain is reported after welcome, and again after a reconnect", async () => {
    const { client, sockets, seen } = connect();
    sockets[0].open();
    sockets[0].receive(welcome);
    sockets[0].receive({ type: "brain_state", attached: true });
    expect(sockets[0].stateRequests()).toEqual([{ type: "state_request" }]);
    // Later brain_state changes don't ask: an attaching brain sends its snapshots itself.
    sockets[0].receive({ type: "brain_state", attached: false });
    sockets[0].receive({ type: "brain_state", attached: true });
    expect(sockets[0].stateRequests()).toHaveLength(1);

    sockets[0].drop();
    await new Promise((r) => setTimeout(r, 1));
    sockets[1].open();
    sockets[1].receive(welcome);
    sockets[1].receive({ type: "brain_state", attached: true });
    expect(sockets[1].stateRequests()).toEqual([{ type: "state_request" }]);
    expect(seen).toEqual(["brain true", "brain false", "brain true", "brain true"]);
    client.close();
  });

  it("is not sent without a brain (the gateway would answer with an error)", () => {
    const { client, sockets } = connect();
    sockets[0].open();
    sockets[0].receive(welcome);
    sockets[0].receive({ type: "brain_state", attached: false });
    expect(sockets[0].stateRequests()).toEqual([]);
    client.close();
  });

  it("is sent on brain_lagged, and snapshots and patches are handed on", () => {
    const { client, sockets, seen } = connect();
    sockets[0].open();
    sockets[0].receive(welcome);
    sockets[0].receive({ type: "brain_lagged", missed: 3 });
    expect(sockets[0].stateRequests()).toHaveLength(1);
    sockets[0].receive({ type: "character", session: "a", state: { hp: 1 } });
    sockets[0].receive({ type: "character_patch", session: "a", patch: { hp: null } });
    expect(seen).toEqual(['character a {"hp":1}', 'patch a {"hp":null}']);
    client.close();
  });
});

describe("the dev panel's messages", () => {
  it("hands on event, brain_info, notice, warning and brain_lagged, and sends resync", () => {
    const socket = new FakeSocket();
    const seen: string[] = [];
    const client = new GatewayClient({
      url: "ws://test/ws",
      socket: () => socket,
      events: {
        event: (id, e) => seen.push(`event ${id} ${e.seq} ${e.name}`),
        brainInfo: (info) => seen.push(`info ${info.build} ${info.started_at}`),
        notice: (kind, text, id) => seen.push(`${kind} ${id ?? "-"} ${text}`),
        brainLagged: (missed) => seen.push(`lagged ${missed}`),
      },
    });
    client.connect();
    socket.open();
    expect(client.resync("a")).toBe(false); // not welcomed yet
    socket.receive({ type: "welcome", protocol: PROTOCOL, server_build: "test", sessions: [] });
    socket.receive({ type: "event", session: "a", seq: 4, line_no: 9, name: "room_title", cat: "room", fields: {}, line: "Town" });
    socket.receive({ type: "brain_info", build: "0.1.0 (abc)", started_at: "2026-10-19T11:02:14Z" });
    socket.receive({ type: "notice", session: "a", text: "re-synced" });
    socket.receive({ type: "warning", text: "world pack not found" });
    socket.receive({ type: "brain_lagged", missed: 7 });
    expect(seen).toEqual([
      "event a 4 room_title",
      "info 0.1.0 (abc) 2026-10-19T11:02:14Z",
      "notice a re-synced",
      "warning - world pack not found",
      "lagged 7",
    ]);
    expect(socket.stateRequests()).toHaveLength(1);
    expect(client.resync("a")).toBe(true);
    expect(socket.sent.at(-1)).toEqual({ type: "resync", session: "a" });
    client.close();
  });
});

const ev = (name: string, line = "", seq = 1, fields: Record<string, unknown> = {}) => ({
  seq,
  line_no: seq,
  name,
  cat: name === "unknown" ? "unclassified" : "test",
  fields,
  line,
});

describe("the dev buffers", () => {
  it("trim keeps the newest", () => {
    const list = [1, 2, 3, 4, 5];
    trim(list, 3);
    expect(list).toEqual([3, 4, 5]);
    trim(list, 3);
    expect(list).toEqual([3, 4, 5]);
  });

  it("keep the last 2,000 events per session", () => {
    const log = emptyDevLog();
    for (let seq = 1; seq <= MAX_EVENTS + 50; seq++) addEvent(log, "a", ev("prompt", "", seq));
    addEvent(log, "b", ev("prompt", "", 1));
    expect(log.events.a).toHaveLength(MAX_EVENTS);
    expect(log.events.a[0].seq).toBe(51);
    expect(log.events.a.at(-1)!.seq).toBe(MAX_EVENTS + 50);
    expect(log.events.b).toHaveLength(1);
  });

  it("keep the last 500 notices per session", () => {
    const log = emptyDevLog();
    for (let i = 0; i < MAX_NOTICES + 10; i++) addNote(log, "notice", `n${i}`, "a", i);
    addNote(log, "warning", "w", undefined, 0);
    expect(log.notes.a).toHaveLength(MAX_NOTICES);
    expect(log.notes.a[0].text).toBe("n10");
    const all = allNotes(log);
    expect(all).toHaveLength(MAX_NOTICES + 1);
    expect(all.at(-1)).toMatchObject({ kind: "warning", text: "w", session: undefined });
  });

  it("group unclassified lines by text, most frequent first", () => {
    const log = emptyDevLog();
    for (const line of ["A wind blows.", "Odd text  ", "Odd text", "A wind blows.", "Odd text", ""]) {
      addEvent(log, "a", ev("unknown", line));
    }
    addEvent(log, "a", ev("room_title", "Town Square"));
    expect(unknownLines(log, "a")).toEqual([
      { text: "Odd text", count: 3 },
      { text: "A wind blows.", count: 2 },
    ]);
    expect(unknownLines(log, "b")).toEqual([]);
  });

  it("forget the oldest line seen once when there are too many", () => {
    const log = emptyDevLog();
    addEvent(log, "a", ev("unknown", "first"));
    addEvent(log, "a", ev("unknown", "first"));
    for (let i = 0; i < MAX_UNKNOWN; i++) addEvent(log, "a", ev("unknown", `line ${i}`));
    const lines = unknownLines(log, "a");
    expect(lines).toHaveLength(MAX_UNKNOWN);
    expect(lines[0]).toEqual({ text: "first", count: 2 });
    expect(lines.some((l) => l.text === "line 0")).toBe(false);
  });
});

describe("the dev panel's text", () => {
  it("formats the unclassified rate to two decimals, red above 0.2%", () => {
    expect(formatRate(undefined)).toEqual({ text: "--", alarm: false });
    expect(formatRate({ lines: 0, unclassified: 0 })).toEqual({ text: "--", alarm: false });
    expect(formatRate({ lines: 1000, unclassified: 2 })).toEqual({ text: "0.20%", alarm: false });
    expect(formatRate({ lines: 1000, unclassified: 3 })).toEqual({ text: "0.30%", alarm: true });
    expect(formatRate({ lines: 412, unclassified: 3 })).toEqual({ text: "0.73%", alarm: true });
    expect(formatRate({ lines: 3, unclassified: 3 })).toEqual({ text: "100.00%", alarm: true });
  });

  it("shows fields compact", () => {
    expect(compactFields({ hp: "28", maxhp: 40 })).toBe("hp=28 maxhp=40");
    expect(compactFields({ exits: ["north", "south"] })).toBe('exits=["north","south"]');
    expect(compactFields({})).toBe("");
  });

  it("filters on name and line, ignoring case", () => {
    const e = { id: 1, ...ev("room_title", "Newhaven, Village Entrance") };
    expect(eventMatches(e, "")).toBe(true);
    expect(eventMatches(e, "ROOM")).toBe(true);
    expect(eventMatches(e, "village")).toBe(true);
    expect(eventMatches(e, "combat")).toBe(false);
  });

  it("copies the unclassified lines as a Markdown list", () => {
    expect(unknownMarkdown([])).toBe("");
    expect(
      unknownMarkdown([
        { text: "A wind blows.", count: 3 },
        { text: "Type `help` here", count: 1 },
        { text: "`quoted`", count: 1 },
      ]),
    ).toBe("- `A wind blows.` (3x)\n- ``Type `help` here`` (1x)\n- `` `quoted` `` (1x)\n");
  });

  it("prints the time of day", () => {
    expect(clock(new Date(2026, 9, 19, 11, 2, 9).getTime())).toBe("11:02:09");
  });
});

describe("the dev panel's prefs", () => {
  const store = (value: string | null) => ({ getItem: () => value });

  it("default to closed on the events tab", () => {
    expect(loadDevPrefs(undefined)).toEqual({ open: false, tab: "events" });
    expect(loadDevPrefs(store(null))).toEqual({ open: false, tab: "events" });
    expect(loadDevPrefs(store("not json"))).toEqual({ open: false, tab: "events" });
    expect(loadDevPrefs(store('{"open":true,"tab":"nonsense"}'))).toEqual({ open: true, tab: "events" });
    expect(
      loadDevPrefs({
        getItem: () => {
          throw new Error("blocked");
        },
      }),
    ).toEqual({ open: false, tab: "events" });
  });

  it("round-trip, and a failing store is ignored", () => {
    const kept: Record<string, string> = {};
    saveDevPrefs({ setItem: (k, v) => (kept[k] = v) }, { open: true, tab: "brain" });
    expect(loadDevPrefs({ getItem: (k) => kept[k] ?? null })).toEqual({ open: true, tab: "brain" });
    expect(() =>
      saveDevPrefs(
        {
          setItem: () => {
            throw new Error("full");
          },
        },
        { open: true, tab: "brain" },
      ),
    ).not.toThrow();
  });
});
