import type Stripe from "stripe";

const CURRENT_SUBSCRIPTION_STATUS_PRIORITY = [
  "active",
  "trialing",
  "paused",
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
