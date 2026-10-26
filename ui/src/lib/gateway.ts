// The protocol client: connect, hello, dispatch, reconnect and history (docs/protocol.md).
// Plain TypeScript with no Svelte in it, so the tests can drive it with a fake socket.
//
// Per session it keeps the `seq` of the last chunk it handed on, and writes every chunk once,
// in order: live output that arrives while history is being fetched waits in a buffer, a gap
// in the live stream (or a `lagged`) fetches the missing chunks, and after a reconnect it asks
// for everything after the last `seq`.

import { PROTOCOL } from "../proto";
import type {
  Blessings,
  BotAction,
  CharacterState,
  Chunk,
  MonsterRuleKind,
  PartyDef,
  PartyStatus,
  RoomKey,
  SessionAction,
  SessionInfo,
  SessionState,
  SettingChange,
  StatsPart,
  ToClient,
  ToGateway,
} from "../proto";
import { clampRadius, LOOP_RADIUS, type Loops, type MapView } from "./map";
import { MIN_QUERY, type FoundMessage } from "./goto";
import type { SettingsMessage } from "./settings";
import type { SpellPlan } from "./spells";
import type { Stats } from "./stats";

/** The part of the browser's `WebSocket` the client uses. */
export interface SocketLike {
  readonly readyState: number;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: unknown) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  send(data: string): void;
  close(): void;
}

export type SocketFactory = (url: string) => SocketLike;

/** Where the token is kept between page loads (`localStorage` in the browser). */
export interface TokenStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export type ConnectionStatus =
  /** Opening the socket or waiting for `welcome`. */
  | "connecting"
  /** Welcomed: output is flowing. */
  | "open"
  /** The socket dropped; a retry is scheduled (`retryInMs`). */
  | "down"
  /** The gateway refused the handshake (`detail` says why); no retry. */
  | "refused";

export interface GatewayEvents {
  /** The whole session list: on `welcome` and on `session_list`. */
  sessions?(sessions: SessionInfo[]): void;
  /** One session's state changed. */
  state?(session: string, state: SessionState): void;
  /** Terminal text for a session, each `seq` exactly once and in ascending order. */
  write?(session: string, text: string, seq: number): void;
  /** The gateway's log for a session restarted below what was shown: clear the terminal. */
  reset?(session: string): void;
  connection?(status: ConnectionStatus, detail?: { reason?: string; retryInMs?: number }): void;
  /** An `error` message from the gateway; the connection stays up. */
  error?(message: string): void;
  /** The brain's whole snapshot of a character. */
  character?(session: string, state: CharacterState): void;
  /** A JSON merge patch to the last snapshot. */
  characterPatch?(session: string, patch: unknown): void;
  /** Whether a brain is attached: right after `welcome`, then on every change. */
  brainState?(attached: boolean): void;
  /** One parsed event, for the dev panel. */
  event?(session: string, event: Extract<ToClient, { type: "event" }>): void;
  /** Who the brain is: on attach and after each `state_request`. */
  brainInfo?(info: Extract<ToClient, { type: "brain_info" }>): void;
  /** The brain's `notice` and `warning` lines. */
  notice?(kind: "notice" | "warning", text: string, session?: string): void;
  /** Relayed brain messages were dropped; a `state_request` has been sent. */
  brainLagged?(missed: number): void;
  /** The rooms around a character, for the map panel. */
  mapView?(view: MapView): void;
  /** The loops of the area around a character, for the map panel. */
  loops?(loops: Loops): void;
  /** A character's last runs, for the stats tab. */
  stats?(stats: Stats): void;
  /** The spells a character lacks and where to buy them, for the tile's errand menu. */
  spellPlan?(plan: SpellPlan): void;
  /** A character's settings, for the settings tab. */
  settings?(settings: SettingsMessage): void;
  /** What a `find` found, for the map panel's go box. */
  found?(found: FoundMessage): void;
  /** Every party and how its members stand, for the Parties panel and the tiles' chips. */
  parties?(parties: PartyStatus[]): void;
  /** Each character's lasting spells, ticked and on, for the Blessings panel. */
  blessings?(characters: Blessings[]): void;
}

