<script lang="ts">
  import { count, sparkline, startedAt, type Stats } from "./lib/stats";
  import type { CharacterState, SessionInfo } from "./proto";

  interface Props {
    /** The focused tile's session, which the panel follows. */
    session: SessionInfo | undefined;
    /** The focused character's snapshot: its current `run`. */
    character: CharacterState | undefined;
    /** The brain's last `stats` for the session. */
    stats: Stats | undefined;
    attached: boolean | null;
    onrefresh: (session: string) => void;
  }

  let { session, character, stats, attached, onrefresh }: Props = $props();

  const run = $derived(character?.run);
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
      {#if run}
        <div class="since">run since {startedAt(run.run_started_at)}</div>
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
