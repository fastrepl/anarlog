import type Stripe from "stripe";

const CURRENT_SUBSCRIPTION_STATUS_PRIORITY = [
  "active",
  "trialing",
  "paused",
] satisfies Stripe.Subscription.Status[];

const PERSONAL_PLAN_REPLACEMENT_STATUS_PRIORITY = [
  "active",
  "trialing",
  "past_due",
  "unpaid",
] satisfies Stripe.Subscription.Status[];

export function selectCurrentSubscription<T extends { status: string }>(
  subscriptions: readonly T[],
): T | null {
  for (const status of CURRENT_SUBSCRIPTION_STATUS_PRIORITY) {
    const subscription = subscriptions.find(
      (candidate) => candidate.status === status,
    );
    if (subscription) {
      return subscription;
    }
  }

  return null;
}

export function selectPersonalPlanReplacement<
  T extends { status: string; cancel_at_period_end: boolean },
>(subscriptions: readonly T[]): T | null {
  for (const status of PERSONAL_PLAN_REPLACEMENT_STATUS_PRIORITY) {
    const subscription = subscriptions.find(
      (candidate) =>
        candidate.status === status && !candidate.cancel_at_period_end,
    );
    if (subscription) {
      return subscription;
    }
  }

  return null;
}

export function getPlanSwitchRoute(
  subscription: Pick<Stripe.Subscription, "status" | "items">,
  targetPriceId: string,
): "portal" | "checkout" | "update" {
  if (subscription.status === "paused") {
    return "portal";
  }

  const item = subscription.items.data[0];
  if (!item) {
    return "checkout";
  }

  return item.price.id === targetPriceId ? "portal" : "update";
}
