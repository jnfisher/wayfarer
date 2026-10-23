// Blessings in the page (C-047): the Blessings panel's boxes, one per character, its lasting
// spells ticked or not. The brain keeps the ticks; this only reads its `blessings` message and
// builds `blessings_save`.

import type { Blessings } from "../proto";

/** The ticked spells (short names) of a character, as the brain has them. */
export function chosenOf(b: Blessings): string[] {
  return b.spells.filter((s) => s.chosen).map((s) => s.short);
}

/** Tick or untick `short` in `chosen`, keeping the order the character's spells are listed in. */
export function toggleSpell(b: Blessings, chosen: string[], short: string): string[] {
  const next = chosen.includes(short) ? chosen.filter((s) => s !== short) : [...chosen, short];
  const order = b.spells.map((s) => s.short);
  return next.sort((x, y) => order.indexOf(x) - order.indexOf(y));
}

/** Whether `chosen` differs from what the brain has. */
export function changed(b: Blessings, chosen: string[]): boolean {
  const have = chosenOf(b);
  return have.length !== chosen.length || have.some((s) => !chosen.includes(s));
}

/** A spell's line: its name, mana and whether it is on. */
export function spellLabel(s: Blessings["spells"][number]): string {
  return `${s.name} (${s.short}, ${s.mana} mana)`;
}
