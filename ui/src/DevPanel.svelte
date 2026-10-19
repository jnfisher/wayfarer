<script lang="ts">
  import {
    allNotes,
    clock,
    compactFields,
    eventMatches,
    formatRate,
    unknownLines,
    unknownMarkdown,
    type DevEvent,
    type DevLog,
    type DevTab,
  } from "./lib/state";
  import type { CharacterState, SessionInfo } from "./proto";

  interface Props {
    /** The focused tile's session, which the panel follows. */
    session: SessionInfo | undefined;
    /** The buffers; not reactive, so `tick` says when they changed. */
    log: DevLog;
    tick: number;
    /** The focused character's snapshot, for its `parse` counts. */
    character: CharacterState | undefined;
    attached: boolean | null;
    tab: DevTab;
    onresync: (session: string) => void;
  }

  let { session, log, tick, character, attached, tab = $bindable(), onresync }: Props = $props();

  const TABS: { id: DevTab; label: string }[] = [
    { id: "events", label: "Events" },
    { id: "unclassified", label: "Unclassified" },
    { id: "brain", label: "Brain" },
  ];

  // Events: filter, pause, the clicked one, and following the bottom until scrolled up.
  let filter = $state("");
  let paused = $state<DevEvent[] | null>(null);
  let picked = $state<DevEvent | null>(null);
  let stick = $state(true);
  let list = $state<HTMLDivElement>();

  const events = $derived.by(() => {
    void tick;
    return session ? (log.events[session.id] ?? []).slice() : [];
  });
  const shown = $derived((paused ?? events).filter((e) => eventMatches(e, filter)));

  const unknown = $derived.by(() => {
    void tick;
    return session ? unknownLines(log, session.id) : [];
  });
  const rate = $derived(formatRate(character?.parse));

  const notes = $derived.by(() => {
    void tick;
    return allNotes(log);
  });
  const info = $derived.by(() => {
    void tick;
    return log.info;
  });

  // A new character, a fresh view (not a new `session` object for the same one).
  const sessionId = $derived(session?.id);
  $effect(() => {
    void sessionId;
    paused = null;
    picked = null;
    stick = true;
  });

  // Keep the newest row in view until the user scrolls up.
  $effect(() => {
    void shown;
    void tab;
    if (stick && list) list.scrollTop = list.scrollHeight;
  });

  function onscroll() {
    if (list) stick = list.scrollTop + list.clientHeight >= list.scrollHeight - 4;
  }

  function togglePause() {
    paused = paused ? null : events.slice();
  }

  let copied = $state("");
  async function copyNotes() {
    try {
      await navigator.clipboard.writeText(unknownMarkdown(unknown));
      copied = `copied ${unknown.length}`;
    } catch {
      copied = "the clipboard refused";
    }
    setTimeout(() => (copied = ""), 2000);
  }

  function started(at: string): string {
    const t = Date.parse(at);
    return Number.isNaN(t) ? at : `${new Date(t).toLocaleDateString()} ${clock(t)}`;
  }
</script>