export interface GatewayOptions {
  /** The WebSocket URL, e.g. `ws://127.0.0.1:7373/ws`. */
  url: string;
  events: GatewayEvents;
  socket?: SocketFactory;
  tokens?: TokenStore;
  /** Asked when the gateway refuses for the token. Return the token, or null to give up. */
  askToken?(reason: string): string | null;
  /** How many chunks to backfill a new tile with, and the page size for catching up. */
  historyChunks?: number;
  /** Reconnect backoff: the first delay, doubling up to `maxRetryMs`. */
  minRetryMs?: number;
  maxRetryMs?: number;
}

export const TOKEN_KEY = "wayfarer.token";
export const HISTORY_CHUNKS = 2000;

const OPEN = 1;

/** One session's place in its output stream. */
interface Sync {
  /** The last `seq` written; null until the backfill has answered. */
  lastSeq: number | null;
  /** The history request in flight, if any. */
  pending: null | { kind: "backfill"; through: number } | { kind: "after" };
  /** Live chunks held back while `pending`. */
  buffer: Chunk[];
}

export class GatewayClient {
  private readonly opts: Required<Omit<GatewayOptions, "askToken" | "tokens">> &
    Pick<GatewayOptions, "askToken" | "tokens">;
  private socket: SocketLike | null = null;
  private welcomed = false;
  /** The first `brain_state` since `welcome` is still to come. */
  private brainUnheard = false;
  private stopped = false;
  private refused = false;
  private retries = 0;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly syncs = new Map<string, Sync>();

  constructor(options: GatewayOptions) {
    this.opts = {
      socket: (url) => new WebSocket(url) as unknown as SocketLike,
      historyChunks: HISTORY_CHUNKS,
      minRetryMs: 500,
      maxRetryMs: 10_000,
      ...options,
    };
  }

  /** Open the connection (and keep reopening it until `close`). */
  connect(): void {
    this.stopped = false;
    this.refused = false;
    this.clearRetry();
    this.open();
  }

  /** Close for good: no more retries. */
  close(): void {
    this.stopped = true;
    this.clearRetry();
    const socket = this.socket;
    this.socket = null;
    socket?.close();
  }

  get connected(): boolean {
    return this.welcomed;
  }

  /** Send a line of input; false when not connected. */
  sendLine(session: string, text: string): boolean {
    return this.send({ type: "input_line", session, text });
  }

  /** Send raw key bytes; false when not connected. */
  sendKeys(session: string, bytes: number[]): boolean {
    return bytes.length > 0 && this.send({ type: "input_keys", session, bytes });
  }

  /** Ask the brain for every character's snapshot again; false when not connected. */
  requestState(): boolean {
    return this.send({ type: "state_request" });
  }

  /** Ask the brain to re-sync a character (`st`, `exp`, `i`, look); false when not connected. */
  resync(session: string): boolean {
    return this.send({ type: "resync", session });
  }

  /** Walk a character to a room (replacing a walk in progress); `stopShort` false walks into a
   * boss lair (left out, the walk stops one room short of one). False when not connected. */
  walkTo(session: string, map: number, room: number, stopShort?: boolean, fight?: boolean): boolean {
    const msg: Extract<ToGateway, { type: "walk_to" }> = { type: "walk_to", session, map, room };
    if (stopShort !== undefined) msg.stop_short = stopShort;
    if (fight !== undefined) msg.fight = fight;
    return this.send(msg);
  }

  /** Find rooms, places and monsters by part of a name (a `found` answers); false when not
   * connected or the query is too short. */
  find(session: string, query: string): boolean {
    const q = query.trim();
    return q.length >= MIN_QUERY && this.send({ type: "find", session, query: q });
  }

  /** Stop a character's walk after the move in flight; false when not connected. */
  walkStop(session: string): boolean {
    return this.send({ type: "walk_stop", session });
  }

  /** Start, stop, pause or resume a character's bot; false when not connected. */
  botControl(session: string, action: BotAction): boolean {
    return this.send({ type: "bot_control", session, action });
  }

  /** Connect, log off or hang up a session; false when not connected. */
  sessionControl(session: string, action: SessionAction): boolean {
    return this.send({ type: "session_control", session, action });
  }

  /** Ask for a `map_view` now, of the rooms within `radius` steps (clamped to 1-30). */
  requestMap(session: string, radius?: number): boolean {
    return this.send({ type: "map_request", session, radius: clampRadius(radius) });
  }

