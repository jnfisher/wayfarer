// The Accounts panel's edits (C-062): the servers, characters and BBS logins in wayfarer.toml and
// credentials.toml, as a draft the panel changes and sends whole with `accounts_save`.

import type { AccountCharacter, AccountServer, Accounts, SessionInfo } from "../proto";

/** The web UI lays out six tiles at most; the gateway refuses a save with more. */
export const MAX_CHARACTERS = 6;
/** What a new row's sign-up details start as. */
export const DEFAULT_EMAIL = "h@h.com";
export const DEFAULT_GENDER = "F";

/** A server row; `fresh` until it has been saved. */
export interface ServerDraft extends AccountServer {
  fresh: boolean;
}

/** A character row; `fresh` until saved (only then may its id change). */
export interface CharacterDraft extends AccountCharacter {
  fresh: boolean;
  /** The id still follows the name (a fresh row whose id was never typed in). */
  idFromName: boolean;
}

export interface Draft {
  servers: ServerDraft[];
  characters: CharacterDraft[];
}

export function draftOf(accounts: Accounts): Draft {
  return {
    servers: accounts.servers.map((s) => ({ ...s, fresh: false })),
    characters: accounts.characters.map((c) => ({ ...c, fresh: false, idFromName: false })),
  };
}

/** A session id from a display name: lower case, only letters, digits, `-` and `_`. */
export function idFromName(name: string): string {
  return name
    .toLowerCase()
    .replace(/\s+/g, "_")
    .replace(/[^a-z0-9_-]/g, "")
    .slice(0, 64);
}

export function newServer(draft: Draft): ServerDraft {
  let n = draft.servers.length + 1;
  while (draft.servers.some((s) => s.id === `server${n}`)) n += 1;
  return {
    id: draft.servers.length === 0 ? "home" : `server${n}`,
    host: "",
    port: 23,
    preset: "mbbsemu",
    custom_logon: false,
    can_sign_up: true,
    fresh: true,
  };
}

export function newCharacter(draft: Draft): CharacterDraft {
  return {
    id: "",
    name: "",
    server: draft.servers[0]?.id ?? "",
    autostart: true,
    user: "",
    password: "",
    email: DEFAULT_EMAIL,
    gender: DEFAULT_GENDER,
    fresh: true,
    idFromName: true,
  };
}

/** `c` with `name` set, and its id following it while it may. */
export function withName(c: CharacterDraft, name: string): CharacterDraft {
  return c.idFromName ? { ...c, name, id: idFromName(name) } : { ...c, name };
}

/** What the gateway is sent: the draft without the panel's own fields. */
export function toAccounts(draft: Draft, saved: Accounts | undefined): Accounts {
  const before = new Map(saved?.characters.map((c) => [c.id, c]));
  return {
    servers: draft.servers.map(({ fresh: _, ...s }) => s),
    characters: draft.characters.map(({ fresh, idFromName: _, email, gender, reason: __, ...c }) => {
      const signsUp = fresh || before.get(c.id)?.user !== c.user;
      return signsUp ? { ...c, email: email || DEFAULT_EMAIL, gender: gender || DEFAULT_GENDER } : c;
    }),
    presets: [],
  };
}

const ID = /^[A-Za-z0-9_-]{1,64}$/;

/**
 * What is wrong with the draft, the checks the gateway makes too, by row: `server:<i>`,
 * `character:<i>`, and `all` for the whole. Empty when it may be saved.
 */
export function problems(draft: Draft): Record<string, string> {
  const out: Record<string, string> = {};
  const serverIds = new Set<string>();
  draft.servers.forEach((s, i) => {
    const key = `server:${i}`;
    if (!ID.test(s.id)) out[key] = "id: 1-64 letters, digits, - or _";
    else if (serverIds.has(s.id)) out[key] = `${s.id} is listed twice`;
    else if (!s.host.trim()) out[key] = "no host";
    else if (!Number.isInteger(s.port) || s.port < 1 || s.port > 65535) out[key] = "port: 1 to 65535";
    serverIds.add(s.id);
  });
  const ids = new Set<string>();
  draft.characters.forEach((c, i) => {
    const key = `character:${i}`;
    if (!c.name.trim()) out[key] = "no name";
    else if (!ID.test(c.id)) out[key] = "id: 1-64 letters, digits, - or _";
    else if (ids.has(c.id)) out[key] = `${c.id} is listed twice`;
    else if (!serverIds.has(c.server)) out[key] = "pick a server";
    else if (c.fresh && !c.user.trim()) out[key] = "no BBS user";
    ids.add(c.id);
  });
  if (draft.characters.length > MAX_CHARACTERS) out.all = `at most ${MAX_CHARACTERS} characters`;
  return out;
}

/** The characters (by name) that use server `id`, which therefore can't be deleted. */
export function usersOf(draft: Draft, id: string): string[] {
  return draft.characters.filter((c) => c.server === id).map((c) => c.name || c.id);
}

/** Whether the draft differs from what was saved. */
export function changed(draft: Draft, saved: Accounts | undefined): boolean {
  if (!saved) return false;
  return JSON.stringify(toAccounts(draft, saved)) !== JSON.stringify(toAccounts(draftOf(saved), saved));
}

export interface SaveEffect {
  /** Open sessions that hang up and log on again: their server or login changed. */
  reconnect: string[];
  /** Open sessions that hang up and go. */
  removed: string[];
  /** New characters, which connect. */
  added: string[];
}

/** What a save would do to the sessions, by name, as the gateway decides it. */
export function saveEffect(draft: Draft, saved: Accounts, sessions: SessionInfo[]): SaveEffect {
  const open = (id: string) => {
    const state = sessions.find((s) => s.id === id)?.state;
    return state !== undefined && state !== "closed";
  };
  const name = (id: string) => sessions.find((s) => s.id === id)?.name ?? id;
  const serverKey = (servers: AccountServer[], id: string) => {
    const s = servers.find((x) => x.id === id);
    return s ? JSON.stringify([s.host, s.port, s.preset ?? null]) : "";
  };
  const effect: SaveEffect = { reconnect: [], removed: [], added: [] };
  for (const was of saved.characters) {
    if (!draft.characters.some((c) => c.id === was.id) && open(was.id)) effect.removed.push(name(was.id));
  }
  for (const c of draft.characters) {
    const was = saved.characters.find((x) => x.id === c.id);
    if (!was) {
      effect.added.push(c.name || c.id);
      continue;
    }
    const moved =
      was.server !== c.server || serverKey(saved.servers, was.server) !== serverKey(draft.servers, c.server);
    const login = was.user !== c.user || was.password !== c.password;
    if ((moved || login) && open(c.id)) effect.reconnect.push(name(c.id));
  }
  return effect;
}

function list(names: string[]): string {
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** What the save button asks before a save that drops connections; null when none drops. */
export function confirmText(effect: SaveEffect): string | null {
  const parts: string[] = [];
  if (effect.reconnect.length) parts.push(`${list(effect.reconnect)} will hang up and log on again`);
  if (effect.removed.length) parts.push(`${list(effect.removed)} will hang up and go`);
  return parts.length ? `Save: ${parts.join("; ")}.` : null;
}
