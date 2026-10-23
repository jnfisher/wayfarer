// Parties in the page (docs/plan/10-party-play.md §10): the chip on each tile's header, its
// menu, the follower's locked controls, and the Parties panel's status lines and drafts. The
// brain keeps the parties; this only reads its `parties` message and builds `party_save`.

import type { CharacterState, MemberStatus, PartyDef, PartyStatus, SessionInfo } from "../proto";

/** The most followers a party takes (the game's six members). */
export const MAX_FOLLOWERS = 5;

/** A session's display name, by id. */
export type Names = Record<string, string>;

export function namesOf(sessions: SessionInfo[]): Names {
  return Object.fromEntries(sessions.map((s) => [s.id, s.name]));
}

/** The party `session` is in, and as what. */
export function partyOf(
  parties: PartyStatus[],
  session: string,
): { party: PartyStatus; leader: boolean; member: MemberStatus | undefined } | null {
  for (const p of parties) {
    const leader = p.party.leader === session;
    if (leader || p.party.followers.includes(session)) {
      return { party: p, leader, member: p.members.find((m) => m.session === session) };
    }
  }
  return null;
}

/** States a follower is out of the leader's party in. */
const APART = new Set<MemberStatus["state"]>(["separated", "walking", "unreachable"]);

export interface PartyChip {
  text: string;
  /** `leader`, `follower`, or `warn` for a follower out of the party. */
  tone: "leader" | "follower" | "warn";
  title: string;
}

/** The chip on a tile's header; null for a character in no party. */
export function chipFor(parties: PartyStatus[], session: string, names: Names): PartyChip | null {
  const p = partyOf(parties, session);
  if (!p) return null;
  const name = (id: string) => names[id] ?? id;
  if (p.leader) {
    const followers = p.party.party.followers.map(name).join(", ");
    return { text: `★ Leading: ${followers}`, tone: "leader", title: `Leads ${followers}` };
  }
  const leader = name(p.party.party.leader);
  const state = p.member?.state;
  if (state && APART.has(state)) {
    return { text: "Separated", tone: "warn", title: memberLine(p.member!, leader) };
  }
  return {
    text: `Following ${leader}`,
    tone: "follower",
    title: p.member ? memberLine(p.member, leader) : `Following ${leader}`,
  };
}

export type ChipAction = { kind: "leave"; label: string } | { kind: "join"; label: string; party: string };

/** The chip's menu: a follower may leave; a character in no party may join any party. */
export function chipMenu(parties: PartyStatus[], session: string, names: Names): ChipAction[] {
  const p = partyOf(parties, session);
  if (p) return p.leader ? [] : [{ kind: "leave", label: "Leave party" }];
  return parties
    .filter((q) => q.party.followers.length < MAX_FOLLOWERS)
    .map((q) => ({
      kind: "join" as const,
      label: `Join ${names[q.party.leader] ?? q.party.leader}'s party`,
      party: q.party.id,
    }));
}

/** The leader's name when `session` follows one: its own walking controls are off. */
export function followingName(parties: PartyStatus[], session: string | undefined, names: Names): string | null {
  if (!session) return null;
  const p = partyOf(parties, session);
  if (!p || p.leader) return null;
  return names[p.party.party.leader] ?? p.party.party.leader;
}

/** The title on a follower's disabled control. */
export function lockedTitle(leader: string): string {
  return `Following ${leader}: it goes where the party goes`;
}

/** Errands that stay in the room: a follower may run them, and cancel any. */
const IN_THE_ROOM = ["pick_up_all", "equip_all", "cancel_errand"];

/** A follower's errand menu: the errands that walk are off, it goes where the party goes. */
export function followerErrands<T extends { action: string; disabled: boolean; title: string }>(
  items: T[],
  following: string | null,
): T[] {
  return items.map((item) =>
    following && !IN_THE_ROOM.includes(item.action)
      ? { ...item, disabled: true, title: lockedTitle(following) }
      : item,
  );
}

/** A member's line in the Parties panel. */
export function memberLine(m: MemberStatus, leader: string): string {
  const detail = m.detail ? `: ${m.detail}` : "";
  switch (m.state) {
    case "joined":
      return "joined";
    case "not_joined":
      return `not joined yet${detail}`;
    case "away":
      return `away${detail}`;
    case "separated":
      return "separated";
    case "walking":
      return `walking to ${leader}${m.detail ? ` (${m.detail})` : ""}`;
    case "resting":
      return `resting${m.detail ? ` ${m.detail}` : ""}`;
    case "unreachable":
      return `can't reach ${leader}${detail}`;
  }
}

/**
 * What a member is doing and its HP, for its line in the Parties panel (C-046):
 * `attacking large giant rat · 72%`; `paused`, `stopped`; null with nothing known.
 */
export function activityLine(c: CharacterState | undefined): string | null {
  if (!c) return null;
  const bot = c.bot;
  const doing = !bot ? null : bot.state === "running" ? (bot.doing ?? null) : bot.state;
  const hp = c.hp !== undefined && c.hp_max ? `${Math.floor((c.hp * 100) / c.hp_max)}%` : null;
  const parts = [doing, hp].filter((p): p is string => !!p);
  return parts.length ? parts.join(" · ") : null;
}

/** A party being edited in the panel. */
export interface Draft {
  /** Empty for a new party. */
  id: string;
  leader: string;
  followers: string[];
}

export function draftOf(p: PartyStatus): Draft {
  return { id: p.party.id, leader: p.party.leader, followers: [...p.party.followers] };
}

/** A new party: led by the first character in no party, else the first. */
export function newDraft(sessions: SessionInfo[], parties: PartyStatus[]): Draft {
  const free = sessions.find((s) => !partyOf(parties, s.id));
  return { id: "", leader: (free ?? sessions[0])?.id ?? "", followers: [] };
}

/** Tick or untick a follower; the leader can't follow, and five is the most. */
export function toggleFollower(d: Draft, session: string): Draft {
  if (session === d.leader) return d;
  if (d.followers.includes(session)) return { ...d, followers: d.followers.filter((f) => f !== session) };
  if (d.followers.length >= MAX_FOLLOWERS) return d;
  return { ...d, followers: [...d.followers, session] };
}

/** A new leader: it stops being a follower. */
export function setLeader(d: Draft, session: string): Draft {
  return { ...d, leader: session, followers: d.followers.filter((f) => f !== session) };
}

/** Why the draft can't be saved, or null. */
export function draftProblem(d: Draft): string | null {
  if (!d.leader) return "pick a leader";
  if (d.followers.length === 0) return "tick at least one follower";
  return null;
}

/** The draft as `party_save` takes it. */
export function toDef(d: Draft): PartyDef {
  return { id: d.id, leader: d.leader, followers: [...d.followers] };
}

/** The other party a character is in, by its leader's name, for the panel's tick list. */
export function elsewhere(parties: PartyStatus[], session: string, draftId: string, names: Names): string | null {
  const p = partyOf(parties, session);
  if (!p || p.party.party.id === draftId) return null;
  return `in ${names[p.party.party.leader] ?? p.party.party.leader}'s party`;
}