<div class="dev">
  <div class="tabs" role="tablist">
    {#each TABS as t (t.id)}
      <button role="tab" aria-selected={tab === t.id} class:on={tab === t.id} onclick={() => (tab = t.id)}>
        {t.label}{t.id === "unclassified" && unknown.length ? ` ${unknown.length}` : ""}
      </button>
    {/each}
  </div>

  {#if tab === "events"}
    <div class="tools">
      <input type="search" placeholder="filter: name or line" aria-label="Filter events" bind:value={filter} />
      <button onclick={togglePause} class:on={paused}>{paused ? "resume" : "pause"}</button>
      <span class="dim">{shown.length}/{events.length}</span>
    </div>
    <div class="list" bind:this={list} {onscroll}>
      {#each shown as e (e.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="row" class:picked={picked?.id === e.id} class:unknown={e.name === "unknown"} onclick={() => (picked = e)}>
          <span class="seq">{e.seq}</span>
          <span class="name">{e.name}</span>
          <span class="cat">{e.cat}</span>
          <span class="fields">{compactFields(e.fields)}</span>
        </div>
      {:else}
        <p class="dim">{session ? "No events yet." : "No character."}</p>
      {/each}
    </div>
    {#if picked}
      <div class="detail">
        <span class="dim">seq {picked.seq} line {picked.line_no} {picked.name}</span>
        <div class="line">{picked.line}</div>
      </div>
    {/if}
  {:else if tab === "unclassified"}
    <div class="tools">
      <span>rate <span class="rate" class:alarm={rate.alarm}>{rate.text}</span></span>
      {#if character?.parse}
        <span class="dim">{character.parse.unclassified} of {character.parse.lines} lines</span>
      {/if}
      <button onclick={copyNotes} disabled={unknown.length === 0}>copy as KB notes</button>
      <span class="dim">{copied}</span>
    </div>
    <div class="list" bind:this={list} {onscroll}>
      {#each unknown as u (u.text)}
        <div class="row"><span class="count">{u.count}</span><span class="text">{u.text}</span></div>
      {:else}
        <p class="dim">No unclassified lines since the page opened.</p>
      {/each}
    </div>
  {:else}
    <div class="info">
      <div>
        brain <span class={attached ? "yes" : "no"}>{attached ? "attached" : attached === false ? "not attached" : "--"}</span>
      </div>
      {#if info}
        <div>build <span class="val">{info.build}</span></div>
        <div>started <span class="val">{started(info.started_at)}</span></div>
        <div title={info.world_pack ? `${info.world_pack.path}\n${info.world_pack.hash}` : undefined}>
          world pack
          {#if info.world_pack}
            <span class="val">{info.world_pack.path}</span> <span class="dim">{info.world_pack.hash}</span>
          {:else}
            <span class="no">none</span>
          {/if}
        </div>
      {/if}
      <div class="tools">
        <button disabled={!attached || !session} onclick={() => session && onresync(session.id)}>
          Re-sync {session?.name ?? ""}
        </button>
      </div>
    </div>
    <div class="list" bind:this={list} {onscroll}>
      {#each notes as n (n.id)}
        <div class="row {n.kind}">
          <span class="dim">{clock(n.at)}</span>
          <span class="who">{n.session ?? ""}</span>
          <span class="text">{n.text}</span>
        </div>
      {:else}
        <p class="dim">Nothing from the brain yet.</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* The dev tab of the drawer (App.svelte). */
  .dev {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
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
  button:disabled {
    color: var(--vga-darkgray);
    cursor: default;
  }
  button.on {
    background: var(--vga-lightcyan);
  }
  .tabs {
    display: flex;
    gap: 1ch;
    padding: 0 1ch 2px;
    border-bottom: 1px solid var(--frame);
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1ch;
    padding: 2px 1ch;
  }
  input {
    flex: 1;
    min-width: 10ch;
    font: inherit;
    line-height: 16px;
    color: var(--vga-white);
    background: var(--vga-black);
    border: 1px solid var(--frame);
    padding: 0 0.5ch;
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
    overflow: hidden;
    cursor: default;
  }
  .row > span {
    flex: none;
  }
  .row > .fields,
  .row > .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row.picked {
    background: var(--vga-blue);
  }
  .seq,
  .count {
    min-width: 6ch;
    text-align: right;
    color: var(--vga-darkgray);
  }
  .count {
    color: var(--vga-white);
  }
  .name {
    color: var(--vga-lightcyan);
  }
  .cat {
    color: var(--vga-cyan);
  }
  .row.unknown .name {
    color: var(--vga-lightred);
  }
  .fields {
    color: var(--vga-white);
  }
  .detail {
    padding: 2px 1ch;
    border-top: 1px solid var(--frame);
  }
  .line {
    color: var(--vga-white);
    white-space: pre-wrap;
    word-break: break-all;
  }
  .rate {
    color: var(--vga-lightgreen);
  }
  .rate.alarm {
    color: var(--vga-lightred);
  }
  .info {
    padding: 2px 1ch;
    border-bottom: 1px solid var(--frame);
  }
  .info > div {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .info .tools {
    padding: 2px 0 0;
  }
  .val {
    color: var(--vga-white);
  }
  .yes {
    color: var(--vga-lightgreen);
  }
  .no {
    color: var(--vga-lightred);
  }
  .who {
    color: var(--vga-yellow);
  }
  .row.warning .text {
    color: var(--vga-lightred);
  }
  .row.brain .text {
    color: var(--vga-lightcyan);
  }
  .dim {
    margin: 0;
    color: var(--vga-darkgray);
  }
</style>
