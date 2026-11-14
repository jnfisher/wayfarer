<script lang="ts">
  import { untrack } from "svelte";
  import { LAYERS, groupRows, isOwn, numberChange, resetChange, show, sourceLabel, type SettingsMessage } from "./lib/settings";
  import MonsterRules from "./MonsterRules.svelte";
  import type { AttackSpellOption, CollectItem, MonsterRuleEntry, MonsterRuleKind, SessionInfo, SettingChange, SettingRow } from "./proto";

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
    /** The character's monster rules (its snapshot's `bot.monster_rules`), folded away at the
     * end of the list. */
    rules?: MonsterRuleEntry[];
    /** Set the player's monster rule for a pattern, or remove it (`null`). */
    onrule?: (session: string, pattern: string, rule: MonsterRuleKind | null) => void;
    /** The attack spells the character knows (`bot.attack_spells`), and the one chosen
     * (`bot.attack_spell`, undefined for auto): C-099. */
    attackSpells?: AttackSpellOption[];
    attackSpell?: string;
    /** Fight with this attack spell, or auto (`null`). */
    onattackspell?: (session: string, spell: string | null) => void;
    /** The items the bot collects and how many of each (`bot.collect_items`): C-102. */
    collectItems?: CollectItem[];
    /** Collect these items, the whole list. */
    oncollect?: (session: string, items: CollectItem[]) => void;
  }

  let {
    session,
    settings,
    attached,
    onrequest,
    onsave,
    rules = [],
    onrule = () => {},
    attackSpells = [],
    attackSpell,
    onattackspell = () => {},
    collectItems = [],
    oncollect = () => {},
  }: Props = $props();

  const COLLECT_HELP =
    "Items this character picks up off the floor where it stands, out of a fight, until it carries as many as the count: quest items, rings to hand out. The ones it wears do not count. Names are the game's (checked: a misspelt one is refused). Kept with the bot, not in a settings file.";

  /** The item being added to the collect list. */
  let collectName = $state("");
  let collectCount = $state(1);

  function addCollect(id: string) {
    const name = collectName.trim();
    if (!name || collectCount < 1) return;
    const rest = collectItems.filter((c) => c.name.toLowerCase() !== name.toLowerCase());
    oncollect(id, [...rest, { name, count: Math.floor(collectCount) }]);
    collectName = "";
    collectCount = 1;
  }

  function setCollectCount(id: string, name: string, count: number) {
    if (!Number.isFinite(count)) return;
    const n = Math.max(0, Math.floor(count));
    oncollect(
      id,
      collectItems.flatMap((c) => (c.name === name ? (n > 0 ? [{ name, count: n }] : []) : [c])),
    );
  }

  const ATTACK_SPELL_HELP =
    "The attack spell this character fights with, in every mode: auto casts the strongest it can pay for. Pick one a monster does not resist (no fire at a greater hellion), and back to auto after. Kept with the bot, not in a settings file.";

  /** The layer being edited, and whether the standard and advanced settings show too. */
  let layer = $state<string>("character");
  let all = $state(true);
  /** Number boxes whose text is not yet a whole number in range. */
  let bad = $state<Record<string, boolean>>({});

  // The panel is mounted while its tab is open: it asks for the focused character's settings
  // when it opens, when the focus moves and when the brain attaches. Only those are tracked;
  // reading the answer or the layer here would ask again for every answer, and the answers
  // would chase each other.
  // (`session` is a new object whenever the session's counters move, so the effect follows a
  // derived string, which says "changed" only when it is a different one.)
  const target = $derived(attached && session ? session.id : undefined);
  $effect(() => {
    const id = target;
    if (id) untrack(() => onrequest(id, layer));
  });

  /** The answer for the layer being edited: one for another layer (a late one, or a reload's
   * after the layer was changed) is not shown as this one's. */
  const shown = $derived(settings && settings.layer === layer ? settings : undefined);
  const groups = $derived(groupRows(shown?.rows ?? [], all));
  const files = $derived(shown?.files ?? []);

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
  {:else if !shown}
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
            {#if row.key === "spells.attack" && session && attackSpells.length > 0}
              {@const id = session.id}
              <div class="row" data-key="attack_spell" class:own={attackSpell !== undefined}>
                <label class="name" for="s-attack-spell" title={ATTACK_SPELL_HELP}>Attack spell</label>
                <span class="control">
                  <select
                    id="s-attack-spell"
                    value={attackSpell ?? ""}
                    onchange={(e) => onattackspell(id, e.currentTarget.value || null)}
                  >
                    <option value="">auto (the strongest it can pay for)</option>
                    {#each attackSpells as sp (sp.short)}
                      <option value={sp.short}>{sp.name} ({sp.short}, {sp.mana} mana)</option>
                    {/each}
                  </select>
                </span>
                <span class="source" title="Kept with the bot, for this character">this character</span>
                <div class="help">{ATTACK_SPELL_HELP}</div>
              </div>
            {/if}
            {#if row.key === "cash.max_encumbrance" && session}
              {@const id = session.id}
              <div class="row" data-key="collect_items" class:own={collectItems.length > 0}>
                <span class="name" title={COLLECT_HELP}>Collect items</span>
                <span class="control collect">
                  {#each collectItems as c (c.name)}
                    <span class="collect-item">
                      <span class="collect-name">{c.name}</span>
                      <input
                        type="number"
                        min="0"
                        max="999"
                        aria-label={`how many ${c.name}`}
                        value={c.count}
                        onchange={(e) => setCollectCount(id, c.name, e.currentTarget.valueAsNumber)}
                      />
                      <button aria-label={`stop collecting ${c.name}`} title="Stop collecting it" onclick={() => setCollectCount(id, c.name, 0)}>×</button>
                    </span>
                  {/each}
                  <form
                    class="collect-add"
                    onsubmit={(e) => {
                      e.preventDefault();
                      addCollect(id);
                    }}
                  >
                    <input type="text" placeholder="item name" aria-label="item to collect" bind:value={collectName} />
                    <input type="number" min="1" max="999" aria-label="how many to collect" bind:value={collectCount} />
                    <button type="submit" disabled={!collectName.trim()}>add</button>
                  </form>
                </span>
                <span class="source" title="Kept with the bot, for this character">this character</span>
                <div class="help">{COLLECT_HELP}</div>
              </div>
            {/if}
          {/each}
        </section>
      {/each}
      {#if session}
        {@const id = session.id}
        <details class="monster-rules">
          <summary title="What the bot does about a monster, by name: the defaults and yours">monster rules ({rules.length})</summary>
          <MonsterRules {rules} onrule={(pattern, rule) => onrule(id, pattern, rule)} />
        </details>
      {/if}
    </div>
  {/if}
</div>

<style>
  .collect {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .collect-item,
  .collect-add {
    display: flex;
    gap: 0.3rem;
    align-items: center;
  }
  .collect-name {
    flex: 1;
  }
  .collect input[type="number"] {
    width: 4em;
  }
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
  .monster-rules {
    margin-top: 8px;
  }
  .monster-rules summary {
    color: var(--vga-yellow);
    border-bottom: 1px solid var(--frame);
    cursor: pointer;
  }
  .all {
    margin-left: auto;
    color: var(--dim);
  }
</style>
