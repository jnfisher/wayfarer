import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  GatewayClient,
  TOKEN_KEY,
  gatewayUrl,
  keyBytes,
  type ConnectionStatus,
  type SocketLike,
  type TokenStore,
} from "../src/lib/gateway";
import { PROTOCOL, type SessionInfo, type ToClient, type ToGateway } from "../src/proto";

class FakeSocket implements SocketLike {
  readyState = 0;
  onopen: SocketLike["onopen"] = null;
  onmessage: SocketLike["onmessage"] = null;
  onclose: SocketLike["onclose"] = null;
  onerror: SocketLike["onerror"] = null;
  readonly sent: ToGateway[] = [];
  closed = false;

  constructor(readonly url: string) {}

  send(data: string) {
    this.sent.push(JSON.parse(data) as ToGateway);
  }
  close() {
    this.closed = true;
    this.readyState = 3;
  }

  // The gateway's side.
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
  /** The history requests sent, without the session field. */
  historyRequests(session: string) {
    return this.sent
      .filter((m) => m.type === "history_request" && m.session === session)
      .map((m) => {
        const { type: _type, session: _session, ...rest } = m as Extract<
          ToGateway,
          { type: "history_request" }
        >;
        return rest;
      });
  }
}

class MemoryStore implements TokenStore {
  readonly items = new Map<string, string>();
  getItem(key: string) {
    return this.items.get(key) ?? null;
  }
  setItem(key: string, value: string) {
    this.items.set(key, value);
  }
  removeItem(key: string) {
    this.items.delete(key);
  }
}

function session(id: string, last_seq: number): SessionInfo {
  return { id, name: id[0].toUpperCase() + id.slice(1), server: "bbs.example.org:23", state: "realm", last_seq };
}

function welcome(...sessions: SessionInfo[]): ToClient {
  return { type: "welcome", protocol: PROTOCOL, server_build: "test", sessions };
}

function output(session: string, seq: number): ToClient {
  return { type: "output", session, seq, text: `<${seq}>` };
}

function history(session: string, from: number, to: number): ToClient {
  const chunks = [];
  for (let seq = from; seq <= to; seq++) chunks.push({ seq, text: `<${seq}>` });
  return { type: "history", session, from_seq: from, chunks };
}

/** A client wired to fake sockets, recording what it hands on. */
function rig(options: { tokens?: TokenStore; askToken?: (reason: string) => string | null; historyChunks?: number } = {}) {
  const sockets: FakeSocket[] = [];
  const writes: Record<string, number[]> = {};
  const statuses: ConnectionStatus[] = [];
  const sessionLists: SessionInfo[][] = [];
  const resets: string[] = [];
  const client = new GatewayClient({
    url: "ws://gateway.test/ws",
    socket: (url) => {
      const s = new FakeSocket(url);
      sockets.push(s);
      return s;
    },
    ...options,
    events: {
      sessions: (list) => sessionLists.push(list),
      write: (id, text, seq) => {
        expect(text).toBe(`<${seq}>`);
        (writes[id] ??= []).push(seq);
      },
      reset: (id) => {
        resets.push(id);
        writes[id] = [];
      },
      connection: (status) => statuses.push(status),
    },
  });
  client.connect();
  const last = () => sockets[sockets.length - 1];
  return { client, sockets, last, writes, statuses, sessionLists, resets };
}

