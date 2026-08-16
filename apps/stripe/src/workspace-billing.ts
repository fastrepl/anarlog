import type Stripe from "stripe";

const WORKSPACE_SUBSCRIPTION_EVENTS: Stripe.Event.Type[] = [
  "customer.subscription.created",
  "customer.subscription.updated",
  "customer.subscription.deleted",
];

export function getWorkspaceBillingUpdate(
  event: Stripe.Event,
  teamPriceIds: ReadonlySet<string>,
) {
  if (!WORKSPACE_SUBSCRIPTION_EVENTS.includes(event.type)) {
    return { seatLimit: null, updateSeatLimit: false };
  }

  const seatLimit = getTeamSubscriptionSeatLimit(
    event.data.object as Stripe.Subscription,
    teamPriceIds,
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

export function getTeamSubscriptionSeatLimit(
  subscription: Pick<Stripe.Subscription, "items">,
  teamPriceIds: ReadonlySet<string>,
) {
  const teamItems = subscription.items.data.filter((item) =>
    teamPriceIds.has(item.price.id),
  );

  if (teamItems.length !== 1) {
    return null;
  }

  const quantity = teamItems[0]?.quantity ?? 1;
  return Number.isSafeInteger(quantity) && quantity > 0 ? quantity : null;
}
