<script lang="ts">
  // The gear planner (C-060, docs/web-ui.md "Gear planner"): full screen over the tiles, every
  // weapon the character could use scored by attack, bash and smash against a monster set.
  // The planner's inputs live here; the brain's answers come in through App.svelte.
  import {
    GEAR_DEBOUNCE_MS,
    METRICS,
    MOVES,
    WHAT_IF_KEYS,
    WHAT_IF_LABELS,
    DEFAULT_SORT,
    blowsText,
    defaultFilters,
    detailRequest,
    drText,
    emptyInputs,
    isBestMove,
    liveValues,
    loadBuffTicks,
    mayKeepSmash,
    missingText,
    moveScore,
    nextSort,
    pct,
    planRequest,
    rangeText,
    roundsText,
    saveBuffTicks,
    sourceText,
    sourcesTitle,
    statusClass,
    statusText,
    strText,
    tenths,
    visibleRows,
    type GearDetailMessage,
    type GearPlanMessage,
    type Metric,
    type PlannerInputs,
    type RowFilters,
    type Sort,
    type SortKey,
    type UseFilter,
    type WhatIfKey,
  } from "./lib/gear";
  import type { CharacterState, GearDetailRequest, GearMove, GearPlanRequest, SessionInfo } from "./proto";

  interface Props {
    sessions: SessionInfo[];
    /** The character it opens on: the focused tile's. */
    initial: string | undefined;
    characters: Record<string, CharacterState>;
    attached: boolean | null;
    /** The brain's last `gear_plan` and `gear_detail` per session. */
    plans: Record<string, GearPlanMessage>;
    details: Record<string, GearDetailMessage>;
    /** Why the brain gave no plan, per session. */
    refusals: Record<string, string>;
    storage?: Pick<Storage, "getItem" | "setItem">;
    onplan: (req: GearPlanRequest) => void;
    ondetail: (req: GearDetailRequest) => void;
    onclose: () => void;
  }

  let { sessions, initial, characters, attached, plans, details, refusals, storage, onplan, ondetail, onclose }: Props =
    $props();

  // Opened on the focused character; the picker switches it from there.
  // svelte-ignore state_referenced_locally
  const first = initial ?? sessions[0]?.id;
  // svelte-ignore state_referenced_locally
  let ticks = $state(loadBuffTicks(storage));
  let sessionId = $state(first);
  let inputs = $state<PlannerInputs>(withTicks(emptyInputs(), first));
  let filters = $state<RowFilters>(defaultFilters());
  let maxGold = $state("");
  let sort = $state<Sort>(DEFAULT_SORT);
  let openItem = $state<number | undefined>(undefined);

  function withTicks(i: PlannerInputs, id: string | undefined): PlannerInputs {
    return { ...i, buffs: id ? [...(ticks[id] ?? [])] : [] };
  }

  /** Another character: its own ticks, the what-ifs cleared. */
  function pick(id: string) {
    sessionId = id;
    inputs = withTicks(emptyInputs(), id);
    openItem = undefined;
  }

  const character = $derived(sessionId ? characters[sessionId] : undefined);
  const plan = $derived(sessionId ? plans[sessionId] : undefined);
  const refusal = $derived(sessionId ? refusals[sessionId] : undefined);
  const live = $derived(liveValues(character, plan));
  const offered = $derived(plan?.buffs_offered.map((b) => b.name));
  const showSmash = $derived(mayKeepSmash(plan?.inputs.class ?? character?.class));
  const missing = $derived(missingText(plan?.missing));
  const rows = $derived(plan ? visibleRows(plan.rows, filters, sort) : []);
  const rowNote = $derived(plan?.rows.find((r) => r.note)?.note);
  const moves = $derived(MOVES.filter((m) => m !== "smash" || (showSmash && inputs.smash)));
  const detail = $derived(sessionId && openItem !== undefined ? details[sessionId] : undefined);

  // Asked for on open and on every change of an input, a moment after the last.
  const request = $derived(
    sessionId ? planRequest(sessionId, { ...inputs, smash: inputs.smash && showSmash }, live, offered) : undefined,
  );
  // Keyed by its text: an answer re-derives an equal request, which is not asked again (until
  // the brain leaves and comes back).
  const requestKey = $derived(request ? JSON.stringify(request) : "");
  let planSent = "";
  $effect(() => {
    const key = requestKey;
    if (!attached) planSent = "";
    if (!key || !attached || key === planSent) return;
    const timer = setTimeout(() => {
      planSent = key;
      onplan(JSON.parse(key) as GearPlanRequest);
    }, GEAR_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });
  const detailKey = $derived(
    sessionId && openItem !== undefined && request
      ? JSON.stringify(detailRequest(sessionId, openItem, { ...inputs, smash: request.smash ?? false }, live, offered))
      : "",
  );
  let detailSent = "";
  $effect(() => {
    const key = detailKey;
    if (!attached) detailSent = "";
    if (!key || !attached || key === detailSent) return;
    const timer = setTimeout(() => {
      detailSent = key;
      ondetail(JSON.parse(key) as GearDetailRequest);
    }, GEAR_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });

  function setWhatIf(k: WhatIfKey, text: string) {
    const v = Number(text);
    const next = { ...inputs.whatIf };
    if (text.trim() === "" || !Number.isFinite(v)) delete next[k];
    else next[k] = v;
    inputs.whatIf = next;
  }
  const changed = (k: WhatIfKey) => inputs.whatIf[k] !== undefined && inputs.whatIf[k] !== live[k];

  function toggleBuff(name: string, on: boolean) {
    const id = sessionId;
    if (!id) return;
    const now = new Set(inputs.buffs);
    if (on) now.add(name);
    else now.delete(name);
    inputs.buffs = [...now].sort();
    ticks = { ...ticks, [id]: inputs.buffs };
    saveBuffTicks(storage, ticks);
  }

  function setMaxGold(text: string) {
    maxGold = text;
    const v = Number(text);
    filters.maxPrice = text.trim() !== "" && Number.isFinite(v) && v >= 0 ? Math.round(v * 100) : undefined;
  }

  function sortBy(key: SortKey) {
    sort = nextSort(sort, key);
  }
  const arrow = (key: SortKey) => (sort.key === key ? (sort.dir === 1 ? "▲" : "▼") : "");

  const COLUMNS: { key: SortKey; label: string; title: string }[] = [
    { key: "name", label: "weapon", title: "Its name" },
    { key: "damage", label: "dmg", title: "Its damage, low to high" },
    { key: "speed", label: "speed", title: "Its speed: the energy a blow costs before the character's own" },
    { key: "str", label: "Str", title: "The Str it needs; red with the shortfall when short" },
    { key: "level", label: "lvl", title: "The level it may be used from" },
    { key: "hands", label: "hands", title: "One- or two-handed" },
    { key: "status", label: "use", title: "Now, or why not" },
    { key: "source", label: "source", title: "The cheapest shop, else the best drop, else quest or placed" },
  ];
  const METRIC_LABELS: Record<Metric, { label: string; title: string }> = {
    kill1: { label: "k1", title: "The chance of a kill in round 1" },
    kill2: { label: "k2", title: "The chance of a kill by round 2" },
    rounds: { label: "rds", title: "The rounds expected to the kill (20+: may run past 20)" },
    dmg1: { label: "dmg1", title: "The damage expected in round 1" },
  };
  const MOVE_NAMES: Record<GearMove, string> = { attack: "Attack", bash: "Bash", smash: "Smash" };
  const USE_FILTERS: { value: UseFilter; label: string }[] = [
    { value: "now", label: "usable now" },
    { value: "usable", label: "now or later" },
    { value: "all", label: "everything" },
  ];
  const columnCount = $derived(COLUMNS.length + moves.length * METRICS.length);
  const metricText = (s: ReturnType<typeof moveScore>, m: Metric) =>
    !s
      ? ""
      : m === "kill1"
        ? pct(s.kill_round1)
        : m === "kill2"
          ? pct(s.kill_round2)
          : m === "rounds"
            ? roundsText(s.rounds, s.rounds_capped)
            : tenths(s.damage_round1);
  const monsterName = (n: number) => detail?.monsters.find((m) => m.number === n)?.name ?? `#${n}`;
</script>

<div class="planner" role="dialog" aria-label="Gear planner">
  <header>
    <div class="top">
      <span class="tabs"><span class="tab on">weapons</span></span>
      <label>
        character
        <select value={sessionId} onchange={(e) => pick(e.currentTarget.value)} aria-label="Character">
          {#each sessions as s (s.id)}
            <option value={s.id}>{s.name}</option>
          {/each}
        </select>
      </label>
      {#if plan?.inputs.class}
        <span class="dim">{plan.inputs.race ?? ""} {plan.inputs.class}{plan.inputs.readied ? `, readied: ${plan.inputs.readied}` : ""}</span>
      {/if}
      <button class="ctl close" onclick={onclose} title="Close (Esc, Alt+E)">[x] Esc</button>
    </div>

    <div class="line" role="group" aria-label="Monsters">
      <span class="head">monsters</span>
      <label
        ><input
          type="radio"
          name="gear-set"
          checked={inputs.monsters.kind === "loop"}
          onchange={() => (inputs.monsters = { kind: "loop" })}
        /> its loop</label
      >
      <label
        ><input
          type="radio"
          name="gear-set"
          checked={inputs.monsters.kind === "area"}
          onchange={() => (inputs.monsters = { kind: "area", area: "" })}
        /> an area</label
      >
      {#if inputs.monsters.kind === "area"}
        <input
          class="text"
          list="gear-areas"
          placeholder="area name"
          aria-label="Area"
          spellcheck="false"
          value={inputs.monsters.area}
          oninput={(e) => (inputs.monsters = { kind: "area", area: e.currentTarget.value })}
        />
        <datalist id="gear-areas">
          {#each plan?.areas ?? [] as a (a)}
            <option value={a}></option>
          {/each}
        </datalist>
      {/if}
      <label
        ><input
          type="radio"
          name="gear-set"
          checked={inputs.monsters.kind === "names"}
          onchange={() => (inputs.monsters = { kind: "names", names: "" })}
        /> names</label
      >
      {#if inputs.monsters.kind === "names"}
        <input
          class="text wide"
          placeholder="minotaur, minotaur champion"
          aria-label="Monster names"
          spellcheck="false"
          value={inputs.monsters.names}
          oninput={(e) => (inputs.monsters = { kind: "names", names: e.currentTarget.value })}
        />
      {/if}
    </div>

    <div class="line" role="group" aria-label="What if">
      <span class="head">what if</span>
      {#each WHAT_IF_KEYS as k (k)}
        <label class:changed={changed(k)}>
          {WHAT_IF_LABELS[k]}
          <input
            class="num"
            type="number"
            min="0"
            aria-label={WHAT_IF_LABELS[k]}
            value={inputs.whatIf[k] ?? live[k] ?? ""}
            oninput={(e) => setWhatIf(k, e.currentTarget.value)}
          />
        </label>
      {/each}
      <span class="dim">(live load {live.load_percent !== undefined ? `${live.load_percent}%` : "?"})</span>
      <button class="ctl" onclick={() => (inputs.whatIf = {})} disabled={Object.keys(inputs.whatIf).length === 0}
        >live</button
      >
    </div>

    {#if (plan?.buffs_offered.length ?? 0) > 0 || showSmash}
      <div class="line" role="group" aria-label="Buffs">
        <span class="head">buffs</span>
        {#each plan?.buffs_offered ?? [] as b (b.name)}
          <label title={b.classes.join(", ")}>
            <input type="checkbox" checked={inputs.buffs.includes(b.name)} onchange={(e) => toggleBuff(b.name, e.currentTarget.checked)} />
            {b.effect}
          </label>
        {/each}
        {#if showSmash}
          <label class="smash">
            <input type="checkbox" bind:checked={inputs.smash} /> score smash (as if it had it)
          </label>
        {/if}
      </div>
    {/if}

    <div class="line" role="group" aria-label="Filters">
      <span class="head">show</span>
      {#each USE_FILTERS as f (f.value)}
        <label><input type="radio" name="gear-use" checked={filters.use === f.value} onchange={() => (filters.use = f.value)} /> {f.label}</label>
      {/each}
      <label><input type="checkbox" bind:checked={filters.hasSource} /> has a source</label>
      <label>
        max price
        <input class="num" type="number" min="0" placeholder="gold" aria-label="Maximum price in gold" value={maxGold} oninput={(e) => setMaxGold(e.currentTarget.value)} />
        gold
      </label>
    </div>

    {#if plan && plan.monsters.length > 0}
      <table class="monsters" aria-label="The monster set">
        <thead><tr><th>monster</th><th>HP</th><th>AC</th><th>DR</th><th>dodge</th><th>rooms</th></tr></thead>
        <tbody>
          {#each plan.monsters as m (m.number)}
            <tr>
              <td>{m.name}{m.boss ? " (boss)" : ""}</td>
              <td>{m.hp}</td>
              <td>{m.ac}</td>
              <td>{drText(m.dr)}</td>
              <td>{m.dodge}</td>
              <td>{m.rooms || ""}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#each [...(plan?.notes ?? []), ...(rowNote ? [rowNote] : [])] as n (n)}
      <p class="note">{n}</p>
    {/each}
  </header>

  <div class="body">
    {#if !sessionId}
      <p class="say">No character.</p>
    {:else if !attached}
      <p class="say">No brain attached: no gear plan.</p>
    {:else if refusal}
      <p class="say">{refusal}</p>
    {:else if !plan}
      <p class="say">Asking the brain for the plan…</p>
    {:else if missing}
      <p class="say">{missing}</p>
    {:else}
      <table class="rows" aria-label="Weapons">
        <thead>
          <tr class="groups">
            <th colspan={COLUMNS.length}></th>
            {#each moves as mv (mv)}
              <th colspan={METRICS.length} class="move">{MOVE_NAMES[mv]}</th>
            {/each}
          </tr>
          <tr>
            {#each COLUMNS as c (c.key)}
              <th><button class="sort" title={c.title} onclick={() => sortBy(c.key)}>{c.label}{arrow(c.key)}</button></th>
            {/each}
            {#each moves as mv (mv)}
              {#each METRICS as m, i (m)}
                {@const key = `${mv}.${m}` as SortKey}
                <th class:first={i === 0}>
                  <button class="sort" title="{MOVE_NAMES[mv]}: {METRIC_LABELS[m].title}" onclick={() => sortBy(key)}
                    >{METRIC_LABELS[m].label}{arrow(key)}</button
                  >
                </th>
              {/each}
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each rows as r (r.item)}
            <tr
              class="row {statusClass(r)}"
              class:best={r.best}
              class:open={openItem === r.item}
              onclick={() => (openItem = openItem === r.item ? undefined : r.item)}
            >
              <td class="name">{r.best ? "★ " : ""}{r.name}</td>
              <td>{r.min_damage}-{r.max_damage}</td>
              <td>{r.speed}</td>
              <td class:short={!!r.str_short}>{strText(r)}</td>
              <td>{r.min_level ?? ""}</td>
              <td>{r.two_handed ? "2H" : "1H"}</td>
              <td class="status">{statusText(r)}</td>
              <td class="source" title={sourcesTitle(r)}
                >{sourceText(r)}{#if r.limited !== undefined}<span class="mark" title="limited: at most {r.limited} in play"> ‡</span>{/if}{#if r.deleted_at_cleanup}<span class="mark" title="deleted at cleanup"> †</span>{/if}</td
              >
              {#each moves as mv (mv)}
                {@const s = moveScore(r, mv)}
                {#each METRICS as m, i (m)}
                  <td class="score" class:first={i === 0} class:bestmove={isBestMove(r, mv)}>{metricText(s, m)}</td>
                {/each}
              {/each}
            </tr>
            {#if openItem === r.item}
              <tr class="detail">
                <td colspan={columnCount}>
                  {#if !detail || detail.item !== r.item}
                    <p class="dim">Asking for {r.name}'s numbers…</p>
                  {:else if detail.missing?.length}
                    <p class="dim">{missingText(detail.missing)}</p>
                  {:else if detail.moves.length === 0}
                    <p class="dim">{r.name} is not scored: {statusText(r)}.</p>
                  {:else}
                    {#each detail.moves as dm (dm.move)}
                      <div class="dmove">
                        <span class="head">{MOVE_NAMES[dm.move]}</span>
                        blows in rounds 1-8: <span class="blows">{blowsText(dm.blows_per_round)}</span>
                        <span class="dim">· energy a blow {dm.energy}</span>
                      </div>
                      <table class="dtable" aria-label="{MOVE_NAMES[dm.move]} per monster">
                        <thead>
                          <tr>
                            <th>monster</th><th>hit</th><th>crit</th><th>dodge</th><th>damage</th><th>crit dmg</th><th>a blow</th
                            ><th>round 1</th><th>k1</th><th>k2</th><th>k3</th><th>rds</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each dm.monsters as ms (ms.monster)}
                            <tr>
                              <td>{monsterName(ms.monster)}{ms.knocks_down ? " (knocked down)" : ""}</td>
                              <td>{pct(ms.hit)}</td>
                              <td>{pct(ms.crit)}</td>
                              <td>{pct(ms.dodge)}</td>
                              <td>{rangeText(ms.damage)}</td>
                              <td>{rangeText(ms.crit_damage)}</td>
                              <td>{tenths(ms.damage_per_blow)}</td>
                              <td>{tenths(ms.damage_round1)}</td>
                              {#each [0, 1, 2] as k (k)}
                                <td title={ms.kill_by_round.map((v, i) => `by ${i + 1}: ${pct(v)}`).join("\n")}
                                  >{ms.kill_by_round[k] !== undefined ? pct(ms.kill_by_round[k]) : ""}</td
                                >
                              {/each}
                              <td>{roundsText(ms.rounds, ms.rounds_capped)}</td>
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    {/each}
                  {/if}
                </td>
              </tr>
            {/if}
          {:else}
            <tr><td colspan={columnCount} class="dim">No weapon passes the filters.</td></tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .planner {
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
    flex: 0 0 auto;
    padding: 0 1ch 4px;
    border-bottom: 1px solid var(--frame);
    max-height: 50%;
    overflow-y: auto;
  }
  .top {
    display: flex;
    gap: 2ch;
    align-items: baseline;
  }
  .tab {
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightcyan);
  }
  .close {
    margin-left: auto;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    gap: 0 2ch;
    align-items: baseline;
    margin-top: 4px;
  }
  .head {
    color: var(--vga-white);
    min-width: 9ch;
  }
  label.changed {
    color: var(--vga-yellow);
  }
  label.changed .num {
    border-color: var(--vga-yellow);
  }
  input,
  select,
  button {
    font: inherit;
    line-height: 16px;
  }
  input.num {
    width: 6ch;
  }
  input.text {
    width: 24ch;
  }
  input.text.wide {
    width: 40ch;
  }
  input.num,
  input.text,
  select {
    color: var(--vga-white);
    background: var(--vga-black);
    border: 1px solid var(--frame);
    padding: 0 2px;
  }
  .ctl {
    padding: 0 1ch;
    color: var(--vga-lightgray);
    background: var(--vga-blue);
    border: none;
    cursor: pointer;
  }
  .ctl:disabled {
    color: var(--dim);
    cursor: default;
  }
  .dim {
    color: var(--dim);
  }
  .note {
    margin: 4px 0 0;
    color: var(--vga-yellow);
  }
  .say {
    margin: 2em;
    color: var(--vga-lightgray);
  }
  table {
    border-collapse: collapse;
  }
  th,
  td {
    padding: 0 1ch 0 0;
    text-align: left;
    white-space: nowrap;
  }
  .monsters {
    margin-top: 4px;
    color: var(--vga-lightgray);
  }
  .monsters th,
  .monsters td,
  .dtable th,
  .dtable td {
    padding-right: 2ch;
  }
  .monsters th {
    color: var(--vga-white);
    font-weight: normal;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 1ch;
  }
  .rows thead th {
    position: sticky;
    background: var(--bg);
    font-weight: normal;
    color: var(--vga-white);
  }
  .rows thead tr.groups th {
    top: 0;
  }
  .rows thead tr:not(.groups) th {
    top: 16px;
  }
  th.move {
    text-align: center;
    border-left: 1px solid var(--frame);
  }
  .first {
    border-left: 1px solid var(--frame);
    padding-left: 1ch;
  }
  .sort {
    padding: 0;
    color: inherit;
    background: none;
    border: none;
    cursor: pointer;
  }
  .sort:hover {
    color: var(--vga-lightcyan);
  }
  tr.row {
    cursor: pointer;
  }
  tr.row:hover td {
    background: #001a33;
  }
  tr.row.now {
    color: var(--vga-lightgreen);
  }
  tr.row.later {
    color: var(--vga-yellow);
  }
  tr.row.never {
    color: var(--dim);
  }
  tr.row.best td {
    background: #002a00;
  }
  tr.row.best td.name {
    color: var(--vga-white);
  }
  tr.row.open td {
    background: #001a33;
  }
  td.short {
    color: var(--vga-lightred);
  }
  td.score {
    text-align: right;
  }
  td.bestmove {
    color: #000;
    background: var(--vga-cyan) !important;
  }
  .mark {
    color: var(--vga-lightmagenta);
  }
  tr.detail > td {
    padding: 4px 0 8px 2ch;
    color: var(--vga-lightgray);
    border-bottom: 1px solid var(--frame);
  }
  .dmove {
    margin-top: 4px;
  }
  .blows {
    color: var(--vga-white);
  }
  .dtable th {
    font-weight: normal;
    color: var(--vga-white);
  }
  .dtable td:not(:first-child) {
    text-align: right;
  }
</style>
