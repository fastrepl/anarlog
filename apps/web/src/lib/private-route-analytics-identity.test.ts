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

test("parses raw PostHog localStorage JSON without decoding its contents", () => {
  const raw = JSON.stringify({
    distinct_id: "anonymous-id",
    $initial_referrer: "https://example.com/%E0%A4%A",
  });

  assert.equal(parsePostHogDistinctId(raw), "anonymous-id");
});

test("ignores malformed or non-string identity cookie values", () => {
  assert.deepEqual(parseAnalyticsIdentity(null), {});
  assert.deepEqual(parseAnalyticsIdentity("not json"), {});
  assert.deepEqual(parseAnalyticsIdentity("[]"), {});
  assert.deepEqual(
    parseAnalyticsIdentity(JSON.stringify({ userId: 7, anonymousId: "a" })),
    { anonymousId: "a" },
  );
});

test("never reuses an identified user as the next anonymous id", () => {
  const identity = createPrivateRouteIdentity(
    createStore(),
    () => "fresh-anonymous-id",
  );

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
});

test("keeps the claim when the browsing context restarts", () => {
  const store = createStore();
  createPrivateRouteIdentity(store).anonymousIdForIdentify(
    "first-user",
    "original-anonymous-id",
  );

  const laterVisit = createPrivateRouteIdentity(
    store,
    () => "fresh-anonymous-id",
  );

  assert.equal(
    laterVisit.anonymousIdForIdentify("second-user", "original-anonymous-id"),
    "fresh-anonymous-id",
  );
});

test("adopts a new anonymous id after PostHog resets", () => {
  const store = createStore();
  const identity = createPrivateRouteIdentity(store);
  identity.anonymousIdForIdentify("first-user", "original-anonymous-id");

  assert.equal(
    identity.distinctIdForEvent("reset-anonymous-id"),
    "reset-anonymous-id",
  );
  assert.equal(
    identity.anonymousIdForIdentify("second-user", "reset-anonymous-id"),
    "reset-anonymous-id",
  );
});

test("clearing the identity lets the next user merge the fresh PostHog id", () => {
  const store = createStore();
  const identity = createPrivateRouteIdentity(store);
  identity.anonymousIdForIdentify("first-user", "original-anonymous-id");

  identity.reset();

  assert.equal(
    identity.anonymousIdForIdentify("second-user", "signed-out-anonymous-id"),
    "signed-out-anonymous-id",
  );
});

test("keeps the claim on sign-out so the next user is not merged in", () => {
  const store = createStore();
  const identity = createPrivateRouteIdentity(
    store,
    () => "fresh-anonymous-id",
  );
  identity.anonymousIdForIdentify("first-user", "original-anonymous-id");

  assert.equal(identity.signOut("original-anonymous-id"), true);

  assert.equal(
    identity.distinctIdForEvent("original-anonymous-id"),
    "fresh-anonymous-id",
  );
  assert.equal(
    identity.anonymousIdForIdentify("second-user", "original-anonymous-id"),
    "fresh-anonymous-id",
  );
});

test("reports nothing to remember when sign-out finds no claimed id", () => {
  assert.equal(createPrivateRouteIdentity(createStore()).signOut(null), false);
});

test("does not claim a PostHog id before a user is identified", () => {
  const store = createStore({
    anonymousId: "original-anonymous-id",
    postHogId: "original-anonymous-id",
  });
  const identity = createPrivateRouteIdentity(
    store,
    () => "fresh-anonymous-id",
  );

  assert.equal(identity.signOut("original-anonymous-id"), false);
  assert.equal(
    identity.anonymousIdForIdentify("first-user", "original-anonymous-id"),
    "original-anonymous-id",
  );
});

test("skips the merge when nothing anonymous was ever captured", () => {
  const identity = createPrivateRouteIdentity(createStore());

  assert.equal(identity.anonymousIdForIdentify("first-user", null), null);
  assert.equal(identity.distinctIdForEvent(null), "first-user");
});

test("does not re-identify the same user twice", () => {
  const identity = createPrivateRouteIdentity(createStore());

  assert.equal(
    identity.anonymousIdForIdentify("first-user", "original-anonymous-id"),
    "original-anonymous-id",
  );
  assert.equal(
    identity.anonymousIdForIdentify("first-user", "original-anonymous-id"),
    null,
  );
});
