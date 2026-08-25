import assert from "node:assert/strict";
import test from "node:test";

import {
  formatAccountPlanDate,
  getAccountPlanCopy,
  getSubscriptionAccessEnd,
} from "./account-plan.ts";

test("prefers cancel_at, then item period end, then subscription period end", () => {
  assert.equal(
    getSubscriptionAccessEnd({
      cancel_at: 100,
      current_period_end: 200,
      items: { data: [{ current_period_end: 300 }] },
    }),
    100,
  );
  assert.equal(
    getSubscriptionAccessEnd({
      cancel_at: null,
      items: {
        data: [{ current_period_end: 200 }, { current_period_end: 350 }],
      },
    }),
    350,
  );
  assert.equal(getSubscriptionAccessEnd({ current_period_end: 400 }), 400);
  assert.equal(getSubscriptionAccessEnd({}), null);
});

test("paused cardless trial copy explains that Pro can be resumed", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaused: true,
      isPaid: false,
      trialDaysRemaining: 0,
      trialEnd: new Date("2026-08-24T00:00:00.000Z"),
      cancelAtPeriodEnd: false,
      currentPeriodEnd: null,
    }),
    {
      planLabel: "Free",
      planDetail: "Your Pro trial ended. Resume it to reactivate Pro.",
    },
  );
});

test("paid copy acknowledges a scheduled cancellation", () => {
  const currentPeriodEnd = new Date("2026-09-17T00:00:00.000Z");

  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: true,
      currentPeriodEnd,
    }),
    {
      planLabel: "Pro",
      planDetail: `Cancels ${formatAccountPlanDate(currentPeriodEnd)}.`,
    },
  );

  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: true,
      currentPeriodEnd: null,
    }),
    {
      planLabel: "Pro",
      planDetail: "Cancels at the end of the billing period.",
    },
  );
});

test("paid copy names the YC founder year when that perk is on the subscription", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: false,
      currentPeriodEnd: new Date("2026-09-17T00:00:00.000Z"),
      hasYcPerk: true,
    }),
    {
      planLabel: "Pro",
      planDetail: "YC founder year is applied.",
    },
  );
});

test("paid copy stays supportive when the subscription is not canceling", () => {
  assert.deepEqual(
    getAccountPlanCopy({
      isTrialing: false,
      isPaid: true,
      trialDaysRemaining: null,
      trialEnd: null,
      cancelAtPeriodEnd: false,
      currentPeriodEnd: new Date("2026-09-17T00:00:00.000Z"),
    }),
    {
      planLabel: "Pro",
      planDetail: "Thanks for supporting Anarlog.",
    },
  );
});
