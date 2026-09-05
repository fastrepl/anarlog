import assert from "node:assert/strict";
import test from "node:test";

import {
  sanitizeAnalyticsEventName,
  sanitizeAnalyticsProperties,
  sanitizePostHogEvent,
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

test("keeps normalized paths and direct referrers", () => {
  for (const pathname of ["/", "/pricing", "/blog/:id"]) {
    const properties = {
      $pathname: pathname,
      $current_url: `https://anarlog.so${pathname}`,
      $initial_current_url: `https://anarlog.so${pathname}`,
      $referrer: "$direct",
      $initial_referrer: "$direct",
    };
    assert.deepEqual(sanitizeAnalyticsProperties(properties), properties);
  }
});

test("applies the 256-character URL limit independently of other strings", () => {
  for (const key of [
    "$pathname",
    "$current_url",
    "$initial_current_url",
    "$referrer",
    "$initial_referrer",
  ]) {
    for (const length of [128, 129, 256, 257]) {
      const prefix = key === "$pathname" ? "/" : "https://anarlog.so/";
      const value = prefix.padEnd(length, "a");
      assert.deepEqual(
        sanitizeAnalyticsProperties({ [key]: value }),
        length <= 256 ? { [key]: value } : {},
        `${key} at ${length} characters`,
      );
    }
  }

  for (const length of [128, 129]) {
    const distinctId = "a".repeat(length);
    assert.deepEqual(
      sanitizeAnalyticsProperties({ distinct_id: distinctId }),
      length <= 128 ? { distinct_id: distinctId } : {},
    );
  }
  for (const length of [96, 97]) {
    const value = "a".repeat(length - 1) + ".";
    assert.deepEqual(
      sanitizeAnalyticsProperties({ category: value }),
      length <= 96 ? { category: value } : {},
    );
  }
});

test("rejects unsafe URL values without relaxing non-URL properties", () => {
  for (const key of ["$pathname", "$current_url", "$referrer"]) {
    for (const value of [
      "",
      "/pricing?token=secret",
      "https://anarlog.so/pricing#secret",
      "patient@example.com",
      "https://anarlog.so/patient@example.com",
      "/private meeting",
      "$secret",
    ]) {
      assert.deepEqual(sanitizeAnalyticsProperties({ [key]: value }), {});
    }
  }
  assert.deepEqual(
    sanitizeAnalyticsProperties({
      category: "/pricing",
      provider: "$direct",
      distinct_id: "/pricing",
      $session_id: "$direct",
      $pathname: "$direct",
      $current_url: "$direct",
    }),
    {},
  );
});

test("preserves page attribution through PostHog URL normalization", () => {
  const origin = "https://anarlog.so";
  const pathname = `/blog/${Array(8).fill("public-article").join("/")}`;
  const properties = {
    $pathname: `${pathname}?token=secret`,
    $current_url: `${origin}${pathname}?token=secret#secret`,
    $initial_current_url: `${origin}/pricing?email=patient@example.com`,
    $referrer: "$direct",
    $initial_referrer: "$direct",
  };

  assert.deepEqual(
    sanitizePostHogEvent(
      { event: "$pageview", properties, uuid: "test-event" },
      origin,
    ),
    {
      event: "$pageview",
      uuid: "test-event",
      properties: {
        $pathname: pathname,
        $current_url: `${origin}${pathname}`,
        $initial_current_url: `${origin}/pricing`,
        $referrer: "$direct",
        $initial_referrer: "$direct",
      },
    },
  );
  assert.equal(properties.$referrer, "$direct");
  assert.equal(properties.$pathname, `${pathname}?token=secret`);
  assert.equal(sanitizePostHogEvent(null, origin), null);
});

test("redacts sensitive path segments before retaining URL properties", () => {
  const origin = "https://anarlog.so";
  const pathname =
    "/people/patient%40example.com/019c1234-abcd-7000-8000-123456789abc";
  assert.deepEqual(
    sanitizePostHogEvent(
      {
        event: "$pageview",
        uuid: "test-event",
        properties: {
          $pathname: pathname,
          $current_url: `${origin}${pathname}?token=secret`,
          $referrer: "https://example.com/pricing?token=secret#secret",
        },
      },
      origin,
    ),
    {
      event: "$pageview",
      uuid: "test-event",
      properties: {
        $pathname: "/people/:id/:id",
        $current_url: `${origin}/people/:id/:id`,
        $referrer: "https://example.com/pricing",
      },
    },
  );
});
