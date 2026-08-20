import { createHash, randomBytes } from "node:crypto";
import type Stripe from "stripe";

export const YC_FOUNDER_COUPON_ID = "yc-founders-1-year-free";

export function getYcPerkClaimId(email: string) {
  return createHash("sha256").update(email.trim().toLowerCase()).digest("hex");
}

export function createYcPromotionCode() {
  return `YC-${randomBytes(12).toString("hex").toUpperCase()}`;
}

export async function getOrCreateYcPromotionCode({
  stripe,
  claimId,
  code,
}: {
  stripe: Stripe;
  claimId: string;
  code: string;
}) {
  const findPromotionCode = async () =>
    (await stripe.promotionCodes.list({ code, limit: 1 })).data[0];
  const toPromotionResult = (
    promotion: Stripe.PromotionCode,
    status: "available" | "claimed",
  ) => ({
    status,
    id: promotion.id,
    code: promotion.code,
    active: promotion.active,
    max_redemptions: promotion.max_redemptions,
    times_redeemed: promotion.times_redeemed,
  });
  const getExistingPromotion = (promotion: Stripe.PromotionCode) =>
    !promotion.active ||
    (promotion.max_redemptions !== null &&
      promotion.times_redeemed >= promotion.max_redemptions)
      ? toPromotionResult(promotion, "claimed")
      : toPromotionResult(promotion, "available");

  const existingPromotion = await findPromotionCode();
  if (existingPromotion) {
    return getExistingPromotion(existingPromotion);
  }

  try {
    const promotionCode = await stripe.promotionCodes.create(
      {
        promotion: {
          type: "coupon",
          coupon: YC_FOUNDER_COUPON_ID,
        },
        code,
        max_redemptions: 1,
        metadata: {
          claim_id: claimId,
          source: "yc_perk_page",
        },
      },
      { idempotencyKey: `yc-perk-promotion:${claimId}` },
    );
    return toPromotionResult(promotionCode, "available");
  } catch (error) {
    const concurrentPromotion = await findPromotionCode();
    if (concurrentPromotion) {
      return getExistingPromotion(concurrentPromotion);
    }
    throw error;
  }
}
