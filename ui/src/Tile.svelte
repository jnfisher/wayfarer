<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import "@xterm/xterm/css/xterm.css";
  import { keyBytes, type GatewayClient } from "./lib/gateway";
  import { InputHistory } from "./lib/input";
  import { TERM_COLS, TERM_ROWS, fitFontSize } from "./lib/layout";
  import type { TerminalSinks } from "./lib/sinks";
  import { FONT_FAMILY, FONT_NAME, VGA_THEME } from "./lib/theme";
  import type { SessionInfo } from "./proto";

  interface Props {
    info: SessionInfo;
    /** 1-based, for Alt+n. */
    number: number;
    focused: boolean;
    /** Font size steps above (or below) "fit to tile". */
    zoom: number;
    /** The gateway connection is down: say so over the terminal. */
    offline: string | null;
    client: GatewayClient;
    sinks: TerminalSinks;
    onfocus: () => void;
  }

  let { info, number, focused, zoom, offline, client, sinks, onfocus }: Props = $props();

  let host: HTMLDivElement;
  let input: HTMLInputElement;
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let keys = $state(false);
  let line = $state("");
  let ready = $state(false);
  const history = new InputHistory();

  onMount(() => {
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let detach: (() => void) | undefined;
    (async () => {
      // Measure with the VGA font, not the fallback.
      await document.fonts.load(`16px "${FONT_NAME}"`).catch(() => undefined);
      if (disposed) return;
      const t = new Terminal({
        cols: TERM_COLS,
        rows: TERM_ROWS,
        scrollback: 10_000,
        fontFamily: FONT_FAMILY,
        fontSize: 16,
        lineHeight: 1,
        fontWeight: "normal",
        fontWeightBold: "normal",
        drawBoldTextInBrightColors: true,
        theme: VGA_THEME,
        cursorBlink: false,
        disableStdin: true,
      });
      term = t;
      fit = new FitAddon();
      t.loadAddon(fit);
      t.open(host);
      try {
        const webgl = new WebglAddon();
        webgl.onContextLoss(() => webgl.dispose());
        t.loadAddon(webgl);
      } catch {
        // No WebGL: xterm's DOM renderer carries on.
      }
      t.onData((data) => {
        if (keys) client.sendKeys(info.id, keyBytes(data));
      });
      refit();
      observer = new ResizeObserver(() => refit());
      observer.observe(host);
      detach = sinks.attach(info.id, { write: (text) => t.write(text), reset: () => t.reset() });
      ready = true;
    })();
    return () => {
      disposed = true;
      observer?.disconnect();
      detach?.();
      term?.dispose();
    };
  });

  /** Size the font so 80 columns fill the tile's width (plus the zoom), rows to fill its height. */
  function refit() {
    if (!term || !fit || host.clientWidth === 0) return;
    let size = Math.max(6, fitFontSize(host.clientWidth) + zoom);
    term.options.fontSize = size;
    if (zoom <= 0) {
      // xterm rounds cells to device pixels and keeps room for its scrollbar: step down until
      // all 80 columns really fit.
      while (size > 6 && (fit.proposeDimensions()?.cols ?? TERM_COLS) < TERM_COLS) {
        size -= 1;
        term.options.fontSize = size;
      }
    }
    const rows = Math.max(1, fit.proposeDimensions()?.rows ?? TERM_ROWS);
    if (term.rows !== rows || term.cols !== TERM_COLS) term.resize(TERM_COLS, rows);
  }

  $effect(() => {
    void zoom;
    refit();
  });

  $effect(() => {
    if (focused && ready) focusInput();
  });

  function focusInput() {
    if (keys) term?.focus();
    else input?.focus();
  }

  function toggleKeys() {
    keys = !keys;
    if (term) term.options.disableStdin = !keys;
    onfocus();
    focusInput();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      client.sendLine(info.id, line);
      history.push(line);
      line = "";
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      line = history.up(line);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      line = history.down();
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<section
  class="tile"
  class:focused
  onclick={() => {
    onfocus();
    focusInput();
  }}
>
  <header>
    <span class="frame">╔═</span>
    <span class="num">{number}</span>
    <span class="name">{info.name}</span>
    <span class="frame fill"></span>
    <span class="server">{info.server}</span>
    <span class="frame">═</span>
    <span class="badge {info.state}">{info.state}</span>
    <span class="frame">═╗</span>
  </header>
  <div class="screen">
    <div class="term" bind:this={host}></div>
    {#if offline}
      <div class="offline">{offline}</div>
    {/if}
  </div>
  <footer>
    <span class="prompt">{keys ? "keys" : ">"}</span>
    <input
      bind:this={input}
      bind:value={line}
      {onkeydown}
      disabled={keys}
      spellcheck="false"
      autocomplete="off"
      aria-label="Input for {info.name}"
    />
    <button
      class:on={keys}
      title="Send keystrokes typed in the terminal straight to the game"
      onclick={(e) => {
        e.stopPropagation();
        toggleKeys();
      }}>keys</button
    >
  </footer>
</section>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border: 4px double var(--frame);
    border-top: none;
    background: var(--bg);
  }
  .tile.focused {
    --frame: var(--frame-focus);
  }
  header {
    display: flex;
    gap: 0.5ch;
    white-space: nowrap;
    overflow: hidden;
    color: var(--frame);
    line-height: 16px;
    height: 16px;
  }
  header .frame.fill {
    flex: 1;
    overflow: hidden;
  }
  header .frame.fill::before {
    content: "══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════";
  }
  .num {
    color: var(--dim);
  }
  .name {
    color: var(--vga-yellow);
  }
  .server {
    color: var(--vga-cyan);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .badge {
    padding: 0 1ch;
    color: #000;
    background: var(--vga-lightgray);
  }
  .badge.connecting {
    background: var(--vga-brown);
  }
  .badge.connected {
    background: var(--vga-white);
  }
  .badge.logon {
    background: var(--vga-cyan);
  }
  .badge.menu {
    background: var(--vga-lightblue);
  }
  .badge.realm {
    background: var(--vga-lightgreen);
  }
  .badge.lost {
    background: var(--vga-lightred);
  }
  .badge.closed {
    background: var(--vga-darkgray);
    color: var(--vga-lightgray);
  }
  .screen {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .term {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .offline {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    padding: 0 1ch;
    text-align: center;
    color: var(--vga-white);
    background: var(--vga-red);
  }
  footer {
    display: flex;
    align-items: stretch;
    border-top: 1px solid var(--frame);
    line-height: 16px;
  }
  .prompt {
    padding: 2px 1ch;
    color: var(--vga-lightgreen);
  }
  input {
    flex: 1;
    min-width: 0;
    font: inherit;
    color: var(--vga-white);
    background: transparent;
    border: none;
    outline: none;
    caret-color: var(--vga-lightgreen);
  }
  input:disabled {
    color: var(--dim);
  }
  button {
    font: inherit;
    color: var(--dim);
    background: transparent;
    border: none;
    border-left: 1px solid var(--frame);
    padding: 0 1ch;
    cursor: pointer;
  }
  button.on {
    color: #000;
    background: var(--vga-yellow);
  }
</style>
