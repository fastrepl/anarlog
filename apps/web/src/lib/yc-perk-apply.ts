import type Stripe from "stripe";

import { YC_FOUNDER_COUPON_ID } from "./yc-perk-promotion.ts";

function couponId(
  coupon: string | { id?: string } | null | undefined,
): string | null {
  if (typeof coupon === "string") {
    return coupon || null;
  }
  return coupon?.id ?? null;
}

export function isYcFounderPromotion(promotion: {
  promotion?: { coupon?: string | { id?: string } | null };
}) {
  return couponId(promotion.promotion?.coupon) === YC_FOUNDER_COUPON_ID;
}

export function subscriptionHasYcPerk(subscription: {
  discounts?: Array<
    | string
    | {
        source?: { coupon?: string | { id?: string } | null };
      }
  >;
}): boolean {
  return (subscription.discounts ?? []).some((discount) => {
    if (typeof discount === "string") {
      return false;
    }
    return couponId(discount.source?.coupon) === YC_FOUNDER_COUPON_ID;
  });
}

export function isYcPromotionAvailable(promotion: {
  active: boolean;
  max_redemptions: number | null;
  times_redeemed: number;
}) {
  return (
    promotion.active &&
    (promotion.max_redemptions === null ||
      promotion.times_redeemed < promotion.max_redemptions)
  );
}

export function pickCurrentSubscription<T extends { status: string }>(
  subscriptions: T[],
): T | null {
  return (
    subscriptions.find((subscription) => subscription.status === "active") ||
    subscriptions.find((subscription) => subscription.status === "trialing") ||
    null
  );
}

export async function findPromotionCodeByCustomerCode(
  stripe: Stripe,
  code: string,
) {
  const listed = await stripe.promotionCodes.list({
    code,
    limit: 1,
  });
  return listed.data[0] ?? null;
}

export async function findYcPromotionCodeByCustomerCode(
  stripe: Stripe,
  code: string,
) {
  const promotion = await findPromotionCodeByCustomerCode(stripe, code);
  if (!promotion || !isYcFounderPromotion(promotion)) {
    return null;
  }
  return promotion;
}

export async function applyYcPromotionToCustomer({
  stripe,
  customerId,
  promotion,
}: {
  stripe: Stripe;
  customerId: string;
  promotion: Pick<
    Stripe.PromotionCode,
    "id" | "active" | "max_redemptions" | "times_redeemed"
  >;
}): Promise<
  | { status: "applied"; subscriptionId: string }
  | { status: "already_applied"; subscriptionId: string }
  | { status: "needs_checkout"; promotionCodeId: string }
  | { status: "claimed" }
> {
  const subscriptions = await stripe.subscriptions.list({
    customer: customerId,
    status: "all",
    limit: 10,
    expand: ["data.discounts"],
  });
  const subscription = pickCurrentSubscription(subscriptions.data);

  if (subscription && subscriptionHasYcPerk(subscription)) {
    return { status: "already_applied", subscriptionId: subscription.id };
  }

  if (!isYcPromotionAvailable(promotion)) {
    return { status: "claimed" };
  }

  if (!subscription) {
    return { status: "needs_checkout", promotionCodeId: promotion.id };
  }

  await stripe.subscriptions.update(subscription.id, {
    discounts: [{ promotion_code: promotion.id }],
    ...(subscription.cancel_at_period_end
      ? { cancel_at_period_end: false }
      : {}),
  });

  return { status: "applied", subscriptionId: subscription.id };
}
