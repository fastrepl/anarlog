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

  const { data, error } = await supabaseAdmin.rpc(
    "assign_profile_stripe_customer",
    {
      p_owner_user_id: userId,
      p_stripe_customer_id: customerId,
    },
  );

  let assignedCustomerId = data?.[0]?.assigned_customer_id as
    | string
    | null
    | undefined;
  if (error) {
    if (error.code !== "PGRST202") {
      throw error;
    }
    const { error: updateError } = await supabaseAdmin
      .from("profiles")
      .update({ stripe_customer_id: customerId })
      .eq("id", userId)
      .is("stripe_customer_id", null);
    if (updateError) {
      throw updateError;
    }
    const { data: profile, error: profileError } = await supabaseAdmin
      .from("profiles")
      .select("stripe_customer_id")
      .eq("id", userId)
      .single();
    if (profileError) {
      throw profileError;
    }
    assignedCustomerId = profile.stripe_customer_id as string | null;
  }
  if (assignedCustomerId !== customerId) {
    await stripe.customers.del(customerId);
  }
}

const isCustomerEvent = (eventType: string) =>
  CUSTOMER_EVENTS.includes(eventType as Stripe.Event.Type);

export const getCustomerId = (
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

export const getStripeCustomer = async (customerId: string) => {
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

export const getUserIdFromCustomer = (
  customer: Stripe.Customer,
): string | null => {
  const metadata = customer.metadata ?? {};

  return (
    metadata["userId"] || metadata["user_id"] || metadata["userID"] || null
  );
};
