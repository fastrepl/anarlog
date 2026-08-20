import assert from "node:assert/strict";
import test from "node:test";
import type Stripe from "stripe";

import {
  createYcPromotionCode,
  getOrCreateYcPromotionCode,
  getYcPerkClaimId,
} from "./yc-perk-promotion.ts";

const claimId = "0123456789abcdef0123456789abcdef";
const code = "YC-0123456789ABCDEF01234567";

test("uses the normalized founder email as the claim identity", () => {
  assert.equal(
    getYcPerkClaimId(" Founder@Example.com "),
    getYcPerkClaimId("founder@example.com"),
  );
});

test("creates an unpredictable customer-facing code", () => {
  assert.match(createYcPromotionCode(), /^YC-[A-F0-9]{24}$/);
});

test("reuses an existing promotion code", async () => {
  let creates = 0;
  const stripe = {
    promotionCodes: {
      list: async () => ({
        data: [
          {
            id: "promo_existing",
            active: true,
            code: "YC-EXISTING",
            max_redemptions: 1,
            metadata: { claim_id: claimId },
            times_redeemed: 0,
          },
        ],
      }),
      create: async () => {
        creates += 1;
        return { id: "promo_new", code: "YC-NEW" };
      },
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await getOrCreateYcPromotionCode({ stripe, claimId, code }),
    {
      status: "available",
      id: "promo_existing",
      code: "YC-EXISTING",
      active: true,
      max_redemptions: 1,
      times_redeemed: 0,
    },
  );
  assert.equal(creates, 0);
});

test("reports an existing redeemed promotion code as claimed", async () => {
  const stripe = {
    promotionCodes: {
      list: async () => ({
        data: [
          {
            id: "promo_claimed",
            active: false,
            code: "YC-CLAIMED",
            max_redemptions: 1,
            metadata: { claim_id: claimId },
            times_redeemed: 1,
          },
        ],
      }),
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await getOrCreateYcPromotionCode({ stripe, claimId, code }),
    {
      status: "claimed",
      id: "promo_claimed",
      code: "YC-CLAIMED",
      active: false,
      max_redemptions: 1,
      times_redeemed: 1,
    },
  );
});

test("creates a single-use promotion code for the one-year YC coupon", async () => {
  const calls: Array<{ params: unknown; options: unknown }> = [];
  const stripe = {
    promotionCodes: {
      list: async () => ({ data: [] }),
      create: async (params: unknown, options: unknown) => {
        calls.push({ params, options });
        return {
          id: "promo_created",
          code,
          active: true,
          max_redemptions: 1,
          times_redeemed: 0,
        };
      },
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await getOrCreateYcPromotionCode({ stripe, claimId, code }),
    {
      status: "available",
      id: "promo_created",
      code,
      active: true,
      max_redemptions: 1,
      times_redeemed: 0,
    },
  );
  assert.deepEqual(calls, [
    {
      params: {
        promotion: { type: "coupon", coupon: "yc-founders-1-year-free" },
        code,
        max_redemptions: 1,
        metadata: { claim_id: claimId, source: "yc_perk_page" },
      },
      options: {
        idempotencyKey: `yc-perk-promotion:${claimId}`,
      },
    },
  ]);
});
