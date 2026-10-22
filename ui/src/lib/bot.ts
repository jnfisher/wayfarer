// The bot's controls on a tile (docs/web-ui.md, "The bot"): the buttons in the header, the
// narration line under it, and what F9 and F10 do. The glyphs are the VGA font's own (code page
// 437 has ► and ■ but no pause sign, so pause is ║).

import type { Bot, BotAction, SessionAction } from "../proto";

export interface BotButton {
  action: BotAction;
  label: string;
  title: string;
}

/** Cells one button takes in the header: `[►]`. */
export const BUTTON_CELLS = 3;

const START: BotButton = { action: "start", label: "►", title: "Start the bot here: fight what comes, rest when low (F10)" };
const PAUSE: BotButton = { action: "pause", label: "║", title: "Pause the bot (F9)" };
const RESUME: BotButton = { action: "resume", label: "►", title: "Resume the bot (F9)" };
const STOP: BotButton = { action: "stop", label: "■", title: "Stop the bot: you play (F10)" };

/** The buttons for the bot's state: start; pause and stop; resume and stop. None without a bot. */
export function botButtons(bot: Bot | undefined): BotButton[] {
  switch (bot?.state) {
    case "stopped":
      return [START];
    case "running":
      return [PAUSE, STOP];
    case "paused":
      return [RESUME, STOP];
    default:
      return [];
  }
}

/** Cells the buttons take, with the ` ═ ` after them; 0 without a bot. */
export function botWidth(bot: Bot | undefined): number {
  const n = botButtons(bot).length;
  return n === 0 ? 0 : n * BUTTON_CELLS + 3;
}

/** The line under the header while the bot is running or paused; null otherwise. */
export function narrationLine(bot: Bot | undefined): string | null {
  if (!bot || bot.state === "stopped") return null;
  return bot.narration || (bot.state === "paused" ? "Paused" : "Running");
}

export const PAUSE_KEY = "F9";
export const START_KEY = "F10";

/** What `F9` (pause or resume) or `F10` (start or stop) asks of a bot in this state. */
export function botKeyAction(key: string, bot: Bot | undefined): BotAction | null {
  if (!bot) return null;
  if (key === PAUSE_KEY) {
    return bot.state === "running" ? "pause" : bot.state === "paused" ? "resume" : null;
  }
  if (key === START_KEY) return bot.state === "stopped" ? "start" : "stop";
  return null;
}

export interface LinkButton {
  action: SessionAction;
  label: string;
  title: string;
}

/**
 * The tile's connection button: connect when the session is closed or lost, log off while it is
 * connected in any way. None while it is connecting.
 */
export function linkButton(state: string): LinkButton | null {
  switch (state) {
    case "closed":
    case "lost":
      return { action: "connect", label: "connect", title: "Connect and log on" };
    case "connected":
    case "logon":
    case "menu":
    case "realm":
      return { action: "log_off", label: "log off", title: "Log off cleanly and disconnect (it stays off until you connect)" };
    default:
      return null;
  }
}

/** Cells the connection button takes, with the space before it; 0 when there is none. */
export function linkWidth(state: string): number {
  const b = linkButton(state);
  return b ? b.label.length + 3 : 0;
}

export interface ErrandItem {
  /** A `bot_control` action, or `spell_plan`: a read-only plan, asked for with its own message. */
  action: BotAction | "spell_plan";
  label: string;
  title: string;
  disabled: boolean;
}

/** Cells the errand menu button `[▼]` takes, with the space before it; 0 when there is none. */
export function menuWidth(state: string, hasBrain: boolean): number {
  return hasBrain && state === "realm" ? 4 : 0;
}

/**
 * The entries of a tile's errand menu: pick up all and equip all (not while the bot runs or an
 * errand is on), and cancel while one is.
 */
export function errandItems(bot: Bot | undefined, errand: string | undefined): ErrandItem[] {
  const busy = errand !== undefined;
  const running = bot?.state === "running";
  const why = busy ? "An errand is in progress" : running ? "Stop or pause the bot first" : "";
  const items: ErrandItem[] = [
    {
      action: "pick_up_all",
      label: "Pick up all",
      title: why || "Take the coins and the items on the floor here",
      disabled: busy || running,
    },
    {
      action: "equip_all",
      label: "Equip all",
      title: why || "Wear and wield what is carried, into empty slots only",
      disabled: busy || running,
    },
  ];
  items.push({
    action: "spell_plan",
    label: "Spell plan…",
    title: "Which spells the class lacks, where to buy them and what they cost",
    disabled: false,
  });
  if (busy) items.push({ action: "cancel_errand", label: "Cancel errand", title: errand, disabled: false });
  return items;
}
