// The class badge at the head of a tile (C-106, docs/web-ui.md "The header"): a coloured block
// with a CP437 glyph and a three-letter tag, so whose tile is whose shows at a glance whatever
// the character is called. Each of the 15 classes has its own one of the 15 VGA colours
// (all but black), grouped loosely by theme; text is black on the bright ones, white on the
// dark ones.

export interface ClassBadge {
  glyph: string;
  tag: string;
  /** A VGA colour variable (app.css). */
  bg: string;
  fg: string;
}

const BLACK = "var(--vga-black)";
const WHITE = "var(--vga-white)";
const badge = (glyph: string, tag: string, colour: string, fg: string): ClassBadge => ({
  glyph,
  tag,
  bg: `var(--vga-${colour})`,
  fg,
});

/** By class name, lowercase. */
export const CLASS_BADGES: Record<string, ClassBadge> = {
  warrior: badge("♠", "WAR", "lightred", BLACK),
  witchunter: badge("¥", "WIT", "yellow", BLACK),
  paladin: badge("±", "PAL", "lightcyan", BLACK),
  cleric: badge("+", "CLR", "lightblue", BLACK),
  priest: badge("Ω", "PRI", "white", BLACK),
  missionary: badge("♥", "MIS", "lightgray", BLACK),
  ninja: badge("•", "NIN", "darkgray", WHITE),
  thief: badge("$", "THF", "blue", WHITE),
  bard: badge("♫", "BRD", "brown", WHITE),
  gypsy: badge("○", "GYP", "magenta", WHITE),
  warlock: badge("Φ", "WLK", "red", WHITE),
  mage: badge("☼", "MAG", "lightmagenta", BLACK),
  druid: badge("♣", "DRU", "lightgreen", BLACK),
  ranger: badge("↑", "RNG", "green", WHITE),
  mystic: badge("∞", "MYS", "cyan", WHITE),
};

/** The badge for a class, by name in any case; none for a class not known yet or not one of
 * the 15. */
export function classBadge(name: string | undefined | null): ClassBadge | undefined {
  return name ? CLASS_BADGES[name.trim().toLowerCase()] : undefined;
}

/** The badge's text, `♠ WAR`, drawn with a cell of padding on each side. */
export function badgeText(b: ClassBadge): string {
  return `${b.glyph} ${b.tag}`;
}

/** Cells the badge takes in the header, the space before it included: 0 without one. */
export function badgeCells(name: string | undefined | null): number {
  const b = classBadge(name);
  return b ? 1 + badgeText(b).length + 2 : 0;
}