function range(from: number, to: number) {
  return Array.from({ length: to - from + 1 }, (_, i) => from + i);
}

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe("handshake", () => {
  it("says hello as a UI with the protocol version, and no token when none is stored", () => {
    const r = rig();
    expect(r.sockets).toHaveLength(1);
    expect(r.last().url).toBe("ws://gateway.test/ws");
    r.last().open();
    expect(r.last().sent).toEqual([{ type: "hello", protocol: PROTOCOL, client: "ui" }]);
    expect(r.client.connected).toBe(false);
  });

  it("sends the stored token", () => {
    const tokens = new MemoryStore();
    tokens.setItem(TOKEN_KEY, "TOKEN");
    const r = rig({ tokens });
    r.last().open();
    expect(r.last().sent[0]).toEqual({ type: "hello", protocol: PROTOCOL, client: "ui", token: "TOKEN" });
  });

  it("is open once welcomed and passes the session list on", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome(session("kasimir", 0), session("brenna", 0)));
    expect(r.client.connected).toBe(true);
    expect(r.statuses).toEqual(["connecting", "open"]);
    expect(r.sessionLists[0].map((s) => s.id)).toEqual(["kasimir", "brenna"]);
  });

  it("sends input only once welcomed", () => {
    const r = rig();
    r.last().open();
    expect(r.client.sendLine("kasimir", "look")).toBe(false);
    r.last().receive(welcome(session("kasimir", 0)));
    expect(r.client.sendLine("kasimir", "look")).toBe(true);
    expect(r.client.sendKeys("kasimir", [13])).toBe(true);
    expect(r.last().sent.slice(1)).toEqual([
      { type: "input_line", session: "kasimir", text: "look" },
      { type: "input_keys", session: "kasimir", bytes: [13] },
    ]);
  });

  it("sends bot_control", () => {
    const r = rig();
    r.last().open();
    expect(r.client.botControl("kasimir", "start")).toBe(false);
    r.last().receive(welcome(session("kasimir", 0)));
    expect(r.client.botControl("kasimir", "pause")).toBe(true);
    expect(r.last().sent.at(-1)).toEqual({ type: "bot_control", session: "kasimir", action: "pause" });
  });

  it("asks for the token when refused for it, keeps it and tries again", () => {
    const tokens = new MemoryStore();
    const askToken = vi.fn(() => " SECRET ");
    const r = rig({ tokens, askToken });
    r.last().open();
    r.last().receive({ type: "refused", reason: "missing or wrong token" });
    expect(askToken).toHaveBeenCalledOnce();
    expect(r.sockets[0].closed).toBe(true);
    expect(tokens.getItem(TOKEN_KEY)).toBe("SECRET");
    expect(r.sockets).toHaveLength(2);
    r.last().open();
    expect(r.last().sent[0]).toEqual({ type: "hello", protocol: PROTOCOL, client: "ui", token: "SECRET" });
  });

  it("stops on a refusal it cannot fix", () => {
    const askToken = vi.fn(() => null);
    const r = rig({ askToken });
    r.last().open();
    r.last().receive({ type: "refused", reason: "protocol mismatch: client speaks 2, gateway speaks 1" });
    r.last().drop();
    vi.advanceTimersByTime(60_000);
    expect(askToken).not.toHaveBeenCalled();
    expect(r.sockets).toHaveLength(1);
    expect(r.statuses.at(-1)).toBe("refused");
  });
});

describe("output", () => {
  it("goes to the right tile, in order", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome(session("kasimir", 0), session("brenna", 0)));
    r.last().receive(output("kasimir", 1));
    r.last().receive(output("brenna", 1));
    r.last().receive(output("kasimir", 2));
    r.last().receive(output("ghost", 1));
    expect(r.writes).toEqual({ kasimir: [1, 2], brenna: [1] });
  });

  it("backfills a new tile with the latest history before the live output", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome(session("kasimir", 5000), session("brenna", 0)));
    expect(r.last().historyRequests("kasimir")).toEqual([{ before_seq: 5001, max_chunks: 2000 }]);
    expect(r.last().historyRequests("brenna")).toEqual([]);
    // Live output arriving before the history waits for it.
    r.last().receive(output("kasimir", 5001));
    r.last().receive(output("kasimir", 5002));
    expect(r.writes.kasimir).toBeUndefined();
    r.last().receive(history("kasimir", 3001, 5000));
    r.last().receive(output("kasimir", 5002)); // a repeat is dropped
    r.last().receive(output("kasimir", 5003));
    expect(r.writes.kasimir).toEqual(range(3001, 5003));
  });

  it("fetches a gap in the live stream, and after `lagged`", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome(session("kasimir", 0)));
    r.last().receive(output("kasimir", 1));
    r.last().receive(output("kasimir", 4));
    expect(r.last().historyRequests("kasimir")).toEqual([{ after_seq: 1, max_chunks: 2000 }]);
    r.last().receive(output("kasimir", 5));
    r.last().receive(history("kasimir", 2, 4));
    expect(r.writes.kasimir).toEqual(range(1, 5));

    r.last().receive({ type: "lagged", session: "kasimir", missed: 3 });
    expect(r.last().historyRequests("kasimir").at(-1)).toEqual({ after_seq: 5, max_chunks: 2000 });
    r.last().receive(output("kasimir", 9));
    r.last().receive(history("kasimir", 6, 8));
    expect(r.writes.kasimir).toEqual(range(1, 9));
  });
});

