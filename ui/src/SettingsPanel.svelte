<script lang="ts">
  import { LAYERS, groupRows, isOwn, numberChange, resetChange, show, sourceLabel, type SettingsMessage } from "./lib/settings";
  import type { SessionInfo, SettingChange, SettingRow } from "./proto";

  interface Props {
    /** The focused tile's session, which the panel follows. */
    session: SessionInfo | undefined;
    /** The brain's last `settings` for the session. */
    settings: SettingsMessage | undefined;
    attached: boolean | null;
    /** Ask for the settings, with the values of a layer's file (`character`, `server`, `mine`). */
    onrequest: (session: string, layer: string) => void;
    /** Change the file of a layer. */
    onsave: (session: string, layer: string, changes: SettingChange[]) => void;
  }

  let { session, settings, attached, onrequest, onsave }: Props = $props();

  /** The layer being edited, and whether the standard and advanced settings show too. */
  let layer = $state<string>("character");
  let all = $state(false);
  /** Number boxes whose text is not yet a whole number in range. */
  let bad = $state<Record<string, boolean>>({});

  /** The layer the brain last answered for, so the box follows a request from elsewhere. */
  $effect(() => {
    if (settings) layer = settings.layer;
  });

  const groups = $derived(groupRows(settings?.rows ?? [], all));
  const files = $derived(settings?.files ?? []);

  function pick(next: string) {
    layer = next;
    if (session) onrequest(session.id, next);
  }

  function save(change: SettingChange | undefined, row: SettingRow) {
    if (!session) return;
    if (!change) {
      bad[row.key] = true;
      return;
    }
    bad[row.key] = false;
    onsave(session.id, layer, [change]);
  }

  function reset(row: SettingRow) {
    if (!session) return;
    bad[row.key] = false;
    onsave(session.id, layer, [resetChange(row)]);
  }

  const rangeText = (row: SettingRow) =>
    row.kind.kind === "int" ? `${row.kind.min} to ${row.kind.max}${row.unit ? " " + row.unit : ""}` : "";
</script>

<div class="settings">
  {#if !session}
    <p class="dim">No character.</p>
  {:else if !attached}
    <p class="dim">No brain attached: no settings.</p>
  {:else if !settings}
    <p class="dim">Asking the brain for {session.name}'s settings…</p>
  {:else}
    <div class="tools" role="group" aria-label="Which settings file">
      <span>edit</span>
      {#each LAYERS as l (l.id)}
        <button class:on={layer === l.id} title={l.title} onclick={() => pick(l.id)}>{l.label}</button>
      {/each}
      <label class="all" title="Show the standard and advanced settings too">
        <input type="checkbox" bind:checked={all} /> all
      </label>
    </div>
    <div class="files dim">
      {#each files.filter((f) => f.layer === layer) as f (f.path)}
        <span title={f.path}>{f.exists ? "saved in" : "will be saved in"} {f.path}</span>
      {/each}
    </div>
    <div class="list">
      {#each groups as g (g.id)}
        <section aria-label={g.label}>
          <h3>{g.label}</h3>
          {#each g.rows as row (row.key)}
            <div class="row" data-key={row.key} class:own={isOwn(row)}>
              <label class="name" for={`s-${row.key}`} title={row.help}>{row.label}</label>
              <span class="control">
                {#if row.kind.kind === "int"}
                  <input
                    id={`s-${row.key}`}
                    type="text"
                    inputmode="numeric"
                    size="5"
                    class:bad={bad[row.key]}
                    value={show(row.own ?? row.value)}
                    title={bad[row.key] ? `A whole number, ${rangeText(row)}` : rangeText(row)}
                    onchange={(e) => save(numberChange(row, e.currentTarget.value), row)}
                  />
                  <span class="unit">{row.unit}</span>
                {:else if row.kind.kind === "bool"}
                  <input
                    id={`s-${row.key}`}
                    type="checkbox"
                    checked={(row.own ?? row.value) === true}
                    onchange={(e) => save({ key: row.key, value: e.currentTarget.checked }, row)}
                  />
                {:else}
                  <select
                    id={`s-${row.key}`}
                    value={String(row.own ?? row.value)}
                    onchange={(e) => save({ key: row.key, value: e.currentTarget.value }, row)}
                  >
                    {#each row.kind.options as o (o)}
                      <option value={o}>{o}</option>
                    {/each}
                  </select>
                {/if}
              </span>
              <span class="source" data-source={row.source} title={`Now ${show(row.value)}, from ${sourceLabel(row.source)}`}>
                {#if isOwn(row)}set here{:else}{sourceLabel(row.source)}{/if}
              </span>
              {#if isOwn(row)}
                <button class="reset" aria-label={`reset ${row.label}`} title="Take this out of the file: the layer below shows through" onclick={() => reset(row)}>reset</button>
              {/if}
              <div class="help">{row.help}</div>
            </div>
          {/each}
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* The settings tab of the drawer (App.svelte). */
  .settings {
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
  .tools {
    display: flex;
    align-items: center;
    gap: 1ch;
    padding: 2px 1ch;
    flex-wrap: wrap;
  }
  .files {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding: 0 1ch 1ch;
  }
  h3 {
    margin: 8px 0 2px;
    font: inherit;
    color: var(--vga-yellow);
    border-bottom: 1px solid var(--frame);
  }
  .row {
    display: grid;
    grid-template-columns: 1fr auto auto auto;
    align-items: center;
    gap: 0 1ch;
    padding: 2px 0;
  }
  .row .help {
    grid-column: 1 / -1;
    color: var(--dim);
    font-size: 0.85em;
    line-height: 1.2;
  }
  .control {
    display: flex;
    align-items: center;
    gap: 0.5ch;
  }
  .unit {
    color: var(--dim);
  }
  .source {
    color: var(--dim);
    font-size: 0.85em;
  }
  .row.own .source {
    color: var(--vga-lightgreen);
  }
  input[type="text"],
  select {
    font: inherit;
    color: var(--vga-lightgray);
    background: #000;
    border: 1px solid var(--frame);
  }
  input.bad {
    border-color: var(--vga-lightred, #f55);
  }
  button {
    font: inherit;
    line-height: 16px;
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightgray);
    border: 0;
    cursor: pointer;
  }
  button.on {
    background: var(--vga-yellow);
  }
  .all {
    margin-left: auto;
    color: var(--dim);
  }
</style>
