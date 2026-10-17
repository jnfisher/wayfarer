<script lang="ts">
  import { onMount } from "svelte";
  import Tile from "./Tile.svelte";
  import { GatewayClient, gatewayUrl, type ConnectionStatus, type TokenStore } from "./lib/gateway";
  import { tileGrid } from "./lib/layout";
  import { TerminalSinks } from "./lib/sinks";
  import type { SessionInfo } from "./proto";

  let sessions = $state<SessionInfo[]>([]);
  let status = $state<ConnectionStatus>("connecting");
  let reason = $state("");
  let retryInMs = $state(0);
  let focused = $state(0);
  let zooms = $state<Record<string, number>>({});
  let gridWidth = $state(0);
  let gridHeight = $state(0);

  const grid = $derived(tileGrid(sessions.length, gridWidth, gridHeight));
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
    if (e.altKey && !e.ctrlKey && !e.metaKey && /^Digit[1-6]$/.test(e.code)) {
      const n = Number(e.code.slice(5)) - 1;
      if (n < sessions.length) {
        e.preventDefault();
        focused = n;
      }
      return;
    }
    const id = sessions[focused]?.id;
    if (!id || !e.ctrlKey || e.altKey || e.metaKey) return;
    const step = e.key === "+" || e.key === "=" ? 1 : e.key === "-" ? -1 : e.key === "0" ? 0 : null;
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
    bind:clientWidth={gridWidth}
    bind:clientHeight={gridHeight}
    style:grid-template-columns="repeat({grid.cols}, minmax(0, 1fr))"
    style:grid-template-rows="repeat({grid.rows}, minmax(0, 1fr))"
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
      />
    {:else}
      <p class="empty">
        {status === "open" ? "No characters are configured in wayfarer.toml." : offline}
      </p>
    {/each}
  </div>
</main>

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
    gap: 2px;
    padding: 2px;
  }
  .empty {
    margin: 2em;
    color: var(--dim);
  }
</style>
