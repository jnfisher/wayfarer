import { describe, expect, it } from "vitest";
import { GatewayClient, type SocketLike } from "../src/lib/gateway";
import {
  applyBrainState,
  applyCharacter,
  applyCharacterPatch,
  emptyBrainView,
  mergePatch,
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
