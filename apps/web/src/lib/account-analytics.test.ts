import assert from "node:assert/strict";
import test from "node:test";

import {
  groupAccountAnalyticsEvents,
  sendPostHogBatch,
  type AccountAnalyticsEvent,
} from "./account-analytics.ts";

const events: AccountAnalyticsEvent[] = [
  {
    id: "event-live",
    event_name: "account_created",
    user_id: "user-live",
    occurred_at: "2026-07-25T03:00:00.000Z",
    properties: { source: "supabase_auth" },
    historical: false,
  },
  {
    id: "event-historical",
    event_name: "account_confirmed",
    user_id: "user-historical",
    occurred_at: "2025-07-25T03:00:00.000Z",
    properties: { source: "supabase_auth" },
    historical: true,
  },
];

test("groups live and historical events into separate batches", () => {
  assert.deepEqual(
    groupAccountAnalyticsEvents(events).map((group) =>
      group.map((event) => event.id),
    ),
    [["event-live"], ["event-historical"]],
  );
});

test("sends stable distinct and insert IDs with original timestamps", async () => {
  let request: { url: string; init?: RequestInit } | undefined;

  await sendPostHogBatch({
    events: [events[1]],
    projectToken: "project-token",
    host: "https://us.i.posthog.com/",
    fetcher: async (url, init) => {
      request = { url: String(url), init };
      return Response.json({ status: "Ok" });
    },
  });

  assert.equal(request?.url, "https://us.i.posthog.com/batch/");
  const body = JSON.parse(String(request?.init?.body));

  assert.equal(body.historical_migration, true);
  assert.equal(body.batch[0].event, "account_confirmed");
  assert.equal(body.batch[0].properties.distinct_id, "user-historical");
  assert.equal(
    body.batch[0].properties.$insert_id,
    "account_confirmed:user-historical",
  );
  assert.equal(body.batch[0].timestamp, "2025-07-25T03:00:00.000Z");
});

test("surfaces PostHog ingestion errors", async () => {
  await assert.rejects(
    sendPostHogBatch({
      events: [events[0]],
      projectToken: "project-token",
      host: "https://us.i.posthog.com",
      fetcher: async () => new Response("invalid token", { status: 401 }),
    }),
    /PostHog batch rejected with 401: invalid token/,
  );
});

test("rejects a malformed PostHog acknowledgement", async () => {
  await assert.rejects(
    sendPostHogBatch({
      events: [events[0]],
      projectToken: "project-token",
      host: "https://us.i.posthog.com",
      fetcher: async () => Response.json({ status: "Ignored" }),
    }),
    /PostHog batch returned an invalid acknowledgement/,
  );
});
