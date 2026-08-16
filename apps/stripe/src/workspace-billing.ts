import type Stripe from "stripe";

const WORKSPACE_SUBSCRIPTION_EVENTS: Stripe.Event.Type[] = [
  "customer.subscription.created",
  "customer.subscription.updated",
  "customer.subscription.deleted",
];

export function getWorkspaceBillingUpdate(event: Stripe.Event) {
  if (!WORKSPACE_SUBSCRIPTION_EVENTS.includes(event.type)) {
    return { seatLimit: null, updateSeatLimit: false };
  }

  const seatLimit = getWorkspaceSubscriptionSeatLimit(
    event.data.object as Stripe.Subscription,
  );

  if (event.type === "customer.subscription.deleted") {
    return {
      seatLimit: null,
      updateSeatLimit: seatLimit !== null,
    };
  }

  return {
    seatLimit,
    updateSeatLimit: seatLimit !== null,
  };
}

export function getWorkspaceSubscriptionSeatLimit(
  subscription: Pick<Stripe.Subscription, "items">,
) {
  if (subscription.items.data.length !== 1) {
    return null;
  }

  const quantity = subscription.items.data[0]?.quantity ?? 1;
  return Number.isSafeInteger(quantity) && quantity > 0 ? quantity : null;
}
