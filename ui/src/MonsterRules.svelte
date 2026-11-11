<script lang="ts">
  // The monster rules (C-035, C-073, C-096): the defaults, the player's, and the ones a session
  // learned; each can be removed (a default switched off) and a rule added. In the settings
  // tab, folded away at the end.
  import type { MonsterRuleEntry, MonsterRuleKind } from "./proto";

  interface Props {
    /** The character's rules, from its snapshot's `bot.monster_rules`. */
    rules: MonsterRuleEntry[];
    /** Set the player's rule for a pattern, or remove it (`null`). */
    onrule: (pattern: string, rule: MonsterRuleKind | null) => void;
  }

  let { rules, onrule }: Props = $props();

  /** The rules a player adds; `off` comes only from removing a default. */
  const RULES: { rule: MonsterRuleKind; label: string }[] = [
    { rule: "ignore", label: "ignore" },
    { rule: "avoid", label: "avoid" },
    { rule: "first", label: "attack first" },
  ];
  const ruleLabel = (r: MonsterRuleKind) => RULES.find((x) => x.rule === r)?.label ?? r;
  /** Patterns the player switched off (a player `off`): the rule under it is shown struck. */
  const switchedOff = $derived(
    new Set(rules.filter((r) => r.source === "player" && r.rule === "off").map((r) => r.pattern.toLowerCase())),
  );
  /** What the `x` on a row does, for its tooltip. */
  function removeTitle(r: MonsterRuleEntry): string {
    if (r.rule === "off") return "Put the rule back";
    if (r.source === "default") return "Switch this default off for this character";
    if (r.source === "session") return "Forget this learned rule";
    return "Remove this rule";
  }
  let pattern = $state("");
  let newRule: MonsterRuleKind = $state("ignore");

  function addRule(e: SubmitEvent) {
    e.preventDefault();
    const p = pattern.trim();
    if (p.length < 2) return;
    onrule(p, newRule);
    pattern = "";
  }
</script>

<section class="rules" aria-label="Monster rules">
  {#each rules as r (r.source + ":" + r.pattern)}
    {@const struck = r.rule !== "off" && r.source !== "player" && switchedOff.has(r.pattern.toLowerCase())}
    <div class="rule" data-source={r.source} data-off={r.rule === "off" || struck} title={r.reason ?? ""}>
      <span class="r-rule">{ruleLabel(r.rule)}</span>
      <span class="r-pattern">{r.pattern}</span>
      <span class="r-source">{r.source}</span>
      {#if r.rule === "off"}
        <button aria-label={`put back ${r.pattern}`} title={removeTitle(r)} onclick={() => onrule(r.pattern, null)}
          >put back</button
        >
      {:else if !struck}
        <button aria-label={`remove ${r.pattern}`} title={removeTitle(r)} onclick={() => onrule(r.pattern, null)}
          >x</button
        >
      {/if}
    </div>
  {/each}
  <form class="add" onsubmit={addRule}>
    <select aria-label="rule" bind:value={newRule}>
      {#each RULES as o (o.rule)}
        <option value={o.rule}>{o.label}</option>
      {/each}
    </select>
    <input
      aria-label="monster name"
      placeholder="full monster name"
      title="Matches any monster whose name holds this, in any case: 'ooze' would also match the glittering ooze. Use the full name."
      bind:value={pattern}
    />
    <button type="submit">add</button>
  </form>
</section>

<style>
  .rules {
    border-bottom: 1px solid var(--frame);
    padding-bottom: 2px;
  }
  .rule {
    display: flex;
    gap: 1ch;
    padding: 0 1ch;
    white-space: pre;
  }
  .rule[data-source="default"] {
    color: var(--dim);
  }
  .rule[data-off="true"] .r-rule,
  .rule[data-off="true"] .r-pattern {
    text-decoration: line-through;
  }
  .r-rule {
    width: 13ch;
    color: var(--vga-yellow);
  }
  .r-pattern {
    flex: 1;
    min-width: 4ch;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .r-source {
    color: var(--dim);
  }
  .add {
    display: flex;
    gap: 1ch;
    padding: 2px 1ch;
  }
  .add input {
    flex: 1;
    min-width: 8ch;
    font: inherit;
    color: var(--vga-lightgray);
    background: #000;
    border: 1px solid var(--frame);
  }
  .add select {
    font: inherit;
    color: var(--vga-lightgray);
    background: #000;
    border: 1px solid var(--frame);
  }
  button {
    font: inherit;
    line-height: 16px;
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightgray);
    border: none;
    cursor: pointer;
  }
</style>
