// The spell plan (docs/web-ui.md "Errands"): the brain's `spell_plan` answer, and how its coins
// and totals are shown.

import type { SpellPlanItem, ToClient } from "../proto";

export type SpellPlan = Extract<ToClient, { type: "spell_plan" }>;

/** Copper in each coin, largest first. */
const COINS: [string, number][] = [
  ["runic", 1_000_000],
  ["platinum", 10_000],
  ["gold", 100],
  ["silver", 10],
  ["copper", 1],
];

/** Copper as coins: `8 gold`, `1 gold 4 silver`, `free` for none. */
export function coinText(copper: number): string {
  if (copper <= 0) return "free";
  const parts: string[] = [];
  let left = copper;
  for (const [name, unit] of COINS) {
    const n = Math.floor(left / unit);
    if (n > 0) parts.push(`${n} ${name}`);
    left -= n * unit;
  }
  return parts.join(" ");
}

/** One row of the plan: the spell, its price, the shop and the way there. */
export function planRow(i: SpellPlanItem): string {
  const where = i.steps === undefined ? "no way there" : `${i.steps} ${i.steps === 1 ? "step" : "steps"}`;
  const when = i.usable ? "" : ` (needs level ${i.spell_level})`;
  return `${i.spell}${when}: ${coinText(i.price)}, ${i.shop}, ${where}`;
}

/** The totals under the plan: what the cash buys now, and what everything usable costs. */
export function planTotals(plan: SpellPlan): string {
  const usable = plan.items.filter((i) => i.usable);
  const buy = usable.filter((i) => i.affordable);
  const sum = (v: SpellPlanItem[]) => v.reduce((a, i) => a + i.price, 0);
  if (plan.items.length === 0) return "Nothing to buy: every spell the shops sell for this class is known.";
  return (
    `With ${coinText(plan.cash)} on hand: ${buy.length} of ${usable.length} usable now (${coinText(sum(buy))}); ` +
    `all ${usable.length} cost ${coinText(sum(usable))}`
  );
}

/** The brain's reason for not giving a plan, from its `warning` (`spell plan: …`); null for any other warning. */
export function planRefusal(text: string): string | null {
  const why = text.match(/^spell plan: (.+)$/s)?.[1];
  return why ? why[0].toUpperCase() + why.slice(1) : null;
}
