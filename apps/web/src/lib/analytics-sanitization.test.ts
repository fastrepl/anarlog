import assert from "node:assert/strict";
import test from "node:test";

import {
  sanitizeAnalyticsEventName,
  sanitizeAnalyticsProperties,
  sanitizePostHogEvent,
  toAnalyticsToken,
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
      "phc_test_project",
    ),
    {
      event: "$pageview",
      uuid: "test-event",
      properties: {
        token: "phc_test_project",
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
  assert.equal(sanitizePostHogEvent(null, origin, "phc_test_project"), null);
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
      "phc_test_project",
    ),
    {
      event: "$pageview",
      uuid: "test-event",
      properties: {
        token: "phc_test_project",
        $pathname: "/people/:id/:id",
        $current_url: `${origin}/people/:id/:id`,
        $referrer: "https://example.com/pricing",
      },
    },
  );
});

test("keeps posthog-js device, browser, and page context", () => {
  const origin = "https://anarlog.so";
  const properties = {
    $os: "Mac OS X",
    $os_version: "10.15.7",
    $browser: "Mobile Safari",
    $browser_version: 26,
    $device: "iPhone",
    $device_type: "Mobile",
    $timezone: "Asia/Seoul",
    $raw_user_agent:
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
    title: "Anarlog - Open source AI meeting notetaker",
    $host: "anarlog.so",
    $device_id: "019c1234-abcd-7000-8000-123456789abc",
    $window_id: "019c1234-abcd-7000-8000-123456789abd",
    $pageview_id: "019c1234-abcd-7000-8000-123456789abe",
    $prev_pageview_pathname: "/pricing",
    $elements_chain:
      'a.btn:attr__href="/download"nth-child="1";div:nth-child="2"',
    $el_text: "Download for Mac",
    utm_campaign: "spring launch",
    "$feature/flag-a": "control",
    $web_vitals_LCP_value: 1234.5,
    $active_feature_flags: ["flag-a"],
    $set_once: {
      $initial_os: "Mac OS X",
      $initial_current_url: `${origin}/?utm_source=x&token=secret`,
      $initial_referrer: "https://www.google.com/?q=secret",
      $initial_pathname: "/",
      $initial_timezone: "Asia/Seoul",
    },
    $initial_person_info: {
      r: "https://www.google.com/?q=secret",
      u: `${origin}/pricing?utm_source=x&email=patient@example.com`,
    },
    $elements: [
      {
        tag_name: "a",
        attr__href: "/download",
        attr__class: "btn primary",
        nth_child: 1,
        text: "Download",
        "attr__data-email": "patient@example.com",
      },
    ],
  };

  assert.deepEqual(
    sanitizePostHogEvent(
      { event: "$autocapture", properties, uuid: "test-event" },
      origin,
      "phc_test_project",
    )?.properties,
    {
      ...properties,
      $set_once: {
        $initial_os: "Mac OS X",
        $initial_current_url: `${origin}/?utm_source=x`,
        $initial_referrer: "https://www.google.com/",
        $initial_pathname: "/",
        $initial_timezone: "Asia/Seoul",
      },
      $initial_person_info: {
        r: "https://www.google.com/",
        u: `${origin}/pricing?utm_source=x`,
      },
      $elements: [
        {
          tag_name: "a",
          attr__href: "/download",
          attr__class: "btn primary",
          nth_child: 1,
          text: "Download",
        },
      ],
      token: "phc_test_project",
    },
  );
});

test("drops email-like values and custom identifiers from posthog-js context", () => {
  assert.deepEqual(
    sanitizePostHogEvent(
      {
        event: "$pageview",
        uuid: "test-event",
        properties: {
          title: "Invite patient@example.com",
          $el_text: "Send to patient@example.com",
          $device_id: "patient@example.com",
          $window_id: "a".repeat(129),
          $current_url: "https://anarlog.so/pricing?email=patient@example.com",
          customer_id: "cus_123",
          note: "Jane Doe has diabetes",
        },
      },
      "https://anarlog.so",
      "phc_test_project",
    )?.properties,
    { $current_url: "https://anarlog.so/pricing", token: "phc_test_project" },
  );
});

