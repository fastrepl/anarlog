import assert from "node:assert/strict";
import test from "node:test";

import type { AnalyticsIdentity } from "./private-route-analytics-identity.ts";
import {
  createPrivateRouteIdentity,
  parseAnalyticsIdentity,
  parsePostHogDistinctId,
  serializeAnalyticsIdentity,
} from "./private-route-analytics-identity.ts";

function createStore(initial: AnalyticsIdentity = {}) {
  let raw = serializeAnalyticsIdentity(initial);
  return {
    read: () => parseAnalyticsIdentity(raw),
    write: (identity: AnalyticsIdentity) => {
      raw = serializeAnalyticsIdentity(identity);
    },
  };
}

test("parses PostHog ids without retaining unrelated state", () => {
  const raw = JSON.stringify({
    distinct_id: "anonymous-id",
    $initial_referrer: "https://example.com/private?token=secret",
  });

  assert.equal(parsePostHogDistinctId(raw), "anonymous-id");
  assert.equal(parsePostHogDistinctId("not json"), null);
});

test("migrates legacy identified cookies without retaining the user id", () => {
  const parsed = parseAnalyticsIdentity(
    JSON.stringify({
      anonymousId: "old-anonymous",
      postHogId: "old-posthog",
      userId: "raw-user-id",
    }),
  );

  assert.deepEqual(parsed, {
    anonymousId: "old-anonymous",
    legacyIdentified: true,
    postHogId: "old-posthog",
  });
  assert.equal(
    serializeAnalyticsIdentity(parsed).includes("raw-user-id"),
    false,
  );
  assert.equal(serializeAnalyticsIdentity(parsed).includes("userId"), false);
});

test("keeps events on a stable anonymous browser identity", () => {
  const identity = createPrivateRouteIdentity(createStore());

  assert.equal(identity.distinctIdForEvent("anonymous-id"), "anonymous-id");
  assert.equal(identity.distinctIdForEvent("anonymous-id"), "anonymous-id");
});

test("rotates a legacy identified browser before its next event", () => {
  let stored: AnalyticsIdentity = {
    legacyIdentified: true,
    postHogId: "raw-user-id",
  };
  const identity = createPrivateRouteIdentity(
    {
      read: () => stored,
      write: (next) => {
        stored = next;
      },
    },
    () => "fresh-anonymous-id",
  );

  assert.equal(
    identity.distinctIdForEvent("raw-user-id"),
    "fresh-anonymous-id",
  );
  assert.deepEqual(stored, {
    anonymousId: "fresh-anonymous-id",
    postHogId: "raw-user-id",
  });
});

test("rotates the anonymous identity on sign-out", () => {
  const store = createStore({
    anonymousId: "anonymous-id",
    postHogId: "anonymous-id",
  });
  const identity = createPrivateRouteIdentity(
    store,
    () => "fresh-anonymous-id",
  );

  assert.equal(identity.signOut("anonymous-id"), true);
  assert.equal(
    identity.distinctIdForEvent("anonymous-id"),
    "fresh-anonymous-id",
  );
});

test("reports no sign-out state when analytics never ran", () => {
  assert.equal(createPrivateRouteIdentity(createStore()).signOut(null), false);
});
