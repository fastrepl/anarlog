import { expect, test } from "vitest";

import { deriveBillingInfo } from "@anlg/supabase/billing";

const secondsFromNow = (seconds: number) =>
  Math.floor(Date.now() / 1000) + seconds;

test("an unexpired trial grants Pro access without a paid entitlement", () => {
  const billing = deriveBillingInfo({
    entitlements: [],
    subscription_status: "trialing",
    trial_end: secondsFromNow(60),
  });

  expect(billing.isTrialing).toBe(true);
  expect(billing.isPro).toBe(true);
  expect(billing.plan).toBe("trial");
});

test("an expired trial no longer grants Pro access", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro"],
    subscription_status: "trialing",
    trial_end: secondsFromNow(-60),
  });

  expect(billing.isTrialing).toBe(false);
  expect(billing.isPro).toBe(false);
  expect(billing.isPaid).toBe(false);
  expect(billing.plan).toBe("free");
});

test("a paused cardless trial loses access but remains resumable", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro"],
    subscription_status: "paused",
    trial_end: secondsFromNow(-60),
  });

  expect(billing.isPaused).toBe(true);
  expect(billing.isTrialing).toBe(false);
  expect(billing.isPro).toBe(false);
  expect(billing.isPaid).toBe(false);
  expect(billing.plan).toBe("free");
});

test("a trial without an end date fails closed", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro"],
    subscription_status: "trialing",
    trial_end: null,
  });

  expect(billing.isTrialing).toBe(false);
  expect(billing.isPro).toBe(false);
  expect(billing.plan).toBe("free");
});

test("Lite remains paid without being treated as Pro", () => {
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_lite"],
    subscription_status: "active",
  });

  expect(billing.isPro).toBe(false);
  expect(billing.isLite).toBe(true);
  expect(billing.isPaid).toBe(true);
});

test("a bare active subscription does not invent an entitlement", () => {
  const billing = deriveBillingInfo({
    entitlements: [],
    subscription_status: "active",
  });

  expect(billing.isPro).toBe(false);
  expect(billing.isPaid).toBe(false);
  expect(billing.plan).toBe("free");
  expect(billing.cancelAtPeriodEnd).toBe(false);
  expect(billing.currentPeriodEnd).toBe(null);
});

test("a scheduled cancellation stays paid through the current period", () => {
  const periodEnd = secondsFromNow(60 * 60 * 24 * 28);
  const billing = deriveBillingInfo({
    entitlements: ["hyprnote_pro"],
    subscription_status: "active",
    cancel_at_period_end: true,
    current_period_end: periodEnd,
  });

  expect(billing.isPaid).toBe(true);
  expect(billing.isPro).toBe(true);
  expect(billing.plan).toBe("pro");
  expect(billing.cancelAtPeriodEnd).toBe(true);
  expect(billing.currentPeriodEnd?.getTime()).toBe(periodEnd * 1000);
});
