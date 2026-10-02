import { isRedirect } from "@tanstack/react-router";
import assert from "node:assert/strict";
import test from "node:test";

import { Route as nightlyDownloadRoute } from "../routes/_view/download/nightly/index.ts";
import {
  desktopDownloadSections,
  detectDownloadPlatform,
  getOrderedDownloadSections,
} from "./download.ts";

test("detects the download platform from the user agent", () => {
  const cases = [
    ["Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36", "windows"],
    [
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
      "macos",
    ],
    ["Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36", "linux"],
    [
      "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15",
      "ios",
    ],
    [
      "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15",
      "ios",
    ],
    [
      "Mozilla/5.0 (iPod touch; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15",
      "ios",
    ],
    ["Mozilla/5.0 (Linux; Android 16; Pixel 10) AppleWebKit/537.36", "android"],
    ["Mozilla/5.0 (X11; CrOS x86_64)", "macos"],
    ["unknown", "macos"],
    ["", "macos"],
  ] as const;

  for (const [userAgent, expected] of cases) {
    assert.equal(detectDownloadPlatform(userAgent), expected, userAgent);
  }

  const macUserAgent =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
  assert.equal(detectDownloadPlatform(macUserAgent, 5), "ios");
  assert.equal(detectDownloadPlatform(macUserAgent, 0), "macos");
  assert.equal(detectDownloadPlatform(macUserAgent, 1), "macos");
});

test("orders the detected platform first", () => {
  assert.deepEqual(
    getOrderedDownloadSections("windows").map((section) => section.platform),
    ["windows", "macos", "linux", "ios", "android"],
  );
  for (const platform of ["ios", "android"] as const) {
    assert.equal(getOrderedDownloadSections(platform)[0].platform, "macos");
  }
});

test("public desktop downloads use only the stable channel", () => {
  for (const section of desktopDownloadSections) {
    for (const download of section.downloads) {
      const url = new URL(download.url);
      if (url.hostname === "desktop.anarlog.so") {
        assert.equal(url.searchParams.get("channel"), "stable");
      }
    }
  }
});

test("old Nightly download links permanently redirect to stable downloads", () => {
  assert.throws(
    () => nightlyDownloadRoute.options.beforeLoad!({} as never),
    (error: unknown) => {
      assert.ok(isRedirect(error));
      assert.equal(error.status, 301);
      assert.equal(error.options.to, "/download/");
      return true;
    },
  );
});
