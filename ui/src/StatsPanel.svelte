<script lang="ts">
  import { age, combatLines, count, perCent, sparkline, startedAt, type Stats } from "./lib/stats";
  import type { CharacterState, MonsterRuleKind, SessionInfo, StatsPart } from "./proto";

  interface Props {
    /** The focused tile's session, which the panel follows. */
    session: SessionInfo | undefined;
    /** The focused character's snapshot: its current `run`. */
    character: CharacterState | undefined;
    /** The brain's last `stats` for the session. */
    stats: Stats | undefined;
    attached: boolean | null;
    onrefresh: (session: string) => void;
    /** `run`: end the current run and start a new one at zero (the old one stays in the
     * history). `combat`: zero the combat numbers. Each section has its own reset. */
    onreset?: (session: string, only: StatsPart) => void;
    /** Set the player's monster rule for a pattern, or remove it (`null`). */
    onrule?: (session: string, pattern: string, rule: MonsterRuleKind | null) => void;
  }

  let { session, character, stats, attached, onrefresh, onreset = () => {}, onrule = () => {} }: Props = $props();

  const rules = $derived(character?.bot?.monster_rules ?? []);
  const RULES: { rule: MonsterRuleKind; label: string }[] = [
    { rule: "flee_from", label: "flee from" },
    { rule: "never_fight", label: "never fight" },
    { rule: "avoid", label: "avoid" },
  ];
  const ruleLabel = (r: MonsterRuleKind) => RULES.find((x) => x.rule === r)?.label ?? r;
  let pattern = $state("");
  let newRule: MonsterRuleKind = $state("flee_from");

  function addRule(e: SubmitEvent) {
    e.preventDefault();
    const p = pattern.trim();
    if (!session || p.length < 2) return;
    onrule(session.id, p, newRule);
    pattern = "";
  }

  const run = $derived(character?.run);
  const combat = $derived(stats?.combat);
  const lines = $derived(combat ? combatLines(combat) : []);
  const spark = $derived(stats ? sparkline(stats.samples) : "");
  const BIG: { key: "exp_per_hour" | "kills" | "deaths" | "flees" | "rests" | "exp_gained"; label: string }[] = [
    { key: "exp_per_hour", label: "exp/hr" },
    { key: "kills", label: "kills" },
    { key: "deaths", label: "deaths" },
    { key: "flees", label: "flees" },
    { key: "rests", label: "rests" },
    { key: "exp_gained", label: "exp" },
  ];
</script>

