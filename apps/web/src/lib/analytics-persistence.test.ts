import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";
import type { CaptureResult } from "posthog-js";

import {
  ANALYTICS_IDENTITY_COOKIE,
  createPrivateRouteIdentity,
  getPostHogPersistenceName,
  parseAnalyticsIdentity,
  parsePostHogDistinctId,
  serializeAnalyticsIdentity,
} from "./private-route-analytics-identity.ts";

const require = createRequire(import.meta.url);
const { JSDOM } = require("jsdom") as {
  JSDOM: new (
    html: string,
    options: { url: string; pretendToBeVisual: boolean },
  ) => { window: Window & typeof globalThis & { close: () => void } };
};

test("starts analytics without reusing legacy account-linked persistence", async (t) => {
  const dom = new JSDOM("", {
    url: "https://anarlog.so/",
    pretendToBeVisual: true,
  });
  for (const key of ["window", "document", "navigator", "location"] as const) {
    const previous = Object.getOwnPropertyDescriptor(globalThis, key);
    Object.defineProperty(globalThis, key, {
      configurable: true,
      value: key === "window" ? dom.window : dom.window[key],
    });
    t.after(() => {
      if (previous) Object.defineProperty(globalThis, key, previous);
      else Reflect.deleteProperty(globalThis, key);
    });
  }
  t.after(() => dom.window.close());
  const { PostHog } = await import("posthog-js");

  function readCookie(name: string) {
    const raw = document.cookie
      .split("; ")
      .find((part) => part.startsWith(`${name}=`))
      ?.slice(name.length + 1);
    return raw ? decodeURIComponent(raw) : null;
  }

  for (const state of ["identified", "anonymous"] as const) {
    for (const storage of ["localStorage+cookie", "cookie"] as const) {
      await t.test(`${state} identity in ${storage}`, (t) => {
        t.mock.timers.enable({ apis: ["setTimeout", "setInterval"] });
        const apiKey = `test_${state}_${storage.replace("+", "_")}`;
        const legacyId =
          state === "identified" ? "raw-account-id" : "linked-browser-id";
        const legacyState = JSON.stringify({
          distinct_id: legacyId,
          $device_id: "linked-device-id",
          $user_state: state,
          $user_id: "raw-account-id",
        });
        const oldKey = `ph_${apiKey}_posthog`;
        document.cookie = `${oldKey}=${encodeURIComponent(legacyState)}; path=/`;
        if (storage === "localStorage+cookie") {
          window.localStorage.setItem(oldKey, legacyState);
        }
        document.cookie = `anlg_analytics_identity=${encodeURIComponent(
          JSON.stringify({
            anonymousId: "linked-browser-id",
            postHogId: legacyId,
            userId: "raw-account-id",
          }),
        )}; path=/`;
        document.cookie = `${ANALYTICS_IDENTITY_COOKIE}=; max-age=0; path=/`;

        const identity = createPrivateRouteIdentity({
          read: () =>
            parseAnalyticsIdentity(readCookie(ANALYTICS_IDENTITY_COOKIE)),
          write: (value) => {
            document.cookie = `${ANALYTICS_IDENTITY_COOKIE}=${encodeURIComponent(serializeAnalyticsIdentity(value))}; path=/`;
          },
        });
        const persistenceName = getPostHogPersistenceName(apiKey);
        const key = `ph_${persistenceName}`;
        const persisted = readCookie(key);
        const privateId = identity.distinctIdForEvent(
          persisted ? parsePostHogDistinctId(persisted) : null,
        );
        assert.notEqual(privateId, legacyId);
        assert.notEqual(privateId, "linked-browser-id");

        const events: CaptureResult[] = [];
        const config = {
          persistence_name: persistenceName,
          persistence: storage,
          autocapture: false,
          capture_pageview: true,
          disable_session_recording: true,
          disable_external_dependency_loading: true,
          advanced_disable_flags: true,
          request_batching: false,
          before_send: (event: CaptureResult | null) => {
            if (event) events.push(event);
            return null;
          },
        };
        const client = new PostHog();
        client.init(apiKey, config);
        t.mock.timers.tick(1);

        const distinctId = client.get_distinct_id();
        assert.ok(distinctId);
        assert.notEqual(distinctId, legacyId);
        assert.notEqual(client.get_property("$device_id"), "linked-device-id");
        assert.equal(client.get_property("$user_id"), undefined);
        assert.equal(client.get_property("$user_state"), "anonymous");
        assert.ok(events.some((event) => event.event === "$pageview"));
        assert.ok(
          events.every((event) => event.properties.distinct_id === distinctId),
        );
        assert.equal(JSON.stringify(events).includes("raw-account-id"), false);

        const persistedId = parsePostHogDistinctId(readCookie(key)!);
        assert.equal(persistedId, distinctId);
        assert.equal(identity.distinctIdForEvent(persistedId), distinctId);
        assert.equal(
          readCookie(ANALYTICS_IDENTITY_COOKIE)?.includes(legacyId),
          false,
        );
        if (storage === "localStorage+cookie") {
          assert.equal(
            parsePostHogDistinctId(window.localStorage.getItem(key)!),
            distinctId,
          );
        }

        const reloadedClient = new PostHog();
        reloadedClient.init(apiKey, config);
        t.mock.timers.tick(1);
        assert.equal(reloadedClient.get_distinct_id(), distinctId);
        assert.equal(events.length, 2);
        assert.equal(events[1].properties.distinct_id, distinctId);
      });
    }
  }
});
