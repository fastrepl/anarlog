import Stripe from "stripe";

import { stripe } from "./integration/stripe";
import { supabaseAdmin } from "./integration/supabase";

const CUSTOMER_EVENTS: Stripe.Event.Type[] = [
  "checkout.session.completed",
  "customer.created",
  "customer.updated",
  "customer.subscription.created",
  "customer.subscription.updated",
];

export async function syncBillingBridge(event: Stripe.Event) {
  if (!isCustomerEvent(event.type)) {
    return;
  }

  const customerId = getCustomerId(event.data.object);

  if (!customerId) {
    return;
  }

  const customer = await getStripeCustomer(customerId);

  if (!customer) {
    return;
  }

  const userId = getUserIdFromCustomer(customer);

  if (!userId) {
    return;
  }

  const { error } = await supabaseAdmin.from("profiles").upsert(
    {
      id: userId,
      stripe_customer_id: customerId,
    },
    { onConflict: "id" },
  );

  if (error) {
    throw error;
  }
}

const isCustomerEvent = (eventType: string) =>
  CUSTOMER_EVENTS.includes(eventType as Stripe.Event.Type);

const getCustomerId = (
  eventObject: Stripe.Event.Data.Object,
): string | null => {
  const obj = eventObject as {
    customer?: string | { id: string };
    id?: string;
  };

  if (typeof obj.customer === "string") {
    return obj.customer;
  }

  if (obj.customer && typeof obj.customer === "object") {
    return obj.customer.id;
  }

  if (obj.id?.startsWith("cus_")) {
    return obj.id;
  }

  return null;
};

const getStripeCustomer = async (customerId: string) => {
  const customer = await stripe.customers.retrieve(customerId);

  if (isDeletedCustomer(customer)) {
    return null;
  }

  return customer;
};

const isDeletedCustomer = (
  customer: Stripe.Customer | Stripe.DeletedCustomer,
): customer is Stripe.DeletedCustomer =>
  "deleted" in customer && customer.deleted === true;

const getUserIdFromCustomer = (customer: Stripe.Customer): string | null => {
  const metadata = customer.metadata ?? {};

  return (
    metadata["userId"] || metadata["user_id"] || metadata["userID"] || null
  );
};
