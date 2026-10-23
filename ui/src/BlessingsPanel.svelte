<script lang="ts">
  // The drawer's Blessings panel (C-047): one box per character, the lasting spells it knows
  // (bless, barkskin, holy aura), ticked for the bot to keep on, ● when one is on now. The
  // brain casts them between fights; this only edits the ticks.
  import { changed, chosenOf, spellLabel, toggleSpell } from "./lib/blessings";
  import type { Blessings, SessionInfo } from "./proto";

  interface Props {
    sessions: SessionInfo[];
    /** The brain's last `blessings`; undefined before the first. */
    blessings: Blessings[] | undefined;
    attached: boolean | null;
    onsave: (session: string, spells: string[]) => void;
  }

  let { sessions, blessings, attached, onsave }: Props = $props();

  /** Ticks being edited, by session. */
  let drafts = $state<Record<string, string[]>>({});

  function ticks(b: Blessings): string[] {
    return drafts[b.session] ?? chosenOf(b);
  }
  function toggle(b: Blessings, short: string) {
    drafts = { ...drafts, [b.session]: toggleSpell(b, ticks(b), short) };
  }
  function forget(session: string) {
    const { [session]: _, ...rest } = drafts;
    drafts = rest;
  }
  function save(b: Blessings) {
    onsave(b.session, ticks(b));
    forget(b.session);
  }
  const name = (id: string) => sessions.find((s) => s.id === id)?.name ?? id;
</script>

<div class="blessings">
  {#if !attached}
    <p class="dim">No brain attached: blessings are the brain's.</p>
  {:else if blessings === undefined}
    <p class="dim">Asking the brain…</p>
  {:else}
    {#each blessings as b (b.session)}
      <section class="character" aria-label="{name(b.session)}'s blessings">
        <div class="head"><span class="name">{name(b.session)}</span></div>
        {#if b.spells.length === 0}
          <p class="dim">No lasting spells known (or its spells not read yet).</p>
        {:else}
          <ul>
            {#each b.spells as s (s.short)}
              <li>
                <label>
                  <input type="checkbox" checked={ticks(b).includes(s.short)} onchange={() => toggle(b, s.short)} />
                  {spellLabel(s)}
                </label>
                {#if s.on}<span class="on" title="On now">●</span>{/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if drafts[b.session] && changed(b, drafts[b.session])}
          <div class="acts">
            <button onclick={() => save(b)}>save</button>
            <button onclick={() => forget(b.session)}>undo</button>
          </div>
        {/if}
      </section>
    {:else}
      <p class="dim">No characters yet.</p>
    {/each}
  {/if}
</div>

<style>
  /* The blessings tab of the drawer (App.svelte), laid out as the parties tab is. */
  .blessings {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding: 2px 0;
  }
  .dim {
    margin: 0;
    padding: 0 1ch;
    color: var(--dim);
  }
  .character {
    padding: 2px 1ch 4px;
    border-bottom: 1px solid var(--frame);
  }
  .character .dim {
    padding: 0;
  }
  .name {
    color: var(--vga-yellow);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    justify-content: space-between;
    gap: 1ch;
  }
  .on {
    color: var(--vga-lightgreen);
  }
  .acts {
    display: flex;
    gap: 1ch;
    padding: 2px 0 0;
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
