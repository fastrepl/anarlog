import assert from "node:assert/strict";
import test from "node:test";

import {
  sanitizeAnalyticsEventName,
  sanitizeAnalyticsProperties,
} from "./analytics-sanitization.ts";

test("replaces free-text analytics event names", () => {
  assert.equal(sanitizeAnalyticsEventName("note_created"), "note_created");
  assert.equal(sanitizeAnalyticsEventName("$pageview"), "$pageview");
  assert.equal(
    sanitizeAnalyticsEventName("opened patient@example.com"),
    "analytics_event",
  );
});

test("removes nested identity and free text while retaining safe analytics values", () => {
  assert.deepEqual(
    sanitizeAnalyticsProperties({
      method: "oauth",
      duration_ms: 125,
      succeeded: true,
      email: "patient@example.com",
      arbitrary_copy: "Jane Doe has diabetes",
      user_reference: "019c1234-abcd-7000-8000-123456789abc",
      nested: {
        transcript: "private meeting content",
        provider: "openai",
      },
    }),
    {
      method: "oauth",
      duration_ms: 125,
      succeeded: true,
      nested: { provider: "openai" },
    },
  );
});

test("keeps anonymous PostHog identifiers but rejects URL secrets", () => {
  assert.deepEqual(
    sanitizeAnalyticsProperties({
      distinct_id: "019c1234-abcd-7000-8000-123456789abc",
      $current_url: "https://anarlog.so/pricing",
      $referrer: "https://example.com/?token=secret",
    }),
    {
      distinct_id: "019c1234-abcd-7000-8000-123456789abc",
      $current_url: "https://anarlog.so/pricing",
    },
  );
});
