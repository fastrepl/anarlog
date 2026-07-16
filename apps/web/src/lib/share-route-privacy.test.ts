import assert from "node:assert/strict";
import test from "node:test";

import {
  clearShareRouteToken,
  getShareRouteToken,
  isCapabilityShareRoutePathname,
  isShareRoutePathname,
  parseShareFragmentToken,
  prepareShareRoutePrivacy,
} from "./share-route-privacy.ts";

const TOKEN = "a".repeat(43);

test("identifies every shared-note route as telemetry private", () => {
  for (const pathname of [
    "/share/abc/",
    "/share/invite/abc/",
    "/share/link/abc/",
    "/share/public/s_abc/",
  ]) {
    assert.equal(isShareRoutePathname(pathname), true);
  }
  assert.equal(isShareRoutePathname("/blog/share/"), false);
});

test("identifies only invitation and link routes as capability routes", () => {
  assert.equal(isCapabilityShareRoutePathname("/share/invite/abc/"), true);
  assert.equal(isCapabilityShareRoutePathname("/share/link/abc/"), true);
  assert.equal(isCapabilityShareRoutePathname("/share/abc/"), false);
  assert.equal(isCapabilityShareRoutePathname("/share/public/s_abc/"), false);
});

test("accepts one valid fragment token and rejects ambiguous fragments", () => {
  assert.equal(parseShareFragmentToken(`#token=${TOKEN}`), TOKEN);
  assert.equal(parseShareFragmentToken(`#token=${TOKEN}&next=/`), null);
  assert.equal(parseShareFragmentToken(`#other=${TOKEN}`), null);
  assert.equal(parseShareFragmentToken("#token=short"), null);
  assert.equal(parseShareFragmentToken(`?token=${TOKEN}`), null);
});

test("scrubs a capability fragment before retaining it for the current tab", () => {
  const pathname = "/share/link/00000000-0000-4000-8000-000000000001/";
  const storage = new Map<string, string>();
  let replacedUrl = "";

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      history: {
        state: null,
        replaceState: (_state: unknown, _title: string, url: string) => {
          replacedUrl = url;
        },
      },
      location: {
        hash: `#token=${TOKEN}`,
        pathname,
        search: "",
      },
      sessionStorage: {
        getItem: (key: string) => storage.get(key) ?? null,
        removeItem: (key: string) => storage.delete(key),
        setItem: (key: string, value: string) => storage.set(key, value),
      },
    },
  });

  try {
    prepareShareRoutePrivacy();
    assert.equal(replacedUrl, pathname);
    assert.equal(replacedUrl.includes(TOKEN), false);
    assert.equal(getShareRouteToken(pathname), TOKEN);
    clearShareRouteToken(pathname);
    assert.equal(getShareRouteToken(pathname), null);
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});
