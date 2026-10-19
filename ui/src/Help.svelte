<script lang="ts">
  import { onMount } from "svelte";
  import { BADGE_MEANINGS, FILES_HELP, HELP_KEY, INPUT_VS_KEYS, KEY_ROWS } from "./lib/keys";

  interface Props {
    /** The focused tile's box, in page coordinates. */
    area: { left: number; top: number; width: number; height: number };
    onclose: () => void;
  }

  let { area, onclose }: Props = $props();

  let closeButton: HTMLButtonElement;
  const groups = [...new Set(KEY_ROWS.map((r) => r.group))];

  onMount(() => {
    const before = document.activeElement;
    closeButton.focus();
    return () => {
      if (before instanceof HTMLElement) before.focus();
    };
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || e.key === HELP_KEY) {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (e.key === "Tab") {
      // The close button is the only stop: keep focus inside.
      e.preventDefault();
      closeButton.focus();
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="backdrop"
  data-testid="help-backdrop"
  style:left="{area.left}px"
  style:top="{area.top}px"
  style:width="{area.width}px"
  style:height="{area.height}px"
  onclick={onclose}
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="box"
    role="dialog"
    aria-modal="true"
    aria-label="wayfarer help"
    tabindex="-1"
    {onkeydown}
    onclick={(e) => e.stopPropagation()}
  >
    <div class="title">
      <span>╔═ wayfarer help ═╗</span>
      <button bind:this={closeButton} onclick={onclose}>Esc</button>
    </div>

    <h2>Keys</h2>
    {#each groups as group}
      <h3>{group}</h3>
      <table>
        <tbody>
          {#each KEY_ROWS.filter((r) => r.group === group) as row}
            <tr><th scope="row">{row.keys}</th><td>{row.does}</td></tr>
          {/each}
        </tbody>
      </table>
    {/each}

    <h2>Input line and keys mode</h2>
    <p>{INPUT_VS_KEYS}</p>

    <h2>State badges</h2>
    <table>
      <tbody>
        {#each Object.entries(BADGE_MEANINGS) as [state, meaning]}
          <tr><th scope="row" class="badge {state}">{state}</th><td>{meaning}</td></tr>
        {/each}
      </tbody>
    </table>

    <h2>Files</h2>
    <ul>
      {#each FILES_HELP as line}
        <li>{line}</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    display: flex;
    align-items: stretch;
    justify-content: center;
    background: rgba(0, 0, 0, 0.6);
    z-index: 10;
  }
  .box {
    box-sizing: border-box;
    width: min(100%, 84ch);
    margin: 4px;
    padding: 0 1ch 1ch;
    overflow: auto;
    color: var(--vga-lightgray);
    background: var(--vga-blue);
    border: 4px double var(--vga-white);
    line-height: 16px;
    outline: none;
  }
  .title {
    display: flex;
    justify-content: space-between;
    color: var(--vga-yellow);
  }
  h2,
  h3 {
    margin: 1em 0 0;
    font: inherit;
    color: var(--vga-lightcyan);
  }
  h3 {
    color: var(--vga-yellow);
  }
  table {
    border-collapse: collapse;
  }
  th {
    padding-right: 2ch;
    text-align: left;
    font-weight: normal;
    color: var(--vga-white);
    vertical-align: top;
    white-space: nowrap;
  }
  td {
    vertical-align: top;
  }
  p,
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  button {
    font: inherit;
    color: #000;
    background: var(--vga-lightgray);
    border: none;
    cursor: pointer;
  }
  th.badge {
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightgray);
  }
  th.badge.connecting {
    background: var(--vga-brown);
  }
  th.badge.connected {
    background: var(--vga-white);
  }
  th.badge.logon {
    background: var(--vga-cyan);
  }
  th.badge.menu {
    background: var(--vga-lightblue);
  }
  th.badge.realm {
    background: var(--vga-lightgreen);
  }
  th.badge.lost {
    background: var(--vga-lightred);
  }
  th.badge.closed {
    background: var(--vga-darkgray);
    color: var(--vga-lightgray);
  }
</style>
