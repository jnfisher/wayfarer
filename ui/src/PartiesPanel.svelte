<script lang="ts">
  // The drawer's Parties panel (docs/plan/10-party-play.md §10): one box per party, its leader
  // picked and its followers ticked, with each member's live status; a new party; delete. It
  // only edits definitions: the brain forms the parties in the game. **re-sync** has the brain
  // form a stuck party (or all of them) up again at once.
  import {
    MAX_FOLLOWERS,
    activityLine,
    draftOf,
    draftProblem,
    elsewhere,
    memberLine,
    namesOf,
    newDraft,
    setLeader,
    toDef,
    toggleFollower,
    type Draft,
  } from "./lib/parties";
  import type { CharacterState, PartyDef, PartyStatus, SessionInfo } from "./proto";

  interface Props {
    sessions: SessionInfo[];
    /** The brain's last `parties`; undefined before the first. */
    parties: PartyStatus[] | undefined;
    attached: boolean | null;
    /** Each character as the brain last showed it: what its bot is doing, its HP. */
    characters?: Record<string, CharacterState>;
    onsave: (party: PartyDef) => void;
    ondelete: (id: string) => void;
    /** Form party `id` (left out: every party) up again now. */
    onresync?: (id?: string) => void;
  }

  let { sessions, parties, attached, characters = {}, onsave, ondelete, onresync = () => {} }: Props = $props();
  const RESYNC_TITLE =
    "Stuck? Every member checks its party with par now, and the invites and follows go again at once";

  const names = $derived(namesOf(sessions));
  /** Parties being edited, by id ("" for the new one). */
  let drafts = $state<Record<string, Draft>>({});
  /** The party whose delete is waiting for a second click. */
  let confirming = $state<string | null>(null);

  function draft(p: PartyStatus): Draft {
    return drafts[p.party.id] ?? draftOf(p);
  }
  function edit(d: Draft) {
    drafts = { ...drafts, [d.id]: d };
  }
  function changed(p: PartyStatus): boolean {
    const d = drafts[p.party.id];
    return !!d && JSON.stringify(toDef(d)) !== JSON.stringify(p.party);
  }
  function save(d: Draft) {
    if (draftProblem(d)) return;
    onsave(toDef(d));
    const { [d.id]: _, ...rest } = drafts;
    drafts = rest;
  }
  function remove(id: string) {
    if (confirming !== id) {
      confirming = id;
      return;
    }
    confirming = null;
    ondelete(id);
  }
  const fresh = $derived(drafts[""]);
</script>

{#snippet editor(d: Draft, status: PartyStatus | undefined)}
  <label class="leader">
    leader
    <select value={d.leader} onchange={(e) => edit(setLeader(d, (e.currentTarget as HTMLSelectElement).value))}>
      {#each sessions as s (s.id)}
        <option value={s.id}>{s.name}</option>
      {/each}
    </select>
  </label>
  <ul class="members">
    {#each sessions.filter((s) => s.id !== d.leader) as s (s.id)}
      {@const ticked = d.followers.includes(s.id)}
      {@const other = elsewhere(parties ?? [], s.id, d.id, names)}
      {@const m = status?.members.find((x) => x.session === s.id)}
      <li>
        <label>
          <input
            type="checkbox"
            checked={ticked}
            disabled={!ticked && d.followers.length >= MAX_FOLLOWERS}
            onchange={() => edit(toggleFollower(d, s.id))}
          />
          {s.name}
        </label>
        {#if ticked && m && !drafts[d.id]}
          {@const doing = activityLine(characters[s.id])}
          <span class="state {m.state}"
            >{memberLine(m, names[d.leader] ?? d.leader)}{#if doing}<span class="doing"> · {doing}</span>{/if}</span
          >
        {:else if other}
          <span class="dim">{other}</span>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

<div class="parties">
  {#if !attached}
    <p class="dim">No brain attached: parties are the brain's.</p>
  {:else if parties === undefined}
    <p class="dim">Asking the brain…</p>
  {:else}
    {#if parties.length > 0}
      <div class="acts top">
        <button class="resync-all" title={RESYNC_TITLE} onclick={() => onresync()}>re-sync all</button>
      </div>
    {/if}
    {#each parties as p (p.party.id)}
      {@const d = draft(p)}
      {@const leader = p.members.find((m) => m.session === p.party.leader)}
      <section class="party" aria-label="{names[p.party.leader] ?? p.party.leader}'s party">
        <div class="head">
          <span class="name">★ {names[p.party.leader] ?? p.party.leader}'s party</span>
          {#if leader && leader.state !== "joined"}
            <span class="state {leader.state}">{memberLine(leader, names[p.party.leader] ?? "")}</span>
          {:else if activityLine(characters[p.party.leader])}
            <span class="state doing">{activityLine(characters[p.party.leader])}</span>
          {/if}
        </div>
        {@render editor(d, p)}
        <div class="acts">
          {#if changed(p)}
            <button disabled={!!draftProblem(d)} title={draftProblem(d) ?? "Save the party"} onclick={() => save(d)}
              >save</button
            >
            <button
              onclick={() => {
                const { [p.party.id]: _, ...rest } = drafts;
                drafts = rest;
              }}>undo</button
            >
          {/if}
          <button class="resync" title={RESYNC_TITLE} onclick={() => onresync(p.party.id)}>re-sync</button>
          <button class="delete" onclick={() => remove(p.party.id)}
            >{confirming === p.party.id ? "really delete?" : "delete"}</button
          >
        </div>
      </section>
    {:else}
      {#if !fresh}
        <p class="dim">No parties yet.</p>
      {/if}
    {/each}
    {#if fresh}
      <section class="party new" aria-label="New party">
        <div class="head"><span class="name">New party</span></div>
        {@render editor(fresh, undefined)}
        <div class="acts">
          <button disabled={!!draftProblem(fresh)} title={draftProblem(fresh) ?? "Save the party"} onclick={() => save(fresh)}
            >save</button
          >
          <button
            onclick={() => {
              const { [""]: _, ...rest } = drafts;
              drafts = rest;
            }}>cancel</button
          >
        </div>
      </section>
    {:else}
      <div class="acts">
        <button onclick={() => edit(newDraft(sessions, parties ?? []))} disabled={sessions.length < 2}>+ New party</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  /* The parties tab of the drawer (App.svelte). */
  .parties {
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
  .party {
    padding: 2px 1ch 4px;
    border-bottom: 1px solid var(--frame);
  }
  .head {
    display: flex;
    gap: 1ch;
    justify-content: space-between;
  }
  .name {
    color: var(--vga-yellow);
  }
  .leader {
    display: block;
    margin: 2px 0;
    color: var(--dim);
  }
  select {
    font: inherit;
    color: var(--vga-lightgray);
    background: var(--vga-black, #000);
    border: 1px solid var(--frame);
  }
  .members {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .members li {
    display: flex;
    justify-content: space-between;
    gap: 1ch;
  }
  .state {
    color: var(--dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state.joined {
    color: var(--vga-lightgreen);
  }
  .doing {
    color: var(--vga-lightgray);
  }
  .state.separated,
  .state.walking,
  .state.unreachable {
    color: var(--vga-yellow);
  }
  .acts {
    display: flex;
    gap: 1ch;
    padding: 2px 1ch;
  }
  .party .acts {
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
  button:disabled {
    cursor: default;
    opacity: 0.5;
  }
  .delete {
    margin-left: auto;
  }
</style>
