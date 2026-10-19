// The bot's controls on a tile (docs/web-ui.md, "The bot"): the buttons in the header, the
// narration line under it, and what F9 and F10 do. The glyphs are the VGA font's own (code page
// 437 has ► and ■ but no pause sign, so pause is ║).

import type { Bot, BotAction } from "../proto";

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
