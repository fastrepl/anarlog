// Renders an anarlog-film page with headless Chromium.
//   node render.mjs <film.html> shots <outDir> [t1,t2,...]   one PNG per film time (default: every 2s)
//   node render.mjs <film.html> frames <outDir> [fps]        every frame, for the MP4 (default 30fps)
// The stage size comes from the page (window.__film.width/height); FILM_SIZE=1080x1920 overrides it.
// Playwright resolves from PLAYWRIGHT_MODULE, then this repo's node_modules, then a sibling
// ~/Dev/char/storybook install.
import { execSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const [, , srcArg, mode, outArg, extra] = process.argv;
if (!srcArg || !["shots", "frames"].includes(mode) || !outArg) {
  console.error(
    "usage: node render.mjs <film.html> shots|frames <outDir> [times|fps]",
  );
  process.exit(1);
}
const src = path.resolve(srcArg);
const out = path.resolve(outArg);
fs.mkdirSync(out, { recursive: true });

function repoRoot() {
  try {
    return execSync("git rev-parse --show-toplevel", {
      cwd: path.dirname(new URL(import.meta.url).pathname),
      stdio: ["ignore", "pipe", "ignore"],
    })
      .toString()
      .trim();
  } catch {
    return null;
  }
}

async function loadPlaywright() {
  const tried = [];
  if (process.env.PLAYWRIGHT_MODULE) {
    const entry = process.env.PLAYWRIGHT_MODULE;
    tried.push(`PLAYWRIGHT_MODULE=${entry}`);
    try {
      const resolved = fs.statSync(entry).isDirectory()
        ? createRequire(path.join(entry, "package.json")).resolve(entry)
        : entry;
      const mod = await import(pathToFileURL(resolved).href);
      return mod.chromium ? mod : mod.default;
    } catch (e) {
      console.error(`PLAYWRIGHT_MODULE could not be loaded: ${e.message}`);
    }
  }
  const bases = [];
  const root = repoRoot();
  if (root) bases.push(path.join(root, "package.json"));
  bases.push(path.join(os.homedir(), "Dev/char/storybook/package.json"));
  for (const base of bases) {
    tried.push(base);
    if (!fs.existsSync(base)) continue;
    try {
      return createRequire(base)("playwright");
    } catch {}
  }
  console.error(
    "Playwright not found. Tried:\n  " +
      tried.join("\n  ") +
      "\nSet PLAYWRIGHT_MODULE to an installed playwright package directory, or install it once" +
      " outside this repo (e.g. `npm i playwright` in a scratch folder, then" +
      " `npx playwright install chromium`).",
  );
  process.exit(1);
}

const { chromium } = await loadPlaywright();

// The film is authored as a page body (an Artifact host adds the document shell); wrap it in a temp
// copy beside the film so relative img/ paths resolve, and remove it afterwards.
const wrapped = path.join(path.dirname(src), `.render-${process.pid}.html`);
fs.writeFileSync(
  wrapped,
  '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"></head><body>' +
    fs.readFileSync(src, "utf8") +
    "</body></html>",
);

const browser = await chromium.launch({
  args: ["--allow-file-access-from-files"],
});
const page = await browser.newPage({
  viewport: { width: 1920, height: 1080 },
  deviceScaleFactor: 1,
});
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
try {
  await page.goto(pathToFileURL(wrapped).href, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  const size = await page.evaluate(() => [
    window.__film.width || 1920,
    window.__film.height || 1080,
  ]);
  const [width, height] = process.env.FILM_SIZE
    ? process.env.FILM_SIZE.split("x").map(Number)
    : size;
  if (width !== size[0] || height !== size[1]) {
    console.error(
      `warning: FILM_SIZE ${width}x${height} differs from the page's ${size[0]}x${size[1]}`,
    );
  }
  await page.setViewportSize({ width, height });
  await page.evaluate(() => window.__film.exportMode());
  const duration = await page.evaluate(() => window.__film.duration);

  const shoot = async (t, file) => {
    await page.evaluate(async (s) => {
      window.__film.seek(s);
      await window.__film.settle?.();
    }, t);
    await page.screenshot({
      path: path.join(out, file),
      clip: { x: 0, y: 0, width, height },
    });
  };

  if (mode === "shots") {
    const times = extra
      ? extra.split(",").map(Number)
      : Array.from({ length: Math.ceil(duration / 2) }, (_, i) => i * 2);
    for (const f of fs.readdirSync(out))
      if (/^t[\d.]+\.png$/.test(f)) fs.rmSync(path.join(out, f));
    for (const t of times)
      await shoot(t, "t" + t.toFixed(2).padStart(6, "0") + ".png");
    console.log(`${times.length} shots (${width}x${height}) in ${out}`);
  } else {
    const fps = Number(extra || 30);
    const n = Math.round(duration * fps);
    // A shorter re-render must not leave the last cut's tail frames for mux.sh to pick up.
    for (const f of fs.readdirSync(out))
      if (/^f\d{5}\.png$/.test(f)) fs.rmSync(path.join(out, f));
    for (let i = 0; i < n; i++)
      await shoot(i / fps, "f" + String(i).padStart(5, "0") + ".png");
    console.log(
      `${n} frames (${duration}s at ${fps}fps, ${width}x${height}) in ${out}`,
    );
  }
} finally {
  await browser.close();
  fs.rmSync(wrapped, { force: true });
}
if (errors.length) {
  console.error("page errors:\n" + [...new Set(errors)].join("\n"));
  process.exitCode = 1;
}
