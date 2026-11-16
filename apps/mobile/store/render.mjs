// Renders App Store / Google Play screenshots and the Play feature graphic.
// Usage: node apps/mobile/store/render.mjs [outDir]
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright-core";

const here = dirname(fileURLToPath(import.meta.url));
const out = resolve(process.argv[2] ?? join(here, "out"));
const page = pathToFileURL(join(here, "screens.html")).href;
const targets = [
  {
    platform: "ios",
    dir: "app-store-iphone-6.9",
    width: 440,
    height: 956,
    scale: 3,
  },
  { platform: "android", dir: "play-phone", width: 360, height: 640, scale: 3 },
];

const browser = await chromium.launch();
for (const target of targets) {
  mkdirSync(join(out, target.dir), { recursive: true });
  const context = await browser.newContext({
    viewport: { width: target.width, height: target.height },
    deviceScaleFactor: target.scale,
  });
  const tab = await context.newPage();
  for (let frame = 0; frame < 5; frame++) {
    await tab.goto(`${page}?platform=${target.platform}&frame=${frame}`);
    await tab.locator(".frame").screenshot({
      path: join(out, target.dir, `${frame + 1}.png`),
      omitBackground: false,
    });
  }
  await context.close();
}
const context = await browser.newContext({
  viewport: { width: 1024, height: 500 },
  deviceScaleFactor: 1,
});
const tab = await context.newPage();
await tab.goto(pathToFileURL(join(here, "feature.html")).href);
await tab
  .locator(".frame")
  .screenshot({ path: join(out, "play-feature-graphic.png") });
await browser.close();
console.log(out);
