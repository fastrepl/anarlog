import assert from "node:assert/strict";
import test from "node:test";

import { deriveBillingInfo } from "./billing.ts";

test("paused subscriptions fail closed when a Pro entitlement lingers", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro"],
    subscription_status: "paused",
  });

  assert.equal(billing.isPaused, true);
  assert.equal(billing.isPro, false);
  assert.equal(billing.isPaid, false);
  assert.equal(billing.plan, "free");
});

test("paused Pro does not suppress an independent Lite entitlement", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro", "hyprnote_lite"],
    subscription_status: "paused",
  });

  assert.equal(billing.isPaused, true);
  assert.equal(billing.isPro, false);
  assert.equal(billing.isLite, true);
  assert.equal(billing.isPaid, true);
});
