import assert from "node:assert/strict";
import test from "node:test";

import { sanitizePrivateRouteAnalyticsProperties } from "./private-route-analytics-sanitization.ts";

test("keeps anonymous checkout diagnostics and drops private values", () => {
  assert.deepEqual(
    sanitizePrivateRouteAnalyticsProperties({
      checkout_type: "paid",
      entry_source: "pricing",
      email: "patient@example.com",
      note: "Private meeting content",
    }),
    {
      checkout_type: "paid",
      entry_source: "pricing",
    },
  );
});

test("keeps anonymous auth funnel properties and drops account identity", () => {
  assert.deepEqual(
    sanitizePrivateRouteAnalyticsProperties({
      method: "oauth",
      provider: "google",
      action: "signup",
      flow: "web",
      view: "email",
      new_account: true,
      user_id: "6d3f9d2e-1a3b-4c5d-8e7f-0123456789ab",
      email: "patient@example.com",
    }),
    {
      method: "oauth",
      provider: "google",
      action: "signup",
      flow: "web",
      view: "email",
      new_account: true,
    },
  );
});
