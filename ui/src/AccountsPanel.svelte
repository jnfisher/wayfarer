<script lang="ts">
  // The drawer's Accounts panel (C-062): the servers and characters in wayfarer.toml, each
  // character's BBS user and password from credentials.toml. Save sends the whole set: the gateway
  // writes both files, connects new characters (making the BBS account if the BBS doesn't know
  // the user, then stopping at the MajorMUD menu), hangs up and logs on again those whose server or
  // login changed, and leaves the rest playing.
  import {
    DEFAULT_EMAIL,
    MAX_CHARACTERS,
    changed,
    confirmText,
    draftOf,
    newCharacter,
    newServer,
    problems,
    saveEffect,
    toAccounts,
    usersOf,
    withName,
    type CharacterDraft,
    type Draft,
    type ServerDraft,
  } from "./lib/accounts";
  import type { Accounts, SessionInfo } from "./proto";

  interface Props {
    /** The gateway's last `accounts`; undefined before the first. */
    accounts: Accounts | undefined;
    sessions: SessionInfo[];
    /** The last `accounts_saved`: `error` when nothing changed. `at` tells two apart. */
    result?: { error?: string; at: number };
    onsave: (accounts: Accounts) => void;
  }

  let { accounts, sessions, result, onsave }: Props = $props();

  /** The edits; null while there are none (the panel shows what was saved). */
  let edits = $state<Draft | null>(null);
  const draft = $derived(edits ?? (accounts ? draftOf(accounts) : { servers: [], characters: [] }));
  const wrong = $derived(problems(draft));
  const dirty = $derived(edits !== null && changed(edits, accounts));
  const impact = $derived(accounts ? saveEffect(draft, accounts, sessions) : null);
  /** The save waits for a second click when it would drop connections. */
  let confirming = $state(false);
  let saving = $state(false);
  /** The rows whose password shows, by index. */
  let shown = $state<Record<number, boolean>>({});
  /** The character whose remove waits for a second click. */
  let removing = $state<number | null>(null);

  // A save answered: done (the next `accounts` shows it) or refused (the edits stay).
  let seen = 0;
  $effect(() => {
    if (!result || result.at === seen) return;
    seen = result.at;
    saving = false;
    if (!result.error) edits = null;
  });

  function edit(change: (d: Draft) => Draft) {
    edits = change(structuredClone($state.snapshot(draft)) as Draft);
    confirming = false;
  }
  function setServer(i: number, patch: Partial<ServerDraft>) {
    edit((d) => {
      const old = d.servers[i].id;
      d.servers[i] = { ...d.servers[i], ...patch };
      // A fresh server's id renamed: its characters follow it.
      if (patch.id !== undefined) for (const c of d.characters) if (c.server === old) c.server = patch.id;
      return d;
    });
  }
  function setCharacter(i: number, change: (c: CharacterDraft) => CharacterDraft) {
    edit((d) => {
      d.characters[i] = change(d.characters[i]);
      return d;
    });
  }
  function save() {
    if (!accounts || Object.keys(wrong).length > 0) return;
    if (impact && confirmText(impact) && !confirming) {
      confirming = true;
      return;
    }
    confirming = false;
    saving = true;
    onsave(toAccounts(draft, accounts));
  }
  function reasonOf(id: string): string | undefined {
    return accounts?.characters.find((c) => c.id === id)?.reason;
  }
  function stateOf(id: string): string | undefined {
    return sessions.find((s) => s.id === id)?.state;
  }
  function signsUp(c: CharacterDraft): boolean {
    return c.fresh || accounts?.characters.find((x) => x.id === c.id)?.user !== c.user;
  }
  const value = (e: Event) => (e.currentTarget as HTMLInputElement).value;
</script>

