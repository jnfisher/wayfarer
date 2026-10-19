// What the brain has told the UI: one character snapshot per session and whether a brain is
// attached (docs/protocol.md, "Events, state and dev"). Plain functions on a plain object, so
// App.svelte can keep it in `$state` and the tests can drive it without Svelte.

import type { CharacterState, ParseCounts, WorldPackInfo } from "../proto";

/** Who the brain is (`brain_info`). */
export interface BrainInfo {
  build: string;
  started_at: string;
  world_pack?: WorldPackInfo;
}

export interface BrainView {
  /** The last snapshot per session, with every patch since applied. */
  characters: Record<string, CharacterState>;
  /** From `brain_state`; null until the gateway has said. */
  attached: boolean | null;
}

export function emptyBrainView(): BrainView {
  return { characters: {}, attached: null };
}

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

/**
 * Apply a JSON merge patch (RFC 7386): objects recurse, `null` deletes, anything else
 * replaces. Changes `target` in place when it is an object (so a Svelte `$state` proxy updates
 * only the fields that changed) and returns the result.
 */
export function mergePatch(target: unknown, patch: unknown): unknown {
  if (!isObject(patch)) return patch;
  const out = isObject(target) ? target : {};
  for (const [key, value] of Object.entries(patch)) {
    if (value === null) delete out[key];
    else out[key] = mergePatch(out[key], value);
  }
  return out;
}

/** `character`: the whole snapshot replaces the last one. */
export function applyCharacter(view: BrainView, session: string, state: CharacterState): void {
  view.characters[session] = state;
}

/** `character_patch`: merged into the last snapshot (or an empty one). */
export function applyCharacterPatch(view: BrainView, session: string, patch: unknown): void {
  const current = view.characters[session];
  const next = mergePatch(current ?? {}, patch);
  if (next !== current) view.characters[session] = isObject(next) ? (next as CharacterState) : {};
}

/**
 * The gateway's whole session list (`welcome`, `session_list`): forget the snapshots of sessions
 * no longer in it, such as a character taken out of the config while the page was open.
 */
export function pruneCharacters(view: BrainView, ids: Iterable<string>): void {
  const keep = new Set(ids);
  for (const id of Object.keys(view.characters)) {
    if (!keep.has(id)) delete view.characters[id];
  }
}

export function applyBrainState(view: BrainView, attached: boolean): void {
  view.attached = attached;
}

// The dev panel's buffers (C-018): what the brain said, per session, bounded so a page left
// open all day does not grow. Kept outside Svelte's deep `$state` (2,000 proxied events per
// session would be slow): App.svelte bumps a counter at most once a frame when they change, and
// DevPanel.svelte is a view of them.

/** The last events kept per session. */
export const MAX_EVENTS = 2000;
/** The last notices and warnings kept per session (and for the brain's own log). */
export const MAX_NOTICES = 500;
/** The most distinct unclassified lines kept per session. */
export const MAX_UNKNOWN = 1000;
/** Above this share of unclassified lines the rate shows red (0.2%). */
export const RATE_ALARM = 0.002;

export interface DevEvent {
  /** Arrival order across the page, for keys. */
  id: number;
  seq: number;
  line_no: number;
  name: string;
  cat: string;
  fields: Record<string, unknown>;
  line: string;
}

export type DevNoteKind = "notice" | "warning" | "brain";

export interface DevNote {
  id: number;
  /** When it arrived, `Date.now()`. */
  at: number;
  kind: DevNoteKind;
  /** The session it is about; undefined for the brain or the whole page. */
  session?: string;
  text: string;
}

export interface UnknownLine {
  text: string;
  count: number;
}

export interface DevLog {
  events: Record<string, DevEvent[]>;
  /** Unclassified lines per session: text to count, in first-seen order. */
  unknown: Record<string, Map<string, number>>;
  /** Notices and warnings per session; under "" those with no session and the brain's log. */
  notes: Record<string, DevNote[]>;
  /** The last `brain_info`. */
  info: BrainInfo | null;
  /** The next id. */
  next: number;
}

export function emptyDevLog(): DevLog {
  return { events: {}, unknown: {}, notes: {}, info: null, next: 1 };
}

/** Drop the oldest so at most `max` are left. */
export function trim<T>(list: T[], max: number): void {
  if (list.length > max) list.splice(0, list.length - max);
}

/** An `event` from the brain: kept, and counted when nothing classified its line. */
export function addEvent(log: DevLog, session: string, e: Omit<DevEvent, "id">): void {
  const list = (log.events[session] ??= []);
  list.push({ ...e, id: log.next++ });
  trim(list, MAX_EVENTS);
  if (e.name === "unknown") countUnknown(log, session, e.line);
}