describe("reconnect", () => {
  it("shows the drop, retries with backoff and resets it once welcomed", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome());
    r.last().drop();
    expect(r.statuses.at(-1)).toBe("down");
    vi.advanceTimersByTime(499);
    expect(r.sockets).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(r.sockets).toHaveLength(2);
    r.last().drop(); // the gateway is still down
    vi.advanceTimersByTime(999);
    expect(r.sockets).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(r.sockets).toHaveLength(3);
    for (let i = 0; i < 10; i++) {
      r.last().drop();
      vi.advanceTimersByTime(10_000);
    }
    expect(r.sockets).toHaveLength(13); // capped at 10 s
    r.last().open();
    r.last().receive(welcome());
    expect(r.statuses.at(-1)).toBe("open");
    r.last().drop();
    vi.advanceTimersByTime(500);
    expect(r.sockets).toHaveLength(14);
  });

  it("asks for everything after the last seq, losing and repeating nothing", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome(session("kasimir", 3), session("brenna", 0)));
    r.last().receive(history("kasimir", 1, 3));
    r.last().receive(output("kasimir", 4));
    r.last().receive(output("brenna", 1));
    r.last().drop();

    vi.advanceTimersByTime(500);
    r.last().open();
    // The gateway carried on while the socket was down.
    r.last().receive(welcome(session("kasimir", 7), session("brenna", 1)));
    expect(r.last().historyRequests("kasimir")).toEqual([{ after_seq: 4, max_chunks: 2000 }]);
    expect(r.last().historyRequests("brenna")).toEqual([]);
    r.last().receive(output("kasimir", 8));
    r.last().receive(history("kasimir", 5, 7));
    r.last().receive(output("kasimir", 8));
    r.last().receive(output("kasimir", 9));
    r.last().receive(output("brenna", 2));
    expect(r.writes).toEqual({ kasimir: range(1, 9), brenna: [1, 2] });
  });

  it("pages through a long gap", () => {
    const r = rig({ historyChunks: 3 });
    r.last().open();
    r.last().receive(welcome(session("kasimir", 0)));
    r.last().receive(output("kasimir", 1));
    r.last().drop();
    vi.advanceTimersByTime(500);
    r.last().open();
    r.last().receive(welcome(session("kasimir", 8)));
    r.last().receive(history("kasimir", 2, 4));
    r.last().receive(history("kasimir", 5, 7));
    r.last().receive(history("kasimir", 8, 8));
    expect(r.last().historyRequests("kasimir")).toEqual([
      { after_seq: 1, max_chunks: 3 },
      { after_seq: 4, max_chunks: 3 },
      { after_seq: 7, max_chunks: 3 },
    ]);
    expect(r.writes.kasimir).toEqual(range(1, 8));
  });

  it("starts again with a fresh backfill if the drop came mid-backfill", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome(session("kasimir", 10)));
    r.last().drop();
    vi.advanceTimersByTime(500);
    r.last().open();
    r.last().receive(welcome(session("kasimir", 12)));
    expect(r.last().historyRequests("kasimir")).toEqual([{ before_seq: 13, max_chunks: 2000 }]);
    r.last().receive(history("kasimir", 1, 12));
    expect(r.writes.kasimir).toEqual(range(1, 12));
  });

  it("clears a tile whose log restarted below what it showed", () => {
    const r = rig();
    r.last().open();
    r.last().receive(welcome(session("kasimir", 0)));
    r.last().receive(output("kasimir", 1));
    r.last().receive(output("kasimir", 2));
    r.last().drop();
    vi.advanceTimersByTime(500);
    r.last().open();
    r.last().receive(welcome(session("kasimir", 1)));
    expect(r.resets).toEqual(["kasimir"]);
    r.last().receive(history("kasimir", 1, 1));
    expect(r.writes.kasimir).toEqual([1]);
  });

  it("does not retry after close", () => {
    const r = rig();
    r.last().open();
    r.client.close();
    r.last().drop();
    vi.advanceTimersByTime(60_000);
    expect(r.sockets).toHaveLength(1);
  });
});

describe("helpers", () => {
  it("builds the WebSocket URL from the page's location", () => {
    expect(gatewayUrl({ protocol: "http:", host: "127.0.0.1:7373" })).toBe("ws://127.0.0.1:7373/ws");
    expect(gatewayUrl({ protocol: "https:", host: "rig.example:443" })).toBe("wss://rig.example:443/ws");
  });

  it("turns terminal key data into ASCII bytes", () => {
    expect(keyBytes("a\r\x1b[A")).toEqual([97, 13, 27, 91, 65]);
    expect(keyBytes("é")).toEqual([]);
  });
});
