import assert from "node:assert/strict";
import test from "node:test";

import {
  consumeDesktopAuthHandoff,
  isTelemetryPrivateLocation,
  prepareAuthRoutePrivacy,
} from "./auth-route-privacy.ts";

test("keeps auth, callback, recovery, and share routes telemetry private", () => {
  for (const pathname of [
    "/auth/",
    "/callback/auth/",
    "/confirm-auth/",
    "/reset-password/",
    "/update-password/",
    "/share/link/example/",
  ]) {
    assert.equal(isTelemetryPrivateLocation(pathname), true);
  }

  assert.equal(isTelemetryPrivateLocation("/blog/auth/"), false);
  assert.equal(isTelemetryPrivateLocation("/"), false);
});

test("keeps legacy root callbacks private only when auth parameters exist", () => {
  assert.equal(isTelemetryPrivateLocation("/", "?code=secret"), true);
  assert.equal(
    isTelemetryPrivateLocation("/", { token_hash: "secret", type: "email" }),
    true,
  );
  assert.equal(isTelemetryPrivateLocation("/", "?period=monthly"), false);
});

test("stores desktop handoff tokens, removes secrets from history, and consumes them once", () => {
  const location = {
    hash: "",
    pathname: "/callback/auth/",
    search:
      "?flow=desktop&scheme=hyprnote&access_token=access&refresh_token=refresh",
  };
  const storage = new Map<string, string>();
  let replacedUrl = "";

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      history: {
        state: { key: "router" },
        replaceState: (_state: unknown, _title: string, url: string) => {
          replacedUrl = url;
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
    prepareAuthRoutePrivacy(1_000);
    assert.equal(
      replacedUrl,
      "/callback/auth/?flow=desktop&scheme=hyprnote&handoff=stored",
    );
    assert.deepEqual(consumeDesktopAuthHandoff(1_000), {
      accessToken: "access",
      refreshToken: "refresh",
    });
    assert.equal(consumeDesktopAuthHandoff(1_000), null);
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});

test("rejects and consumes expired desktop handoff tokens", () => {
  const storage = new Map<string, string>();

  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      history: {
        state: null,
        replaceState: () => {},
      },
      location: {
        hash: "",
        pathname: "/callback/auth/",
        search: "?access_token=access&refresh_token=refresh",
      },
      sessionStorage: {
        getItem: (key: string) => storage.get(key) ?? null,
        removeItem: (key: string) => storage.delete(key),
        setItem: (key: string, value: string) => storage.set(key, value),
      },
    },
  });

  try {
    prepareAuthRoutePrivacy(1_000);
    assert.equal(consumeDesktopAuthHandoff(5 * 60 * 1000 + 1_001), null);
    assert.equal(consumeDesktopAuthHandoff(1_000), null);
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});

test("does not rewrite one-time verification URLs before the router exchanges them", () => {
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
        hash: "",
        pathname: "/",
        search: "?token_hash=secret&type=recovery&flow=web",
      },
      sessionStorage: {
        getItem: () => null,
        setItem: () => {},
      },
    },
  });

  try {
    prepareAuthRoutePrivacy();
    assert.equal(replacedUrl, "");
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});

test("keeps desktop handoff tokens in the URL when storage is unavailable", () => {
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
        hash: "",
        pathname: "/callback/auth/",
        search: "?access_token=access&refresh_token=refresh",
      },
      sessionStorage: {
        getItem: () => null,
        setItem: () => {
          throw new Error("storage unavailable");
        },
      },
    },
  });

  try {
    prepareAuthRoutePrivacy();
    assert.equal(replacedUrl, "");
    assert.equal(consumeDesktopAuthHandoff(), null);
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});
