import { describe, expect, it } from "vitest";
import { changed, chosenOf, othersOf, spellLabel, toggleSpell, toggleTarget } from "../src/lib/blessings";
import type { Blessings } from "../src/proto";

const lauren: Blessings = {
  session: "lauren",
  spells: [
    {
      short: "bles",
      name: "bless",
      mana: 4,
      chosen: true,
      on: true,
      others: [
        { name: "Penny", chosen: true, on: false },
        { name: "Riley", chosen: false, on: true },
      ],
    },
    { short: "aura", name: "holy aura", mana: 15, chosen: false, on: false, others: [] },
    { short: "skin", name: "barkskin", mana: 15, chosen: false, on: false, others: [] },
  ],
};

describe("blessings", () => {
  it("reads the ticks and toggles them in the character's order", () => {
    expect(chosenOf(lauren)).toEqual(["bles"]);
    expect(toggleSpell(lauren, ["skin"], "bles")).toEqual(["bles", "skin"]);
    expect(toggleSpell(lauren, ["bles", "skin"], "bles")).toEqual(["skin"]);
  });
  it("reads the other characters a spell is kept on and toggles them in their order (C-131)", () => {
    expect(othersOf(lauren)).toEqual({ bles: ["Penny"] });
    expect(toggleTarget(lauren, { bles: ["Riley"] }, "bles", "Penny")).toEqual({ bles: ["Penny", "Riley"] });
    expect(toggleTarget(lauren, { bles: ["Penny"] }, "bles", "Penny")).toEqual({});
  });
  it("knows when the ticks differ from the brain's", () => {
    expect(changed(lauren, { spells: ["bles"], others: { bles: ["Penny"] } })).toBe(false);
    expect(changed(lauren, { spells: [], others: { bles: ["Penny"] } })).toBe(true);
    expect(changed(lauren, { spells: ["aura"], others: { bles: ["Penny"] } })).toBe(true);
    expect(changed(lauren, { spells: ["bles"], others: {} })).toBe(true);
    expect(changed(lauren, { spells: ["bles"], others: { bles: ["Penny", "Riley"] } })).toBe(true);
  });
  it("labels a spell with its word and mana", () => {
    expect(spellLabel(lauren.spells[1])).toBe("holy aura (aura, 15 mana)");
  });
});
