import { describe, expect, it } from "vitest";
import { CLASS_BADGES, badgeCells, badgeText, classBadge } from "../src/lib/classes";

// The 15 classes of the game (WCCCLASS).
const CLASSES = [
  "Warrior", "Witchunter", "Paladin", "Cleric", "Priest", "Missionary", "Ninja", "Thief",
  "Bard", "Gypsy", "Warlock", "Mage", "Druid", "Ranger", "Mystic",
];

describe("class badges (C-106)", () => {
  it("gives every class a badge with its own colour, glyph and tag", () => {
    const badges = CLASSES.map((c) => classBadge(c)!);
    expect(badges.every(Boolean)).toBe(true);
    expect(Object.keys(CLASS_BADGES)).toHaveLength(15);
    for (const key of ["bg", "glyph", "tag"] as const) {
      expect(new Set(badges.map((b) => b[key])).size).toBe(15);
    }
    // Never black: the tile is black.
    expect(badges.some((b) => b.bg.includes("black"))).toBe(false);
    for (const b of badges) expect(b.tag).toMatch(/^[A-Z]{3}$/);
  });

  it("is found in any case, and none for a class not known", () => {
    expect(classBadge("mage")).toBe(classBadge("Mage"));
    expect(badgeText(classBadge("Mage")!)).toBe("☼ MAG");
    expect(classBadge(undefined)).toBeUndefined();
    expect(classBadge("Necromancer")).toBeUndefined();
  });

  it("takes 8 header cells: a space, then the badge padded a cell each side", () => {
    expect(badgeCells("Warrior")).toBe(8);
    expect(badgeCells(undefined)).toBe(0);
  });
});