<div class="accounts">
  {#if !accounts}
    <p class="dim">Asking the gateway…</p>
  {:else}
    <h3>Servers</h3>
    {#each draft.servers as s, i (i)}
      {@const users = usersOf(draft, s.id)}
      <section class="row" aria-label="Server {s.id || i + 1}">
        <div class="fields">
          <label
            >id
            {#if s.fresh}
              <input value={s.id} oninput={(e) => setServer(i, { id: value(e) })} />
            {:else}
              <span class="fixed">{s.id}</span>
            {/if}
          </label>
          <label>host <input value={s.host} oninput={(e) => setServer(i, { host: value(e) })} /></label>
          <label
            >port <input
              type="number"
              min="1"
              max="65535"
              value={s.port}
              oninput={(e) => setServer(i, { port: Number(value(e)) })}
            /></label
          >
          <label
            >logon
            <select
              value={s.preset ?? ""}
              onchange={(e) => setServer(i, { preset: value(e) || undefined })}
            >
              {#each accounts.presets as p (p)}
                <option value={p}>{p}</option>
              {/each}
              <option value="">none: log on by hand</option>
            </select>
          </label>
        </div>
        {#if s.custom_logon}
          <p class="note">Has its own logon script in wayfarer.toml, kept as it is.</p>
        {/if}
        {#if wrong[`server:${i}`]}
          <p class="problem">{wrong[`server:${i}`]}</p>
        {/if}
        <div class="acts">
          <button
            class="delete"
            disabled={users.length > 0}
            title={users.length > 0 ? `Used by ${users.join(", ")}` : "Delete the server"}
            onclick={() =>
              edit((d) => {
                d.servers.splice(i, 1);
                return d;
              })}>delete</button
          >
        </div>
      </section>
    {/each}
    <div class="acts">
      <button
        onclick={() =>
          edit((d) => {
            d.servers.push(newServer(d));
            return d;
          })}>+ Server</button
      >
    </div>

    <h3>Characters</h3>
    {#each draft.characters as c, i (i)}
      {@const state = stateOf(c.id)}
      {@const reason = c.fresh ? undefined : reasonOf(c.id)}
      <section class="row" aria-label="Character {c.name || i + 1}">
        <div class="head">
          <span class="name">{c.name || "New character"}</span>
          {#if !c.fresh && state}<span class="badge {state}">{state}</span>{/if}
        </div>
        {#if reason}<p class="reason">{reason}</p>{/if}
        <div class="fields">
          <label>name <input value={c.name} oninput={(e) => setCharacter(i, (x) => withName(x, value(e)))} /></label>
          <label
            >id
            {#if c.fresh}
              <input
                value={c.id}
                oninput={(e) => setCharacter(i, (x) => ({ ...x, id: value(e), idFromName: false }))}
              />
            {:else}
              <span class="fixed">{c.id}</span>
            {/if}
          </label>
          <label
            >server
            <select value={c.server} onchange={(e) => setCharacter(i, (x) => ({ ...x, server: value(e) }))}>
              {#each draft.servers as s (s.id)}
                <option value={s.id}>{s.id}{s.host ? ` (${s.host}:${s.port})` : ""}</option>
              {/each}
            </select>
          </label>
          <label>BBS user <input value={c.user} oninput={(e) => setCharacter(i, (x) => ({ ...x, user: value(e) }))} /></label>
          <label
            >password
            <span class="pw">
              <input
                type={shown[i] ? "text" : "password"}
                value={c.password}
                oninput={(e) => setCharacter(i, (x) => ({ ...x, password: value(e) }))}
              />
              <button
                class="show"
                title={shown[i] ? "Hide the password" : "Show the password"}
                onclick={() => (shown = { ...shown, [i]: !shown[i] })}>{shown[i] ? "hide" : "show"}</button
              >
            </span>
          </label>
          <label class="tick"
            ><input
              type="checkbox"
              checked={c.autostart}
              onchange={(e) => setCharacter(i, (x) => ({ ...x, autostart: (e.currentTarget as HTMLInputElement).checked }))}
            /> connect when the gateway starts</label
          >
          {#if signsUp(c)}
            <label
              >e-mail <input
                value={c.email ?? DEFAULT_EMAIL}
                oninput={(e) => setCharacter(i, (x) => ({ ...x, email: value(e) }))}
              /></label
            >
            <label
              >gender
              <select value={c.gender ?? "F"} onchange={(e) => setCharacter(i, (x) => ({ ...x, gender: value(e) }))}>
                <option value="F">F</option>
                <option value="M">M</option>
              </select>
            </label>
            <p class="note">E-mail and gender are used only if the BBS doesn't know the user: the account is made then.</p>
          {/if}
        </div>
        {#if wrong[`character:${i}`]}
          <p class="problem">{wrong[`character:${i}`]}</p>
        {/if}
        <div class="acts">
          <button
            class="delete"
            onclick={() => {
              if (removing !== i) {
                removing = i;
                return;
              }
              removing = null;
              edit((d) => {
                d.characters.splice(i, 1);
                return d;
              });
            }}>{removing === i ? "really remove?" : "remove"}</button
          >
        </div>
      </section>
    {:else}
      <p class="dim">No characters yet.</p>
    {/each}
    <div class="acts">
      <button
        disabled={draft.characters.length >= MAX_CHARACTERS || draft.servers.length === 0}
        title={draft.servers.length === 0 ? "Add a server first" : `Up to ${MAX_CHARACTERS} characters`}
        onclick={() =>
          edit((d) => {
            d.characters.push(newCharacter(d));
            return d;
          })}>+ Character</button
      >
    </div>

    <div class="save">
      {#if wrong.all}<p class="problem">{wrong.all}</p>{/if}
      {#if confirming && impact}<p class="confirm">{confirmText(impact)}</p>{/if}
      {#if result?.error && edits}<p class="problem">Not saved: {result.error}</p>
      {:else if dirty && !confirming && !saving}<p class="confirm">Not saved yet: nothing changes until you save.</p>{/if}
      <div class="acts">
        <button
          class="primary"
          class:pending={dirty && !saving}
          disabled={!dirty || saving || Object.keys(wrong).length > 0}
          title={Object.values(wrong)[0] ?? "Write wayfarer.toml and credentials.toml, and apply them"}
          onclick={save}>{saving ? "saving…" : confirming ? "really save?" : "save"}</button
        >
        <button
          disabled={edits === null || saving}
          onclick={() => {
            edits = null;
            confirming = false;
          }}>revert</button
        >
      </div>
    </div>
  {/if}
</div>

<style>
  /* The accounts tab of the drawer (App.svelte). */
  .accounts {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding: 2px 0;
  }
  h3 {
    margin: 4px 1ch 0;
    font: inherit;
    color: var(--vga-yellow);
  }
  .dim {
    margin: 0;
    padding: 0 1ch;
    color: var(--dim);
  }
  .row {
    padding: 2px 1ch 4px;
    border-bottom: 1px solid var(--frame);
  }
  .head {
    display: flex;
    gap: 1ch;
    justify-content: space-between;
  }
  .name {
    color: var(--vga-lightcyan, #5ff);
  }
  .badge {
    color: var(--dim);
  }
  .badge.realm,
  .badge.menu {
    color: var(--vga-lightgreen);
  }
  .badge.lost {
    color: var(--vga-yellow);
  }
  .fields {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 2px 1ch;
    align-items: center;
  }
  .fields label {
    display: contents;
    color: var(--dim);
  }
  .fields label.tick {
    display: block;
    grid-column: 1 / -1;
  }
  .fields .note {
    grid-column: 1 / -1;
  }
  .fixed {
    color: var(--vga-lightgray);
  }
  .pw {
    display: flex;
    gap: 1ch;
    min-width: 0;
  }
  .pw input {
    flex: 1;
  }
  input:not([type="checkbox"]),
  select {
    min-width: 0;
    font: inherit;
    color: var(--vga-lightgray);
    background: var(--vga-black, #000);
    border: 1px solid var(--frame);
  }
  .note {
    margin: 2px 0 0;
    color: var(--dim);
  }
  .reason,
  .confirm {
    margin: 2px 0;
    color: var(--vga-yellow);
  }
  .problem {
    margin: 2px 0;
    color: var(--vga-lightred, #f55);
  }
  .acts {
    display: flex;
    gap: 1ch;
    padding: 2px 1ch;
  }
  .row .acts {
    padding: 2px 0 0;
  }
  .save {
    padding: 4px 1ch;
  }
  .save .acts {
    padding: 2px 0;
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
  /* Edits waiting for the save. */
  button.pending {
    background: var(--vga-yellow);
  }
  .delete {
    margin-left: auto;
  }
</style>
