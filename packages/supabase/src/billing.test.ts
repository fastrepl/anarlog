import assert from "node:assert/strict";
import test from "node:test";

import { deriveBillingInfo } from "@hypr/supabase/billing";

const secondsFromNow = (seconds: number) =>
  Math.floor(Date.now() / 1000) + seconds;

test("an unexpired trial grants Pro access without a paid entitlement", () => {
  const billing = deriveBillingInfo({
    entitlements: [],
    subscription_status: "trialing",
    trial_end: secondsFromNow(60),
  });

  assert.equal(billing.isTrialing, true);
  assert.equal(billing.isPro, true);
  assert.equal(billing.plan, "trial");
});

test("an expired trial no longer grants Pro access", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro"],
    subscription_status: "trialing",
    trial_end: secondsFromNow(-60),
  });

  assert.equal(billing.isTrialing, false);
  assert.equal(billing.isPro, false);
  assert.equal(billing.isPaid, false);
  assert.equal(billing.plan, "free");
});

test("a trial without an end date fails closed", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro"],
    subscription_status: "trialing",
    trial_end: null,
  });

  assert.equal(billing.isTrialing, false);
  assert.equal(billing.isPro, false);
  assert.equal(billing.plan, "free");
});

test("Lite remains paid without being treated as Pro", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_lite"],
    subscription_status: "active",
  });

  assert.equal(billing.isPro, false);
  assert.equal(billing.isLite, true);
  assert.equal(billing.isPaid, true);
});

test("a bare active subscription does not invent an entitlement", () => {
  const billing = deriveBillingInfo({
    entitlements: [],
    subscription_status: "active",
  });

  assert.equal(billing.isPro, false);
  assert.equal(billing.isPaid, false);
  assert.equal(billing.plan, "free");
});
