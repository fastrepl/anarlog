import assert from "node:assert/strict";
import test from "node:test";

import { deriveBillingInfo } from "./billing.ts";

test("the shared 21-day trial ends at its exact server deadline, even with stale Pro claims", () => {
  const start = Date.UTC(2026, 8, 6);
  const end = start + 21 * 24 * 60 * 60 * 1000;
  const payload = {
    subscription_status: "trialing",
    trial_end: end / 1000,
    entitlements: ["hyprnote_pro"],
  };
  assert.equal(deriveBillingInfo(payload, start).trialDaysRemaining, 21);
  assert.equal(deriveBillingInfo(payload, end - 1).isPro, true);
  const expired = deriveBillingInfo(payload, end);
  assert.equal(expired.isPro, false);
  assert.equal(expired.plan, "free");
  assert.equal(expired.trialDaysRemaining, 0);
  assert.equal(
    deriveBillingInfo({ ...payload, subscription_status: "active" }, end).isPro,
    true,
  );
});

test("an incomplete or missing trial claim cannot grant Pro access", () => {
  assert.equal(deriveBillingInfo(null).isPro, false);
  assert.equal(
    deriveBillingInfo({
      subscription_status: "trialing",
      entitlements: ["hyprnote_pro"],
    }).isPro,
    false,
  );
});

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
