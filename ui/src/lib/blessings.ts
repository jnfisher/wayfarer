// Blessings in the page (C-047): the Blessings panel's boxes, one per character, its lasting
// spells ticked or not. The brain keeps the ticks; this only reads its `blessings` message and
// builds `blessings_save`. A spell the game lets go on another player lists the other
// characters as targets, ticked per spell (C-131).

import type { Blessings } from "../proto";

/** The ticks of one character being edited: its own spells, and per spell the other characters it keeps it on. */
export type Ticks = { spells: string[]; others: Record<string, string[]> };

/** The ticked spells (short names) of a character, as the brain has them. */
export function chosenOf(b: Blessings): string[] {
  return b.spells.filter((s) => s.chosen).map((s) => s.short);
}

/** The other characters each spell is kept on, as the brain has them (spells with none left out). */
export function othersOf(b: Blessings): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const s of b.spells) {
    const names = (s.others ?? []).filter((o) => o.chosen).map((o) => o.name);
    if (names.length > 0) out[s.short] = names;
  }
  return out;
}

/** Both, as the brain has them. */
export function ticksOf(b: Blessings): Ticks {
  return { spells: chosenOf(b), others: othersOf(b) };
}

/** Tick or untick `short` in `chosen`, keeping the order the character's spells are listed in. */
export function toggleSpell(b: Blessings, chosen: string[], short: string): string[] {
  const next = chosen.includes(short) ? chosen.filter((s) => s !== short) : [...chosen, short];
  const order = b.spells.map((s) => s.short);
  return next.sort((x, y) => order.indexOf(x) - order.indexOf(y));
}

/** Tick or untick the character `name` as a target of `short`, in the order the targets are listed. */
export function toggleTarget(b: Blessings, others: Record<string, string[]>, short: string, name: string): Record<string, string[]> {
  const have = others[short] ?? [];
  const next = have.includes(name) ? have.filter((n) => n !== name) : [...have, name];
  const order = (b.spells.find((s) => s.short === short)?.others ?? []).map((o) => o.name);
  next.sort((x, y) => order.indexOf(x) - order.indexOf(y));
  const out = { ...others };
  if (next.length > 0) out[short] = next;
  else delete out[short];
  return out;
}

function sameList(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((x) => b.includes(x));
}

/** Whether `ticks` differ from what the brain has. */
export function changed(b: Blessings, ticks: Ticks): boolean {
  if (!sameList(chosenOf(b), ticks.spells)) return true;
  const have = othersOf(b);
  const shorts = new Set([...Object.keys(have), ...Object.keys(ticks.others)]);
  return [...shorts].some((short) => !sameList(have[short] ?? [], ticks.others[short] ?? []));
}

/** A spell's line: its name, mana and whether it is on. */
export function spellLabel(s: Blessings["spells"][number]): string {
  return `${s.name} (${s.short}, ${s.mana} mana)`;
}
