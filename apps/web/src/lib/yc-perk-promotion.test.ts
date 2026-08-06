import assert from "node:assert/strict";
import test from "node:test";
import type Stripe from "stripe";

import {
  createYcPromotionCode,
  getOrCreateYcPromotionCode,
  getYcPerkClaimId,
  YC_FOUNDER_COUPON_ID,
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
        return { code: "YC-NEW" };
      },
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await getOrCreateYcPromotionCode({ stripe, claimId, code }),
    { status: "available", code: "YC-EXISTING" },
  );
  assert.equal(creates, 0);
});

test("reports an existing redeemed promotion code as claimed", async () => {
  const stripe = {
    promotionCodes: {
      list: async () => ({
        data: [
          {
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
    { status: "claimed" },
  );
});

test("creates a single-use promotion code for the YC coupon", async () => {
  const calls: Array<{ params: unknown; options: unknown }> = [];
  const stripe = {
    promotionCodes: {
      list: async () => ({ data: [] }),
      create: async (params: unknown, options: unknown) => {
        calls.push({ params, options });
        return { code };
      },
    },
  } as unknown as Stripe;

  assert.deepEqual(
    await getOrCreateYcPromotionCode({ stripe, claimId, code }),
    { status: "available", code },
  );
  assert.deepEqual(calls, [
    {
      params: {
        promotion: { type: "coupon", coupon: YC_FOUNDER_COUPON_ID },
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
