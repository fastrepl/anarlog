import assert from "node:assert/strict";
import test from "node:test";

import {
  comingSoonPlatforms,
  desktopDownloadSections,
  detectDesktopPlatform,
  getOrderedDesktopDownloadSections,
} from "./download.ts";

test("offers macOS, Windows, and Linux downloads", () => {
  assert.deepEqual(
    desktopDownloadSections.map((section) => section.platform),
    ["macos", "windows", "linux"],
  );
  assert.deepEqual(comingSoonPlatforms, [
    "iOS",
    "Android",
    "Apple Watch",
    "Galaxy Watch",
  ]);

  const windowsDownloads = desktopDownloadSections.find(
    (section) => section.platform === "windows",
  )!.downloads;
  const linuxDownloads = desktopDownloadSections.find(
    (section) => section.platform === "linux",
  )!.downloads;

  assert.match(windowsDownloads[0].url, /\/nsis-x86_64\?/);
  assert.deepEqual(
    linuxDownloads.map((download) =>
      new URL(download.url).pathname.split("/").at(-1),
    ),
    ["appimage-x86_64", "debian-x86_64", "appimage-aarch64", "debian-aarch64"],
  );
});

test("detects supported desktop platforms from browser user agents", () => {
  assert.equal(
    detectDesktopPlatform(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    ),
    "windows",
  );
  assert.equal(
    detectDesktopPlatform(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
    ),
    "macos",
  );
  assert.equal(
    detectDesktopPlatform("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"),
    "linux",
  );
});

test("falls back to macOS for unsupported and unknown platforms", () => {
  assert.equal(
    detectDesktopPlatform(
      "Mozilla/5.0 (Linux; Android 16; Pixel 10) AppleWebKit/537.36",
    ),
    "macos",
  );
  assert.equal(detectDesktopPlatform("unknown"), "macos");
});

test("orders the detected platform first", () => {
  assert.deepEqual(
    getOrderedDesktopDownloadSections("windows").map(
      (section) => section.platform,
    ),
    ["windows", "macos", "linux"],
  );
  assert.equal(
    getOrderedDesktopDownloadSections("macos")[0].downloads[0].name,
    "Apple Silicon",
  );
});