<div class="stats">
  {#if !session}
    <p class="dim">No character.</p>
  {:else if !attached}
    <p class="dim">No brain attached: no statistics.</p>
  {:else}
    <section class="current" aria-label="Current run">
      <div class="since">
        {#if run}run since {startedAt(run.run_started_at)}{:else}no run{/if}
        {#if run}
          <button
            class="reset"
            data-reset="run"
            title="End this run and start a new one at zero. The old run stays in the list below; the combat numbers are kept."
            onclick={() => onreset(session.id, "run")}>reset</button
          >
        {/if}
      </div>
      {#if run}
        <div class="big">
          {#each BIG as b (b.key)}
            <div class="cell">
              <span class="n" data-stat={b.key}>{count(run[b.key])}</span>
              <span class="label">{b.label}</span>
            </div>
          {/each}
        </div>
      {:else}
        <div class="dim">No run: Start the bot (F10) to begin one.</div>
      {/if}
      {#if spark}
        <div class="spark" title="exp over the newest run, a bar a minute" aria-label="exp over time">{spark}</div>
      {/if}
    </section>
    <section class="combat" aria-label="Combat">
      {#if combat}
        <div class="since" title="Kept until you press reset: across runs, stops and restarts">
          combat since {startedAt(combat.since)} ({age(combat.since)})
          <button
            class="reset"
            data-reset="combat"
            title="Zero the combat numbers. The run goes on."
            onclick={() => onreset(session.id, "combat")}>reset</button
          >
        </div>
        <div class="summary">
          {count(combat.swings)} swings · {perCent(combat.accuracy_per_mille)} accuracy · {perCent(combat.crit_rate_per_mille)} crits
        </div>
        <div class="table" role="table" aria-label="Combat numbers">
          <div class="row head" role="row">
            <span class="k"></span><span class="c-num">n</span><span class="c-num">%</span><span class="c-range">min-max</span><span class="c-num">avg</span>
          </div>
          {#each lines as l (l.label)}
            <div class="row" role="row" data-kind={l.label}>
              <span class="k">{l.label}</span><span class="c-num">{l.count}</span><span class="c-num">{l.share}</span><span class="c-range">{l.range}</span><span class="c-num">{l.avg}</span>{#if l.note}<span class="note">{l.note}</span>{/if}
            </div>
          {/each}
        </div>
        <div class="monsters">
          killed {count(combat.kills)} · fled {count(combat.flees)}{#if combat.lowest_hp !== undefined}
            · lowest hp {combat.lowest_hp}{/if}{#if combat.lowest_mana !== undefined}
            · lowest mana {combat.lowest_mana}{/if}
        </div>
      {:else}
        <div class="dim">No combat numbers since the last reset.</div>
      {/if}
    </section>
    <section class="rules" aria-label="Monster rules">
      <div class="tools"><span>monster rules</span></div>
      {#each rules as r (r.source + ":" + r.pattern)}
        <div class="rule" data-source={r.source}>
          <span class="r-rule">{ruleLabel(r.rule)}</span>
          <span class="r-pattern">{r.pattern}</span>
          <span class="r-source">{r.source}</span>
          {#if r.source === "player"}
            <button aria-label={`remove ${r.pattern}`} onclick={() => onrule(session.id, r.pattern, null)}>x</button>
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
    <div class="tools">
      <span>last runs</span>
      <button onclick={() => onrefresh(session.id)}>refresh</button>
    </div>
    <div class="list" role="table" aria-label="Last runs">
      <div class="row head" role="row">
        <span class="c-start">started</span>
        <span class="c-loop">loop</span>
        <span class="c-num">min</span>
        <span class="c-num">exp/hr</span>
        <span class="c-num">kills</span>
        <span class="c-num">deaths</span>
        <span class="c-num">flees</span>
      </div>
      {#each stats?.runs ?? [] as r (r.started_at)}
        <div class="row" role="row" class:live={r.ended_at === undefined}>
          <span class="c-start">{startedAt(r.started_at)}</span>
          <span class="c-loop" title={r.loop_id ?? "fight here"}>{r.loop_id ?? "here"}</span>
          <span class="c-num">{r.minutes}</span>
          <span class="c-num">{count(r.exp_per_hour)}</span>
          <span class="c-num">{r.kills}</span>
          <span class="c-num">{r.deaths}</span>
          <span class="c-num">{r.flees}</span>
        </div>
      {:else}
        <div class="dim">No runs yet.</div>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* The stats tab of the drawer (App.svelte). */
  .stats {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .dim {
    margin: 0;
    padding: 0 1ch;
    color: var(--dim);
  }
  .current {
    padding: 2px 1ch 4px;
    border-bottom: 1px solid var(--frame);
  }
  .since {
    color: var(--dim);
  }
  .since .reset {
    margin-left: 0.5em;
  }
  .big {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 4px 1ch;
    margin: 4px 0;
  }
  .cell {
    display: flex;
    flex-direction: column;
  }
  .n {
    font-size: 32px;
    line-height: 32px;
    color: var(--vga-yellow);
  }
  .label {
    color: var(--dim);
  }
  .spark {
    color: var(--vga-lightgreen);
    white-space: pre;
    overflow: hidden;
  }
  .tools {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1ch;
    padding: 2px 1ch;
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
  .rules {
    border-bottom: 1px solid var(--frame);
    padding-bottom: 2px;
  }
  .combat {
    padding: 2px 1ch 4px;
    border-bottom: 1px solid var(--frame);
  }
  .combat .dim {
    padding: 0;
  }
  .summary {
    color: var(--vga-yellow);
  }
  .table .k {
    width: 6ch;
  }
  .c-range {
    width: 9ch;
    text-align: right;
  }
  .note {
    color: var(--dim);
  }
  .monsters {
    color: var(--dim);
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
  .r-rule {
    width: 11ch;
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
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 1ch;
  }
  .row {
    display: flex;
    gap: 1ch;
    white-space: pre;
  }
  .row.head {
    color: var(--frame);
  }
  .row.live {
    color: var(--vga-lightcyan);
  }
  .c-start {
    width: 11ch;
  }
  .c-loop {
    flex: 1;
    min-width: 4ch;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .c-num {
    width: 6ch;
    text-align: right;
  }
</style>