  /** Ask for the nearest loops, at most `limit`, those matching `query` if given, from anywhere
   * the character can walk to; `radius` (clamped to 1-30) is how far around it the generated
   * ones are made. */
  requestLoops(session: string, radius = LOOP_RADIUS, query?: string, limit?: number): boolean {
    return this.send({
      type: "loops_request",
      session,
      radius: clampRadius(radius),
      ...(query ? { query } : {}),
      ...(limit ? { limit } : {}),
    });
  }

  /** Set the player's monster rule for names holding `pattern` (any case), or remove it
   * (`null`); the bot's `monster_rules` shows the result. False when not connected or the
   * pattern is shorter than two characters. */
  setMonsterRule(session: string, pattern: string, rule: MonsterRuleKind | null): boolean {
    const p = pattern.trim();
    return p.length >= 2 && this.send({ type: "set_monster_rule", session, pattern: p, rule });
  }

  /** Ask for the spells the character lacks (a `spell_plan` answer); false when not connected. */
  requestSpellPlan(session: string): boolean {
    return this.send({ type: "spell_plan_request", session });
  }

  /** Ask for the character's last runs (a `stats` answer); false when not connected. */
  requestStats(session: string): boolean {
    return this.send({ type: "stats_request", session });
  }

  /** Ask for the character's settings, with the values the file of `layer` holds
   * (`character`, `server` or `mine`); false when not connected. */
  requestSettings(session: string, layer: string): boolean {
    return this.send({ type: "settings_request", session, layer });
  }

  /** Change the file of `layer`: each change sets a value, or takes the key out when it has
   * none; false when not connected. */
  saveSettings(session: string, layer: string, changes: SettingChange[]): boolean {
    return this.send({ type: "settings_save", session, layer, changes });
  }

  /** Zero the character's numbers: `run` ends the current run and starts a new one at zero
   * (the old one stays in the history), `combat` zeroes the combat numbers; left out, both.
   * False when not connected. */
  resetStats(session: string, only?: StatsPart): boolean {
    const msg: Extract<ToGateway, { type: "stats_reset" }> = { type: "stats_reset", session };
    if (only !== undefined) msg.only = only;
    return this.send(msg);
  }

  /** Walk a generated loop once; false when not connected. */
  walkLoop(session: string, id: string): boolean {
    return this.send({ type: "walk_loop", session, id });
  }

  /** Run a generated loop (Task mode), passing by the stops in `skip`; for the loop the bot
   * already runs, only change `skip`. False when not connected. */
  runLoop(session: string, id: string, skip: RoomKey[] = []): boolean {
    return this.send({ type: "run_loop", session, id, skip });
  }

  /** Make or change a custom loop: `name`, and the `stops` in order, made from the loop
   * `base`. The same name again replaces it. False when not connected or the name is blank. */
  saveLoop(session: string, name: string, base: string | undefined, stops: RoomKey[]): boolean {
    const n = name.trim();
    return n.length > 0 && stops.length > 0 && this.send({ type: "save_loop", session, name: n, base, stops });
  }

  /** Delete a custom loop (`custom:...`); false when not connected. */
  deleteLoop(session: string, id: string): boolean {
    return this.send({ type: "delete_loop", session, id });
  }

  /** Ask for the parties (a `parties` answer); false when not connected. */
  requestParties(): boolean {
    return this.send({ type: "parties_request" });
  }

  /** Make a party (`id` empty) or replace one; false when not connected. */
  saveParty(party: PartyDef): boolean {
    return this.send({ type: "party_save", party });
  }

  /** Delete a party; its followers leave it in the game. False when not connected. */
  deleteParty(id: string): boolean {
    return this.send({ type: "party_delete", id });
  }

  /** Take a follower out of its party; false when not connected. */
  leaveParty(session: string): boolean {
    return this.send({ type: "party_leave", session });
  }

  /** Add a character to party `party` as a follower; false when not connected. */
  joinParty(session: string, party: string): boolean {
    return this.send({ type: "party_join", session, party });
  }

  /** Form party `party` (left out: every party) up again now: each member checks with `par`
   * and invites and follows go at once. False when not connected. */
  resyncParties(party?: string): boolean {
    return this.send(party === undefined ? { type: "party_resync" } : { type: "party_resync", party });
  }

  /** Ask for every character's blessings (a `blessings` answer); false when not connected. */
  requestBlessings(): boolean {
    return this.send({ type: "blessings_request" });
  }

  /** The lasting spells (short names) `session` keeps on; false when not connected. */
  saveBlessings(session: string, spells: string[]): boolean {
    return this.send({ type: "blessings_save", session, spells });
  }

