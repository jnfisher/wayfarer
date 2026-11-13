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

/** Cells the connection button takes, with the space before it (and connect's `▾`); 0 when
 * there is none. */
export function linkWidth(state: string): number {
  const b = linkButton(state);
  return b ? b.label.length + 3 + (b.action === "connect" ? 1 : 0) : 0;
}

/** What the connect menu's recovery does (C-093). */
export const RECOVER_TITLE =
  "Connect with the bot left stopped (so it does not hang up again at low HP) and sysop goto Silvermere at the first prompt, before anything attacks";

/** The towns `sysop goto` takes (C-080, `kb/navigation.md` "SYSOP GOTO"), as the brain names them. */
export const TOWNS: { town: string; label: string }[] = [
  { town: "newhaven", label: "Newhaven" },
  { town: "silvermere", label: "Silvermere" },
  { town: "rhudaur", label: "Rhudaur" },
  { town: "khazarad", label: "Khazarad" },
  { town: "lostcity", label: "Lost City" },
];

/** What a sysop goto from `who`'s menu does: its whole party goes when it is in one. */
export function sysGotoTitle(party: boolean): string {
  return party
    ? "Teleport the whole party with sysop goto; it forms up again there (a running bot keeps running)"
    : "Teleport with sysop goto (a running bot keeps running)";
}

export interface ErrandItem {
  /** A `bot_control` action, or `spell_plan`: a read-only plan, asked for with its own message. */
  action: BotAction | "spell_plan";
  label: string;
  title: string;
  disabled: boolean;
}

/** Cells the boss fight toggle `[-]` (`[☠]` when on, C-100; only with a bot) and the errand
 * menu button `[▼]` (with a brain) take, each with the space before it. */
export function menuWidth(state: string, hasBrain: boolean, hasBot = hasBrain): number {
  if (state !== "realm") return 0;
  return (hasBrain ? 4 : 0) + (hasBot ? 4 : 0);
}

/** The boss fight toggle's title: what it does now (C-100). */
export function bossTitle(on: boolean): string {
  return on
    ? "Boss fight mode is on: fighting where it stands, no fleeing, no walking, no blessings; hangs up at its own line. Click to turn it off."
    : "Boss fight mode: fight where it stands, never flee or walk, no blessings, its own casts limit and hang-up line. Turning it on stops a loop or Go to and starts a stopped bot.";
}

/**
 * The entries of a tile's errand menu: pick up all, equip all, train and recover the death pile
 * (not while an errand is on), and cancel while one is. With the bot running they go too (C-079):
 * its loop or Go to stops, and it keeps running beside the errand, fighting what comes.
 */
export function errandItems(
  bot: Bot | undefined,
  errand: string | undefined,
  places: Place[] = [],
): ErrandItem[] {
  const busy = errand !== undefined;
  const running = bot?.state === "running";
  const why = busy ? "An errand is in progress" : "";
  /** What a running bot does when the errand starts. */
  const beside = running ? " (the bot's loop or Go to stops; it keeps running and fights)" : "";
  const items: ErrandItem[] = [
    {
      action: "pick_up_all",
      label: "Pick up all",
      title: why || `Take the coins and the items on the floor here${beside}`,
      disabled: busy,
    },
    {
      action: "equip_all",
      label: "Equip all",
      title: why || `Wear and wield what is carried, into empty slots only${beside}`,
      disabled: busy,
    },
  ];
  const died = places.some((p) => p.kind === "death");
  items.push(
    {
      action: "train",
      label: "Train",
      title: why || `Walk to the right trainer and train levels; stays there for you to train stats${beside}`,
      disabled: busy,
    },
    {
      action: "recover_pile",
      label: "Recover death pile",
      title:
        why ||
        (died ? `Walk to where the character last died, pick up what is there and equip it${beside}` : "No death on record"),
      disabled: busy || !died,
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
