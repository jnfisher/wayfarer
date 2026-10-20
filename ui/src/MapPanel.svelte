<script lang="ts">
  import { untrack } from "svelte";
  import {
    carryPan,
    DEFAULT_ZOOM,
    describeRoom,
    drawMap,
    keyOf,
    loopText,
    roomAt,
    walkText,
    zoomAt,
    ZOOMS,
    type Loops,
    type MapView,
    type ViewState,
  } from "./lib/map";
  import { FONT_FAMILY } from "./lib/theme";
  import type { CharacterState, MapRoom, SessionInfo } from "./proto";

  interface Props {
    /** The focused tile's session, which the panel follows. */
    session: SessionInfo | undefined;
    /** The brain's last `map_view` for it. */
    view: MapView | undefined;
    character: CharacterState | undefined;
    attached: boolean | null;
    onwalk: (session: string, map: number, room: number) => void;
    onstop: (session: string) => void;
    /** The brain's last `loops` for it. */
    loops?: Loops;
    /** Ask for the loops around the character, through doors or not. */
    onloops?: (session: string, doors: boolean) => void;
    /** Walk a generated loop once. */
    onwalkloop?: (session: string, id: string) => void;
  }

  let { session, view, character, attached, onwalk, onstop, loops, onloops, onwalkloop }: Props = $props();

  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  let height = $state(0);
  let zoom = $state(DEFAULT_ZOOM);
  let panX = $state(0);
  let panY = $state(0);
  let level = $state(0);
  /** The user has panned or changed level: new views keep the place instead of recentring. */
  let moved = $state(false);
  let hover = $state<MapRoom | null>(null);
  let tip = $state<{ x: number; y: number; lines: string[] } | null>(null);

  const position = $derived(character?.position);
  const known = $derived(position?.room !== undefined);
  const walking = $derived(character?.walking);
  // The loops list: open or not, through doors or not, the loop hovered and the one picked
  // (a generated loop's id, or `mm:` and a MegaMud file).
  let loopsOpen = $state(false);
  let doors = $state(true);
  let hoverLoop = $state<string | null>(null);
  let pickedLoop = $state<string | null>(null);

  /** A listed loop as the map draws it: its path and its stops. */
  function loopShape(id: string | null) {
    if (!id || !loops) return null;
    const g = loops.generated.find((l) => l.id === id);
    if (g) return { path: g.path, stops: g.stops.map((s) => ({ map: s.map, room: s.room })) };
    const m = loops.megamud.find((l) => `mm:${l.file}` === id);
    return m ? { path: m.path, stops: [] } : null;
  }
  const drawnLoop = $derived(loopShape(hoverLoop ?? pickedLoop));
  const loopsAround = $derived.by(() => {
    if (!loops) return "";
    const a = loops.anchor;
    const r = view?.rooms.find((x) => keyOf(x) === keyOf(a));
    return `${loops.radius} steps around ${r?.name || `${a.map}:${a.room}`}`;
  });

  function askLoops() {
    if (session) onloops?.(session.id, doors);
  }

  function toggleLoops() {
    loopsOpen = !loopsOpen;
    if (loopsOpen && (!loops || loops.session !== session?.id)) askLoops();
  }

  /** Pick a loop to keep it drawn (again to let it go), showing the floor of its first stop. */
  function pickLoop(id: string) {
    pickedLoop = pickedLoop === id ? null : id;
    const first = loopShape(pickedLoop)?.path[0];
    const r = first && view?.rooms.find((x) => keyOf(x) === keyOf(first) && !x.collides);
    if (r && r.level !== level) {
      level = r.level;
      moved = true;
    }
  }

  const vs = $derived<ViewState>({ width, height, zoom, panX, panY, level, hover, loop: drawnLoop });
  const levels = $derived(view ? [...new Set(view.rooms.map((r) => r.level))].sort((a, b) => a - b) : []);

  function recentre() {
    moved = false;
    panX = 0;
    panY = 0;
    level = view?.level ?? 0;
  }

  // A new view: recentre on the believed room, or keep a moved view where it was.
  let last: MapView | undefined;
  $effect(() => {
    const next = view;
    if (next === last) return;
    const prev = last;
    last = next;
    if (!next) return;
    untrack(() => {
      if (moved && prev && prev.session === next.session) {
        ({ panX, panY } = carryPan(prev, next, vs));
      } else {
        recentre();
      }
      hover = null;
      tip = null;
    });
  });

  // Draw at the device's pixel ratio so the VGA look stays sharp on Retina.
  $effect(() => {
    if (!canvas || !view || width === 0 || height === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    drawMap(ctx, view, vs, FONT_FAMILY);
  });

  // Pointer: a press that does not move is a click (walk there); one that moves pans.
  let press: { x: number; y: number; panX: number; panY: number; dragged: boolean } | null = null;

  function local(e: PointerEvent | WheelEvent): [number, number] {
    const r = canvas!.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  }

  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0) return;
    canvas?.setPointerCapture(e.pointerId);
    press = { x: e.clientX, y: e.clientY, panX, panY, dragged: false };
  }

  function onpointermove(e: PointerEvent) {
    if (press) {
      const [dx, dy] = [e.clientX - press.x, e.clientY - press.y];
      if (press.dragged || Math.hypot(dx, dy) > 4) {
        press.dragged = true;
        moved = true;
        panX = press.panX + dx;
        panY = press.panY + dy;
        tip = null;
        return;
      }
    }
    if (!view) return;
    const [x, y] = local(e);
    const room = roomAt(view, vs, x, y);
    hover = room;
    tip = room ? { x, y, lines: describeRoom(view, room) } : null;
  }

  function onpointerup(e: PointerEvent) {
    const p = press;
    press = null;
    if (!p || p.dragged || !view || !session) return;
    const [x, y] = local(e);
    const room = roomAt(view, vs, x, y);
    if (room && !(room.map === view.center.map && room.room === view.center.room)) {
      onwalk(session.id, room.map, room.room);
    }
  }

  function onpointerleave() {
    if (!press) {
      hover = null;
      tip = null;
    }
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    if (e.deltaY === 0) return;
    const [x, y] = local(e);
    const next = zoomAt(vs, e.deltaY < 0 ? 1 : -1, x, y);
    if (next.zoom === zoom) return;
    moved = moved || next.panX !== 0 || next.panY !== 0;
    ({ zoom, panX, panY } = next);
    tip = null;
  }

  // Svelte adds wheel handlers as passive, which can't stop the page scrolling.
  $effect(() => {
    const c = canvas;
    if (!c) return;
    c.addEventListener("wheel", onwheel, { passive: false });
    return () => c.removeEventListener("wheel", onwheel);
  });

  function changeLevel(step: number) {
    const i = levels.indexOf(level) + step;
    if (i < 0 || i >= levels.length) return;
    level = levels[i];
    moved = level !== view?.level || panX !== 0 || panY !== 0;
  }

  const route = $derived(walkText(walking));
