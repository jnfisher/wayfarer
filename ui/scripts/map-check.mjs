// Drive the map panel against a running gateway with a brain and a character in the realm
// (docs/web-ui.md, "The map"): open it, walk by hand, click a room three steps away, stop a
// walk, and screenshot each. Needs Chromium once (`npx playwright install chromium`). Not part
// of CI; it moves the character.
//
//   node scripts/map-check.mjs URL SESSION OUT_DIR [WIDTHxHEIGHT]

import { mkdirSync } from "node:fs";
import { chromium } from "playwright";

const [url, session, out, size = "1600x900"] = process.argv.slice(2);
if (!url || !session || !out) {
  console.error("usage: map-check.mjs URL SESSION OUT_DIR [WIDTHxHEIGHT]");
  process.exit(2);
}
const [width, height] = size.split("x").map(Number);
mkdirSync(out, { recursive: true });

// A second UI connection, to watch what the brain sends.
const seen = { view: null, character: {}, notices: [] };
const ws = new WebSocket(url.replace(/^http/, "ws").replace(/\/?$/, "/ws"));
const merge = (t, p) => {
  if (typeof p !== "object" || p === null || Array.isArray(p)) return p;
  const o = typeof t === "object" && t !== null && !Array.isArray(t) ? t : {};
  for (const [k, v] of Object.entries(p)) v === null ? delete o[k] : (o[k] = merge(o[k], v));
  return o;
};
ws.onopen = () => ws.send(JSON.stringify({ type: "hello", protocol: 1, client: "ui" }));
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.type === "welcome") ws.send(JSON.stringify({ type: "state_request" }));
  if (m.session !== session) return;
  if (m.type === "map_view") seen.view = m;
  if (m.type === "character") seen.character = m.state;
  if (m.type === "character_patch") seen.character = merge(seen.character, m.patch);
  if (m.type === "notice" || m.type === "warning") {
    seen.notices.push(`${m.type}: ${m.text}`);
    console.log(`${m.type}: ${m.text}`);
  }
};

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, f, ms = 30_000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = f();
    if (v) return v;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${what}`);
}
const key = (k) => `${k.map}:${k.room}`;
const here = () => seen.character.position && `${seen.character.position.map}:${seen.character.position.room}`;

/** Rooms of the view by steps from its centre, not through hidden exits (the walker won't search). */
function distances(view) {
  const adj = new Map();
  const add = (a, b) => (adj.get(key(a)) ?? adj.set(key(a), []).get(key(a))).push(key(b));
  for (const e of view.exits) {
    if (e.kind === "hidden") continue;
    add(e.from, e.to);
    if (!e.one_way) add(e.to, e.from);
  }
  const dist = new Map([[key(view.center), 0]]);
  const queue = [key(view.center)];
  while (queue.length) {
    const k = queue.shift();
    for (const n of adj.get(k) ?? []) if (!dist.has(n)) dist.set(n, dist.get(k) + 1), queue.push(n);
  }
  return dist;
}

/** A room `steps` away: on the centre's level, nothing spawning, not collided; not a shop unless `any`. */
function pick(view, steps, any = false) {
  const dist = distances(view);
  return view.rooms.find(
    (r) => dist.get(key(r)) === steps && r.level === 0 && !r.spawns && !r.collides && (any || !r.kind),
  );
}

const browser = await chromium.launch();
const results = [];
const check = (name, ok, note = "") => {
  results.push(`${ok ? "ok  " : "FAIL"} ${name}${note ? `: ${note}` : ""}`);
  console.log(results.at(-1));
};
try {
  const page = await browser.newPage({ viewport: { width, height } });
  page.on("console", (msg) => console.log(`page: ${msg.text()}`));
  await page.goto(url);
  await page.waitForSelector(".tile .xterm-screen");
  await page.evaluate(() => document.fonts.ready);
  await until("the character's position", () => here());

  // Open the map with Alt+M.
  await page.keyboard.press("Alt+KeyM");
  await page.waitForSelector(".drawer canvas");
  await until("a map_view", () => seen.view && key(seen.view.center) === here());
  await sleep(500);
  await page.screenshot({ path: `${out}/map-open.png` });
  check("Alt+M opens the map on the believed room", true, `${here()} ${seen.character.position.title}`);

  const canvas = await page.locator(".drawer canvas").boundingBox();
  // At the default zoom rooms are 40 px apart across and 48 down, the believed room centred.
  const at = (r) => [canvas.x + canvas.width / 2 + r.x * 40, canvas.y + canvas.height / 2 + r.y * 48];
  const input = page.locator(".tile input").first();

  // Walk by hand: type an exit's direction; the lit room moves.
  const start = seen.view.center;
  const exit = seen.view.exits.find((e) => key(e.from) === key(start) && ["n", "s", "e", "w"].includes(e.dir));
  if (exit) {
    await input.fill(exit.dir);
    await input.press("Enter");
    await until("the map to follow a typed move", () => key(seen.view.center) === key(exit.to));
    await sleep(500);
    await page.screenshot({ path: `${out}/map-typed.png` });
    check("typing a move moves the lit room", here() === key(exit.to), `${exit.dir} to ${key(exit.to)}`);
  } else {
    check("typing a move moves the lit room", false, "no plain compass exit here");
  }

  // Click a room three steps away: it walks there, the route drawn.
  const target = pick(seen.view, 3);
  if (!target) throw new Error("no room three steps away to walk to");
  const [x, y] = at(target);
  await page.mouse.move(x, y);
  await sleep(300);
  await page.screenshot({ path: `${out}/map-hover.png` });
  await page.mouse.click(x, y);
  await until("the walk to start", () => seen.character.walking);
  await until("the route on the map", () => seen.view.route?.length > 1);
  await sleep(200);
  await page.screenshot({ path: `${out}/map-walking.png` });
  const bar = await page.locator(".drawer .bar").innerText();
  check("the bar shows the walk", /step \d+, \d+ left/.test(bar), bar.replace(/\s+/g, " "));
  await until("arrival", () => !seen.character.walking, 60_000);
  check("clicking a room 3 steps away walks there", here() === key(target), `${key(target)} ${target.name}`);
  await sleep(500);
  await page.screenshot({ path: `${out}/map-arrived.png` });

  // Stop: start a longer walk and stop it after the first step.
  const far = [8, 7, 6, 5, 4].map((n) => pick(seen.view, n, true)).find(Boolean);
  if (far) {
    const [fx, fy] = at(far);
    await page.mouse.click(fx, fy);
    await until("the long walk to start", () => seen.character.walking);
    await until("a first step", () => seen.character.walking?.step >= 2, 30_000).catch(() => null);
    await page.getByRole("button", { name: "stop" }).click();
    await until("the walk to stop", () => !seen.character.walking, 30_000);
    check("stop stops the walk", here() !== key(far), `stopped at ${here()}, was going to ${key(far)}`);
  } else {
    const dist = distances(seen.view);
    const counts = {};
    for (const d of dist.values()) counts[d] = (counts[d] ?? 0) + 1;
    check("stop stops the walk", false, `no room far enough from ${key(seen.view.center)}: ${JSON.stringify(counts)}`);
  }

  // Back to where it started, so the character is left where it was found.
  await until("a view", () => seen.view);
  const home = seen.view.rooms.find((r) => key(r) === key(start));
  if (home && here() !== key(start)) {
    ws.send(JSON.stringify({ type: "walk_to", session, map: start.map, room: start.room }));
    await until("the walk home to end", () => here() === key(start) && !seen.character.walking, 60_000).catch(() => null);
  }
  check("back where it started", here() === key(start), here());
} finally {
  await browser.close();
  ws.close();
  console.log(`\n${results.join("\n")}`);
}
