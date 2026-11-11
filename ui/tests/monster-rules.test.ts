// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import MonsterRules from "../src/MonsterRules.svelte";
import type { MonsterRuleEntry } from "../src/proto";

describe("the monster rules", () => {
  afterEach(cleanup);

  const OOZE_WHY = "unfightable: its armour takes every blow and it regenerates 500 HP a round";
  const withRules = (extra: MonsterRuleEntry[] = []): MonsterRuleEntry[] => [
    { pattern: "gigantic black ooze", rule: "ignore", source: "default", reason: OOZE_WHY },
    { pattern: "stone golem", rule: "ignore", source: "session" },
    { pattern: "giant rat", rule: "avoid", source: "player" },
    ...extra,
  ];

  it("lists every rule; any can be removed, and one added", async () => {
    const onrule = vi.fn();
    const { container } = render(MonsterRules, { rules: withRules(), onrule });
    const rows = [...container.querySelectorAll(".rule")];
    const listed = rows.map((r) => [...r.querySelectorAll("span")].map((s) => s.textContent!.trim()).join(" | "));
    expect(listed).toEqual([
      "ignore | gigantic black ooze | default",
      "ignore | stone golem | session",
      "avoid | giant rat | player",
    ]);
    // The default's reason is its tooltip.
    expect(rows[0].getAttribute("title")).toBe(OOZE_WHY);
    // Every row has a remove button: the default's switches it off, the session's forgets it.
    await fireEvent.click(screen.getByRole("button", { name: "remove gigantic black ooze" }));
    expect(onrule).toHaveBeenLastCalledWith("gigantic black ooze", null);
    expect(screen.getByRole("button", { name: "remove gigantic black ooze" }).getAttribute("title")).toBe(
      "Switch this default off for this character",
    );
    await fireEvent.click(screen.getByRole("button", { name: "remove stone golem" }));
    expect(onrule).toHaveBeenLastCalledWith("stone golem", null);
    await fireEvent.click(screen.getByRole("button", { name: "remove giant rat" }));
    expect(onrule).toHaveBeenLastCalledWith("giant rat", null);
    // Adding: ignore by default, or avoid.
    await fireEvent.input(screen.getByRole("textbox", { name: "monster name" }), {
      target: { value: " fierce kobold thief " },
    });
    await fireEvent.click(screen.getByRole("button", { name: "add" }));
    expect(onrule).toHaveBeenLastCalledWith("fierce kobold thief", "ignore");
    expect((screen.getByRole("textbox", { name: "monster name" }) as HTMLInputElement).value).toBe("");
    const options = [...screen.getByRole("combobox", { name: "rule" }).querySelectorAll("option")].map((o) => o.value);
    expect(options).toEqual(["ignore", "avoid", "first"]);
    expect(screen.getByRole("combobox", { name: "rule" }).textContent).toContain("attack first");
  });

  it("shows a default switched off struck through, with a button to put it back", async () => {
    const onrule = vi.fn();
    const { container } = render(MonsterRules, {
      rules: withRules([{ pattern: "gigantic black ooze", rule: "off", source: "player" }]),
      onrule,
    });
    const off = [...container.querySelectorAll(".rule")].map((r) => r.getAttribute("data-off"));
    expect(off).toEqual(["true", "false", "false", "true"]);
    // The struck default has no `x`; the `off` row puts it back.
    expect(screen.queryByRole("button", { name: "remove gigantic black ooze" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "put back gigantic black ooze" }));
    expect(onrule).toHaveBeenLastCalledWith("gigantic black ooze", null);
  });
});
