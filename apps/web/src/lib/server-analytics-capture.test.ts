import assert from "node:assert/strict";
import test from "node:test";

import { sendServerAnalytics } from "./server-analytics-capture.ts";

const checkout = {
  apiKey: "test-api-key",
  host: "https://posthog.example.test///",
  appVersion: "1.0.0",
  event: "checkout_started",
  insertId: "checkout-started:cs_test_checkout_1",
  timestamp: new Date("2026-09-05T07:00:00.000Z"),
  properties: {
    plan: "pro",
    period: "monthly",
    checkout_type: "paid",
    entry_point: "pricing",
  },
};

test("retries a checkout with the same anonymous deduplication fields", async (t) => {
  const requests: { url: string; init?: RequestInit }[] = [];
  const fetcher: typeof fetch = async (url, init) => {
    requests.push({ url: String(url), init });
    if (requests.length === 1) throw new Error("Response lost after ingestion");
    return Response.json({ status: 1 });
  };
  t.mock.timers.enable({ apis: ["Date"], now: checkout.timestamp });

  await assert.rejects(
    sendServerAnalytics({ ...checkout, fetcher }),
    /Response lost after ingestion/,
  );
  t.mock.timers.tick(86_400_000);
  await sendServerAnalytics({ ...checkout, fetcher });

  const first = JSON.parse(String(requests[0].init?.body));
  const retry = JSON.parse(String(requests[1].init?.body));
  assert.deepEqual(retry, first);
  assert.equal(requests[0].url, "https://posthog.example.test/capture/");
  assert.equal(requests[0].init?.method, "POST");
  assert.equal(
    new Headers(requests[0].init?.headers).get("Content-Type"),
    "application/json",
  );
  assert.ok(requests[0].init?.signal instanceof AbortSignal);
  assert.match(
    first.uuid,
    /^[\da-f]{8}-[\da-f]{4}-8[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/,
  );
  assert.equal(first.timestamp, "2026-09-05T07:00:00.000Z");
  assert.equal(first.event, "checkout_started");
  assert.deepEqual(first.properties, {
    ...checkout.properties,
    distinct_id: first.uuid,
    $insert_id: first.uuid,
    $process_person_profile: false,
    surface: "api",
    analytics_schema_version: 1,
    app_version: "1.0.0",
  });
  assert.ok(!JSON.stringify(first).includes(checkout.insertId));
  assert.ok(!JSON.stringify(first).includes("cs_test_checkout_1"));
});

test("keeps separate checkouts, event names, and unkeyed captures distinct", async () => {
  const uuids: string[] = [];
  const identities: string[] = [];
  const fetcher: typeof fetch = async (_url, init) => {
    const body = JSON.parse(String(init?.body));
    uuids.push(body.uuid);
    identities.push(body.properties.distinct_id);
    return Response.json({ status: 1 });
  };

  for (const overrides of [
    {},
    { insertId: "checkout-started:cs_test_checkout_2" },
    { event: "checkout_completed" },
    { insertId: undefined },
    { insertId: undefined },
  ]) {
    await sendServerAnalytics({ ...checkout, ...overrides, fetcher });
  }

  assert.equal(new Set(uuids).size, 5);
  assert.equal(new Set(identities).size, 5);
});

test("filters private properties and prevents overriding capture identity", async () => {
  await sendServerAnalytics({
    ...checkout,
    event: "private@example.com",
    properties: {
      plan: "pro",
      period: "private@example.com",
      user_id: "private-user-id",
      email: "private@example.com",
      checkout_id: "cs_test_checkout_1",
      note: "private meeting content",
      distinct_id: "private-user-id",
      $insert_id: "raw-insert-id",
      $groups: { account: "private-user-id" },
      $process_person_profile: true,
    },
    fetcher: async (_url, init) => {
      const body = JSON.parse(String(init?.body));
      assert.equal(body.event, "analytics_event");
      assert.deepEqual(body.properties, {
        plan: "pro",
        distinct_id: body.uuid,
        $insert_id: body.uuid,
        $process_person_profile: false,
        surface: "api",
        analytics_schema_version: 1,
        app_version: "1.0.0",
      });
      return Response.json({ status: 1 });
    },
  });
});

test("reports a rejected capture without including its response body", async () => {
  await assert.rejects(
    sendServerAnalytics({
      ...checkout,
      fetcher: async () => new Response("private response", { status: 500 }),
    }),
    { message: "PostHog capture failed with 500" },
  );
});
