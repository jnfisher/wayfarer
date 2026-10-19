// Check that a tile's side borders meet the header's ╔ ╗ and the bottom's ╚ ╝ glyphs, at a few
// browser zooms (emulated with the device pixel ratio). Needs a running gateway and Chromium
// (`npx playwright install chromium`). Not part of CI.
//
//   node scripts/frame-check.mjs http://127.0.0.1:7373/ OUT_DIR [WIDTHxHEIGHT]
//
// For each zoom it writes OUT_DIR/page-<zoom>.png and OUT_DIR/corners-<zoom>.png (the four
// corners of the first tile, enlarged 8x), and prints which pixel columns are lit in the ╔ glyph
// and in the side border just below it: they should be the same.

import { mkdirSync } from "node:fs";
import { chromium } from "playwright";

const [url = "http://127.0.0.1:7373/", out = "frame-check", size = "1600x900"] = process.argv.slice(2);
const [width, height] = size.split("x").map(Number);
const ZOOMS = [0.9, 1, 1.25];
const CORNER = { w: 24, h: 40 };
mkdirSync(out, { recursive: true });

const browser = await chromium.launch();
let failed = false;
try {
  for (const zoom of ZOOMS) {
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: zoom });
    await page.goto(url);
    await page.waitForSelector(".tile .xterm-screen");
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(1500);
    await page.screenshot({ path: `${out}/page-${zoom}.png` });
    const r = await page.locator(".tile").first().boundingBox();
    const clips = [
      { x: r.x, y: r.y },
      { x: r.x + r.width - CORNER.w, y: r.y },
      { x: r.x, y: r.y + r.height - CORNER.h },
      { x: r.x + r.width - CORNER.w, y: r.y + r.height - CORNER.h },
    ].map((p) => ({ ...p, width: CORNER.w, height: CORNER.h }));
    const shots = [];
    for (const clip of clips) shots.push((await page.screenshot({ clip, scale: "device" })).toString("base64"));
    await page.close();

    // Lay the corners out side by side, enlarged, and read the lit columns of the top-left one.
    const view = await browser.newPage({ viewport: { width: 1200, height: 400 } });
    await view.setContent(
      `<body style="margin:0;background:#333;display:flex;gap:16px;padding:16px">` +
        shots
          .map(
            (b) =>
              `<img src="data:image/png;base64,${b}" style="width:${CORNER.w * 8}px;height:${CORNER.h * 8}px;image-rendering:pixelated">`,
          )
          .join("") +
        `</body>`,
    );
    const lit = await view.evaluate(async ({ b64, zoom }) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = document.createElement("canvas");
      c.width = img.width;
      c.height = img.height;
      const g = c.getContext("2d");
      g.drawImage(img, 0, 0);
      // Device-pixel rows: 12 CSS px is in the glyph's vertical strokes, 32 in the side border.
      const row = (cssY) => {
        const y = Math.round(cssY * zoom);
        const d = g.getImageData(0, y, Math.round(8 * zoom), 1).data;
        let s = "";
        for (let x = 0; x < d.length / 4; x++) s += d[x * 4] + d[x * 4 + 1] + d[x * 4 + 2] > 300 ? "#" : ".";
        return s;
      };
      return { glyph: row(12), side: row(32) };
    }, { b64: shots[0], zoom });
    await view.screenshot({ path: `${out}/corners-${zoom}.png`, fullPage: true });
    await view.close();
    const ok = lit.glyph === lit.side;
    failed ||= !ok;
    console.log(`zoom ${zoom}: ╔ ${lit.glyph}  side ${lit.side}  ${ok ? "match" : "DIFFER"}`);
  }
} finally {
  await browser.close();
}
process.exit(failed ? 1 : 0);
