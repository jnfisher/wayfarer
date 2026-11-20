<script lang="ts">
  // The combat zoom (C-125, docs/web-ui.md "Combat zoom"): full screen over the tiles, a
  // character's combat numbers over a window with their histograms and each monster fought, and
  // its party side by side. The brain's answers come in through App.svelte.
  import {
    WINDOWS,
    bin,
    detailKey,
    foeLines,
    histChoices,
    histKey,
    median,
    partyLine,
    type CombatDetailMessage,
  } from "./lib/combat-zoom";
  import { attackLines, count, MIN_SAMPLE, STATS_REFRESH_MS, startedAt, takenLine } from "./lib/stats";
  import type { CombatWindow } from "./proto";

  interface Props {
    /** The party of the character it opens on, the leader first: id and name. */
    party: { id: string; name: string }[];
    /** The character it opens on. */
    initial: string;
    /** The brain's last `combat_detail` per session and window ({@link detailKey}). */
    details: Record<string, CombatDetailMessage>;
    onrequest: (session: string, window: CombatWindow) => void;
    onclose: () => void;
  }

  let { party, initial, details, onrequest, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let sessionId = $state(initial);
  let window = $state<CombatWindow>("reset");
  let chosenHist = $state<string | undefined>(undefined);

  // Every party member's numbers for the window, asked again when it changes and every minute.
  $effect(() => {
    const ask = () => {
      for (const p of party) onrequest(p.id, window);
    };
    ask();
    const timer = setInterval(ask, STATS_REFRESH_MS);
    return () => clearInterval(timer);
  });

  const detail = $derived(details[detailKey(sessionId, window)]);
  const stats = $derived(detail?.stats);
  const name = $derived(party.find((p) => p.id === sessionId)?.name ?? sessionId);
  const choices = $derived(histChoices(detail?.hists ?? []));
  const histName = $derived(
    chosenHist && choices.some((c) => c.key === chosenHist) ? chosenHist : choices[0]?.key,
  );
  const hist = $derived(detail?.hists.find((h) => histKey(h) === histName));
  const bars = $derived(hist ? bin(hist.values) : []);
  const top = $derived(Math.max(1, ...bars.map((b) => b.n)));
  const total = $derived(hist ? hist.values.reduce((s, [, n]) => s + n, 0) : 0);
  const mid = $derived(hist ? median(hist.values) : undefined);
  const attacks = $derived(stats ? attackLines(stats.attacks) : []);
  const foes = $derived(stats ? foeLines(stats.monsters) : []);
  const lines = $derived(party.map((p) => partyLine(p.id, p.name, details[detailKey(p.id, window)]?.stats)));
</script>

<div class="zoom" role="dialog" aria-label="Combat numbers">
  <header>
    <span class="tab">combat · {name}</span>
    {#each WINDOWS as w (w.window)}
      <button class="win" class:on={window === w.window} title={w.title} onclick={() => (window = w.window)}
        >{w.label}</button
      >
    {/each}
    {#if stats}<span class="dim">since {startedAt(stats.since)}</span>{/if}
    <button class="close" title="Close (Esc)" onclick={onclose}>close</button>
  </header>
  <div class="body">
    {#if party.length > 1}
      <section aria-label="The party">
        <div class="title">the party (click a name to show it)</div>
        <div class="table" role="table">
          <div class="row head" role="row">
            <span class="p-name"></span><span class="c-num">round</span><span class="c-num">swing</span><span
              class="c-num">bs hit</span
            ><span class="c-num">bs avg</span><span class="c-num">hit you</span><span class="c-num">kills</span>
          </div>
          {#each lines as l (l.session)}
            <div class="row" role="row" class:few={l.few} class:on={l.session === sessionId} data-party={l.name}>
              <button class="p-name link" onclick={() => (sessionId = l.session)}>{l.name}</button><span class="c-num"
                >{l.round}</span
              ><span class="c-num">{l.swingHit}</span><span class="c-num">{l.bsHit}</span><span class="c-num"
                >{l.bsAvg}</span
              ><span class="c-num">{l.takenHit}</span><span class="c-num">{l.kills}</span>
            </div>
          {/each}
        </div>
      </section>
    {/if}
    {#if !stats}
      <p class="dim">{detail ? "Nothing counted in this window." : "Asking the brain..."}</p>
    {:else}
      <section aria-label="Histogram">
        <div class="title">
          <select aria-label="Histogram" value={histName} onchange={(e) => (chosenHist = e.currentTarget.value)}>
            {#each choices as c (c.key)}
              <option value={c.key}>{c.label}</option>
            {/each}
          </select>
          {#if hist}<span class="dim">{count(total)} in all{mid !== undefined ? `, median ${mid}` : ""}</span>{/if}
        </div>
        {#if bars.length > 0}
          <div class="hist" data-hist={histName}>
            {#each bars as b (b.lo)}
              <div class="bar" title="{b.lo}-{b.hi}: {count(b.n)}">
                <span class="n">{b.n > 0 ? count(b.n) : ""}</span>
                <span class="well"><span class="fill" style:height="{(b.n / top) * 100}%"></span></span>
                <span class="lo">{b.lo}</span>
              </div>
            {/each}
          </div>
        {/if}
      </section>
      <section aria-label="By attack">
        <div class="title">by attack</div>
        <div class="table" role="table">
          <div class="row head" role="row">
            <span class="a-k"></span><span class="c-num">tries</span><span class="c-num">hit</span><span class="c-num"
              >crit</span
            ><span class="c-num">miss</span><span class="c-range">min-max</span><span class="c-num">avg</span>
          </div>
          {#each attacks as a (a.kind + a.label)}
            <div
              class="row"
              class:few={a.few}
              role="row"
              data-attack={a.label}
              title={a.few ? `Fewer than ${MIN_SAMPLE} tries: too few to go by` : undefined}
            >
              <span class="a-k">{a.label}</span><span class="c-num">{a.tries}</span><span class="c-num">{a.hit}</span
              ><span class="c-num">{a.crit}</span><span class="c-num">{a.miss}</span><span class="c-range">{a.range}</span
              ><span class="c-num">{a.avg}</span>{#if a.note}<span class="note">{a.note}</span>{/if}
            </div>
          {/each}
        </div>
        {#if stats.taken}<div class="taken">{takenLine(stats.taken)}</div>{/if}
      </section>
      {#if foes.length > 0}
        <section aria-label="Monsters">
          <div class="title">each monster</div>
          <div class="table" role="table">
            <div class="row head" role="row">
              <span class="f-name"></span><span class="c-num">kills</span><span class="c-num" title="rounds a kill"
                >rnd/kill</span
              ><span class="c-num" title="your damage a round">dealt</span><span
                class="c-num"
                title="its damage to you a round">taken</span
              ><span class="c-num" title="its blows that hit you">hit you</span><span class="c-num">exp/kill</span>
            </div>
            {#each foes as f (f.name)}
              <div class="row" role="row" class:few={f.few} data-foe={f.name}>
                <span class="f-name">{f.name}</span><span class="c-num">{f.kills}</span><span class="c-num"
                  >{f.roundsPerKill}</span
                ><span class="c-num">{f.dealtPerRound}</span><span class="c-num">{f.takenPerRound}</span><span
                  class="c-num">{f.hitsYou}</span
                ><span class="c-num">{f.expPerKill}</span>
              </div>
            {/each}
          </div>
        </section>
      {/if}
    {/if}
  </div>
</div>

<style>
  .zoom {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    flex-direction: column;
    margin: 2px;
    color: var(--vga-lightgray);
    background: var(--bg);
    border: 4px double var(--frame);
    line-height: 16px;
    font-size: 16px;
    min-height: 0;
  }
  header {
    display: flex;
    gap: 1ch;
    align-items: baseline;
    padding: 0 1ch 4px;
    border-bottom: 1px solid var(--frame);
  }
  .tab {
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightcyan);
  }
  .close {
    margin-left: auto;
  }
  button,
  select {
    font: inherit;
    line-height: 16px;
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightgray);
    border: none;
    cursor: pointer;
  }
  button.win.on {
    background: var(--vga-yellow);
  }
  button.link {
    padding: 0;
    color: var(--vga-lightcyan);
    background: none;
    text-align: left;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 4px 1ch;
  }
  section {
    margin-bottom: 8px;
  }
  .title {
    display: flex;
    gap: 1ch;
    align-items: baseline;
    color: var(--vga-white);
    margin-bottom: 2px;
  }
  .dim,
  .note {
    color: var(--dim);
  }
  .row {
    display: flex;
    gap: 1ch;
    white-space: pre;
  }
  .row.head {
    color: var(--frame);
  }
  .row.few {
    color: var(--dim);
  }
  .row.on .p-name {
    color: var(--vga-yellow);
  }
  .p-name,
  .a-k {
    width: 14ch;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .f-name {
    width: 22ch;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .c-num {
    width: 8ch;
    text-align: right;
  }
  .c-range {
    width: 9ch;
    text-align: right;
  }
  .taken {
    color: var(--vga-lightred);
    margin-top: 2px;
  }
  .hist {
    display: flex;
    align-items: stretch;
    gap: 2px;
    height: 160px;
    max-width: 100ch;
  }
  .bar {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    min-width: 0;
    font-size: 12px;
    line-height: 12px;
    text-align: center;
  }
  .bar .well {
    flex: 1;
    display: flex;
    align-items: flex-end;
  }
  .bar .fill {
    display: block;
    width: 100%;
    background: var(--vga-lightgreen);
  }
  .bar .n {
    color: var(--dim);
    overflow: hidden;
  }
  .bar .lo {
    color: var(--dim);
    overflow: hidden;
  }
</style>
