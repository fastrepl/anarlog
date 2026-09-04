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
