import assert from "node:assert/strict";
import test from "node:test";

import {
  comingSoonPlatforms,
  desktopDownloadSections,
  detectDesktopPlatform,
  getOrderedDesktopDownloadSections,
} from "./download.ts";

test("offers macOS downloads while Linux and Windows are coming soon", () => {
  assert.deepEqual(
    desktopDownloadSections.map((section) => section.platform),
    ["macos"],
  );
  assert.deepEqual(comingSoonPlatforms.slice(0, 2), ["Linux", "Windows"]);
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
    ["macos"],
  );
  assert.equal(
    getOrderedDesktopDownloadSections("macos")[0].downloads[0].name,
    "Apple Silicon",
  );
});
