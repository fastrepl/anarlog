import Stripe from "stripe";

type BillingAnalyticsPayload = {
  event: string;
  properties: Record<string, boolean | number | string | null>;
};

export function getBillingAnalyticsPayload(
  event: Stripe.Event,
): BillingAnalyticsPayload | null {
  switch (event.type) {
    case "checkout.session.completed": {
      const session = event.data.object as Stripe.Checkout.Session;
      return {
        event: "checkout_completed",
        properties: {
          checkout_type: session.metadata?.["checkout_type"] ?? "paid",
          entry_source: session.metadata?.["source"] ?? "unknown",
          amount_total: session.amount_total,
          currency: session.currency,
        },
      };
    }
    case "checkout.session.expired": {
      const session = event.data.object as Stripe.Checkout.Session;
      return {
        event: "checkout_abandoned",
        properties: {
          checkout_type: session.metadata?.["checkout_type"] ?? "paid",
          entry_source: session.metadata?.["source"] ?? "unknown",
        },
      };
    }
    case "customer.subscription.created": {
      const subscription = event.data.object as Stripe.Subscription;
      return subscriptionPayload(
        subscription.status === "trialing"
          ? "trial_started"
          : "subscription_activated",
        subscription,
      );
    }
    case "customer.subscription.updated": {
      const subscription = event.data.object as Stripe.Subscription;
      const previousStatus = (
        event.data.previous_attributes as { status?: string } | undefined
      )?.status;

      if (subscription.status === "active" && previousStatus === "trialing") {
        return subscriptionPayload("subscription_activated", subscription);
      }

      return null;
    }
    case "customer.subscription.deleted":
      return subscriptionPayload(
        "subscription_canceled",
        event.data.object as Stripe.Subscription,
      );
    case "invoice.paid": {
      const invoice = event.data.object as Stripe.Invoice;
      if (invoice.amount_paid <= 0) {
        return null;
      }
      return invoicePayload("subscription_payment_succeeded", invoice);
    }
    case "invoice.payment_failed":
      return invoicePayload(
        "subscription_payment_failed",
        event.data.object as Stripe.Invoice,
      );
    default:
      return null;
  }
}

function subscriptionPayload(
  event: string,
  subscription: Stripe.Subscription,
): BillingAnalyticsPayload {
  const price = subscription.items.data[0]?.price;
  return {
    event,
    properties: {
      plan: "pro",
      status: subscription.status,
      interval: price?.recurring?.interval ?? "unknown",
      price_id: price?.id ?? "unknown",
      checkout_type: subscription.metadata?.["checkout_type"] ?? "unknown",
      entry_source: subscription.metadata?.["source"] ?? "unknown",
      cancel_at_period_end: subscription.cancel_at_period_end,
      trial_end: subscription.trial_end,
    },
  };
}

function invoicePayload(
  event: string,
  invoice: Stripe.Invoice,
): BillingAnalyticsPayload {
  return {
    event,
    properties: {
      plan: "pro",
      amount_paid: invoice.amount_paid,
      amount_due: invoice.amount_due,
      currency: invoice.currency,
      billing_reason: invoice.billing_reason,
      attempt_count: invoice.attempt_count,
    },
  };
}
