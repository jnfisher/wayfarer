<script lang="ts" module>
  /** The last Alt+G the go box answered, kept while the panel is unmounted (another tab). */
  let handledGo = 0;
  /** How many loops the list shows, and how long after the last ask a move of the character asks again. */
  const LOOPS_SHOWN = 12;
  const LOOPS_REFRESH_MS = 5000;
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import {
    carryPan,
    DEFAULT_ZOOM,
    LOOP_RADIUS,
    describeRoom,
    drawMap,
    insertStops,
    keyOf,
    loopText,
    roomAt,
    stopTip,
    walkText,
    zoomAt,
    ZOOMS,
    type Loops,
    type MapView,
    type StagedStop,
    type ViewState,
  } from "./lib/map";
  import { centreOn, FIND_DEBOUNCE_MS, goRows, MIN_QUERY, moveChoice, type FoundMessage, type GoRow } from "./lib/goto";
  import { FONT_FAMILY } from "./lib/theme";
  import type { CharacterState, GeneratedLoop, LoopStop, MapRoom, MegaMudLoop, RoomKey, SessionInfo } from "./proto";

  interface Props {
    /** The focused tile's session, which the panel follows. */
    session: SessionInfo | undefined;
    /** The brain's last `map_view` for it. */
    view: MapView | undefined;
    character: CharacterState | undefined;
    attached: boolean | null;
    /** Walk there; `stopShort` false walks into a boss lair (left out: the brain's default). */
    onwalk: (session: string, map: number, room: number, stopShort?: boolean) => void;
    onstop: (session: string) => void;
    /** The brain's last `loops` for it. */
    loops?: Loops;
    /** Ask for the nearest loops, at most `limit`, those matching `query` if given (`radius`:
     * how far the generated ones are made around the character). */
    onloops?: (session: string, radius?: number, query?: string, limit?: number) => void;
    /** Walk a loop once (a generated loop's id, or a MegaMud loop's file). */
    onwalkloop?: (session: string, id: string) => void;
    /** Run a loop (by id or file), passing by the stops in `skip` (the unticked ones). */
    onrunloop?: (session: string, id: string, skip: RoomKey[]) => void;
    /** Keep a loop of the player's own: a name, the stops in order, the loop it was made from. */
    onsaveloop?: (session: string, name: string, base: string | undefined, stops: RoomKey[]) => void;
    /** Delete a loop of the player's own (`custom:...`). */
    ondeleteloop?: (session: string, id: string) => void;
    /** The brain's last `found` for it. */
    found?: FoundMessage;
    /** Find rooms, places and monsters by part of a name. */
    onfind?: (session: string, query: string) => void;
    /** Bumped by Alt+G: open the go box and focus it. */
    goAsked?: number;
  }

  let {
    session,
    view,
    character,
    attached,
    onwalk,
    onstop,
    loops,
    onloops,
    onwalkloop,
    onrunloop,
    onsaveloop,
    ondeleteloop,
    found,
    onfind,
    goAsked = 0,
  }: Props = $props();

  // The go box: open or not, what is typed, the row chosen, the results opened onto their
  // rooms, and the boss-lair rows whose "stop short" was unticked.
  let goOpen = $state(false);
  let query = $state("");
  let chosen = $state(-1);
  let expanded = $state.raw<Set<number>>(new Set());
  let walkIn = $state.raw<Set<string>>(new Set());
  let goInput = $state<HTMLInputElement>();
  let findTimer: ReturnType<typeof setTimeout> | undefined;

  const results = $derived(
    query.trim().length >= MIN_QUERY && found && found.session === session?.id ? found.results : [],
  );
  const rows = $derived(goRows(results, expanded));

  // Other results shown: the first row chosen, nothing opened or unticked.
  $effect(() => {
    void results;
    untrack(() => {
      chosen = rows.length > 0 ? 0 : -1;
      expanded = new Set();
      walkIn = new Set();
    });
  });

  // Alt+G: open the box and put the cursor in it.
  $effect(() => {
    if (goAsked <= handledGo) return;
    handledGo = goAsked;
    untrack(() => {
      goOpen = true;
    });
    queueMicrotask(() => goInput?.focus());
  });

  function toggleGo() {
    goOpen = !goOpen;
    if (goOpen) queueMicrotask(() => goInput?.focus());
  }

  function oninput() {
    clearTimeout(findTimer);
    const q = query.trim();
    if (!session || q.length < MIN_QUERY) return;
    const id = session.id;
    findTimer = setTimeout(() => onfind?.(id, q), FIND_DEBOUNCE_MS);
  }

  function choose(i: number) {
    chosen = i;
    const c = centreOn(view, zoom, rows[i]?.room);
    if (c) {
      ({ panX, panY, level } = c);
      moved = true;
    }
  }

  function go(row: GoRow | undefined) {
    const r = row?.room;
    if (!r || !session) return;
    onwalk(session.id, r.map, r.room, r.boss ? !walkIn.has(row.id) : undefined);
  }

  function toggleOpen(i: number) {
    const next = new Set(expanded);
    if (!next.delete(i)) next.add(i);
    expanded = next;
  }

  function toggleShort(id: string) {
    const next = new Set(walkIn);
    if (!next.delete(id)) next.add(id);
    walkIn = next;
  }

  function ongokeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      choose(moveChoice(rows, chosen, e.key === "ArrowDown" ? 1 : -1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      go(rows[chosen] ?? rows[0]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      clearTimeout(findTimer);
      if (query) query = "";
      else goOpen = false;
    }
  }

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
  // The loops list: open or not, the loop hovered and the one picked
  // (a generated loop's id, or `mm:` and a MegaMud file).
  let loopsOpen = $state(false);
  let hoverLoop = $state<string | null>(null);
  let pickedLoop = $state<string | null>(null);

  /** A listed loop as the map draws it: its path and its stops. */
  function loopShape(id: string | null) {
    if (!id || !loops) return null;
    const g = loops.generated.find((l) => l.id === id);
    if (g) return { path: g.path, stops: g.stops.map((s) => ({ map: s.map, room: s.room })) };
    const c = (loops.custom ?? []).find((l) => l.id === id);
    if (c) return { path: c.path, stops: c.stops.map((s) => ({ map: s.map, room: s.room })) };
    const m = loops.megamud.find((l) => `mm:${l.file}` === id);
    return m ? { path: m.path, stops: m.stops.map((s) => ({ map: s.map, room: s.room })) } : null;
  }

  /** The picked loop as a base for one of the player's own: the id `save_loop` records and its stops. */
  const base = $derived.by(() => {
    if (!pickedLoop || !loops) return null;
    const g = loops.generated.find((l) => l.id === pickedLoop) ?? (loops.custom ?? []).find((l) => l.id === pickedLoop);
    if (g) return { id: g.id, name: g.name, stops: g.stops };
    const m = loops.megamud.find((l) => `mm:${l.file}` === pickedLoop);
    return m ? { id: m.file, name: m.description || m.file, stops: m.stops } : null;
  });

  // Making a loop from the picked one: its name, and the rooms staged to go in (from the go
  // box), each after a stop.
  let newName = $state("");
  let staged = $state<StagedStop[]>([]);

  function stage(row: GoRow) {
    const r = row.room;
    if (!r) return;
    staged = [...staged, { map: r.map, room: r.room, name: row.text, after: null }];
  }

  function saveMine() {
    if (!session || !base) return;
    onsaveloop?.(session.id, newName, base.id, insertStops(base.stops, staged));
    staged = [];
  }
  const drawnLoop = $derived(loopShape(hoverLoop ?? pickedLoop));

  /** A loop `run_loop` names: a generated loop by its id, a MegaMud loop by its file. */
  type Runnable = Pick<GeneratedLoop, "id" | "name" | "stops">;
  function mmRunnable(m: MegaMudLoop): Runnable {
    return { id: m.file, name: m.description || m.file, stops: m.stops };
  }

  // Running a loop: the bot's task, while it is not stopped. Its stops' ticks are the brain's;
  // another loop's are kept here until it is run.
  const task = $derived(character?.bot && character.bot.state !== "stopped" ? character.bot.task : undefined);
  let unticked = $state<Record<string, string[]>>({});

  function skipped(l: Runnable): string[] {
    return task?.loop_id === l.id ? task.skip.map(keyOf) : (unticked[l.id] ?? []);
  }

  function skipOf(l: Runnable, keys: string[]): RoomKey[] {
    return l.stops.filter((s) => keys.includes(keyOf(s))).map((s) => ({ map: s.map, room: s.room }));
  }

  /** Tick or untick a stop: for the loop the bot runs, the brain is told at once. */
  function toggleStop(l: Runnable, s: LoopStop) {
    const k = keyOf(s);
    const was = skipped(l);
    const now = was.includes(k) ? was.filter((x) => x !== k) : [...was, k];
    unticked = { ...unticked, [l.id]: now };
    if (session && task?.loop_id === l.id) onrunloop?.(session.id, l.id, skipOf(l, now));
  }

  /** A loop's tooltip: its stops in order. */
  function stopsTip(stops: LoopStop[]): string {
    return stops.map((s, i) => `${i + 1}. ${s.name}${s.arena ? " (arena)" : s.lair ? " (lair)" : ""}`).join("\n");
  }

  function runLoop(l: Runnable) {
    if (session) onrunloop?.(session.id, l.id, skipOf(l, skipped(l)));
  }
  const loopsAround = $derived.by(() => {
    if (!loops) return "";
    const a = loops.anchor;
    const r = view?.rooms.find((x) => keyOf(x) === keyOf(a));
    return `${loops.radius} steps around ${r?.name || `${a.map}:${a.room}`}`;
  });

  // The list is the nearest loops, from the character's room, nearest first: the ones that
  // match what is typed, from anywhere the character can walk to. The brain does the finding.
  let loopQuery = $state("");
  let loopTimer: ReturnType<typeof setTimeout> | undefined;
  let lastAsked = 0;

  /** The loops of the brain's last answer, nearest first (generated, the player's own and MegaMud's together). */
  type Entry = { key: string; kind: "generated" | "custom" | "megamud"; distance: number; g?: GeneratedLoop; m?: MegaMudLoop };
  const entries = $derived.by((): Entry[] => {
    if (!loops || loops.session !== session?.id) return [];
    const far = Number.MAX_SAFE_INTEGER;
    const all: Entry[] = [
      ...loops.generated.map((g): Entry => ({ key: g.id, kind: "generated", distance: g.distance ?? far, g })),
      ...(loops.custom ?? []).map((g): Entry => ({ key: g.id, kind: "custom", distance: g.distance ?? far, g })),
      ...loops.megamud.map((m): Entry => ({ key: `mm:${m.file}`, kind: "megamud", distance: m.distance ?? far, m })),
    ];
    const name = (e: Entry) => (e.g?.name ?? e.m?.description ?? "").toLowerCase();
    return all.sort((x, y) => x.distance - y.distance || name(x).localeCompare(name(y)));
  });

  function askLoops() {
    lastAsked = Date.now();
    if (session) onloops?.(session.id, LOOP_RADIUS, loopQuery.trim() || undefined, LOOPS_SHOWN);
  }

  /** Ask after a pause in the typing. */
  function askLoopsSoon(ms = FIND_DEBOUNCE_MS) {
    clearTimeout(loopTimer);
    loopTimer = setTimeout(askLoops, ms);
  }

  // Opening the list asks again, so it is around where the character is now.
  function toggleLoops() {
    loopsOpen = !loopsOpen;
    if (loopsOpen) askLoops();
  }

  function onloopkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && loopQuery) {
      e.preventDefault();
      e.stopPropagation();
      loopQuery = "";
      askLoopsSoon(0);
    }
  }

  // The character moves on while the list is open and standing still: the nearest are not the
  // same, so ask again (not more than every few seconds).
  $effect(() => {
    const room = position?.room;
    if (!loopsOpen || room === undefined || walking) return;
    untrack(() => {
      if (Date.now() - lastAsked > LOOPS_REFRESH_MS) askLoopsSoon(1500);
    });
  });

  /** Pick a loop to keep it drawn (again to let it go), showing the floor of its first stop. */
  function pickLoop(id: string) {
    pickedLoop = pickedLoop === id ? null : id;
    staged = [];
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

<!-- A loop's run button (or that it is running), and its stops to tick while picked or running. -->
{#snippet runAndStops(l: Runnable, picked: boolean)}
  {#if task?.loop_id === l.id}
    <span class="running">running: stop {task.stop + 1}/{task.stops.length}</span>
  {:else}
    <button class="run" onclick={() => runLoop(l)} title="Hunt round this loop: fight at each ticked stop">run</button>
  {/if}
  {#if picked || task?.loop_id === l.id}
    <ul class="stops" aria-label={`Stops of ${l.name}`}>
      {#each l.stops as s, i (keyOf(s))}
        <li class:at={task?.loop_id === l.id && task.stop === i}>
          <label title={stopTip(s)}>
            <input type="checkbox" checked={!skipped(l).includes(keyOf(s))} onchange={() => toggleStop(l, s)} />
            {i + 1}. {s.name}
          </label>
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

<div class="map">
  {#if !attached}
    <p class="msg">No brain attached: the map needs one (<code>wayfarer brain</code>).</p>
  {:else if !session}
    <p class="msg">No character.</p>
  {:else if !known}
    <p class="msg">position unknown: re-sync or look</p>
  {:else}
    <div class="tools">
      <button class="gobutton" aria-expanded={goOpen} onclick={toggleGo} title="Go to a room, a place or a monster by name (Alt+G)"
        >go</button
      >
      <span class="dim">level</span>
      <button aria-label="Level down" disabled={levels.indexOf(level) <= 0} onclick={() => changeLevel(-1)}>▼</button>
      <button class="gobutton" aria-expanded={loopsOpen} onclick={toggleLoops} title="Find a loop to run, near the character or anywhere you search">loops {loopsOpen ? "▾" : "▸"}</button>
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
    {#if goOpen}
      <div class="go" aria-label="Go to">
        <input
          bind:this={goInput}
          bind:value={query}
          {oninput}
          onkeydown={ongokeydown}
          placeholder="go to: part of a name (woo, chief, skali)"
          aria-label="Go to"
          spellcheck="false"
          autocomplete="off"
        />
        {#if query.trim().length >= MIN_QUERY}
          {#if !found || found.session !== session.id || found.query !== query.trim()}
            {#if rows.length === 0}<p class="dim">Asking the brain…</p>{/if}
          {:else if rows.length === 0}
            <p class="dim">Nothing by that name.</p>
          {/if}
          {#if rows.length > 0}
            <ul aria-label="Found">
              {#each rows as row, i (row.id)}
                <li class:sub={row.sub} class:on={chosen === i} class:none={!row.room}>
                  {#if row.expandable}
                    <button
                      class="open"
                      aria-label={expanded.has(row.result) ? "Close the rooms" : "Open the rooms"}
                      aria-expanded={expanded.has(row.result)}
                      onclick={() => toggleOpen(row.result)}>{expanded.has(row.result) ? "▾" : "▸"}</button
                    >
                  {/if}
                  <button class="row" aria-pressed={chosen === i} onclick={() => choose(i)} ondblclick={() => go(row)}
                    >{row.text}</button
                  >
                  {#if row.room?.boss}
                    <label title="Stop in the room before the lair, to look before going in">
                      <input type="checkbox" checked={!walkIn.has(row.id)} onchange={() => toggleShort(row.id)} /> stop short
                    </label>
                  {/if}
                  {#if chosen === i && row.room}
                    <button class="walk" onclick={() => go(row)}>go</button>
                    {#if base}
                      <button class="walk" onclick={() => stage(row)} title={`Put this room in a loop made from ${base.name}`}
                        >add to loop</button
                      >
                    {/if}
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
      </div>
    {/if}
    <div class="loops" aria-label="Loops">
      <div class="head">
        <input
          type="search"
          class="filter"
          placeholder="find a loop by name, file, author or stop…"
          aria-label="Find a loop"
          bind:value={loopQuery}
          oninput={() => askLoopsSoon()}
          onkeydown={onloopkeydown}
        />
      </div>
      {#if loopsOpen}
        {#if !loops || loops.session !== session?.id}
          <p class="dim">Asking the brain…</p>
        {:else}
          <div class="dim around">
            {#if loopQuery.trim()}
              {entries.length} nearest of {loops.matches} matching “{loopQuery.trim()}”
            {:else}
              the {entries.length} nearest of {loops.matches} loops
            {/if}
            · nearest first, from {loopsAround}
          </div>
          <ul aria-label="Loops">
            {#each entries as e (e.key)}
              {@const l = e.g}
              {@const m = e.m}
              <li class:on={pickedLoop === e.key}>
                <span class="dist" title="Steps from the character to the nearest room of this loop">{e.distance === Number.MAX_SAFE_INTEGER ? "?" : e.distance}</span>
                <button
                  class="pick"
                  aria-pressed={pickedLoop === e.key}
                  title={stopsTip((l ?? m)!.stops)}
                  onclick={() => pickLoop(e.key)}
                  onpointerenter={() => (hoverLoop = e.key)}
                  onpointerleave={() => (hoverLoop = null)}
                  onfocus={() => (hoverLoop = e.key)}
                  onblur={() => (hoverLoop = null)}>{l ? l.name : m!.file}</button
                >
                {#if l}
                  <span class="dim">{e.kind === "custom" ? "mine, " : ""}{loopText(l)}</span>
                  <button class="walk" onclick={() => session && onwalkloop?.(session.id, l.id)}>walk once</button>
                  {@render runAndStops(l, pickedLoop === e.key)}
                  {#if e.kind === "custom"}
                    <button class="walk" title="Forget this loop" onclick={() => session && ondeleteloop?.(session.id, l.id)}
                      >delete</button
                    >
                  {/if}
                {:else if m}
                  <span class="dim">{m.description}, {m.stops.length} stops, {m.steps} steps</span>
                  <button class="walk" onclick={() => session && onwalkloop?.(session.id, m.file)}>walk once</button>
                  {#if m.runnable}
                    {@render runAndStops(mmRunnable(m), pickedLoop === e.key)}
                  {:else}
                    <button class="run" disabled title={m.reason}>run</button>
                    <span class="dim reason">{m.reason}</span>
                  {/if}
                {/if}
              </li>
            {:else}
              <li class="dim">
                {#if loopQuery.trim()}
                  No loop matches “{loopQuery.trim()}”.
                {:else}
                  No loops in reach.
                {/if}
              </li>
            {/each}
          </ul>
          {#if base}
            <div class="make" aria-label="Make a loop from {base.name}">
              <span class="dim">Make a loop from {base.name}:</span>
              <input bind:value={newName} placeholder="name for your loop" aria-label="Name for your loop" spellcheck="false" />
              {#each staged as s, i (i)}
                <div class="staged">
                  + {s.name}
                  <label>
                    after stop
                    <input
                      type="number"
                      min="0"
                      max={base.stops.length}
                      placeholder="end"
                      value={s.after ?? ""}
                      onchange={(e) => {
                        const v = e.currentTarget.value;
                        staged = staged.map((x, j) => (j === i ? { ...x, after: v === "" ? null : Number(v) } : x));
                      }}
                    />
                  </label>
                  <button class="walk" onclick={() => (staged = staged.filter((_, j) => j !== i))}>remove</button>
                </div>
              {/each}
              <button class="run" disabled={newName.trim() === ""} onclick={saveMine}
                >save as my loop{staged.length > 0 ? ` (+${staged.length})` : ""}</button
              >
              <span class="dim">Rooms to add: find them in the go box, then “add to loop”.</span>
            </div>
          {/if}
        {/if}
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
    <div class="bar" aria-label="Walk">
      {#if walking}
        <span class="to">→ {walking.name}</span>
        {#if walking.short_of}
          <span class="short">one room short of {walking.short_of.name}{walking.short_of.bosses.length
              ? ` (boss: ${walking.short_of.bosses.join(", ")})`
              : ""}</span
          >
        {/if}
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
    border-bottom: 1px solid var(--frame);
  }
  .go {
    padding: 2px 1ch;
    max-height: 40%;
    overflow-y: auto;
    border-bottom: 1px solid var(--frame);
  }
  .go input:not([type]) {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    color: var(--vga-white);
    background: var(--vga-black);
    border: 1px solid var(--vga-darkgray);
  }
  .go ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .go li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1ch;
  }
  .go li.sub {
    padding-left: 2ch;
  }
  .go p {
    margin: 0;
  }
  .go .row,
  .go .open {
    color: var(--vga-lightgray);
    background: none;
    padding: 0;
    text-align: left;
  }
  .go li.none .row {
    color: var(--vga-darkgray);
  }
  .go li.on .row {
    color: #000;
    background: var(--vga-lightcyan);
    padding: 0 1ch;
  }
  .go .walk {
    background: var(--vga-lightgreen);
  }
  .loops {
    padding: 2px 1ch;
    max-height: 45%;
    overflow-y: auto;
  }
  .loops .filter {
    flex: 1 1 18ch;
    min-width: 12ch;
    font: inherit;
    color: var(--vga-lightgray);
    background: #000;
    border: 1px solid var(--frame);
  }
  .loops .around {
    padding: 2px 0;
  }
  .loops .dist {
    min-width: 3ch;
    text-align: right;
    color: var(--vga-lightgreen);
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
  .loops .run {
    background: var(--vga-lightgreen);
  }
  .loops .run:disabled {
    background: var(--vga-darkgray);
    cursor: default;
  }
  .loops .running {
    color: var(--vga-lightgreen);
  }
  .loops ul.stops {
    flex-basis: 100%;
    padding-left: 2ch;
  }
  .loops .make {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5ch 1ch;
    padding: 2px 0;
  }
  .loops .make .staged {
    flex-basis: 100%;
  }
  .loops ul.stops li.at {
    color: var(--vga-white);
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
  .short {
    color: var(--vga-yellow);
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
