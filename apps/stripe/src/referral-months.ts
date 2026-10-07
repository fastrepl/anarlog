import type Stripe from "stripe";

export class ReferralReviewError extends Error {
  constructor(public readonly reason: string) {
    super(reason);
    this.name = "ReferralReviewError";
  }
}

export function addCalendarMonth(timestamp: number) {
  const date = new Date(timestamp * 1000);
  const day = date.getUTCDate();
  date.setUTCDate(1);
  date.setUTCMonth(date.getUTCMonth() + 1);
  const lastDay = new Date(
    Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 0),
  ).getUTCDate();
  date.setUTCDate(Math.min(day, lastDay));
  return Math.floor(date.getTime() / 1000);
}

export function planReferralExtension(
  subscription: Stripe.Subscription,
  prices: string[],
  now: number,
) {
  const item = subscription.items.data[0];
  if (subscription.items.data.length !== 1 || !item || item.quantity !== 1)
    throw new ReferralReviewError("unsupported_subscription_items");
  if (
    !prices.includes(item.price.id) ||
    !["month", "year"].includes(item.price.recurring?.interval ?? "") ||
    item.price.recurring?.interval_count !== 1
  )
    throw new ReferralReviewError("unsupported_subscription_price");
  if (!["active", "trialing"].includes(subscription.status))
    throw new ReferralReviewError("subscription_not_active");
  if (subscription.cancel_at_period_end || subscription.cancel_at)
    throw new ReferralReviewError("subscription_cancellation_pending");
  if (subscription.schedule)
    throw new ReferralReviewError("subscription_schedule_requires_review");
  if (subscription.pending_update || subscription.pause_collection)
    throw new ReferralReviewError("subscription_update_or_pause_pending");
  const previousEnd =
    subscription.status === "trialing"
      ? subscription.trial_end
      : item.current_period_end;
  if (!previousEnd || previousEnd <= now)
    throw new ReferralReviewError("subscription_period_requires_review");
  return { previousEnd, targetEnd: addCalendarMonth(previousEnd) };
}

export function isReferralExtension(subscription: Stripe.Subscription) {
  return subscription.metadata?.referral_extension === "true";
}

export async function applyPreparedReferralExtension({
  subscription,
  prices,
  previousEnd,
  targetEnd,
  referralId,
  now,
  update,
}: {
  subscription: Stripe.Subscription;
  prices: string[];
  previousEnd: number;
  targetEnd: number;
  referralId: string;
  now: number;
  update: (
    id: string,
    params: Stripe.SubscriptionUpdateParams,
    options: Stripe.RequestOptions,
  ) => Promise<Stripe.Subscription>;
}) {
  if (subscription.metadata.referral_last_reward !== referralId) {
    const plan = planReferralExtension(subscription, prices, now);
    if (plan.previousEnd !== previousEnd || plan.targetEnd !== targetEnd)
      throw new ReferralReviewError("subscription_changed_before_reward");
    subscription = await update(
      subscription.id,
      {
        trial_end: targetEnd,
        proration_behavior: "none",
        metadata: {
          referral_extension:
            subscription.status === "active" ||
            subscription.metadata.referral_extension === "true"
              ? "true"
              : "false",
          referral_last_reward: referralId,
        },
      },
      { idempotencyKey: `referral-month:${referralId}` },
    );
  }
  if (
    subscription.trial_end !== targetEnd ||
    subscription.metadata.referral_last_reward !== referralId
  )
    throw new ReferralReviewError("extension_not_confirmed");
  if (subscription.billing_cycle_anchor !== targetEnd)
    throw new ReferralReviewError("extension_renewal_not_confirmed");
  return subscription;
}
