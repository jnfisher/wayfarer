// Parties in the page (docs/plan/10-party-play.md §10): the chip on each tile's header, its
// menu, the follower's locked controls, and the Parties panel's status lines and drafts. The
// brain keeps the parties; this only reads its `parties` message and builds `party_save`.

import type { CharacterState, GuardPair, MemberStatus, PartyDef, PartyStatus, SessionInfo } from "../proto";

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

/** Everyone in the party `session` is in, the leader first; just `session` out of a party. */
export function partySessions(parties: PartyStatus[], session: string): string[] {
  const p = partyOf(parties, session);
  return p ? [p.party.party.leader, ...p.party.party.followers] : [session];
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

/** Errands that stay in the room: a follower runs them in the party. */
const IN_THE_ROOM = ["pick_up_all", "equip_all", "cancel_errand", "spell_plan"];

/** What an errand that walks does to a follower: it leaves the party first. */
export function leavesFirst(leader: string): string {
  return `Leaves ${leader}'s party first`;
}

/** A follower's errand menu: the errands that walk say it leaves the party first. */
export function followerErrands<T extends { action: string; disabled: boolean; title: string }>(
  items: T[],
  following: string | null,
): T[] {
  return items.map((item) =>
    following && !item.disabled && !IN_THE_ROOM.includes(item.action)
      ? { ...item, title: `${leavesFirst(following)}. ${item.title}` }
      : item,
  );
}

/** A member's line in the Parties panel: its state, rank and guarding (1.20 `guard`). */
export function memberLine(m: MemberStatus, leader: string): string {
  const parts = [stateLine(m, leader)];
  if (m.rank) parts.push(RANK_WORD[m.rank]);
  if (m.guarding) parts.push(`guarding ${m.guarding}`);
  if (m.guarded_by) parts.push(`guarded by ${m.guarded_by}`);
  return parts.join(", ");
}

/** A rank as a member's line says it (`joined, back rank`). */
const RANK_WORD: Record<NonNullable<MemberStatus["rank"]>, string> = {
  front: "front rank",
  mid: "middle rank",
  back: "back rank",
};

function stateLine(m: MemberStatus, leader: string): string {
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
  /** Who guards whom (Quicksilver 1.20 `guard`, C-068). */
  guards: GuardPair[];
  /** The member that attacks last (C-132), or null for nobody. */
  attack_last: string | null;
}

export function draftOf(p: PartyStatus): Draft {
  return {
    id: p.party.id,
    leader: p.party.leader,
    followers: [...p.party.followers],
    guards: (p.party.guards ?? []).map((g) => ({ ...g })),
    attack_last: p.party.attack_last ?? null,
  };
}

/** A new party: led by the first character in no party, else the first. */
export function newDraft(sessions: SessionInfo[], parties: PartyStatus[]): Draft {
  const free = sessions.find((s) => !partyOf(parties, s.id));
  return { id: "", leader: (free ?? sessions[0])?.id ?? "", followers: [], guards: [], attack_last: null };
}

/** The guard pairs whose two are still in the party, and the one attacking last while it is. */
function keptGuards(d: Draft): Draft {
  const members = new Set([d.leader, ...d.followers]);
  return {
    ...d,
    guards: d.guards.filter((g) => members.has(g.guard) && members.has(g.ward)),
    attack_last: d.attack_last && members.has(d.attack_last) ? d.attack_last : null,
  };
}

/** Tick or untick a follower; the leader can't follow, and five is the most. */
export function toggleFollower(d: Draft, session: string): Draft {
  if (session === d.leader) return d;
  if (d.followers.includes(session)) return keptGuards({ ...d, followers: d.followers.filter((f) => f !== session) });
  if (d.followers.length >= MAX_FOLLOWERS) return d;
  return { ...d, followers: [...d.followers, session] };
}

/** A new leader: it stops being a follower. */
export function setLeader(d: Draft, session: string): Draft {
  return keptGuards({ ...d, leader: session, followers: d.followers.filter((f) => f !== session) });
}

/** `guard` guards `ward`, or nobody (null): one ward a guard. */
export function setGuard(d: Draft, guard: string, ward: string | null): Draft {
  const others = d.guards.filter((g) => g.guard !== guard);
  return { ...d, guards: ward && ward !== guard ? [...others, { guard, ward }] : others };
}

/** Whom `guard` guards in the draft, or "". */
export function wardOf(d: Draft, guard: string): string {
  return d.guards.find((g) => g.guard === guard)?.ward ?? "";
}

/** `session` attacks last, or nobody (null): one a party (C-132). */
export function setAttackLast(d: Draft, session: string | null): Draft {
  const members = [d.leader, ...d.followers];
  return { ...d, attack_last: session && members.includes(session) ? session : null };
}

/** Why a party has a member that attacks last, for the control's title. */
export const ATTACK_LAST_TITLE =
  "Attacks after the others each round: when another member engages, it sends its attack again, " +
  "so a spell that lands several times (meteor swarm) spends only what the monster has left";

/** Guards can be set when every member's server is known to play Quicksilver 1.20; else why
 * not. */
export function guardsOff(d: Draft, characters: Record<string, CharacterState | undefined>): string | null {
  const all = [d.leader, ...d.followers];
  return all.every((s) => characters[s]?.rule_set?.rules === "1.20") ? null : "1.20 servers only";
}

/** Why the draft can't be saved, or null. */
export function draftProblem(d: Draft): string | null {
  if (!d.leader) return "pick a leader";
  if (d.followers.length === 0) return "tick at least one follower";
  return null;
}

/** The draft as `party_save` takes it. */
export function toDef(d: Draft): PartyDef {
  const def: PartyDef = { id: d.id, leader: d.leader, followers: [...d.followers] };
  if (d.guards.length) def.guards = d.guards.map((g) => ({ ...g }));
  if (d.attack_last) def.attack_last = d.attack_last;
  return def;
}

/** The other party a character is in, by its leader's name, for the panel's tick list. */
export function elsewhere(parties: PartyStatus[], session: string, draftId: string, names: Names): string | null {
  const p = partyOf(parties, session);
  if (!p || p.party.party.id === draftId) return null;
  return `in ${names[p.party.party.leader] ?? p.party.party.leader}'s party`;
}
