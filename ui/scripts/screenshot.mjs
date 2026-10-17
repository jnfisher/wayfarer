// Screenshot the UI as served by a running gateway (needs Chromium once:
// `npx playwright install chromium`). Not part of CI.
//
//   npm run screenshot -- http://127.0.0.1:7373/ out.png [WIDTHxHEIGHT]

import { chromium } from "playwright";

const [url = "http://127.0.0.1:7373/", out = "screenshot.png", size = "1600x900"] =
  process.argv.slice(2);
const [width, height] = size.split("x").map(Number);

const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width, height } });
  page.on("console", (msg) => console.log(`page: ${msg.text()}`));
  await page.goto(url);
  await page.waitForSelector(".tile .xterm-screen");
  await page.evaluate(() => document.fonts.ready);
  // Let the backfill and the replay land.
  await page.waitForTimeout(2000);
  await page.screenshot({ path: out });
  console.log(`wrote ${out}`);
} finally {
  await browser.close();
}
