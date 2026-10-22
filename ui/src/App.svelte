<script lang="ts">
  import { onMount } from "svelte";
  import Tile from "./Tile.svelte";
  import { GatewayClient, gatewayUrl, type ConnectionStatus, type TokenStore } from "./lib/gateway";
  import Help from "./Help.svelte";
  import DevPanel from "./DevPanel.svelte";
  import MapPanel from "./MapPanel.svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
  import StatsPanel from "./StatsPanel.svelte";
  import type { SettingsMessage } from "./lib/settings";
  import { STATS_REFRESH_MS, type Stats } from "./lib/stats";
  import { PAUSE_KEY, START_KEY, botKeyAction } from "./lib/bot";
  import {
    DEV_PANEL_CODE,
    FOCUS_TILE_CODE,
    GO_CODE,
    MAP_PANEL_CODE,
    RESET_LAYOUT_CODE,
    SETTINGS_PANEL_CODE,
    STATS_PANEL_CODE,
    ZOOM_IN_KEYS,
    ZOOM_OUT_KEYS,
    ZOOM_RESET_KEYS,
  } from "./lib/keys";
  import { LOOP_RADIUS, MAP_RADIUS, type Loops, type MapView } from "./lib/map";
  import type { FoundMessage } from "./lib/goto";
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
  import {
    addEvent,
    addNote,
    applyBrainState,
    applyCharacter,
    applyCharacterPatch,
    pruneCharacters,
    emptyBrainView,
    emptyDevLog,
    loadDevPrefs,
    saveDevPrefs,
    toggleDrawer,
    type DrawerPanel,
  } from "./lib/state";
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

  // The dev panel's buffers are plain (not `$state`); `devTick` says they changed, at most once
  // a frame, so a burst of events costs one redraw of the panel.
  const dev = emptyDevLog();
  let devTick = $state(0);
  let devFrame = 0;
  function devChanged() {
    if (devFrame) return;
    devFrame = requestAnimationFrame(() => {
      devFrame = 0;
      devTick += 1;
    });
  }
  const devPrefs = $state(loadDevPrefs(browserStorage()));
  $effect(() => saveDevPrefs(browserStorage(), { open: devPrefs.open, panel: devPrefs.panel, tab: devPrefs.tab }));

  const DRAWER_BUTTONS: { panel: DrawerPanel; title: string }[] = [
    { panel: "map", title: "Map (Alt+M)" },
    { panel: "stats", title: "Stats (Alt+S)" },
    { panel: "settings", title: "Settings (Alt+T)" },
    { panel: "dev", title: "Dev panel (Alt+D)" },
  ];

  // The brain's last `map_view` per session (replaced whole, so not deeply reactive).
  let mapViews = $state.raw<Record<string, MapView>>({});
  // The brain's last `loops` per session.
  let loopsBySession = $state.raw<Record<string, Loops>>({});
  // The brain's last `stats` per session.
  let statsBySession = $state.raw<Record<string, Stats>>({});
  // The brain's last `settings` per session, for the settings tab.
  let settingsBySession = $state.raw<Record<string, SettingsMessage>>({});
  // The brain's last `found` per session, for the map panel's go box.
  let foundBySession = $state.raw<Record<string, FoundMessage>>({});
  /** Bumped by Alt+G: the map panel opens its go box and focuses it. */
  let goAsked = $state(0);
  /** The last line the brain held per session (`input_held`), until sent anyway or dismissed. */
  let held = $state.raw<Record<string, { text: string; reason: string }>>({});
  // Sessions whose map has been asked for since the connection opened or the brain attached:
  // the brain sends a view by itself only when the believed room or the route changes.
  const mapAsked = new Set<string>();
  const focusedId = $derived(sessions[focused]?.id);
  $effect(() => {
    const id = focusedId;
    if (!devPrefs.open || devPrefs.panel !== "map" || !id || status !== "open" || !brain.attached) return;
    if (brain.characters[id]?.position?.room === undefined || mapAsked.has(id)) return;
    if (client.requestMap(id, MAP_RADIUS)) mapAsked.add(id);
  });

  // The stats tab asks for the focused character's runs when it opens, when a run starts or
  // ends, and every minute while it stays open (the brain samples once a minute).
  const runKey = $derived(focusedId ? (brain.characters[focusedId]?.run?.run_started_at ?? "none") : "");
  $effect(() => {
    const id = focusedId;
    void runKey;
    if (!devPrefs.open || devPrefs.panel !== "stats" || !id || status !== "open" || !brain.attached) return;
    client.requestStats(id);
    const timer = setInterval(() => client.requestStats(id), STATS_REFRESH_MS);
    return () => clearInterval(timer);
  });

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
        pruneCharacters(
          brain,
          list.map((s) => s.id),
        );
      },
      state: (id, state) => {
        const info = sessions.find((s) => s.id === id);
        if (info) info.state = state;
      },
      write: (id, text) => sinks.write(id, text),
      reset: (id) => sinks.reset(id),
      connection: (s, detail) => {
        if (s !== "open") mapAsked.clear();
        status = s;
        reason = detail?.reason ?? "";
        retryInMs = detail?.retryInMs ?? 0;
      },
      error: (message) => console.warn("gateway:", message),
      character: (id, state) => applyCharacter(brain, id, state),
      characterPatch: (id, patch) => applyCharacterPatch(brain, id, patch),
      brainState: (attached) => {
        if (attached !== brain.attached) {
          const text = attached ? "brain attached" : brain.attached ? "brain left" : "no brain attached";
          addNote(dev, "brain", text, undefined, Date.now());
          devChanged();
        }
        if (attached !== brain.attached) mapAsked.clear();
        applyBrainState(brain, attached);
      },
      mapView: (view) => {
        mapViews = { ...mapViews, [view.session]: view };
      },
      loops: (loops) => {
        loopsBySession = { ...loopsBySession, [loops.session]: loops };
      },
      stats: (stats) => {
        statsBySession = { ...statsBySession, [stats.session]: stats };
      },
      settings: (settings) => {
        settingsBySession = { ...settingsBySession, [settings.session]: settings };
      },
      found: (found) => {
        foundBySession = { ...foundBySession, [found.session]: found };
      },
      inputHeld: (id, text, reason) => {
        held = { ...held, [id]: { text, reason } };
        addNote(dev, "warning", `held: ${text} (${reason})`, id, Date.now());
        devChanged();
      },
      event: (id, e) => {
        addEvent(dev, id, e);
        devChanged();
      },
      brainInfo: (info) => {
        if (info.started_at !== dev.info?.started_at) {
          addNote(dev, "brain", `brain ${info.build} started ${info.started_at}`, undefined, Date.now());
        }
        dev.info = { build: info.build, started_at: info.started_at, world_pack: info.world_pack };
        devChanged();
      },
      notice: (kind, text, id) => {
        addNote(dev, kind, text, id, Date.now());
        devChanged();
      },
      brainLagged: (missed) => {
        addNote(dev, "brain", `brain lagged: ${missed} messages dropped; asked for state again`, undefined, Date.now());
        devChanged();
      },
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

  // Alt+M, Alt+S and Alt+D open the drawer on the map, stats or dev panel; Alt+G on the map's go
  // box; Alt+1..Alt+6 focus a tile; Ctrl +/-/0
  // zoom the focused one; F9 and F10 pause or resume, start or stop its bot. Captured before
  // xterm sees them.
  function onkeydown(e: KeyboardEvent) {
    if ((e.key === PAUSE_KEY || e.key === START_KEY) && !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
      e.preventDefault();
      const id = sessions[focused]?.id;
      const action = id && brain.attached ? botKeyAction(e.key, brain.characters[id]?.bot) : null;
      if (id && action) client.botControl(id, action);
      return;
    }
    const drawer: DrawerPanel | null =
      e.code === MAP_PANEL_CODE
        ? "map"
        : e.code === STATS_PANEL_CODE
          ? "stats"
          : e.code === SETTINGS_PANEL_CODE
            ? "settings"
            : e.code === DEV_PANEL_CODE
              ? "dev"
              : null;
    if (e.altKey && !e.ctrlKey && !e.metaKey && drawer) {
      e.preventDefault();
      toggleDrawer(devPrefs, drawer);
      return;
    }
    if (e.altKey && !e.ctrlKey && !e.metaKey && e.code === GO_CODE) {
      e.preventDefault();
      devPrefs.open = true;
      devPrefs.panel = "map";
      goAsked += 1;
      return;
    }
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
    <span class="right">
      {#each DRAWER_BUTTONS as b (b.panel)}
        <button
          class="devbtn"
          class:on={devPrefs.open && devPrefs.panel === b.panel}
          onclick={() => toggleDrawer(devPrefs, b.panel)}
          title={b.title}
        >
          {b.panel}
        </button>
      {/each}
      <span class="conn {status}">{status === "open" ? "connected" : status}</span>
    </span>
  </div>
  <div class="body">
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
          held={held[info.id] ?? null}
          onheld={() => {
            const { [info.id]: _, ...rest } = held;
            held = rest;
          }}
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
    {#if devPrefs.open}
      <!-- A drawer on the right of the tiles, a third of the width at most; the tiles refit. -->
      <aside class="drawer" aria-label="{devPrefs.panel} panel">
        <div class="title">
          <span class="tabs" role="tablist">
            {#each DRAWER_BUTTONS as b (b.panel)}
              <button
                role="tab"
                aria-selected={devPrefs.panel === b.panel}
                class:on={devPrefs.panel === b.panel}
                onclick={() => (devPrefs.panel = b.panel)}
              >
                {b.panel}
              </button>
            {/each}
          </span>
          <span>═ <span class="who">{sessions[focused]?.name ?? "no character"}</span> ═</span>
          <button onclick={() => (devPrefs.open = false)} title="Close">x</button>
        </div>
        {#if devPrefs.panel === "map"}
          <MapPanel
            session={sessions[focused]}
            view={focusedId ? mapViews[focusedId] : undefined}
            character={focusedId ? brain.characters[focusedId] : undefined}
            attached={brain.attached}
            onwalk={(id, map, room, stopShort) => client.walkTo(id, map, room, stopShort)}
            found={focusedId ? foundBySession[focusedId] : undefined}
            onfind={(id, query) => client.find(id, query)}
            {goAsked}
            onstop={(id) => client.walkStop(id)}
            loops={focusedId ? loopsBySession[focusedId] : undefined}
            onloops={(id, doors) => client.requestLoops(id, LOOP_RADIUS, doors)}
            onwalkloop={(id, loop) => client.walkLoop(id, loop)}
            onrunloop={(id, loop, skip) => client.runLoop(id, loop, skip)}
            onsaveloop={(id, name, base, stops) => client.saveLoop(id, name, base, stops)}
            ondeleteloop={(id, loop) => client.deleteLoop(id, loop)}
          />
        {:else if devPrefs.panel === "settings"}
          <SettingsPanel
            session={sessions[focused]}
            settings={focusedId ? settingsBySession[focusedId] : undefined}
            attached={brain.attached}
            onrequest={(id, layer) => client.requestSettings(id, layer)}
            onsave={(id, layer, changes) => client.saveSettings(id, layer, changes)}
          />
        {:else if devPrefs.panel === "stats"}
          <StatsPanel
            session={sessions[focused]}
            character={focusedId ? brain.characters[focusedId] : undefined}
            stats={focusedId ? statsBySession[focusedId] : undefined}
            attached={brain.attached}
            onrefresh={(id) => client.requestStats(id)}
            onreset={(id) => client.resetStats(id)}
            onrule={(id, pattern, rule) => client.setMonsterRule(id, pattern, rule)}
          />
        {:else}
          <DevPanel
            session={sessions[focused]}
            log={dev}
            tick={devTick}
            character={focusedId ? brain.characters[focusedId] : undefined}
            attached={brain.attached}
            bind:tab={devPrefs.tab}
            onresync={(id) => client.resync(id)}
          />
        {/if}
      </aside>
    {/if}
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
  .right {
    display: flex;
    gap: 2ch;
  }
  .devbtn {
    font: inherit;
    line-height: 16px;
    padding: 0 1ch;
    color: var(--vga-lightgray);
    background: var(--vga-blue);
    border: none;
    cursor: pointer;
  }
  .devbtn.on {
    color: #000;
    background: var(--vga-lightcyan);
  }
  .drawer {
    flex: 0 0 auto;
    width: min(33.333%, 100ch);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    margin: 2px 2px 2px 0;
    color: var(--vga-lightgray);
    background: var(--bg);
    border: 4px double var(--frame);
    line-height: 16px;
    font-size: 16px;
  }
  .drawer .title {
    display: flex;
    justify-content: space-between;
    gap: 1ch;
    padding: 0 1ch 2px;
    color: var(--frame);
    white-space: pre;
    border-bottom: 1px solid var(--frame);
  }
  .drawer .tabs {
    display: flex;
    gap: 1ch;
  }
  .drawer .who {
    color: var(--vga-yellow);
  }
  .drawer button {
    font: inherit;
    line-height: 16px;
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightgray);
    border: none;
    cursor: pointer;
  }
  .drawer button.on {
    background: var(--vga-lightcyan);
  }
  /* The tiles, and the dev panel to their right when it is open. */
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
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
    min-width: 0;
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
