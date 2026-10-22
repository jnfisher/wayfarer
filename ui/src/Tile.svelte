<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import "@xterm/xterm/css/xterm.css";
  import { botButtons, errandItems, linkButton, narrationLine } from "./lib/bot";
  import { keyBytes, type GatewayClient } from "./lib/gateway";
  import { HEADER_CELL_PX, bar, expText, fitHeader, levelClass, roomText } from "./lib/header";
  import { InputHistory } from "./lib/input";
  import { HELP_KEY } from "./lib/keys";
  import { TERM_COLS, TERM_ROWS, fitFontSize } from "./lib/layout";
  import type { TerminalSinks } from "./lib/sinks";
  import { FONT_FAMILY, FONT_NAME, VGA_THEME } from "./lib/theme";
  import type { SpellPlan } from "./lib/spells";
  import { planRow, planTotals } from "./lib/spells";
  import type { CharacterState, SessionInfo } from "./proto";

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
    /** Open or close the help overlay. */
    onhelp: () => void;
    /** What the brain knows of the character; null when no brain is attached. */
    character?: CharacterState | null;
    /** The brain's last spell plan for the character. */
    plan?: SpellPlan;
  }

  let {
    info,
    number,
    focused,
    zoom,
    offline,
    client,
    sinks,
    onfocus,
    onhelp,
    character = null,
    plan,
  }: Props = $props();

  let headerWidth = $state(0);
  const c = $derived(character);
  const hp = $derived(bar(c?.hp, c?.hp_max));
  const mana = $derived(bar(c?.mana, c?.mana_max));
  const buttons = $derived(botButtons(c?.bot));
  const narration = $derived(narrationLine(c?.bot));
  const link = $derived(linkButton(info.state));
  const errand = $derived(c?.errand ?? undefined);
  const errands = $derived(errandItems(c?.bot, errand));
  /** The errand menu is open. */
  let menuOpen = $state(false);
  /** The spell plan is showing. */
  let planOpen = $state(false);
  const shown = $derived(
    fitHeader(
      { number, name: info.name, server: info.server, state: info.state, character: c },
      Math.floor(headerWidth / HEADER_CELL_PX),
    ),
  );

  let host: HTMLDivElement;
  let input: HTMLInputElement;
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let keys = $state(false);
  let line = $state("");
  let ready = $state(false);
  const history = new InputHistory();
  /** A side of the frame: enough ║ for a tile 4,800 px tall. */
  const SIDE = "║\n".repeat(300);

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
    if (e.key === HELP_KEY && line === "") {
      e.preventDefault();
      onhelp();
    } else if (e.key === "Enter") {
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

<svelte:window
  onclick={() => (menuOpen = false)}
  onkeydown={(e) => {
    if (e.key === 'Escape') menuOpen = false;
  }}
/>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<section
  class="tile"
  class:focused
  onclick={() => {
    onfocus();
    focusInput();
  }}
>
  <!-- Every span's width in cells is counted in lib/header.ts: change both together. -->
  <header bind:clientWidth={headerWidth}>
    <span class="frame">{"╔═ "}</span>
    <span class="num">{number}</span>
    <span class="name">{" " + info.name}</span>
    {#if c}
      {#if shown.has("level")}
        <span class="level">{"  " + levelClass(c)}</span>
      {/if}
      {#if shown.has("hp")}
        <span class="frame">{" ═ "}</span>
        <span class="label">{"HP "}</span>
        <span class="bar {hp.tone}"><span class="full">{hp.full}</span><span class="empty">{hp.empty}</span></span>
        <span class="nums">{" " + hp.text}</span>
      {/if}
      {#if shown.has("mana")}
        <span class="label">{"  MA "}</span>
        <span class="bar {mana.tone}"><span class="full">{mana.full}</span><span class="empty">{mana.empty}</span></span>
        <span class="nums">{" " + mana.text}</span>
      {/if}
      {#if shown.has("exp")}
        <span class="frame">{" ═ "}</span>
        <span class="exp">{expText(c)}</span>
      {/if}
      {#if shown.has("room")}
        <span class="frame">{" ═ "}</span>
        <span class="room">{roomText(c)}</span>
      {/if}
    {/if}
    <span class="frame fill"></span>
    {#if buttons.length > 0}
      {#each buttons as b (b.action)}
        <button
          class="ctl {b.action}"
          title={b.title}
          aria-label="{b.action} the bot"
          onclick={(e) => {
            e.stopPropagation();
            client.botControl(info.id, b.action);
          }}>[{b.label}]</button
        >
      {/each}
      <span class="frame">{" ═ "}</span>
    {/if}
    {#if shown.has("server")}
      <span class="server">{info.server}</span>
      <span class="frame">{" ═ "}</span>
    {/if}
    <span class="badge {info.state}">{info.state}</span>
    {#if link}
      <button
        class="ctl link {link.action}"
        title={link.title}
        onclick={(e) => {
          e.stopPropagation();
          client.sessionControl(info.id, link.action);
        }}>[{link.label}]</button
      >
    {/if}
    {#if c && info.state === "realm"}
      <span class="menu">
        <button
          class="ctl"
          aria-label="Errands"
          aria-expanded={menuOpen}
          title="Errands: pick up all, equip all"
          onclick={(e) => {
            e.stopPropagation();
            menuOpen = !menuOpen;
          }}>[▼]</button
        >
      </span>
    {/if}
    {#if !c}
      <span class="nobrain">{" no brain"}</span>
    {/if}
    <span class="frame">{" ═╗"}</span>
  </header>
    {#if menuOpen}
      <ul class="errands" role="menu" aria-label="Errands">
        {#each errands as item (item.action)}
          <li role="none">
            <button
              role="menuitem"
              disabled={item.disabled}
              title={item.title}
              onclick={(e) => {
                e.stopPropagation();
                menuOpen = false;
                if (item.action === "spell_plan") {
                  planOpen = true;
                  client.requestSpellPlan(info.id);
                } else {
                  client.botControl(info.id, item.action);
                }
              }}>{item.label}</button
            >
          </li>
        {/each}
      </ul>
    {/if}
  {#if planOpen}
    <div class="plan" role="region" aria-label="Spell plan">
      <div class="head">
        <span>Spell plan</span>
        <button class="ctl" aria-label="Close the spell plan" onclick={() => (planOpen = false)}>[x]</button>
      </div>
      {#if !plan}
        <p>Asking the brain…</p>
      {:else}
        <ul>
          {#each plan.items as item (item.spell)}
            <li class:later={!item.usable} class:affordable={item.affordable}>{item.affordable ? "✓ " : "  "}{planRow(item)}</li>
          {/each}
        </ul>
        <p class="totals">{planTotals(plan)}</p>
        <button
          class="buy-btn"
          disabled={errands[0]?.disabled !== false || !plan.items.some((i) => i.usable)}
          title="Withdraw from the Bank of Godfrey if the cash falls short, walk to the shops, buy what the money covers (the cheapest first) and read the scrolls"
          onclick={() => client.botControl(info.id, "buy_spells")}>buy spells</button
        >
      {/if}
    </div>
  {/if}
  {#if errand}
    <div class="narration errand" title="An errand: cancel it from the [▼] menu">{errand}</div>
  {:else if narration !== null}
    <div class="narration {c?.bot?.state}" title={c?.bot?.reason ?? ""}>{narration}</div>
  {/if}
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
    <button
      title="Show wayfarer's keys and controls"
      aria-label="Help"
      onclick={(e) => {
        e.stopPropagation();
        onhelp();
      }}>?</button
    >
  </footer>
  <div class="bottom" aria-hidden="true">
    <span>╚</span><span class="fill"></span><span>╝</span>
  </div>
  <div class="side left" aria-hidden="true">{SIDE}</div>
  <div class="side right" aria-hidden="true">{SIDE}</div>
</section>

<style>
  /* The frame is drawn with the VGA font's own double-line glyphs, at the fixed 16 px of the
     header: ╔ ═ ╗ in the header, a column of ║ down each side, ╚ ═ ╝ at the bottom. So the
     strokes meet at every tile size and browser zoom (in the 8x16 cell ║ is two 2 px strokes
     at x 2-3 and 5-6, and ═ two 1 px strokes at rows 5 and 7; docs/web-ui.md). */
  .tile {
    --cell: 8px;
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    padding: 0 var(--cell);
    background: var(--bg);
  }
  /* 1 px into the header and the bottom row, where ╔ ╗ ╚ ╝ have the same two strokes, so no
     seam shows where they meet. */
  .side {
    position: absolute;
    top: 15px;
    bottom: calc(var(--cell) - 1px);
    width: var(--cell);
    overflow: hidden;
    white-space: pre;
    line-height: 16px;
    color: var(--frame);
    /* The same strokes under the glyphs, for the seams between them at fractional zooms. */
    background: linear-gradient(
      to right,
      transparent 2px,
      var(--frame) 2px 4px,
      transparent 4px 5px,
      var(--frame) 5px 7px,
      transparent 7px
    );
    pointer-events: none;
  }
  .side.left {
    left: 0;
  }
  .side.right {
    right: 0;
  }
  .tile.focused {
    --frame: var(--frame-focus);
  }
  header,
  .bottom {
    display: flex;
    margin: 0 calc(-1 * var(--cell));
    white-space: pre;
    overflow: hidden;
    color: var(--frame);
    line-height: 16px;
  }
  header {
    height: 16px;
  }
  /* ╚ and ╝ have all their strokes in the cell's top half. */
  .bottom {
    height: var(--cell);
  }
  .fill {
    flex: 1;
    min-width: 1ch;
    overflow: hidden;
  }
  header .fill {
    margin: 0 1ch;
  }
  .fill::before {
    content: "══════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════════";
  }
  .num {
    color: var(--dim);
  }
  .name {
    color: var(--vga-yellow);
  }
  .level,
  .label {
    color: var(--vga-lightgray);
  }
  .nums,
  .room {
    color: var(--vga-white);
  }
  .exp {
    color: var(--vga-lightcyan);
  }
  .room {
    max-width: 30ch; /* ROOM_MAX in lib/header.ts */
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar .empty {
    color: var(--dim);
  }
  .bar.good .full {
    color: var(--vga-lightgreen);
  }
  .bar.warn .full {
    color: var(--vga-yellow);
  }
  .bar.bad .full {
    color: var(--vga-lightred);
  }
  .nobrain {
    color: var(--dim);
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
  /* The bot's buttons sit in the header's 16 px line: one cell each for the brackets and the
     glyph (lib/bot.ts BUTTON_CELLS). */
  header .ctl {
    padding: 0;
    border: none;
    line-height: 16px;
    color: var(--vga-lightgray);
  }
  header .ctl:hover {
    color: var(--vga-white);
  }
  header .ctl.start,
  header .ctl.resume {
    color: var(--vga-lightgreen);
  }
  header .menu {
    position: relative;
    margin-left: 1ch;
  }
  /* Outside the header, which clips what overflows it. */
  ul.errands {
    position: absolute;
    right: var(--cell);
    top: 16px;
    z-index: 5;
    margin: 0;
    padding: 2px 0;
    list-style: none;
    background: var(--vga-black);
    border: 1px solid var(--vga-darkgray);
    white-space: nowrap;
  }
  ul.errands button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 0 1ch;
    color: var(--vga-lightgray);
  }
  ul.errands button:hover:not(:disabled) {
    color: #000;
    background: var(--vga-lightcyan);
  }
  ul.errands button:disabled {
    color: var(--vga-darkgray);
  }
  .plan {
    max-height: 40%;
    overflow-y: auto;
    padding: 2px 1ch;
    background: var(--vga-black);
    border-bottom: 1px solid var(--vga-darkgray);
    font-size: 0.9em;
  }
  .plan .head {
    display: flex;
    justify-content: space-between;
    color: var(--vga-lightcyan);
  }
  .plan ul {
    margin: 0;
    padding: 0;
    list-style: none;
    white-space: pre;
    color: var(--vga-lightgray);
  }
  .plan li.later {
    color: var(--vga-darkgray);
  }
  .plan li.affordable {
    color: var(--vga-lightgreen);
  }
  .plan .buy-btn {
    margin-top: 2px;
    background: var(--vga-lightgreen);
    color: #000;
  }
  .plan .buy-btn:disabled {
    background: var(--vga-darkgray);
  }
  .plan .totals {
    margin: 2px 0 0;
    color: var(--vga-yellow, #ff5);
  }
  .narration.errand {
    color: var(--vga-lightcyan);
  }
  header .ctl.link {
    margin-left: 1ch;
  }
  header .ctl.connect {
    color: var(--vga-lightgreen);
  }
  header .ctl.stop {
    color: var(--vga-lightred);
  }
  /* What the bot is doing, under the header while it runs or is paused. */
  .narration {
    height: 16px;
    line-height: 16px;
    padding: 0 1ch;
    overflow: hidden;
    white-space: pre;
    text-overflow: ellipsis;
    color: var(--vga-lightcyan);
  }
  .narration.paused {
    color: var(--vga-yellow);
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
