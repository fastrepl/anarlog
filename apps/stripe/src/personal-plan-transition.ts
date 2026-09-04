import type Stripe from "stripe";

import { getCustomerOwner } from "./customer-metadata";

const TEAM_SUBSCRIPTION_EVENTS: Stripe.Event.Type[] = [
  "customer.subscription.created",
  "customer.subscription.updated",
];

type PersonalPlanTransitionDependencies = {
  getCustomer: (customerId: string) => Promise<Stripe.Customer | null>;
  getSubscription: (subscriptionId: string) => Promise<Stripe.Subscription>;
  scheduleCancellation: (subscriptionId: string) => Promise<void>;
};

export async function scheduleReplacedPersonalPlanCancellation(
  event: Stripe.Event,
  dependencies?: PersonalPlanTransitionDependencies,
) {
  if (!TEAM_SUBSCRIPTION_EVENTS.includes(event.type)) {
    return;
  }

  const teamSubscription = event.data.object as Stripe.Subscription;
  const metadata = teamSubscription.metadata;
  const personalSubscriptionId = metadata.replaces_personal_subscription_id;
  const personalUserId = metadata.replaces_personal_user_id;
  const workspaceId = metadata.workspace_id;
  const hasTransitionMetadata = Boolean(
    personalSubscriptionId || personalUserId,
  );

  if (!hasTransitionMetadata) {
    return;
  }

  if (
    metadata.checkout_type !== "team" ||
    !workspaceId ||
    !personalSubscriptionId ||
    !personalUserId
  ) {
    throw new Error("Team personal plan transition metadata is invalid");
  }

  if (!isActiveTeamSubscription(teamSubscription)) {
    return;
  }

  const teamCustomerId = getCustomerId(teamSubscription.customer);
  if (!teamCustomerId) {
    throw new Error("Team subscription customer is unavailable");
  }

  const activeDependencies =
    dependencies ?? (await createDefaultDependencies());
  const teamCustomer = await activeDependencies.getCustomer(teamCustomerId);
  const teamOwner = teamCustomer
    ? getCustomerOwner(teamCustomer.metadata)
    : null;
  if (teamOwner?.kind !== "workspace" || teamOwner.id !== workspaceId) {
    throw new Error("Team subscription customer ownership is invalid");
  }

  const personalSubscription = await activeDependencies.getSubscription(
    personalSubscriptionId,
  );
  const personalCustomerId = getCustomerId(personalSubscription.customer);
  if (!personalCustomerId || personalCustomerId === teamCustomerId) {
    throw new Error("Personal subscription customer is invalid");
  }

  const personalCustomer =
    await activeDependencies.getCustomer(personalCustomerId);
  const personalOwner = personalCustomer
    ? getCustomerOwner(personalCustomer.metadata)
    : null;
  if (personalOwner?.kind !== "user" || personalOwner.id !== personalUserId) {
    throw new Error("Personal subscription customer ownership is invalid");
  }

  if (
    !shouldCancelPersonalSubscription(personalSubscription) ||
    personalSubscription.cancel_at_period_end
  ) {
    return;
  }

  await activeDependencies.scheduleCancellation(personalSubscriptionId);
}

const isActiveTeamSubscription = (subscription: Stripe.Subscription) =>
  subscription.status === "active" || subscription.status === "trialing";

const shouldCancelPersonalSubscription = (subscription: Stripe.Subscription) =>
  ["active", "trialing", "past_due", "unpaid"].includes(subscription.status);

const getCustomerId = (
  customer: string | Stripe.Customer | Stripe.DeletedCustomer,
) => (typeof customer === "string" ? customer : customer.id);

async function createDefaultDependencies(): Promise<PersonalPlanTransitionDependencies> {
  const { stripe } = await import("./integration/stripe");

  return {
    async getCustomer(customerId) {
      const customer = await stripe.customers.retrieve(customerId);
      return "deleted" in customer && customer.deleted ? null : customer;
    },
    async getSubscription(subscriptionId) {
      return stripe.subscriptions.retrieve(subscriptionId);
    },
    async scheduleCancellation(subscriptionId) {
      await stripe.subscriptions.update(subscriptionId, {
        cancel_at_period_end: true,
      });
    },
  };
}
