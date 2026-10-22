// The settings tab: the brain's `settings` answer, and how its rows are grouped and checked
// before they are sent back (the brain checks them again, with the HP lines in order).

import type { SettingChange, SettingRow, SettingValue, ToClient } from "../proto";

export type SettingsMessage = Extract<ToClient, { type: "settings" }>;

/** The layers the player edits, most specific first. */
export const LAYERS: { id: "character" | "server" | "mine"; label: string; title: string }[] = [
  { id: "character", label: "this character", title: "Only for this character" },
  { id: "server", label: "this server", title: "For every character on this game server" },
  { id: "mine", label: "my defaults", title: "For all characters" },
];

/** The groups in the order the forms show them, with their names. */
export const GROUPS: { id: string; label: string }[] = [
  { id: "health", label: "Health" },
  { id: "combat", label: "Combat" },
  { id: "spells", label: "Spells" },
  { id: "cash", label: "Coins" },
  { id: "movement", label: "Movement" },
  { id: "bot", label: "The bot" },
];

export interface SettingsGroup {
  id: string;
  label: string;
  rows: SettingRow[];
}

/** The rows by group, in the forms' order; advanced ones only when `all`. Empty groups are left out. */
export function groupRows(rows: SettingRow[], all: boolean): SettingsGroup[] {
  const shown = rows.filter((r) => all || r.tier !== "advanced");
  const known = GROUPS.map((g) => ({ ...g, rows: shown.filter((r) => r.group === g.id) }));
  // A group the UI does not know yet still shows, last, under its table name.
  const ids = new Set(GROUPS.map((g) => g.id));
  const other = [...new Set(shown.map((r) => r.group))]
    .filter((g) => !ids.has(g))
    .map((g) => ({ id: g, label: g, rows: shown.filter((r) => r.group === g) }));
  return [...known, ...other].filter((g) => g.rows.length > 0);
}

/** A value as text (`35`, `on`, `medium`). */
export function show(v: SettingValue): string {
  return typeof v === "boolean" ? (v ? "on" : "off") : String(v);
}

/** The words for where a value comes from, for the badge. */
export function sourceLabel(source: string): string {
  switch (source) {
    case "default":
      return "default";
    case "preset":
      return "class & style";
    case "mine":
      return "my defaults";
    case "server":
      return "this server";
    case "character":
      return "this character";
    default:
      return source;
  }
}

/**
 * The change that sets `row` to what a number box holds: `undefined` for text that is not a
 * whole number in the setting's range (the box is left for the player to mend).
 */
export function numberChange(row: SettingRow, text: string): SettingChange | undefined {
  if (row.kind.kind !== "int" || !/^-?\d+$/.test(text.trim())) return undefined;
  const n = Number(text.trim());
  if (n < row.kind.min || n > row.kind.max) return undefined;
  return { key: row.key, value: n };
}

/** The change that takes `row` out of the layer's file, so the layer below shows through. */
export function resetChange(row: SettingRow): SettingChange {
  return { key: row.key };
}

/** Whether the layer's own file sets `row`. */
export function isOwn(row: SettingRow): boolean {
  return row.own !== undefined;
}