function countUnknown(log: DevLog, session: string, line: string): void {
  const text = line.trimEnd();
  if (!text) return;
  const counts = (log.unknown[session] ??= new Map());
  counts.set(text, (counts.get(text) ?? 0) + 1);
  if (counts.size > MAX_UNKNOWN) {
    // Forget the oldest line seen only once; failing that, the oldest.
    let drop = counts.keys().next().value!;
    for (const [t, n] of counts) {
      if (n === 1) {
        drop = t;
        break;
      }
    }
    counts.delete(drop);
  }
}

/** A notice, warning or brain change that arrived `at`. */
export function addNote(log: DevLog, kind: DevNoteKind, text: string, session: string | undefined, at: number): void {
  const list = (log.notes[session ?? ""] ??= []);
  list.push({ id: log.next++, at, kind, session, text });
  trim(list, MAX_NOTICES);
}

/** Every session's notes and the brain's, oldest first. */
export function allNotes(log: DevLog): DevNote[] {
  return Object.values(log.notes)
    .flat()
    .sort((a, b) => a.id - b.id);
}

/** A session's unclassified lines, most frequent first (first seen first among equals). */
export function unknownLines(log: DevLog, session: string): UnknownLine[] {
  const counts = log.unknown[session];
  if (!counts) return [];
  return [...counts].map(([text, count]) => ({ text, count })).sort((a, b) => b.count - a.count);
}

/** The unclassified rate from a snapshot's `parse` counts: `0.73%`, or `--` before any line. */
export function formatRate(parse: ParseCounts | undefined): { text: string; alarm: boolean } {
  if (!parse || parse.lines <= 0) return { text: "--", alarm: false };
  const share = parse.unclassified / parse.lines;
  return { text: `${(share * 100).toFixed(2)}%`, alarm: share > RATE_ALARM };
}

/** An event's fields, compact: `hp=28 maxhp=40`. */
export function compactFields(fields: Record<string, unknown>): string {
  return Object.entries(fields)
    .map(([k, v]) => `${k}=${typeof v === "string" ? v : JSON.stringify(v)}`)
    .join(" ");
}

/** Whether an event matches the filter box: its name or line, ignoring case. */
export function eventMatches(e: DevEvent, filter: string): boolean {
  const f = filter.trim().toLowerCase();
  return !f || e.name.toLowerCase().includes(f) || e.line.toLowerCase().includes(f);
}

/** Text as Markdown inline code, fenced with more backticks than any run inside it. */
function code(text: string): string {
  const longest = Math.max(0, ...(text.match(/`+/g) ?? []).map((r) => r.length));
  const fence = "`".repeat(longest + 1);
  const pad = text.startsWith("`") || text.endsWith("`") ? " " : "";
  return `${fence}${pad}${text}${pad}${fence}`;
}

/** The unclassified lines as a Markdown list, for a PR's "KB notes". */
export function unknownMarkdown(lines: UnknownLine[]): string {
  return lines.map((l) => `- ${code(l.text)} (${l.count}x)\n`).join("");
}

/** `11:02:10`, local time. */
export function clock(at: number): string {
  const d = new Date(at);
  return [d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => String(n).padStart(2, "0")).join(":");
}

/** Where the dev panel's open state and tab are kept. */
export const DEV_KEY = "wayfarer.dev";
export const DEV_TABS = ["events", "unclassified", "brain"] as const;
export type DevTab = (typeof DEV_TABS)[number];
export interface DevPrefs {
  open: boolean;
  tab: DevTab;
}

/** The panel's open state and tab; closed on the events tab when nothing (readable) is kept. */
export function loadDevPrefs(storage: Pick<Storage, "getItem"> | undefined): DevPrefs {
  try {
    const raw: unknown = JSON.parse(storage?.getItem(DEV_KEY) ?? "null");
    if (isObject(raw)) {
      return { open: raw.open === true, tab: DEV_TABS.find((t) => t === raw.tab) ?? "events" };
    }
  } catch {
    // Unreadable or blocked: the defaults.
  }
  return { open: false, tab: "events" };
}

export function saveDevPrefs(storage: Pick<Storage, "setItem"> | undefined, prefs: DevPrefs): void {
  try {
    storage?.setItem(DEV_KEY, JSON.stringify(prefs));
  } catch {
    // Full or blocked: the panel still works and is forgotten on reload.
  }
}
