import assert from "node:assert/strict";
import test from "node:test";

import {
  clearPersistedShareRouteToken,
  clearShareRouteToken,
  getShareRouteToken,
  isCapabilityShareRoutePathname,
  isShareRoutePathname,
  loadShareRouteContinuation,
  parseShareFragmentToken,
  prepareShareRoutePrivacy,
  readShareRouteContinuationCookie,
  retainShareRouteToken,
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

test("reads a valid continuation cookie without consuming it", () => {
  let clearCount = 0;
  const clearInvalid = () => {
    clearCount += 1;
  };

  assert.equal(readShareRouteContinuationCookie(TOKEN, clearInvalid), TOKEN);
  assert.equal(clearCount, 0);
  assert.equal(readShareRouteContinuationCookie("short", clearInvalid), null);
  assert.equal(clearCount, 1);
});

test("preserves recovery state across cancellation and persistence failure", async () => {
  const controller = new AbortController();
  let retainedToken: string | null = null;
  let clearedPersisted = 0;

  await assert.rejects(
    loadShareRouteContinuation({
      clearPersisted: () => {
        clearedPersisted += 1;
      },
      localToken: null,
      persist: async () => true,
      restore: async () => {
        controller.abort();
        return TOKEN;
      },
      retain: (token) => {
        retainedToken = token;
        return true;
      },
      signal: controller.signal,
    }),
    { name: "AbortError" },
  );
  assert.equal(retainedToken, null);
  assert.equal(clearedPersisted, 0);

  await assert.rejects(
    loadShareRouteContinuation({
      clearPersisted: () => {
        clearedPersisted += 1;
      },
      localToken: null,
      persist: async () => false,
      restore: async () => TOKEN,
      retain: (token) => {
        retainedToken = token;
        return true;
      },
    }),
    /share continuation unavailable/,
  );
  assert.equal(retainedToken, TOKEN);
  assert.equal(clearedPersisted, 0);

  assert.equal(
    await loadShareRouteContinuation({
      clearPersisted: () => {
        clearedPersisted += 1;
      },
      localToken: retainedToken,
      persist: async () => true,
      restore: async () => null,
      retain: () => false,
    }),
    TOKEN,
  );
  assert.equal(clearedPersisted, 1);
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
    assert.equal(storage.size, 1);
    clearPersistedShareRouteToken(pathname);
    assert.equal(storage.size, 0);
    assert.equal(getShareRouteToken(pathname), TOKEN);
    clearShareRouteToken(pathname);
    assert.equal(getShareRouteToken(pathname), null);
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});

test("retains capability tokens across trailing-slash normalization", () => {
  for (const route of ["link", "invite"]) {
    const pathname = `/share/${route}/00000000-0000-4000-8000-000000000001`;
    const location = {
      hash: `#token=${TOKEN}`,
      pathname,
      search: "?scheme=hyprnote-staging",
    };
    const storage = new Map<string, string>();
    let replacedUrl = "";

    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: {
        history: {
          state: null,
          replaceState: (_state: unknown, _title: string, url: string) => {
            replacedUrl = url;
            location.hash = "";
          },
        },
        location,
        sessionStorage: {
          getItem: (key: string) => storage.get(key) ?? null,
          removeItem: (key: string) => storage.delete(key),
          setItem: (key: string, value: string) => storage.set(key, value),
        },
      },
    });

    try {
      prepareShareRoutePrivacy();
      assert.equal(replacedUrl, `${pathname}${location.search}`);
      location.pathname = `${pathname}/`;
      prepareShareRoutePrivacy();
      assert.equal(getShareRouteToken(location.pathname), TOKEN);
      clearShareRouteToken(location.pathname);
      assert.equal(getShareRouteToken(pathname), null);
    } finally {
      Reflect.deleteProperty(globalThis, "window");
    }
  }
});

test("restores only valid capability tokens for their exact route", () => {
  const pathname = "/share/invite/00000000-0000-4000-8000-000000000001/";
  const storage = new Map<string, string>();

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      sessionStorage: {
        getItem: (key: string) => storage.get(key) ?? null,
        removeItem: (key: string) => storage.delete(key),
        setItem: (key: string, value: string) => storage.set(key, value),
      },
    },
  });

  try {
    assert.equal(retainShareRouteToken(pathname, TOKEN), true);
    assert.equal(getShareRouteToken(pathname), TOKEN);
    assert.equal(
      retainShareRouteToken("/share/public/s_example/", TOKEN),
      false,
    );
    assert.equal(retainShareRouteToken(pathname, "short"), false);
  } finally {
    clearShareRouteToken(pathname);
    Reflect.deleteProperty(globalThis, "window");
  }
});