</script>

<div class="map">
  {#if !attached}
    <p class="msg">No brain attached: the map needs one (<code>wayfarer brain</code>).</p>
  {:else if !session}
    <p class="msg">No character.</p>
  {:else if !known}
    <p class="msg">position unknown: re-sync or look</p>
  {:else}
    <div class="tools">
      <span class="dim">level</span>
      <button aria-label="Level down" disabled={levels.indexOf(level) <= 0} onclick={() => changeLevel(-1)}>▼</button>
      <span class="level">{level}</span>
      <button aria-label="Level up" disabled={levels.indexOf(level) >= levels.length - 1} onclick={() => changeLevel(1)}>
        ▲
      </button>
      <span class="dim">zoom {zoom + 1}/{ZOOMS.length}</span>
      {#if moved}
        <button onclick={recentre}>centre</button>
      {/if}
      {#if (position?.candidates ?? 0) > 1}
        <span class="maybe">{position?.candidates} candidates</span>
      {/if}
    </div>
    <div class="canvas" bind:clientWidth={width} bind:clientHeight={height}>
      {#if view}
        <canvas
          bind:this={canvas}
          style:width="{width}px"
          style:height="{height}px"
          aria-label="Map around {position?.title ?? 'the character'}"
          {onpointerdown}
          {onpointermove}
          {onpointerup}
          {onpointerleave}
        ></canvas>
        {#if tip}
          <div class="tip" style:left="{tip.x + 12}px" style:top="{tip.y + 12}px">
            {#each tip.lines as l, i}
              <div class:name={i === 0}>{l}</div>
            {/each}
          </div>
        {/if}
      {:else}
        <p class="msg">Waiting for the map from the brain…</p>
      {/if}
    </div>
    <div class="loops" aria-label="Loops">
      <div class="head">
        <button class="toggle" aria-expanded={loopsOpen} onclick={toggleLoops}>{loopsOpen ? "▾" : "▸"} loops</button>
        {#if loopsOpen}
          <label><input type="checkbox" bind:checked={doors} onchange={askLoops} /> doors</label>
          <button onclick={askLoops} title="The loops around the character's room now">here</button>
          <span class="dim">{loopsAround}</span>
        {/if}
      </div>
      {#if loopsOpen}
        {#if !loops}
          <p class="dim">Asking the brain…</p>
        {:else}
          <ul aria-label="Generated loops">
            {#each loops.generated as l (l.id)}
              <li class:on={pickedLoop === l.id}>
                <button
                  class="pick"
                  aria-pressed={pickedLoop === l.id}
                  title={l.stops.map((s, i) => `${i + 1}. ${s.name}${s.arena ? " (arena)" : s.lair ? " (lair)" : ""}`).join("\n")}
                  onclick={() => pickLoop(l.id)}
                  onpointerenter={() => (hoverLoop = l.id)}
                  onpointerleave={() => (hoverLoop = null)}
                  onfocus={() => (hoverLoop = l.id)}
                  onblur={() => (hoverLoop = null)}>{l.name}</button
                >
                <span class="dim">{loopText(l)}</span>
                <button class="walk" onclick={() => session && onwalkloop?.(session.id, l.id)}>walk once</button>
              </li>
            {:else}
              <li class="dim">No hunt rooms in reach.</li>
            {/each}
          </ul>
          {#if loops.megamud.length > 0}
            <div class="dim">MegaMud, verified</div>
            <ul aria-label="MegaMud loops">
              {#each loops.megamud as m (m.file)}
                <li class:on={pickedLoop === `mm:${m.file}`}>
                  <button
                    class="pick"
                    aria-pressed={pickedLoop === `mm:${m.file}`}
                    onclick={() => pickLoop(`mm:${m.file}`)}
                    onpointerenter={() => (hoverLoop = `mm:${m.file}`)}
                    onpointerleave={() => (hoverLoop = null)}
                    onfocus={() => (hoverLoop = `mm:${m.file}`)}
                    onblur={() => (hoverLoop = null)}>{m.file}</button
                  >
                  <span class="dim">{m.description}, {m.steps} steps</span>
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
      {/if}
    </div>
    <div class="bar" aria-label="Walk">
      {#if walking}
        <span class="to">→ {walking.name}</span>
        <span class="dim">step {walking.step}, {walking.steps_left} left</span>
        <button class="stop" onclick={() => onstop(session.id)}>stop</button>
        <div class="route">{route || "…"}</div>
      {:else}
        <span class="dim">Click a room to walk there.</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .map {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .msg {
    margin: 1em 1ch;
    color: var(--vga-darkgray);
  }
  .tools,
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1ch;
    padding: 2px 1ch;
  }
  .bar,
  .loops {
    border-top: 1px solid var(--frame);
  }
  .loops {
    padding: 2px 1ch;
    max-height: 40%;
    overflow-y: auto;
  }
  .loops .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1ch;
  }
  .loops ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .loops li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1ch;
  }
  .loops p {
    margin: 0;
  }
  .loops .toggle,
  .loops .pick {
    color: var(--vga-yellow);
    background: none;
    padding: 0;
  }
  .loops li.on .pick {
    color: #000;
    background: var(--vga-yellow);
    padding: 0 1ch;
  }
  .loops .toggle {
    color: var(--vga-lightgray);
  }
  label {
    display: flex;
    align-items: center;
    gap: 0.5ch;
  }
  .canvas {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  canvas {
    position: absolute;
    inset: 0;
    display: block;
    cursor: pointer;
    touch-action: none;
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
  .stop {
    background: var(--vga-lightred);
  }
  .level {
    color: var(--vga-white);
  }
  .maybe {
    color: var(--vga-yellow);
  }
  .to {
    color: var(--vga-lightgreen);
  }
  .route {
    flex-basis: 100%;
    color: var(--vga-white);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tip {
    position: absolute;
    pointer-events: none;
    padding: 0 1ch;
    max-width: 40ch;
    color: var(--vga-lightgray);
    background: var(--vga-blue);
    border: 1px solid var(--vga-lightcyan);
    white-space: nowrap;
    z-index: 1;
  }
  .tip .name {
    color: var(--vga-white);
  }
  .tip div {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dim {
    color: var(--vga-darkgray);
  }
  code {
    color: var(--vga-lightgray);
  }
</style>
