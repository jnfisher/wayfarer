// The bot's controls on a tile (docs/web-ui.md, "The bot"): the buttons in the header, the
// narration line under it, and what F9 and F10 do. The glyphs are the VGA font's own (code page
// 437 has ► and ■ but no pause sign, so pause is ║).

import type { Bot, BotAction, Place, SessionAction } from "../proto";

export interface BotButton {
  action: BotAction;
  label: string;
  title: string;
}

/** Cells one button takes in the header: `[►]`. */
export const BUTTON_CELLS = 3;

const START: BotButton = { action: "start", label: "►", title: "Start the bot here: fight what comes, rest when low (F9)" };
const PAUSE: BotButton = { action: "pause", label: "║", title: "Pause the loop or Go to; ► goes on with it (F9)" };
const OFF: BotButton = { action: "pause", label: "║", title: "Turn the bot off: you play (F9)" };
const RESUME: BotButton = { action: "resume", label: "►", title: "Go on with the loop or Go to (F9)" };
const STOP: BotButton = { action: "stop", label: "■", title: "Drop the loop or Go to: the bot plays where it is (F10)" };
const DROP: BotButton = { action: "stop", label: "■", title: "Drop the loop or Go to: the bot stays off (F10)" };

/** The bot is on a loop or a Go to (a journey): ║ keeps it, ■ drops it. */
export function hasRoute(bot: Bot | undefined): boolean {
  return !!(bot?.task || bot?.journey);
}

/**
 * The buttons for the bot's state (docs/web-ui.md "The bot"): ► when off; ║ when on, and ■ beside
 * either while it has a loop or a Go to to drop. None without a bot.
 */
export function botButtons(bot: Bot | undefined): BotButton[] {
  const route = hasRoute(bot);
  switch (bot?.state) {
    case "stopped":
      return [START];
    case "running":
      return route ? [PAUSE, STOP] : [OFF];
    case "paused":
      return route ? [RESUME, DROP] : [RESUME];
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

/**
 * What a key asks of a bot in this state: `F9` is ║ and ► (pause, resume, or start when off);
 * `F10` is ■ (it drops the loop or Go to; with none, it turns a bot that is on off, and starts
 * one that is off).
 */
export function botKeyAction(key: string, bot: Bot | undefined): BotAction | null {
  if (!bot) return null;
  if (key === PAUSE_KEY) {
    return bot.state === "running" ? "pause" : bot.state === "paused" ? "resume" : "start";
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
export function errandItems(
  bot: Bot | undefined,
  errand: string | undefined,
  places: Place[] = [],
): ErrandItem[] {
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
  const died = places.some((p) => p.kind === "death");
  items.push(
    {
      action: "train",
      label: "Train",
      title: why || "Walk to the right trainer and train levels; stays there for you to train stats",
      disabled: busy || running,
    },
    {
      action: "recover_pile",
      label: "Recover death pile",
      title:
        why ||
        (died ? "Walk to where the character last died, pick up what is there and equip it" : "No death on record"),
      disabled: busy || running || !died,
    },
  );
  items.push({
    action: "spell_plan",
    label: "Spell plan…",
    title: "Which spells the class lacks, where to buy them and what they cost",
    disabled: false,
  });
  if (busy) items.push({ action: "cancel_errand", label: "Cancel errand", title: errand, disabled: false });
  return items;
}
