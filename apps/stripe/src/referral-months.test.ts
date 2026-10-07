import { expect, test } from "bun:test";
import type Stripe from "stripe";

import {
  addCalendarMonth,
  applyPreparedReferralExtension,
  planReferralExtension,
} from "./referral-months";

const epoch = (value: string) => Date.parse(value) / 1000;
const now = epoch("2026-10-05T00:00:00Z");
function subscription(interval = "month") {
  return {
    id: "sub_personal",
    customer: "cus_personal",
    status: "active",
    metadata: {},
    items: {
      data: [
        {
          quantity: 1,
          current_period_end: epoch("2026-10-31T09:00:00Z"),
          price: {
            id: "price_pro",
            recurring: { interval, interval_count: 1 },
          },
        },
      ],
    },
  } as unknown as Stripe.Subscription;
}

test("calendar extensions preserve paid time on monthly and annual plans, including leap-day boundaries", () => {
  for (const interval of ["month", "year"]) {
    for (const type of ["classic", "flexible"] as const) {
      const plan = planReferralExtension(
        { ...subscription(interval), billing_mode: { type, flexible: null } },
        ["price_pro"],
        now,
      );
      expect(plan.targetEnd).toBe(epoch("2026-11-30T09:00:00Z"));
    }
  }
  expect(addCalendarMonth(epoch("2028-01-31T09:00:00Z"))).toBe(
    epoch("2028-02-29T09:00:00Z"),
  );
});

test("a Stripe success followed by a database failure can be replayed without granting an extra month", async () => {
  let live = {
    ...subscription(),
    billing_mode: { type: "flexible" },
    discounts: ["di_lifetime_pro"],
  } as Stripe.Subscription;
  const plan = planReferralExtension(live, ["price_pro"], now);
  const update = async (
    _id: string,
    params: Stripe.SubscriptionUpdateParams,
  ) => {
    live = {
      ...live,
      status: "trialing",
      trial_end: params.trial_end as number,
      billing_cycle_anchor: params.trial_end as number,
      metadata: params.metadata as Record<string, string>,
    };
    return live;
  };
  const args = {
    prices: ["price_pro"],
    ...plan,
    referralId: "reward-1",
    now,
    update,
  };
  await applyPreparedReferralExtension({ ...args, subscription: live });
  // The persisted target survives a crash; the second attempt reads Stripe's receipt.
  await applyPreparedReferralExtension({
    ...args,
    subscription: live,
    update: async () => {
      throw new Error("must not issue a second reward");
    },
  });
  expect(live.trial_end).toBe(epoch("2026-11-30T09:00:00Z"));
  for (const id of ["reward-2", "reward-3"]) {
    await applyPreparedReferralExtension({
      subscription: live,
      prices: ["price_pro"],
      ...planReferralExtension(live, ["price_pro"], now),
      referralId: id,
      now,
      update,
    });
  }
  expect(live.trial_end).toBe(epoch("2027-01-30T09:00:00Z"));
  expect(live.billing_cycle_anchor).toBe(live.trial_end!);
  expect(live.billing_mode.type).toBe("flexible");
  expect(live.discounts).toEqual(["di_lifetime_pro"]);
  expect(live.items.data[0].price.id).toBe("price_pro");
  expect(live.metadata.referral_extension).toBe("true");
});

test("changed, team-sized, canceled, scheduled and unsupported subscriptions cannot be automatically extended", async () => {
  for (const patch of [
    { cancel_at_period_end: true },
    { schedule: "sub_sched" },
    { status: "past_due" },
    { items: { data: [{ ...subscription().items.data[0], quantity: 5 }] } },
  ]) {
    expect(() =>
      planReferralExtension(
        { ...subscription(), ...patch } as Stripe.Subscription,
        ["price_pro"],
        now,
      ),
    ).toThrow();
  }
  const original = planReferralExtension(subscription(), ["price_pro"], now);
  const changed = subscription();
  changed.items.data[0].current_period_end += 86400;
  await expect(
    applyPreparedReferralExtension({
      subscription: changed,
      prices: ["price_pro"],
      ...original,
      referralId: "a",
      now,
      update: async () => {
        throw new Error("must not change billing");
      },
    }),
  ).rejects.toThrow("subscription_changed_before_reward");
});

test("cardless trial referrals extend the existing trial without turning it into a paid extension", async () => {
  const trial = {
    ...subscription(),
    billing_mode: { type: "flexible" },
    status: "trialing",
    trial_end: epoch("2026-10-08T09:00:00Z"),
    trial_settings: { end_behavior: { missing_payment_method: "pause" } },
  } as Stripe.Subscription;
  const plan = planReferralExtension(trial, ["price_pro"], now);
  const extended = await applyPreparedReferralExtension({
    subscription: trial,
    prices: ["price_pro"],
    ...plan,
    referralId: "trial-reward",
    now,
    update: async (_id, params) => ({
      ...trial,
      trial_end: params.trial_end as number,
      billing_cycle_anchor: params.trial_end as number,
      metadata: params.metadata as Record<string, string>,
    }),
  });
  expect(extended.trial_end).toBe(epoch("2026-11-08T09:00:00Z"));
  expect(extended.metadata.referral_extension).toBe("false");
  expect(extended.trial_settings?.end_behavior.missing_payment_method).toBe(
    "pause",
  );
  expect(() =>
    planReferralExtension({ ...trial, trial_end: now - 1 }, ["price_pro"], now),
  ).toThrow();
});

test("a trial extension without the intended renewal date is left for review", async () => {
  const live = {
    ...subscription("year"),
    billing_mode: { type: "flexible" },
    billing_cycle_anchor: epoch("2025-10-31T09:00:00Z"),
  } as Stripe.Subscription;
  const plan = planReferralExtension(live, ["price_pro"], now);
  await expect(
    applyPreparedReferralExtension({
      subscription: live,
      prices: ["price_pro"],
      ...plan,
      referralId: "anchor-not-moved",
      now,
      update: async (_id, params) => ({
        ...live,
        trial_end: params.trial_end as number,
        metadata: params.metadata as Record<string, string>,
      }),
    }),
  ).rejects.toThrow("extension_renewal_not_confirmed");
});
