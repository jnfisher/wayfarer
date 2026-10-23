import { describe, expect, it } from "vitest";
import { changed, chosenOf, spellLabel, toggleSpell } from "../src/lib/blessings";
import type { Blessings } from "../src/proto";

const lauren: Blessings = {
  session: "lauren",
  spells: [
    { short: "bles", name: "bless", mana: 4, chosen: true, on: true },
    { short: "aura", name: "holy aura", mana: 15, chosen: false, on: false },
    { short: "skin", name: "barkskin", mana: 15, chosen: false, on: false },
  ],
};

describe("blessings", () => {
  it("reads the ticks and toggles them in the character's order", () => {
    expect(chosenOf(lauren)).toEqual(["bles"]);
    expect(toggleSpell(lauren, ["skin"], "bles")).toEqual(["bles", "skin"]);
    expect(toggleSpell(lauren, ["bles", "skin"], "bles")).toEqual(["skin"]);
  });
  it("knows when the ticks differ from the brain's", () => {
    expect(changed(lauren, ["bles"])).toBe(false);
    expect(changed(lauren, [])).toBe(true);
    expect(changed(lauren, ["aura"])).toBe(true);
  });
  it("labels a spell with its word and mana", () => {
    expect(spellLabel(lauren.spells[1])).toBe("holy aura (aura, 15 mana)");
  });
});
