import { describe, expect, it } from "vitest";
import { coinText, planRefusal } from "../src/lib/spells";

describe("coinText", () => {
  it("says copper as the coins it is", () => {
    expect(coinText(0)).toBe("free");
    expect(coinText(7)).toBe("7 copper");
    expect(coinText(800)).toBe("8 gold");
    expect(coinText(432)).toBe("4 gold 3 silver 2 copper");
    expect(coinText(1_010_000)).toBe("1 runic 1 platinum");
  });
});

describe("planRefusal", () => {
  it("reads the brain's reason for no plan", () => {
    expect(planRefusal("spell plan: the position is unknown")).toBe("The position is unknown");
    expect(planRefusal("following Penny: it goes where the party goes")).toBeNull();
  });
});