  private send(message: ToGateway): boolean {
    if (!this.welcomed || !this.socket || this.socket.readyState !== OPEN) return false;
    this.socket.send(JSON.stringify(message));
    return true;
  }

  private open(): void {
    this.welcomed = false;
    this.opts.events.connection?.("connecting");
    const socket = this.opts.socket(this.opts.url);
    this.socket = socket;
    socket.onopen = () => {
      if (this.socket !== socket) return;
      const token = this.opts.tokens?.getItem(TOKEN_KEY) ?? undefined;
      const hello: ToGateway = { type: "hello", protocol: PROTOCOL, client: "ui" };
      if (token) hello.token = token;
      socket.send(JSON.stringify(hello));
    };
    socket.onmessage = (ev) => {
      if (this.socket !== socket || typeof ev.data !== "string") return;
      let message: ToClient;
      try {
        message = JSON.parse(ev.data) as ToClient;
      } catch {
        return;
      }
      this.dispatch(message);
    };
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.dropped();
    };
    socket.onerror = () => {
      // A close event always follows.
    };
  }

  private dropped(): void {
    this.socket = null;
    this.welcomed = false;
    for (const sync of this.syncs.values()) {
      sync.pending = null;
      sync.buffer = [];
    }
    if (this.stopped || this.refused) return;
    const delay = Math.min(this.opts.minRetryMs * 2 ** this.retries, this.opts.maxRetryMs);
    this.retries += 1;
    this.opts.events.connection?.("down", { retryInMs: delay });
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null;
      this.open();
    }, delay);
  }

  private clearRetry(): void {
    if (this.retryTimer !== null) clearTimeout(this.retryTimer);
    this.retryTimer = null;
  }

  private dispatch(message: ToClient): void {
    switch (message.type) {
      case "welcome":
        this.welcomed = true;
        this.brainUnheard = true;
        this.retries = 0;
        this.opts.events.connection?.("open");
        this.sessionList(message.sessions);
        break;
      case "refused":
        this.refusedFor(message.reason);
        break;
      case "session_list":
        this.sessionList(message.sessions);
        break;
      case "session_state":
        this.opts.events.state?.(message.session, message.state);
        break;
      case "output":
        this.output(message.session, { seq: message.seq, text: message.text });
        break;
      case "history":
        this.history(message.session, message.chunks);
        break;
      case "lagged": {
        const sync = this.syncs.get(message.session);
        if (sync && !sync.pending && sync.lastSeq !== null) this.catchUp(message.session, sync);
        break;
      }
      case "error":
        this.opts.events.error?.(message.message);
        break;
      case "brain_state":
        this.opts.events.brainState?.(message.attached);
        // A page that opens (or reconnects) after the brain attached asks for the snapshots;
        // a brain that attaches later sends them itself.
        if (this.brainUnheard && message.attached) this.requestState();
        this.brainUnheard = false;
        break;
      case "brain_lagged":
        // Relayed patches were dropped: start again from whole snapshots.
        this.requestState();
        this.opts.events.brainLagged?.(message.missed);
        break;
      case "event":
        this.opts.events.event?.(message.session, message);
        break;
      case "brain_info":
        this.opts.events.brainInfo?.(message);
        break;
      case "notice":
      case "warning":
        this.opts.events.notice?.(message.type, message.text, message.session);
        break;
      case "character":
        this.opts.events.character?.(message.session, message.state);
        break;
      case "character_patch":
        this.opts.events.characterPatch?.(message.session, message.patch);
        break;
      case "map_view":
        this.opts.events.mapView?.(message);
        break;
      case "loops":
        this.opts.events.loops?.(message);
        break;
      case "stats":
        this.opts.events.stats?.(message);
        break;
      case "spell_plan":
        this.opts.events.spellPlan?.(message);
        break;
      case "settings":
        this.opts.events.settings?.(message);
        break;
      case "found":
        this.opts.events.found?.(message);
        break;
      case "parties":
        this.opts.events.parties?.(message.parties);
        break;
      case "blessings":
        this.opts.events.blessings?.(message.characters);
        break;
    }
  }

  private refusedFor(reason: string): void {
    this.refused = true;
    const socket = this.socket;
    this.socket = null;
    socket?.close();
    if (/token/i.test(reason) && this.opts.askToken) {
      this.opts.tokens?.removeItem(TOKEN_KEY);
      const token = this.opts.askToken(reason)?.trim();
      if (token) {
        this.opts.tokens?.setItem(TOKEN_KEY, token);
        this.refused = false;
        this.open();
        return;
      }
    }
    this.opts.events.connection?.("refused", { reason });
  }

  private sessionList(sessions: SessionInfo[]): void {
    const ids = new Set(sessions.map((s) => s.id));
    for (const id of this.syncs.keys()) if (!ids.has(id)) this.syncs.delete(id);
    this.opts.events.sessions?.(sessions);
    for (const info of sessions) {
      let sync = this.syncs.get(info.id);
      if (!sync) {
        sync = { lastSeq: null, pending: null, buffer: [] };
        this.syncs.set(info.id, sync);
      }
      if (sync.pending) continue;
      if (sync.lastSeq === null) {
        this.backfill(info.id, sync, info.last_seq);
      } else if (info.last_seq < sync.lastSeq) {
        // The log was replaced (a fresh data directory): start the tile over.
        this.opts.events.reset?.(info.id);
        this.backfill(info.id, sync, info.last_seq);
      } else if (info.last_seq > sync.lastSeq) {
        this.catchUp(info.id, sync);
      }
    }
  }

  /** Fetch the latest chunks up to and including `through` (the session's `last_seq`). */
  private backfill(session: string, sync: Sync, through: number): void {
    sync.lastSeq = null;
    if (through === 0) {
      sync.lastSeq = 0;
      return;
    }
    sync.pending = { kind: "backfill", through };
    this.send({
      type: "history_request",
      session,
      before_seq: through + 1,
      max_chunks: this.opts.historyChunks,
    });
  }

  /** Fetch the chunks after the last one written. */
  private catchUp(session: string, sync: Sync): void {
    sync.pending = { kind: "after" };
    this.send({
      type: "history_request",
      session,
      after_seq: sync.lastSeq ?? 0,
      max_chunks: this.opts.historyChunks,
    });
  }

  private output(session: string, chunk: Chunk): void {
    const sync = this.syncs.get(session);
    if (!sync) return;
    if (sync.pending) {
      sync.buffer.push(chunk);
    } else {
      this.apply(session, sync, chunk);
    }
  }

  /** Write a chunk if it is the next one; drop one already written; fetch a gap. */
  private apply(session: string, sync: Sync, chunk: Chunk): void {
    const last = sync.lastSeq ?? 0;
    if (chunk.seq <= last) return;
    if (chunk.seq === last + 1) {
      sync.lastSeq = chunk.seq;
      this.opts.events.write?.(session, chunk.text, chunk.seq);
      return;
    }
    sync.buffer.push(chunk);
    this.catchUp(session, sync);
  }

  private history(session: string, chunks: Chunk[]): void {
    const sync = this.syncs.get(session);
    if (!sync?.pending) return;
    const request = sync.pending;
    sync.pending = null;
    if (request.kind === "backfill") {
      sync.lastSeq = chunks.length > 0 ? chunks[0].seq - 1 : request.through;
    }
    this.feed(session, sync, chunks);
    if (sync.pending) return;
    if (request.kind === "after" && chunks.length >= this.opts.historyChunks) {
      // A full page: there may be more.
      this.catchUp(session, sync);
      return;
    }
    const held = sync.buffer.sort((a, b) => a.seq - b.seq);
    sync.buffer = [];
    this.feed(session, sync, held);
  }

  /** Apply chunks in order until one opens a gap; hold the rest for when it is filled. */
  private feed(session: string, sync: Sync, chunks: Chunk[]): void {
    for (const chunk of chunks) {
      if (sync.pending) sync.buffer.push(chunk);
      else this.apply(session, sync, chunk);
    }
  }
}

/** The gateway's WebSocket URL for a page served by it (or by Vite, which proxies `/ws`). */
export function gatewayUrl(location: { protocol: string; host: string }): string {
  const scheme = location.protocol === "https:" ? "wss:" : "ws:";
  return `${scheme}//${location.host}/ws`;
}

/** Key strings from the terminal as bytes for `input_keys`: ASCII only (the game is CP437). */
export function keyBytes(data: string): number[] {
  const bytes: number[] = [];
  for (const ch of data) {
    const code = ch.codePointAt(0)!;
    if (code < 0x80) bytes.push(code);
  }
  return bytes;
}
