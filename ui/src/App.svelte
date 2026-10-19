<script lang="ts">
  import { onMount } from "svelte";
  import Tile from "./Tile.svelte";
  import { GatewayClient, gatewayUrl, type ConnectionStatus, type TokenStore } from "./lib/gateway";
  import Help from "./Help.svelte";
  import { FOCUS_TILE_CODE, RESET_LAYOUT_CODE, ZOOM_IN_KEYS, ZOOM_OUT_KEYS, ZOOM_RESET_KEYS } from "./lib/keys";
  import {
    drag,
    equalWeights,
    fitsGrid,
    loadLayout,
    saveLayout,
    shapeKey,
    tileGrid,
    toTemplate,
    TERM_COLS,
    CELL_ASPECT,
    type Axis,
    type Weights,
  } from "./lib/layout";
  import { TerminalSinks } from "./lib/sinks";
  import { applyBrainState, applyCharacter, applyCharacterPatch, emptyBrainView } from "./lib/state";
  import type { SessionInfo } from "./proto";

  let sessions = $state<SessionInfo[]>([]);
  const brain = $state(emptyBrainView());
  let status = $state<ConnectionStatus>("connecting");
  let reason = $state("");
  let retryInMs = $state(0);
  let focused = $state(0);
  let zooms = $state<Record<string, number>>({});
  let gridWidth = $state(0);
  let gridHeight = $state(0);
  let helpArea = $state<{ left: number; top: number; width: number; height: number } | null>(null);

  /** Open (over the focused tile) or close the help overlay. */
  function toggleHelp() {
    if (helpArea) {
      helpArea = null;
      return;
    }
    const r = document.querySelectorAll(".grid > .tile")[focused]?.getBoundingClientRect();
    helpArea = r
      ? { left: r.left, top: r.top, width: r.width, height: r.height }
      : { left: 0, top: 0, width: window.innerWidth, height: window.innerHeight };
  }

  const grid = $derived(tileGrid(sessions.length, gridWidth, gridHeight));

  // Column and row weights per grid shape, kept in the browser until settings exist (C-013).
  const GAP = 6;
  const PAD = 2;
  /** 80 columns at the smallest font (6 px, 3 px cells) plus the tile's 8 px side frames. */
  const MIN_TILE_WIDTH = TERM_COLS * 6 * CELL_ASPECT + 16;
  const MIN_TILE_HEIGHT = 120;
  let layout = $state(loadLayout(browserStorage()));
  let dragging = $state(false);

  const weights = $derived.by((): Weights => {
    const stored = layout.shapes[shapeKey(grid)];
    return stored && fitsGrid(stored, grid) ? stored : equalWeights(grid);
  });

  /** The pixels an axis' tracks share, without gaps and padding. */
  function room(axis: Axis): number {
    const [size, n] = axis === "cols" ? [gridWidth, grid.cols] : [gridHeight, grid.rows];
    return size - 2 * PAD - GAP * (n - 1);
  }

  /** Where the handle after track `i` of `axis` lies, in px from the grid's edge. */
  function handleAt(axis: Axis, i: number): number {
    const tracks = weights[axis];
    const sum = tracks.reduce((a, b) => a + b, 0);
    const before = tracks.slice(0, i + 1).reduce((a, b) => a + b, 0);
    return PAD + (before / sum) * room(axis) + GAP * i;
  }

  function setWeights(next: Weights) {
    layout.shapes[shapeKey(grid)] = next;
  }

  function persist() {
    saveLayout(browserStorage(), $state.snapshot(layout));
  }

  function startDrag(e: PointerEvent, axis: Axis, index: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const start = $state.snapshot(weights);
    const from = axis === "cols" ? e.clientX : e.clientY;
    const total = room(axis);
    const min = axis === "cols" ? MIN_TILE_WIDTH : MIN_TILE_HEIGHT;
    dragging = true;
    const move = (m: PointerEvent) => {
      const at = axis === "cols" ? m.clientX : m.clientY;
      setWeights(drag(start, axis, index, at - from, total, min));
    };
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
      dragging = false;
      persist();
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }

  function resetAxes(axes: Axis[]) {
    const equal = equalWeights(grid);
    const next = { ...weights };
    for (const axis of axes) next[axis] = equal[axis];
    setWeights(next);
    persist();
  }
  const offline = $derived(
    status === "open"
      ? null
      : status === "down"
        ? `gateway connection lost: retrying in ${Math.ceil(retryInMs / 1000)}s`
        : status === "refused"
          ? `refused: ${reason}`
          : "connecting to the gateway",
  );

  const sinks = new TerminalSinks();
  const client = new GatewayClient({
    url: gatewayUrl(location),
    tokens: browserStorage(),
    askToken: (why) =>
      window.prompt(`The gateway asks for its token (${why}).\nPaste it from \`wayfarer gateway --new-token\`:`),
    events: {
      sessions: (list) => {
        sessions = list;
        if (focused >= list.length) focused = 0;
      },
      state: (id, state) => {
        const info = sessions.find((s) => s.id === id);
        if (info) info.state = state;
      },
      write: (id, text) => sinks.write(id, text),
      reset: (id) => sinks.reset(id),
      connection: (s, detail) => {
        status = s;
        reason = detail?.reason ?? "";
        retryInMs = detail?.retryInMs ?? 0;
      },
      error: (message) => console.warn("gateway:", message),
      character: (id, state) => applyCharacter(brain, id, state),
      characterPatch: (id, patch) => applyCharacterPatch(brain, id, patch),
      brainState: (attached) => applyBrainState(brain, attached),
    },
  });

  onMount(() => {
    client.connect();
    return () => client.close();
  });

  function browserStorage(): TokenStore | undefined {
    try {
      const storage = window.localStorage;
      storage.getItem("wayfarer.probe");
      return storage;
    } catch {
      return undefined;
    }
  }

  // Alt+1..Alt+6 focus a tile; Ctrl +/-/0 zoom the focused one. Captured before xterm sees them.
  function onkeydown(e: KeyboardEvent) {
    if (e.altKey && !e.ctrlKey && !e.metaKey && e.code === RESET_LAYOUT_CODE) {
      e.preventDefault();
      resetAxes(["cols", "rows"]);
      return;
    }
    if (e.altKey && !e.ctrlKey && !e.metaKey && FOCUS_TILE_CODE.test(e.code)) {
      const n = Number(e.code.slice(5)) - 1;
      if (n < sessions.length) {
        e.preventDefault();
        focused = n;
      }
      return;
    }
    const id = sessions[focused]?.id;
    if (!id || !e.ctrlKey || e.altKey || e.metaKey) return;
    const step = ZOOM_IN_KEYS.includes(e.key)
      ? 1
      : ZOOM_OUT_KEYS.includes(e.key)
        ? -1
        : ZOOM_RESET_KEYS.includes(e.key)
          ? 0
          : null;
    if (step === null) return;
    e.preventDefault();
    zooms[id] = step === 0 ? 0 : (zooms[id] ?? 0) + step;
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

<main>
  <div class="bar">
    <span class="title">wayfarer</span>
    <span class="conn {status}">{status === "open" ? "connected" : status}</span>
  </div>
  <div
    class="grid"
    class:dragging
    bind:clientWidth={gridWidth}
    bind:clientHeight={gridHeight}
    style:grid-template-columns={toTemplate(weights.cols)}
    style:grid-template-rows={toTemplate(weights.rows)}
  >
    {#each sessions as info, i (info.id)}
      <Tile
        {info}
        number={i + 1}
        focused={i === focused}
        zoom={zooms[info.id] ?? 0}
        {offline}
        {client}
        {sinks}
        onfocus={() => (focused = i)}
        onhelp={toggleHelp}
        character={brain.attached ? (brain.characters[info.id] ?? {}) : null}
      />
    {:else}
      <p class="empty">
        {status === "open" ? "No characters are configured in wayfarer.toml." : offline}
      </p>
    {/each}
    {#each weights.cols.slice(0, -1) as _, i}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
      <div
        class="handle cols"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize columns {i + 1} and {i + 2}"
        style:left="{handleAt('cols', i)}px"
        onpointerdown={(e) => startDrag(e, "cols", i)}
        ondblclick={() => resetAxes(["cols"])}
      ></div>
    {/each}
    {#each weights.rows.slice(0, -1) as _, i}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
      <div
        class="handle rows"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize rows {i + 1} and {i + 2}"
        style:top="{handleAt('rows', i)}px"
        onpointerdown={(e) => startDrag(e, "rows", i)}
        ondblclick={() => resetAxes(["rows"])}
      ></div>
    {/each}
  </div>
</main>

{#if helpArea}
  <Help area={helpArea} onclose={() => (helpArea = null)} />
{/if}

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .bar {
    display: flex;
    justify-content: space-between;
    padding: 0 1ch;
    line-height: 16px;
    color: #000;
    background: var(--vga-lightgray);
  }
  .conn.open {
    color: var(--vga-green);
  }
  .conn.down,
  .conn.refused {
    color: var(--vga-red);
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    position: relative;
    gap: 6px;
    padding: 2px;
  }
  .grid.dragging {
    user-select: none;
  }
  /* Sits in the gap between tiles, so it never covers a tile's scrollbar or click-to-focus. */
  .handle {
    position: absolute;
    background: var(--frame);
    touch-action: none;
  }
  .handle:hover,
  .grid.dragging .handle {
    background: var(--frame-focus);
  }
  .handle.cols {
    top: 2px;
    bottom: 2px;
    width: 6px;
    cursor: col-resize;
  }
  .handle.rows {
    left: 2px;
    right: 2px;
    height: 6px;
    cursor: row-resize;
  }
  .empty {
    margin: 2em;
    color: var(--dim);
  }
</style>
