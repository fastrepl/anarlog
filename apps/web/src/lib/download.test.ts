import { isRedirect } from "@tanstack/react-router";
import assert from "node:assert/strict";
import test from "node:test";

import { Route as nightlyDownloadRoute } from "../routes/_view/download/nightly/index.ts";
import {
  comingSoonPlatforms,
  desktopDownloadSections,
  detectDownloadPlatform,
  getOrderedDownloadSections,
  mobileDownloadSections,
  mobileLaunchLabel,
  windowsStoreDownloadUrl,
} from "./download.ts";

test("offers macOS, Windows, and Linux downloads", () => {
  assert.deepEqual(
    desktopDownloadSections.map((section) => section.platform),
    ["macos", "windows", "linux"],
  );
  assert.deepEqual(
    desktopDownloadSections.map((section) => section.status),
    [null, null, null],
  );
  assert.deepEqual(
    mobileDownloadSections.map((section) => section.status),
    [mobileLaunchLabel, mobileLaunchLabel],
  );
  assert.deepEqual(
    mobileDownloadSections.map((section) => section.available),
    [false, false],
  );
  assert.deepEqual(comingSoonPlatforms, ["Apple Watch", "Galaxy Watch"]);

  const macosDownloads = desktopDownloadSections.find(
    (section) => section.platform === "macos",
  )!.downloads;
  const windowsDownloads = desktopDownloadSections.find(
    (section) => section.platform === "windows",
  )!.downloads;
  const linuxDownloads = desktopDownloadSections.find(
    (section) => section.platform === "linux",
  )!.downloads;

  assert.match(
    macosDownloads[0].url,
    /^https:\/\/desktop\.anarlog\.so\/download\/latest\/platform\/dmg-aarch64\?/,
  );
  assert.equal(windowsDownloads.length, 2);
  assert.equal(windowsDownloads[0].name, "Windows x64");
  assert.match(
    windowsDownloads[0].url,
    /^https:\/\/desktop\.anarlog\.so\/download\/latest\/platform\/nsis-x86_64\?/,
  );
  assert.equal(windowsDownloads[1].name, "Microsoft Store");
  assert.equal(windowsDownloads[1].url, windowsStoreDownloadUrl);
  assert.equal(windowsDownloads[1].actionLabel, "Get from Store");
  assert.deepEqual(
    linuxDownloads.map((download) =>
      new URL(download.url).pathname.split("/").at(-1),
    ),
    [
      "desktop-installation",
      "appimage-x86_64",
      "debian-x86_64",
      "appimage-aarch64",
      "debian-aarch64",
      "anarlog-bin",
    ],
  );
});

test("detects supported desktop platforms from browser user agents", () => {
  assert.equal(
    detectDownloadPlatform(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    ),
    "windows",
  );
  assert.equal(
    detectDownloadPlatform(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
    ),
    "macos",
  );
  assert.equal(
    detectDownloadPlatform(
      "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
    ),
    "linux",
  );
});

test("detects phones and tablets but keeps desktop downloads first until mobile launches", () => {
  for (const userAgent of [
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15",
    "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15",
    "Mozilla/5.0 (iPod touch; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15",
  ]) {
    const platform = detectDownloadPlatform(userAgent);
    assert.equal(platform, "ios");
    assert.equal(getOrderedDownloadSections(platform)[0].platform, "macos");
  }

  const android = detectDownloadPlatform(
    "Mozilla/5.0 (Linux; Android 16; Pixel 10) AppleWebKit/537.36",
  );
  assert.equal(android, "android");
  assert.equal(getOrderedDownloadSections(android)[0].platform, "macos");
  assert.deepEqual(
    mobileDownloadSections.map((section) => section.platform),
    ["ios", "android"],
  );
});

test("recognizes iPad desktop browsing without treating Macs as iPads", () => {
  const userAgent =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
  assert.equal(detectDownloadPlatform(userAgent, 5), "ios");
  assert.equal(detectDownloadPlatform(userAgent, 0), "macos");
  assert.equal(detectDownloadPlatform(userAgent, 1), "macos");
});

test("falls back to macOS for unsupported and unknown platforms", () => {
  assert.equal(
    detectDownloadPlatform("Mozilla/5.0 (X11; CrOS x86_64)"),
    "macos",
  );
  assert.equal(detectDownloadPlatform("unknown"), "macos");
  assert.equal(detectDownloadPlatform(""), "macos");
});

test("orders the detected platform first", () => {
  assert.deepEqual(
    getOrderedDownloadSections("windows").map((section) => section.platform),
    ["windows", "macos", "linux", "ios", "android"],
  );
  assert.equal(
    getOrderedDownloadSections("macos")[0].downloads[0].name,
    "Apple Silicon",
  );
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