test("keeps marketing query parameters while stripping the rest", () => {
  assert.deepEqual(
    sanitizePostHogEvent(
      {
        event: "$pageview",
        uuid: "test-event",
        properties: {
          $current_url:
            "https://anarlog.so/download?utm_source=producthunt&utm_medium=social&ref=hn&token=secret&email=patient@example.com#section",
          $referrer:
            "https://news.ycombinator.com/item?id=123&utm_source=keepme",
          $pathname: "/download?token=secret",
        },
      },
      "https://anarlog.so",
      "phc_test_project",
    )?.properties,
    {
      $current_url:
        "https://anarlog.so/download?utm_source=producthunt&utm_medium=social&ref=hn",
      $referrer: "https://news.ycombinator.com/item",
      $pathname: "/download",
      token: "phc_test_project",
    },
  );
});

test("keeps long blog slugs while redacting identifier-like segments", () => {
  const origin = "https://anarlog.so";
  const slug = "can-you-transcribe-meetings-without-sending-data-to-cloud";
  const cases: Array<[string, string]> = [
    [`/blog/${slug}`, `/blog/${slug}`],
    ["/changelog/1.4.25", "/changelog/1.4.25"],
    ["/blog/019c1234-abcd-7000-8000-123456789abc", "/blog/:id"],
    ["/share/public/s_0123456789abcdef0123456789abcdef", "/share/public/:id"],
    [`/t/${"f".repeat(64)}`, "/t/:id"],
    ["/people/patient%40example.com", "/people/:id"],
    ["/orders/12345678", "/orders/:id"],
    [`/x/${"a".repeat(129)}`, "/x/:id"],
    [`/x/${"Ab9".repeat(12)}`, "/x/:id"],
  ];
  for (const [pathname, expected] of cases) {
    assert.deepEqual(
      sanitizePostHogEvent(
        {
          event: "$pageview",
          uuid: "test-event",
          properties: {
            $pathname: pathname,
            $current_url: `${origin}${pathname}`,
          },
        },
        origin,
        "phc_test_project",
      )?.properties,
      {
        $pathname: expected,
        $current_url: `${origin}${expected}`,
        token: "phc_test_project",
      },
      pathname,
    );
  }
});

test("normalizes display labels into analytics tokens", () => {
  assert.equal(toAnalyticsToken("Apple Silicon"), "apple_silicon");
  assert.equal(toAnalyticsToken(" AppImage ARM64 "), "appimage_arm64");
  assert.equal(toAnalyticsToken("TestFlight"), "testflight");
});

test("restores only the configured project token at the SDK boundary", () => {
  const properties = {
    token: "private-access-token",
    email: "patient@example.com",
    nested: { token: "private-nested-token", provider: "openai" },
  };
  assert.deepEqual(sanitizeAnalyticsProperties(properties), {
    nested: { provider: "openai" },
  });
  assert.deepEqual(
    sanitizePostHogEvent(
      { event: "$pageview", uuid: "test-event", properties },
      "https://anarlog.so",
      "phc_public_project_key",
    )?.properties,
    { nested: { provider: "openai" }, token: "phc_public_project_key" },
  );
});

test("normalizes heatmap page URL keys so query secrets are not sent", () => {
  const result = sanitizePostHogEvent(
    {
      event: "$$heatmap",
      uuid: "test-event",
      properties: {
        $heatmap_data: {
          "https://anarlog.so/blog/best-ai-notetaker-for-in-person-meetings/?utm_source=slack%20test&token=SECRET&email=a@b.com":
            [{ x: 402, y: 158, target_fixed: false, type: "click" }],
          "not a url ::": [{ x: 1, y: 1, type: "click" }],
        },
      },
    },
    "https://anarlog.so",
    "phc_public_project_key",
  );
  const properties: Record<string, unknown> = result?.properties ?? {};
  assert.deepEqual(properties.$heatmap_data, {
    "https://anarlog.so/blog/best-ai-notetaker-for-in-person-meetings/?utm_source=slack+test":
      [{ x: 402, y: 158, target_fixed: false, type: "click" }],
  });
  assert.equal(JSON.stringify(result).includes("SECRET"), false);
});
