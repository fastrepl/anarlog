import assert from "node:assert/strict";
import test from "node:test";

import {
  createPrivateRouteIdentity,
  parsePostHogDistinctId,
} from "./private-route-analytics-identity.ts";

test("parses raw PostHog localStorage JSON without decoding its contents", () => {
  const raw = JSON.stringify({
    distinct_id: "anonymous-id",
    $initial_referrer: "https://example.com/%E0%A4%A",
  });

  assert.equal(parsePostHogDistinctId(raw), "anonymous-id");
});

test("never reuses an identified user as the next anonymous id", () => {
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
    const identity = createPrivateRouteIdentity(() => "fresh-anonymous-id");

    assert.equal(
      identity.distinctIdForEvent("original-anonymous-id"),
      "original-anonymous-id",
    );
    assert.equal(
      identity.anonymousIdForIdentify("first-user", "original-anonymous-id"),
      "original-anonymous-id",
    );
    assert.equal(
      identity.distinctIdForEvent("original-anonymous-id"),
      "first-user",
    );
    assert.equal(
      identity.anonymousIdForIdentify("second-user", "original-anonymous-id"),
      "fresh-anonymous-id",
    );
    assert.equal(
      identity.distinctIdForEvent("original-anonymous-id"),
      "second-user",
    );
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});

test("adopts a new anonymous id after PostHog resets", () => {
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
    const identity = createPrivateRouteIdentity();
    identity.anonymousIdForIdentify("first-user", "original-anonymous-id");

    assert.equal(
      identity.distinctIdForEvent("reset-anonymous-id"),
      "reset-anonymous-id",
    );
    assert.equal(
      identity.anonymousIdForIdentify("second-user", "reset-anonymous-id"),
      "reset-anonymous-id",
    );
  } finally {
    Reflect.deleteProperty(globalThis, "window");
  }
});
