import assert from "node:assert/strict";
import test from "node:test";
import type Stripe from "stripe";

import {
  applyYcPromotionToCustomer,
  isYcFounderPromotion,
  isYcPromotionAvailable,
  pickCurrentSubscription,
  subscriptionHasYcPerk,
} from "./yc-perk-apply.ts";

const promotion = {
  id: "promo_yc",
  active: true,
  max_redemptions: 1,
  times_redeemed: 0,
};

test("only accepts promotion codes for the YC founder coupon", () => {
  assert.equal(
    isYcFounderPromotion({
      promotion: { coupon: "yc-founders-1-year-free" },
    }),
    true,
  );
  assert.equal(
    isYcFounderPromotion({
      promotion: { coupon: { id: "yc-founders-1-year-free" } },
    }),
    true,
  );
  assert.equal(
    isYcFounderPromotion({ promotion: { coupon: "other-coupon" } }),
    false,
  );
});

test("detects the YC founder coupon on expanded subscription discounts", () => {
  assert.equal(
    subscriptionHasYcPerk({
      discounts: [{ source: { coupon: "yc-founders-1-year-free" } }],
    }),
    true,
  );
  assert.equal(
    subscriptionHasYcPerk({
      discounts: [{ source: { coupon: { id: "yc-founders-1-year-free" } } }],
    }),
    true,
  );
  assert.equal(subscriptionHasYcPerk({ discounts: ["di_123"] }), false);
  assert.equal(subscriptionHasYcPerk({ discounts: [] }), false);
});

test("treats a fully redeemed promotion as unavailable", () => {
  assert.equal(isYcPromotionAvailable(promotion), true);
  assert.equal(
    isYcPromotionAvailable({
      ...promotion,
      times_redeemed: 1,
    }),
    false,
  );
  assert.equal(
    isYcPromotionAvailable({
      ...promotion,
      active: false,
    }),
    false,
  );
});

test("prefers active and trialing subscriptions but preserves paused trials", () => {
  assert.equal(
    pickCurrentSubscription([
      { status: "canceled" },
      { status: "trialing" },
      { status: "active" },
    ])?.status,
    "active",
  );
  assert.equal(
    pickCurrentSubscription([{ status: "trialing" }])?.status,
    "trialing",
  );
  assert.equal(
    pickCurrentSubscription([{ status: "canceled" }, { status: "paused" }])
      ?.status,
    "paused",
  );
  assert.equal(pickCurrentSubscription([{ status: "canceled" }]), null);
});

test("applies the promotion to an existing subscription and clears a scheduled cancel", async () => {
  const updates: unknown[] = [];
  const stripe = {
    subscriptions: {
      list: async () => ({
        data: [
          {
            id: "sub_pro",
            status: "active",
            cancel_at_period_end: true,
            discounts: [],
          },
        ],
      }),
      update: async (id: string, params: unknown) => {
        updates.push({ id, params });
        return { id };
      },
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await applyYcPromotionToCustomer({
      stripe,
      customerId: "cus_pro",
      promotion,
    }),
    { status: "applied", subscriptionId: "sub_pro" },
  );
  assert.deepEqual(updates, [
    {
      id: "sub_pro",
      params: {
        discounts: [{ promotion_code: "promo_yc" }],
        cancel_at_period_end: false,
      },
    },
  ]);
});

test("applies the promotion to a paused cardless trial", async () => {
  const updates: unknown[] = [];
  const stripe = {
    subscriptions: {
      list: async () => ({
        data: [
          {
            id: "sub_paused",
            status: "paused",
            cancel_at_period_end: false,
            discounts: [],
          },
        ],
      }),
      update: async (id: string, params: unknown) => {
        updates.push({ id, params });
        return { id };
      },
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await applyYcPromotionToCustomer({
      stripe,
      customerId: "cus_trial",
      promotion,
    }),
    { status: "applied", subscriptionId: "sub_paused" },
  );
  assert.deepEqual(updates, [
    {
      id: "sub_paused",
      params: { discounts: [{ promotion_code: "promo_yc" }] },
    },
  ]);
});

test("does not re-redeem when the subscription already has the YC perk", async () => {
  let updates = 0;
  const stripe = {
    subscriptions: {
      list: async () => ({
        data: [
          {
            id: "sub_pro",
            status: "active",
            cancel_at_period_end: false,
            discounts: [{ source: { coupon: "yc-founders-1-year-free" } }],
          },
        ],
      }),
      update: async () => {
        updates += 1;
        return { id: "sub_pro" };
      },
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await applyYcPromotionToCustomer({
      stripe,
      customerId: "cus_pro",
      promotion: { ...promotion, times_redeemed: 1, active: false },
    }),
    { status: "already_applied", subscriptionId: "sub_pro" },
  );
  assert.equal(updates, 0);
});

test("sends customers without a subscription to checkout with the promotion", async () => {
  const stripe = {
    subscriptions: {
      list: async () => ({ data: [] }),
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await applyYcPromotionToCustomer({
      stripe,
      customerId: "cus_free",
      promotion,
    }),
    { status: "needs_checkout", promotionCodeId: "promo_yc" },
  );
});

test("rejects a code that another customer already redeemed", async () => {
  const stripe = {
    subscriptions: {
      list: async () => ({
        data: [
          {
            id: "sub_pro",
            status: "active",
            cancel_at_period_end: false,
            discounts: [],
          },
        ],
      }),
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await applyYcPromotionToCustomer({
      stripe,
      customerId: "cus_pro",
      promotion: { ...promotion, times_redeemed: 1, active: false },
    }),
    { status: "claimed" },
  );
});
